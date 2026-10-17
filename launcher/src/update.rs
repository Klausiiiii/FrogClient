use crate::http::{DownloadJob, Http, HttpError};
use crate::paths::AppPaths;
use serde::Deserialize;
use std::path::PathBuf;

pub const GITHUB_API: &str = "https://api.github.com";
pub const REPO: &str = "Klausiiiii/FrogClient";

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    #[serde(rename = "tag_name")]
    pub tag: String,
    pub html_url: String,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
}

/// Latest GitHub release; `None` while the repository has no releases.
pub async fn latest_release(http: &Http, api_base: &str, repo: &str) -> Result<Option<Release>, HttpError> {
    match http.get_json::<Release>(&format!("{api_base}/repos/{repo}/releases/latest")).await {
        Ok(r) => Ok(Some(r)),
        Err(HttpError::Status(404, _)) => Ok(None),
        Err(e) => Err(e),
    }
}

fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let mut it = s.trim().trim_start_matches(['v', 'V']).split(['.', '-', '+']);
    let major = it.next()?.parse().ok()?;
    let minor = it.next().unwrap_or("0").parse().ok()?;
    let patch = it.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

/// `v`-prefix tolerant numeric comparison of `major.minor.patch`.
pub fn is_newer(tag: &str, current: &str) -> bool {
    match (parse_version(tag), parse_version(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

/// The FrogClient mod jar to put into every instance: `FROGCLIENT_MOD_JAR` (development),
/// else the release asset (cached per tag), else the last cached jar, else none.
pub async fn frog_mod_jar(http: &Http, paths: &AppPaths, release: Option<&Release>) -> Option<PathBuf> {
    if let Some(dev) = std::env::var_os("FROGCLIENT_MOD_JAR").map(PathBuf::from) {
        if dev.is_file() {
            return Some(dev);
        }
    }
    let cache_dir = paths.home.join("shared").join("frogclient");
    if let Some(release) = release {
        let asset = release.assets.iter().find(|a| a.name.starts_with("frogclient") && a.name.ends_with(".jar"));
        if let Some(asset) = asset {
            let dest = cache_dir.join(format!("{}.jar", release.tag));
            if dest.is_file() {
                return Some(dest);
            }
            let job = DownloadJob { url: asset.browser_download_url.clone(), dest: dest.clone(), sha1: None, size: None };
            if http.download_all(vec![job], 1, |_| {}).await.is_ok() {
                return Some(dest);
            }
        }
    }
    newest_jar(&cache_dir)
}

fn newest_jar(dir: &std::path::Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jar"))
        .filter_map(|p| std::fs::metadata(&p).and_then(|m| m.modified()).ok().map(|t| (t, p)))
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Tests reading or writing FROGCLIENT_MOD_JAR must not run concurrently.
    static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[test]
    fn version_compare() {
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.12"));
        assert!(!is_newer("v0.1.0", "0.2.0"));
        assert!(!is_newer("garbage", "0.1.0"));
    }

    #[tokio::test]
    async fn no_releases_is_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/repos/o/r/releases/latest"))
            .respond_with(ResponseTemplate::new(404)).mount(&server).await;
        assert!(latest_release(&Http::new(), &server.uri(), "o/r").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn parses_release() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/repos/o/r/releases/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "tag_name": "v0.2.0", "html_url": "https://github.com/o/r/releases/v0.2.0",
                "assets": [{"name": "frogclient-0.2.0.jar", "browser_download_url": format!("{}/dl/frog.jar", server.uri())}]
            }))).mount(&server).await;
        let r = latest_release(&Http::new(), &server.uri(), "o/r").await.unwrap().unwrap();
        assert_eq!(r.tag, "v0.2.0");
        assert_eq!(r.assets[0].name, "frogclient-0.2.0.jar");
    }

    #[tokio::test]
    async fn release_asset_downloaded_and_cached() {
        let _env = ENV_LOCK.lock().await;
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/dl/frog.jar"))
            .respond_with(ResponseTemplate::new(200).set_body_string("JAR")).expect(1).mount(&server).await;
        let dir = tempfile::tempdir().unwrap();
        let p = AppPaths { home: dir.path().to_path_buf() };
        let release = Release {
            tag: "v0.2.0".into(),
            html_url: String::new(),
            assets: vec![ReleaseAsset { name: "frogclient-0.2.0.jar".into(), browser_download_url: format!("{}/dl/frog.jar", server.uri()) }],
        };
        let jar = frog_mod_jar(&Http::new(), &p, Some(&release)).await.unwrap();
        assert_eq!(std::fs::read_to_string(&jar).unwrap(), "JAR");
        // Second call uses the cache (mock expects exactly one download).
        assert_eq!(frog_mod_jar(&Http::new(), &p, Some(&release)).await.unwrap(), jar);
        // Offline / no release: the last cached jar is still used.
        assert_eq!(frog_mod_jar(&Http::new(), &p, None).await.unwrap(), jar);
    }

    #[tokio::test]
    async fn env_jar_wins_and_none_without_sources() {
        let _env = ENV_LOCK.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let p = AppPaths { home: dir.path().to_path_buf() };
        assert!(frog_mod_jar(&Http::new(), &p, None).await.is_none());
        let local = dir.path().join("local.jar");
        std::fs::write(&local, "dev").unwrap();
        // SAFETY: only this test touches FROGCLIENT_MOD_JAR.
        unsafe { std::env::set_var("FROGCLIENT_MOD_JAR", &local) };
        let got = frog_mod_jar(&Http::new(), &p, None).await;
        unsafe { std::env::remove_var("FROGCLIENT_MOD_JAR") };
        assert_eq!(got.unwrap(), local);
    }
}
