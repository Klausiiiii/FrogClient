use super::install::{InstallError, MojangEndpoints};
use crate::http::{DownloadJob, Http, Progress};
use crate::paths::AppPaths;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Mojang's key for the current platform in the Java runtime index.
pub fn platform_key() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "aarch64") => "windows-arm64",
        ("windows", "x86") => "windows-x86",
        ("windows", _) => "windows-x64",
        ("macos", "aarch64") => "mac-os-arm64",
        ("macos", _) => "mac-os",
        (_, "x86") => "linux-i386",
        _ => "linux",
    }
}

pub fn java_executable(paths: &AppPaths, component: &str) -> PathBuf {
    let bin = paths.runtimes().join(component).join("bin");
    if cfg!(windows) { bin.join("javaw.exe") } else { bin.join("java") }
}

#[derive(Deserialize)]
struct RuntimeEntry {
    manifest: RuntimeManifestRef,
}

#[derive(Deserialize)]
struct RuntimeManifestRef {
    url: String,
}

#[derive(Deserialize)]
struct RuntimeManifest {
    files: BTreeMap<String, RuntimeFile>,
}

#[derive(Deserialize)]
struct RuntimeFile {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    executable: bool,
    #[serde(default)]
    downloads: Option<RuntimeDownloads>,
    #[serde(default)]
    #[cfg_attr(windows, allow(dead_code))]
    target: Option<String>,
}

#[derive(Deserialize)]
struct RuntimeDownloads {
    raw: RawDownload,
}

#[derive(Deserialize)]
struct RawDownload {
    url: String,
    sha1: String,
    size: u64,
}

pub async fn install_java(
    http: &Http,
    paths: &AppPaths,
    ep: &MojangEndpoints,
    component: &str,
    progress: impl Fn(Progress) + Send + Sync,
) -> Result<PathBuf, InstallError> {
    let exe = java_executable(paths, component);
    let index: BTreeMap<String, BTreeMap<String, Vec<RuntimeEntry>>> = match http.get_json(&ep.java_index).await {
        Ok(i) => i,
        // Offline with a runtime from an earlier install.
        Err(_) if exe.is_file() => return Ok(exe),
        Err(e) => return Err(e.into()),
    };
    let entry = index
        .get(platform_key())
        .and_then(|c| c.get(component))
        .and_then(|v| v.first())
        .ok_or_else(|| InstallError::NoJava(component.into()))?;
    let manifest: RuntimeManifest = http.get_json(&entry.manifest.url).await?;

    let root = paths.runtimes().join(component);
    let mut jobs = Vec::new();
    let mut executables = Vec::new();
    for (rel, f) in &manifest.files {
        let dest = root.join(rel);
        match f.kind.as_str() {
            "directory" => std::fs::create_dir_all(&dest)?,
            "file" => {
                let Some(d) = &f.downloads else { continue };
                if f.executable {
                    executables.push(dest.clone());
                }
                jobs.push(DownloadJob { url: d.raw.url.clone(), dest, sha1: Some(d.raw.sha1.clone()), size: Some(d.raw.size) });
            }
            #[cfg(unix)]
            "link" => {
                if let Some(target) = &f.target {
                    if let Some(parent) = dest.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    let _ = std::fs::remove_file(&dest);
                    std::os::unix::fs::symlink(target, &dest)?;
                }
            }
            // Links only matter on Unix runtimes.
            _ => {}
        }
    }
    http.download_all(jobs, 16, progress).await?;

    #[cfg(unix)]
    for path in executables {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(not(unix))]
    drop(executables);

    if !exe.is_file() {
        return Err(InstallError::NoJava(component.into()));
    }
    Ok(exe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mojang::install::tests::sha;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn fake_java(server: &MockServer) -> MojangEndpoints {
        let u = server.uri();
        let exe = if cfg!(windows) { "bin/javaw.exe" } else { "bin/java" };
        let manifest = json!({"files": {
            "bin": {"type": "directory"},
            exe: {"type": "file", "executable": true, "downloads": {"raw": {"url": format!("{u}/java-bin"), "sha1": sha("java"), "size": 4}}},
            "lib/x": {"type": "link", "target": "../bin"}
        }}).to_string();
        let index = json!({ platform_key(): {"java-runtime-epsilon": [
            {"manifest": {"url": format!("{u}/rt-manifest.json"), "sha1": sha(&manifest), "size": manifest.len()}}
        ]}}).to_string();
        Mock::given(method("GET")).and(path("/all.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(index)).mount(server).await;
        Mock::given(method("GET")).and(path("/rt-manifest.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(manifest)).mount(server).await;
        Mock::given(method("GET")).and(path("/java-bin"))
            .respond_with(ResponseTemplate::new(200).set_body_string("java")).mount(server).await;
        MojangEndpoints { manifest: String::new(), java_index: format!("{u}/all.json"), resources: u }
    }

    #[tokio::test]
    async fn java_install_returns_executable() {
        let server = MockServer::start().await;
        let ep = fake_java(&server).await;
        let dir = tempfile::tempdir().unwrap();
        let p = AppPaths { home: dir.path().to_path_buf() };
        let exe = install_java(&Http::new(), &p, &ep, "java-runtime-epsilon", |_| {}).await.unwrap();
        assert_eq!(exe, java_executable(&p, "java-runtime-epsilon"));
        assert!(exe.is_file());
        if cfg!(windows) {
            assert!(exe.ends_with("bin/javaw.exe"));
        }
    }

    #[tokio::test]
    async fn missing_component_errors() {
        let server = MockServer::start().await;
        let ep = fake_java(&server).await;
        let dir = tempfile::tempdir().unwrap();
        let p = AppPaths { home: dir.path().to_path_buf() };
        let err = install_java(&Http::new(), &p, &ep, "java-runtime-nope", |_| {}).await.unwrap_err();
        assert!(matches!(err, InstallError::NoJava(_)));
    }

    #[tokio::test]
    async fn offline_with_installed_runtime_returns_it() {
        let dir = tempfile::tempdir().unwrap();
        let p = AppPaths { home: dir.path().to_path_buf() };
        let exe = java_executable(&p, "java-runtime-epsilon");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, "java").unwrap();
        let ep = MojangEndpoints { manifest: String::new(), java_index: "http://127.0.0.1:9/all.json".into(), resources: String::new() };
        assert_eq!(install_java(&Http::new(), &p, &ep, "java-runtime-epsilon", |_| {}).await.unwrap(), exe);
    }
}
