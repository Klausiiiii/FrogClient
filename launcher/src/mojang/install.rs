use super::model::{AssetIndex, VersionJson, VersionManifest};
use super::rules::{select_libraries, Env};
use crate::http::{DownloadJob, Http, HttpError, Progress};
use crate::paths::AppPaths;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MojangEndpoints {
    pub manifest: String,
    pub java_index: String,
    pub resources: String,
}

impl Default for MojangEndpoints {
    fn default() -> Self {
        MojangEndpoints {
            manifest: "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json".into(),
            java_index: "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json".into(),
            resources: "https://resources.download.minecraft.net".into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error(transparent)]
    Http(#[from] HttpError),
    #[error("Dateifehler: {0}")]
    Io(String),
    #[error("Unbekannte Minecraft-Version {0}")]
    UnknownVersion(String),
    #[error("Ungültige Daten: {0}")]
    Parse(String),
    #[error("Keine Java-Runtime {0} für dieses System verfügbar")]
    NoJava(String),
}

impl From<std::io::Error> for InstallError {
    fn from(e: std::io::Error) -> Self {
        InstallError::Io(e.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct InstalledVersion {
    pub json: VersionJson,
    pub client_jar: PathBuf,
    pub library_paths: Vec<PathBuf>,
    pub logging_config: Option<PathBuf>,
}

pub async fn install_version(
    http: &Http,
    paths: &AppPaths,
    ep: &MojangEndpoints,
    version_id: &str,
    progress: impl Fn(Progress) + Send + Sync,
) -> Result<InstalledVersion, InstallError> {
    let version_dir = paths.versions().join(version_id);
    let json_path = version_dir.join(format!("{version_id}.json"));

    match http.get_json::<VersionManifest>(&ep.manifest).await {
        Ok(manifest) => {
            let entry = manifest
                .versions
                .iter()
                .find(|v| v.id == version_id)
                .ok_or_else(|| InstallError::UnknownVersion(version_id.into()))?;
            http.download_all(
                vec![DownloadJob { url: entry.url.clone(), dest: json_path.clone(), sha1: Some(entry.sha1.clone()), size: None }],
                1,
                |_| {},
            )
            .await?;
        }
        // Offline: fall back to the version JSON from an earlier install.
        Err(e) if !json_path.is_file() => return Err(e.into()),
        Err(_) => {}
    }

    let json: VersionJson = serde_json::from_str(&std::fs::read_to_string(&json_path)?)
        .map_err(|e| InstallError::Parse(format!("{}: {e}", json_path.display())))?;
    let env = Env::current();

    let client_jar = version_dir.join(format!("{version_id}.jar"));
    let index_path = paths.assets().join("indexes").join(format!("{}.json", json.asset_index.id));
    let mut jobs = vec![
        DownloadJob {
            url: json.downloads.client.url.clone(),
            dest: client_jar.clone(),
            sha1: Some(json.downloads.client.sha1.clone()),
            size: Some(json.downloads.client.size),
        },
        DownloadJob {
            url: json.asset_index.url.clone(),
            dest: index_path.clone(),
            sha1: Some(json.asset_index.sha1.clone()),
            size: None,
        },
    ];

    let mut library_paths = Vec::new();
    for lib in select_libraries(&json, &env) {
        let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) else { continue };
        let Some(rel) = &artifact.path else { continue };
        let dest = paths.libraries().join(rel);
        library_paths.push(dest.clone());
        jobs.push(DownloadJob { url: artifact.url.clone(), dest, sha1: Some(artifact.sha1.clone()), size: Some(artifact.size) });
    }

    let logging_config = json.logging.as_ref().and_then(|l| l.client.as_ref()).map(|c| {
        let dest = paths.assets().join("log_configs").join(&c.file.id);
        jobs.push(DownloadJob { url: c.file.url.clone(), dest: dest.clone(), sha1: Some(c.file.sha1.clone()), size: Some(c.file.size) });
        dest
    });

    http.download_all(jobs, 16, &progress).await?;

    let index: AssetIndex = serde_json::from_str(&std::fs::read_to_string(&index_path)?)
        .map_err(|e| InstallError::Parse(format!("asset index: {e}")))?;
    let mut seen = std::collections::HashSet::new();
    let asset_jobs: Vec<DownloadJob> = index
        .objects
        .values()
        .filter(|o| seen.insert(o.hash.clone()))
        .map(|o| {
            let prefix = &o.hash[..2];
            DownloadJob {
                url: format!("{}/{prefix}/{}", ep.resources, o.hash),
                dest: paths.assets().join("objects").join(prefix).join(&o.hash),
                sha1: Some(o.hash.clone()),
                size: Some(o.size),
            }
        })
        .collect();
    http.download_all(asset_jobs, 32, &progress).await?;

    Ok(InstalledVersion { json, client_jar, library_paths, logging_config })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::http::sha1_file;
    use serde_json::json;
    use sha1::{Digest, Sha1};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    pub fn sha(s: &str) -> String {
        hex::encode(Sha1::digest(s.as_bytes()))
    }

    async fn file(server: &MockServer, p: &str, body: &str, expect: Option<u64>) {
        let mut m = Mock::given(method("GET")).and(path(p.to_string()))
            .respond_with(ResponseTemplate::new(200).set_body_string(body.to_string()));
        if let Some(n) = expect {
            m = m.expect(n);
        }
        m.mount(server).await;
    }

    /// Serves a minimal fake Mojang backend: one version, one library, two assets.
    pub async fn fake_mojang(server: &MockServer, expect_files: Option<u64>) -> MojangEndpoints {
        let u = server.uri();
        let index = json!({"objects": {
            "a.txt": {"hash": sha("asset-a"), "size": 7},
            "b.txt": {"hash": sha("asset-b"), "size": 7}
        }}).to_string();
        let version = json!({
            "id": "26.3", "mainClass": "net.minecraft.client.main.Main", "type": "release",
            "assets": "34",
            "arguments": {"game": ["--username", "${auth_player_name}"], "jvm": ["-cp", "${classpath}"]},
            "assetIndex": {"id": "34", "url": format!("{u}/index.json"), "sha1": sha(&index)},
            "downloads": {"client": {"url": format!("{u}/client.jar"), "sha1": sha("client"), "size": 6}},
            "javaVersion": {"component": "java-runtime-epsilon", "majorVersion": 25},
            "libraries": [
                {"name": "a:lib:1", "downloads": {"artifact": {"path": "a/lib/1/lib-1.jar", "url": format!("{u}/lib.jar"), "sha1": sha("lib"), "size": 3}}},
                {"name": "a:mac:1", "rules": [{"action": "allow", "os": {"name": "osx"}}],
                 "downloads": {"artifact": {"path": "a/mac/1/mac-1.jar", "url": format!("{u}/mac.jar"), "sha1": sha("mac"), "size": 3}}}
            ],
            "logging": {"client": {"argument": "-Dlog4j.configurationFile=${path}",
                "file": {"id": "client-1.21.2.xml", "sha1": sha("log"), "size": 3, "url": format!("{u}/log.xml")}}}
        }).to_string();
        let manifest = json!({"versions": [{"id": "26.3", "url": format!("{u}/26.3.json"), "sha1": sha(&version)}]}).to_string();

        Mock::given(method("GET")).and(path("/manifest.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(manifest)).mount(server).await;
        // The version JSON embeds this server's URLs, so its hash changes per server.
        file(server, "/26.3.json", &version, None).await;
        file(server, "/index.json", &index, expect_files).await;
        file(server, "/client.jar", "client", expect_files).await;
        file(server, "/lib.jar", "lib", expect_files).await;
        file(server, "/log.xml", "log", expect_files).await;
        file(server, &format!("/{}/{}", &sha("asset-a")[..2], sha("asset-a")), "asset-a", expect_files).await;
        file(server, &format!("/{}/{}", &sha("asset-b")[..2], sha("asset-b")), "asset-b", expect_files).await;
        file(server, "/mac.jar", "mac", Some(0)).await;

        MojangEndpoints {
            manifest: format!("{u}/manifest.json"),
            java_index: format!("{u}/java/all.json"),
            resources: u,
        }
    }

    fn paths(dir: &tempfile::TempDir) -> AppPaths {
        AppPaths { home: dir.path().to_path_buf() }
    }

    #[tokio::test]
    async fn installs_minimal_version_layout() {
        let server = MockServer::start().await;
        let ep = fake_mojang(&server, None).await;
        let dir = tempfile::tempdir().unwrap();
        let p = paths(&dir);
        let v = install_version(&Http::new(), &p, &ep, "26.3", |_| {}).await.unwrap();

        assert_eq!(v.client_jar, p.versions().join("26.3/26.3.jar"));
        assert_eq!(sha1_file(&v.client_jar).unwrap(), sha("client"));
        assert!(p.versions().join("26.3/26.3.json").is_file());
        assert_eq!(v.library_paths, vec![p.libraries().join("a/lib/1/lib-1.jar")]);
        assert!(v.library_paths[0].is_file());
        assert_eq!(v.logging_config, Some(p.assets().join("log_configs/client-1.21.2.xml")));
        assert!(p.assets().join("indexes/34.json").is_file());
        let h = sha("asset-a");
        assert!(p.assets().join(format!("objects/{}/{}", &h[..2], h)).is_file());
    }

    #[tokio::test]
    async fn second_install_makes_no_downloads() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(&dir);
        {
            let server = MockServer::start().await;
            let ep = fake_mojang(&server, None).await;
            install_version(&Http::new(), &p, &ep, "26.3", |_| {}).await.unwrap();
        }
        let server = MockServer::start().await;
        let ep = fake_mojang(&server, Some(0)).await;
        // URLs differ per server, but valid files on disk must never be fetched again.
        install_version(&Http::new(), &p, &ep, "26.3", |_| {}).await.unwrap();
    }

    #[tokio::test]
    async fn uses_cached_json_when_offline() {
        let dir = tempfile::tempdir().unwrap();
        let p = paths(&dir);
        {
            let server = MockServer::start().await;
            let ep = fake_mojang(&server, None).await;
            install_version(&Http::new(), &p, &ep, "26.3", |_| {}).await.unwrap();
        }
        let dead = MockServer::start().await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(500)).mount(&dead).await;
        let ep = MojangEndpoints { manifest: format!("{}/manifest.json", dead.uri()), java_index: String::new(), resources: dead.uri() };
        let v = install_version(&Http::new(), &p, &ep, "26.3", |_| {}).await.unwrap();
        assert_eq!(v.json.id, "26.3");
    }

    #[tokio::test]
    async fn unknown_version_errors() {
        let server = MockServer::start().await;
        let ep = fake_mojang(&server, None).await;
        let dir = tempfile::tempdir().unwrap();
        let err = install_version(&Http::new(), &paths(&dir), &ep, "1.0", |_| {}).await.unwrap_err();
        assert!(matches!(err, InstallError::UnknownVersion(_)), "{err:?}");
    }
}
