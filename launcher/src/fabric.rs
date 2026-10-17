use crate::http::{DownloadJob, Http, HttpError, Progress};
use crate::mojang::install::{InstallError, InstalledVersion};
use crate::mojang::model::VersionJson;
use crate::paths::AppPaths;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const FABRIC_META: &str = "https://meta.fabricmc.net";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricProfile {
    pub id: String,
    pub main_class: String,
    pub arguments: FabricArguments,
    pub libraries: Vec<FabricLibrary>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FabricArguments {
    #[serde(default)]
    pub game: Vec<String>,
    #[serde(default)]
    pub jvm: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FabricLibrary {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

/// Everything needed to build the java command line for a Fabric instance.
#[derive(Debug, Clone)]
pub struct LaunchProfile {
    pub main_class: String,
    pub classpath: Vec<PathBuf>,
    pub extra_jvm_args: Vec<String>,
    pub extra_game_args: Vec<String>,
    pub json: VersionJson,
    pub logging_config: Option<PathBuf>,
}

pub async fn fetch_profile(http: &Http, base: &str, mc: &str, loader: &str) -> Result<FabricProfile, HttpError> {
    http.get_json(&format!("{base}/v2/versions/loader/{mc}/{loader}/profile/json")).await
}

/// `group:artifact:version[:classifier]` → `group/path/artifact/version/artifact-version[-classifier].jar`
pub fn maven_path(coord: &str) -> String {
    let parts: Vec<&str> = coord.split(':').collect();
    let (group, artifact, version) = (parts[0], parts[1], parts.get(2).copied().unwrap_or(""));
    let file = match parts.get(3) {
        Some(classifier) => format!("{artifact}-{version}-{classifier}.jar"),
        None => format!("{artifact}-{version}.jar"),
    };
    format!("{}/{artifact}/{version}/{file}", group.replace('.', "/"))
}

/// Identity of a library independent of its version: `group:artifact[:classifier]`.
pub fn key_from_coord(coord: &str) -> String {
    let parts: Vec<&str> = coord.split(':').collect();
    match parts.get(3) {
        Some(classifier) => format!("{}:{}:{classifier}", parts[0], parts[1]),
        None => format!("{}:{}", parts[0], parts.get(1).copied().unwrap_or("")),
    }
}

/// Same identity derived from a path relative to the libraries directory.
pub fn key_from_path(rel: &Path) -> Option<String> {
    let segs: Vec<String> = rel.iter().map(|s| s.to_string_lossy().into_owned()).collect();
    if segs.len() < 4 {
        return None;
    }
    let n = segs.len();
    let (file, version, artifact) = (&segs[n - 1], &segs[n - 2], &segs[n - 3]);
    let group = segs[..n - 3].join(".");
    let stem = file.strip_suffix(".jar")?;
    let base = format!("{artifact}-{version}");
    match stem.strip_prefix(&base)?.strip_prefix('-') {
        Some(classifier) => Some(format!("{group}:{artifact}:{classifier}")),
        None => Some(format!("{group}:{artifact}")),
    }
}

/// Downloads the Fabric libraries (profile cached for offline starts) and returns profile + jar paths.
pub async fn install_fabric(
    http: &Http,
    paths: &AppPaths,
    base: &str,
    mc: &str,
    loader: &str,
    progress: impl Fn(Progress) + Send + Sync,
) -> Result<(FabricProfile, Vec<PathBuf>), InstallError> {
    let cache = paths.versions().join(format!("fabric-loader-{loader}-{mc}")).join("profile.json");
    let profile = match fetch_profile(http, base, mc, loader).await {
        Ok(p) => {
            std::fs::create_dir_all(cache.parent().unwrap())?;
            std::fs::write(&cache, serde_json::to_vec_pretty(&p).map_err(|e| InstallError::Parse(e.to_string()))?)?;
            p
        }
        Err(_) if cache.is_file() => serde_json::from_slice(&std::fs::read(&cache)?)
            .map_err(|e| InstallError::Parse(format!("fabric profile: {e}")))?,
        Err(e) => return Err(e.into()),
    };

    let mut jobs = Vec::new();
    let mut lib_paths = Vec::new();
    for lib in &profile.libraries {
        let rel = maven_path(&lib.name);
        let dest = paths.libraries().join(&rel);
        lib_paths.push(dest.clone());
        let url = format!("{}/{rel}", lib.url.trim_end_matches('/'));
        jobs.push(DownloadJob { url, dest, sha1: lib.sha1.clone(), size: lib.size });
    }
    http.download_all(jobs, 8, progress).await?;
    Ok((profile, lib_paths))
}

/// Fabric libraries first, then vanilla libraries that Fabric does not replace (same
/// group:artifact:classifier), then the client jar.
pub fn merge(paths: &AppPaths, vanilla: &InstalledVersion, fabric: &FabricProfile, fabric_lib_paths: Vec<PathBuf>) -> LaunchProfile {
    let fabric_keys: HashSet<String> = fabric.libraries.iter().map(|l| key_from_coord(&l.name)).collect();
    let mut classpath = fabric_lib_paths;
    for lib in &vanilla.library_paths {
        let key = lib.strip_prefix(paths.libraries()).ok().and_then(key_from_path);
        if key.is_some_and(|k| fabric_keys.contains(&k)) {
            continue;
        }
        classpath.push(lib.clone());
    }
    classpath.push(vanilla.client_jar.clone());
    LaunchProfile {
        main_class: fabric.main_class.clone(),
        classpath,
        extra_jvm_args: fabric.arguments.jvm.clone(),
        extra_game_args: fabric.arguments.game.clone(),
        json: vanilla.json.clone(),
        logging_config: vanilla.logging_config.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mojang::model::VersionJson;

    fn fixture() -> FabricProfile {
        serde_json::from_str(include_str!("../tests/fixtures/fabric-profile.json")).unwrap()
    }

    fn vanilla(paths: &AppPaths, libs: &[&str]) -> InstalledVersion {
        let json: VersionJson = serde_json::from_str(include_str!("../tests/fixtures/26.3.json")).unwrap();
        InstalledVersion {
            json,
            client_jar: paths.versions().join("26.3/26.3.jar"),
            library_paths: libs.iter().map(|l| paths.libraries().join(l)).collect(),
            logging_config: None,
        }
    }

    #[test]
    fn maven_path_basic() {
        assert_eq!(maven_path("org.ow2.asm:asm:9.10.1"), "org/ow2/asm/asm/9.10.1/asm-9.10.1.jar");
    }

    #[test]
    fn maven_path_classifier() {
        assert_eq!(maven_path("a.b:c:1:natives-windows"), "a/b/c/1/c-1-natives-windows.jar");
    }

    #[test]
    fn keys_from_coord_and_path_agree() {
        assert_eq!(key_from_coord("org.ow2.asm:asm:9.6"), "org.ow2.asm:asm");
        assert_eq!(key_from_path(Path::new("org/ow2/asm/asm/9.6/asm-9.6.jar")).unwrap(), "org.ow2.asm:asm");
        assert_eq!(
            key_from_path(Path::new("org/lwjgl/lwjgl/3.4.3/lwjgl-3.4.3-natives-windows.jar")).unwrap(),
            key_from_coord("org.lwjgl:lwjgl:3.4.3:natives-windows")
        );
        assert_ne!(key_from_coord("org.lwjgl:lwjgl:3.4.3:natives-windows"), key_from_coord("org.lwjgl:lwjgl:3.4.3"));
    }

    #[test]
    fn parses_fixture_main_class() {
        assert_eq!(fixture().main_class, "net.fabricmc.loader.impl.launch.knot.KnotClient");
    }

    #[test]
    fn merge_dedups_by_group_artifact_fabric_wins() {
        let p = AppPaths { home: PathBuf::from("H") };
        let v = vanilla(&p, &["org/ow2/asm/asm/9.6/asm-9.6.jar", "org/lwjgl/lwjgl/3.4.3/lwjgl-3.4.3.jar",
            "org/lwjgl/lwjgl/3.4.3/lwjgl-3.4.3-natives-windows.jar"]);
        let f = fixture();
        let fabric_paths: Vec<PathBuf> = f.libraries.iter().map(|l| p.libraries().join(maven_path(&l.name))).collect();
        let lp = merge(&p, &v, &f, fabric_paths);
        let cp: Vec<String> = lp.classpath.iter().map(|x| x.to_string_lossy().replace('\\', "/")).collect();
        assert!(cp.iter().any(|c| c.ends_with("asm-9.10.1.jar")));
        assert!(!cp.iter().any(|c| c.ends_with("asm-9.6.jar")));
        assert!(cp.iter().any(|c| c.ends_with("lwjgl-3.4.3-natives-windows.jar")));
        assert!(cp.iter().any(|c| c.ends_with("lwjgl-3.4.3.jar")));
        assert_eq!(lp.main_class, "net.fabricmc.loader.impl.launch.knot.KnotClient");
    }

    #[test]
    fn merge_puts_client_jar_last() {
        let p = AppPaths { home: PathBuf::from("H") };
        let v = vanilla(&p, &["x/y/1/y-1.jar"]);
        let lp = merge(&p, &v, &fixture(), vec![]);
        assert_eq!(lp.classpath.last().unwrap(), &v.client_jar);
    }
}
