use crate::fsutil::write_atomic;
use crate::http::{DownloadJob, HttpError};
use crate::modrinth::{ModVersion, Modrinth, Project};
use crate::paths::AppPaths;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;

/// Mods every instance needs; cannot be disabled or removed.
pub const REQUIRED_SLUGS: [&str; 1] = ["fabric-api"];
/// Performance mods offered on first setup (all preselected).
pub const DEFAULT_CHOICES: [&str; 3] = ["sodium", "lithium", "ferrite-core"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstalledMod {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub version_id: String,
    pub version_number: String,
    pub filename: String,
    pub sha1: String,
    pub enabled: bool,
    pub required: bool,
}

#[derive(Debug, Clone)]
pub struct ModStore {
    pub path: PathBuf,
    pub mods: Vec<InstalledMod>,
}

#[derive(Debug, thiserror::Error)]
pub enum ModError {
    #[error("Keine Version für Minecraft {1}: {0}")]
    NoCompatibleVersion(String, String),
    #[error("Abhängigkeit nicht verfügbar: {0}")]
    MissingDependency(String),
    #[error("Pflicht-Mod: {0}")]
    Required(String),
    #[error(transparent)]
    Http(#[from] HttpError),
    #[error("Dateifehler: {0}")]
    Io(String),
}

impl From<std::io::Error> for ModError {
    fn from(e: std::io::Error) -> Self {
        ModError::Io(e.to_string())
    }
}

impl ModError {
    pub fn user_message(&self) -> String {
        match self {
            ModError::NoCompatibleVersion(m, mc) => format!("{m} gibt es noch nicht für Minecraft {mc} mit Fabric."),
            ModError::MissingDependency(d) => format!("Eine benötigte Abhängigkeit ({d}) ist für diese Version nicht verfügbar."),
            ModError::Required(m) => format!("{m} wird von FrogClient benötigt und kann nicht entfernt oder deaktiviert werden."),
            ModError::Http(e) => format!("Modrinth ist nicht erreichbar ({e})."),
            ModError::Io(e) => format!("Datei konnte nicht gespeichert werden ({e})."),
        }
    }
}

impl ModStore {
    pub fn load(paths: &AppPaths) -> ModStore {
        let path = paths.mods_json();
        let mods = match std::fs::read_to_string(&path) {
            Err(_) => Vec::new(),
            Ok(text) => match serde_json::from_str::<ModsFile>(&text) {
                Ok(f) => f.mods,
                Err(e) => {
                    eprintln!("mods.json is unreadable ({e}), starting empty; backup: mods.json.bak");
                    let _ = std::fs::copy(&path, path.with_extension("json.bak"));
                    Vec::new()
                }
            },
        };
        ModStore { path, mods }
    }

    /// True until every required mod (Fabric API) is installed — not merely until mods.json exists,
    /// since a user may install other mods before the first start.
    pub fn needs_first_setup(&self) -> bool {
        REQUIRED_SLUGS.iter().any(|slug| !self.is_installed(slug))
    }

    pub fn save(&self) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(&ModsFile { mods: self.mods.clone() }).map_err(std::io::Error::other)?;
        write_atomic(&self.path, &json)
    }

    fn is_installed(&self, project: &str) -> bool {
        self.mods.iter().any(|m| m.project_id == project || m.slug == project)
    }

    /// Installs `project` and all required dependencies. All files are downloaded and
    /// verified before mods.json changes, so a failure leaves no partial install.
    pub async fn install(&mut self, mr: &Modrinth, paths: &AppPaths, project: &str, mc: &str) -> Result<Vec<String>, ModError> {
        let mut plan: Vec<(Project, ModVersion)> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<(String, bool)> = VecDeque::from([(project.to_string(), true)]);

        while let Some((wanted, is_root)) = queue.pop_front() {
            if self.is_installed(&wanted) || seen.contains(&wanted) {
                continue;
            }
            let info = match mr.project(&wanted).await {
                Ok(info) => info,
                // A dependency pointing at a deleted project is a missing dependency, not a network problem.
                Err(HttpError::Status(404, _)) if !is_root => return Err(ModError::MissingDependency(wanted)),
                Err(e) => return Err(e.into()),
            };
            if self.is_installed(&info.id) || seen.contains(&info.id) {
                continue;
            }
            seen.extend([wanted.clone(), info.id.clone(), info.slug.clone()]);
            let Some(version) = mr.best_version(&wanted, mc).await? else {
                return Err(if is_root {
                    ModError::NoCompatibleVersion(info.title, mc.to_string())
                } else {
                    ModError::MissingDependency(info.title)
                });
            };
            for dep in version.dependencies.iter().filter(|d| d.dependency_type == "required") {
                let dep_project = match (&dep.project_id, &dep.version_id) {
                    (Some(p), _) => p.clone(),
                    (None, Some(v)) => match mr.version(v).await {
                        Ok(found) => found.project_id,
                        Err(HttpError::Status(404, _)) => return Err(ModError::MissingDependency(v.clone())),
                        Err(e) => return Err(e.into()),
                    },
                    (None, None) => continue,
                };
                queue.push_back((dep_project, false));
            }
            plan.push((info, version));
        }

        let mut jobs = Vec::new();
        let mut new_files = Vec::new();
        for (info, version) in &plan {
            let file = version.primary_file().ok_or_else(|| ModError::MissingDependency(info.title.clone()))?;
            let dest = paths.mods().join(&file.filename);
            if !dest.exists() {
                new_files.push(dest.clone());
            }
            jobs.push(DownloadJob { url: file.url.clone(), dest, sha1: Some(file.hashes.sha1.clone()), size: None });
        }
        if let Err(e) = mr.http().download_all(jobs, 4, |_| {}).await {
            for f in new_files {
                let _ = std::fs::remove_file(f);
            }
            return Err(e.into());
        }

        let mut titles = Vec::new();
        for (info, version) in plan {
            let file = version.primary_file().expect("checked above");
            titles.push(info.title.clone());
            self.mods.push(InstalledMod {
                required: REQUIRED_SLUGS.contains(&info.slug.as_str()),
                project_id: info.id,
                slug: info.slug,
                title: info.title,
                version_id: version.id.clone(),
                version_number: version.version_number.clone(),
                filename: file.filename.clone(),
                sha1: file.hashes.sha1.clone(),
                enabled: true,
            });
        }
        self.save()?;
        Ok(titles)
    }

    fn find_mut(&mut self, project_id: &str) -> Option<&mut InstalledMod> {
        self.mods.iter_mut().find(|m| m.project_id == project_id)
    }

    pub fn set_enabled(&mut self, project_id: &str, enabled: bool) -> Result<(), ModError> {
        let Some(m) = self.find_mut(project_id) else { return Ok(()) };
        if m.required && !enabled {
            return Err(ModError::Required(m.title.clone()));
        }
        m.enabled = enabled;
        self.save()?;
        Ok(())
    }

    pub fn remove(&mut self, project_id: &str, paths: &AppPaths) -> Result<(), ModError> {
        let Some(index) = self.mods.iter().position(|m| m.project_id == project_id) else { return Ok(()) };
        if self.mods[index].required {
            return Err(ModError::Required(self.mods[index].title.clone()));
        }
        let removed = self.mods.remove(index);
        let _ = std::fs::remove_file(paths.mods().join(&removed.filename));
        self.save()?;
        Ok(())
    }

    pub async fn check_updates(&self, mr: &Modrinth, mc: &str) -> Result<Vec<(String, ModVersion)>, ModError> {
        let mut updates = Vec::new();
        for m in &self.mods {
            if let Some(v) = mr.best_version(&m.project_id, mc).await? {
                if v.id != m.version_id {
                    updates.push((m.project_id.clone(), v));
                }
            }
        }
        Ok(updates)
    }

    pub async fn update(&mut self, mr: &Modrinth, paths: &AppPaths, project_id: &str, version: &ModVersion) -> Result<(), ModError> {
        let file = version.primary_file().ok_or_else(|| ModError::MissingDependency(project_id.into()))?;
        let dest = paths.mods().join(&file.filename);
        mr.http()
            .download_all(vec![DownloadJob { url: file.url.clone(), dest, sha1: Some(file.hashes.sha1.clone()), size: None }], 1, |_| {})
            .await?;
        let mods_dir = paths.mods();
        let Some(m) = self.find_mut(project_id) else { return Ok(()) };
        if m.filename != file.filename {
            let _ = std::fs::remove_file(mods_dir.join(&m.filename));
        }
        m.version_id = version.id.clone();
        m.version_number = version.version_number.clone();
        m.filename = file.filename.clone();
        m.sha1 = file.hashes.sha1.clone();
        self.save()?;
        Ok(())
    }

    /// First setup: Fabric API always, plus the chosen default mods.
    pub async fn ensure_defaults(&mut self, mr: &Modrinth, paths: &AppPaths, mc: &str, choices: &[&str]) -> Result<(), ModError> {
        for slug in REQUIRED_SLUGS.iter().chain(choices) {
            if !self.is_installed(slug) {
                self.install(mr, paths, slug, mc).await?;
            }
        }
        self.save()?;
        Ok(())
    }

    pub fn enabled_jars(&self, paths: &AppPaths) -> Vec<PathBuf> {
        self.mods.iter().filter(|m| m.enabled).map(|m| paths.mods().join(&m.filename)).collect()
    }

    /// Titles of installed mods that have no Fabric build for `mc_other` (version switch check).
    // Spec 6.2: prepared for switching Minecraft versions; no UI uses it until a second version exists.
    #[allow(dead_code)]
    pub async fn missing_for(&self, mr: &Modrinth, mc_other: &str) -> Result<Vec<String>, ModError> {
        let mut missing = Vec::new();
        for m in &self.mods {
            if mr.best_version(&m.project_id, mc_other).await?.is_none() {
                missing.push(m.title.clone());
            }
        }
        Ok(missing)
    }
}

/// Shared, serialized access to the mod store: installs, updates and launches queue up
/// instead of working on diverging copies.
#[derive(Clone)]
pub struct ModsHandle(std::sync::Arc<tokio::sync::Mutex<ModStore>>);

impl ModsHandle {
    pub fn new(store: ModStore) -> ModsHandle {
        ModsHandle(std::sync::Arc::new(tokio::sync::Mutex::new(store)))
    }

    pub async fn lock(&self) -> tokio::sync::MutexGuard<'_, ModStore> {
        self.0.lock().await
    }

    pub async fn install(&self, mr: &Modrinth, paths: &AppPaths, project: &str, mc: &str) -> Result<Vec<String>, ModError> {
        self.0.lock().await.install(mr, paths, project, mc).await
    }

    pub async fn snapshot(&self) -> Vec<InstalledMod> {
        self.0.lock().await.mods.clone()
    }

    pub async fn enabled_jars(&self, paths: &AppPaths) -> Vec<PathBuf> {
        self.0.lock().await.enabled_jars(paths)
    }
}

#[derive(Serialize, Deserialize)]
struct ModsFile {
    mods: Vec<InstalledMod>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::Http;
    use crate::modrinth::tests::version;
    use crate::mojang::install::tests::sha;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    struct Fake {
        server: MockServer,
        mr: Modrinth,
        _dir: tempfile::TempDir,
        paths: AppPaths,
    }

    async fn fake() -> Fake {
        let server = MockServer::start().await;
        let mr = Modrinth::new(Http::new(), &server.uri());
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths { home: dir.path().to_path_buf() };
        paths.ensure_dirs().unwrap();
        Fake { server, mr, _dir: dir, paths }
    }

    /// Registers project `slug` with one version; `file_ok=false` makes its download 404.
    async fn project(f: &Fake, slug: &str, vid: &str, deps: serde_json::Value, file_ok: bool) {
        let u = f.server.uri();
        let body = format!("jar-{vid}");
        Mock::given(method("GET")).and(path(format!("/project/{slug}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": format!("id-{slug}"), "slug": slug, "title": slug.to_uppercase()})))
            .mount(&f.server).await;
        Mock::given(method("GET")).and(path(format!("/project/id-{slug}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": format!("id-{slug}"), "slug": slug, "title": slug.to_uppercase()})))
            .mount(&f.server).await;
        let v = version(vid, &format!("id-{slug}"), "release", "2026-10-01T00:00:00Z", &u, &sha(&body), deps);
        for p in [format!("/project/{slug}/version"), format!("/project/id-{slug}/version")] {
            Mock::given(method("GET")).and(path(p))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!([v.clone()]))).mount(&f.server).await;
        }
        let resp = if file_ok { ResponseTemplate::new(200).set_body_string(body) } else { ResponseTemplate::new(404) };
        Mock::given(method("GET")).and(path(format!("/files/{vid}.jar"))).respond_with(resp).mount(&f.server).await;
    }

    fn required_dep(project_id: &str) -> serde_json::Value {
        json!([{"project_id": project_id, "dependency_type": "required"}, {"project_id": "id-optional", "dependency_type": "optional"}])
    }

    #[tokio::test]
    async fn install_pulls_required_dependency() {
        let f = fake().await;
        project(&f, "sodium", "s1", required_dep("id-fabric-api"), true).await;
        project(&f, "fabric-api", "f1", json!([]), true).await;
        let mut store = ModStore::load(&f.paths);
        let installed = store.install(&f.mr, &f.paths, "sodium", "26.3").await.unwrap();
        assert_eq!(installed, vec!["SODIUM".to_string(), "FABRIC-API".to_string()]);
        let reloaded = ModStore::load(&f.paths);
        assert_eq!(reloaded.mods.len(), 2);
        assert!(reloaded.mods.iter().any(|m| m.slug == "fabric-api" && m.required));
        assert!(f.paths.mods().join("s1.jar").is_file());
    }

    #[tokio::test]
    async fn install_is_all_or_nothing() {
        let f = fake().await;
        project(&f, "sodium", "s1", required_dep("id-fabric-api"), true).await;
        project(&f, "fabric-api", "f1", json!([]), false).await;
        let mut store = ModStore::load(&f.paths);
        assert!(store.install(&f.mr, &f.paths, "sodium", "26.3").await.is_err());
        assert!(store.mods.is_empty());
        assert!(!f.paths.mods_json().exists());
        assert!(!f.paths.mods().join("s1.jar").exists());
    }

    #[tokio::test]
    async fn no_compatible_version_error() {
        let f = fake().await;
        Mock::given(method("GET")).and(path("/project/old/version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([]))).mount(&f.server).await;
        Mock::given(method("GET")).and(path("/project/old"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "id-old", "slug": "old", "title": "Old"}))).mount(&f.server).await;
        let mut store = ModStore::load(&f.paths);
        let err = store.install(&f.mr, &f.paths, "old", "26.3").await.unwrap_err();
        assert!(matches!(err, ModError::NoCompatibleVersion(..)), "{err:?}");
        assert!(err.user_message().contains("26.3"));
    }

    #[tokio::test]
    async fn missing_dependency_error() {
        let f = fake().await;
        project(&f, "sodium", "s1", required_dep("id-gone"), true).await;
        Mock::given(method("GET")).and(path("/project/id-gone/version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([]))).mount(&f.server).await;
        Mock::given(method("GET")).and(path("/project/id-gone"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "id-gone", "slug": "gone", "title": "Gone"}))).mount(&f.server).await;
        let mut store = ModStore::load(&f.paths);
        let err = store.install(&f.mr, &f.paths, "sodium", "26.3").await.unwrap_err();
        assert!(matches!(err, ModError::MissingDependency(_)), "{err:?}");
        assert!(store.mods.is_empty());
    }

    #[tokio::test]
    async fn deleted_dependency_project_is_missing_dependency() {
        let f = fake().await;
        project(&f, "sodium", "s1", required_dep("id-deleted"), true).await;
        Mock::given(method("GET")).and(path("/project/id-deleted"))
            .respond_with(ResponseTemplate::new(404)).mount(&f.server).await;
        let mut store = ModStore::load(&f.paths);
        let err = store.install(&f.mr, &f.paths, "sodium", "26.3").await.unwrap_err();
        assert!(matches!(err, ModError::MissingDependency(_)), "{err:?}");
    }

    #[tokio::test]
    async fn first_setup_needed_until_required_mods_installed() {
        let f = fake().await;
        project(&f, "lithium", "l1", json!([]), true).await;
        project(&f, "fabric-api", "f1", json!([]), true).await;
        let mut store = ModStore::load(&f.paths);
        assert!(store.needs_first_setup());
        store.install(&f.mr, &f.paths, "lithium", "26.3").await.unwrap();
        assert!(store.needs_first_setup(), "a user-installed mod must not skip installing Fabric API");
        store.install(&f.mr, &f.paths, "fabric-api", "26.3").await.unwrap();
        assert!(!store.needs_first_setup());
    }

    #[tokio::test]
    async fn concurrent_installs_through_handle_keep_both() {
        let f = fake().await;
        project(&f, "lithium", "l1", json!([]), true).await;
        project(&f, "sodium", "s1", json!([]), true).await;
        let handle = ModsHandle::new(ModStore::load(&f.paths));
        let (a, b) = tokio::join!(
            handle.install(&f.mr, &f.paths, "lithium", "26.3"),
            handle.install(&f.mr, &f.paths, "sodium", "26.3"),
        );
        a.unwrap();
        b.unwrap();
        assert_eq!(handle.snapshot().await.len(), 2);
        assert_eq!(ModStore::load(&f.paths).mods.len(), 2);
        assert_eq!(handle.enabled_jars(&f.paths).await.len(), 2);
    }

    #[tokio::test]
    async fn required_mod_cannot_be_disabled_or_removed() {
        let f = fake().await;
        project(&f, "fabric-api", "f1", json!([]), true).await;
        let mut store = ModStore::load(&f.paths);
        store.install(&f.mr, &f.paths, "fabric-api", "26.3").await.unwrap();
        assert!(matches!(store.set_enabled("id-fabric-api", false), Err(ModError::Required(_))));
        assert!(matches!(store.remove("id-fabric-api", &f.paths), Err(ModError::Required(_))));
    }

    #[tokio::test]
    async fn toggle_and_remove_optional_mod() {
        let f = fake().await;
        project(&f, "lithium", "l1", json!([]), true).await;
        let mut store = ModStore::load(&f.paths);
        store.install(&f.mr, &f.paths, "lithium", "26.3").await.unwrap();
        store.set_enabled("id-lithium", false).unwrap();
        assert!(store.enabled_jars(&f.paths).is_empty());
        store.remove("id-lithium", &f.paths).unwrap();
        assert!(!f.paths.mods().join("l1.jar").exists());
        assert!(ModStore::load(&f.paths).mods.is_empty());
    }

    #[tokio::test]
    async fn check_updates_finds_newer_version() {
        let f = fake().await;
        project(&f, "lithium", "l1", json!([]), true).await;
        let mut store = ModStore::load(&f.paths);
        store.install(&f.mr, &f.paths, "lithium", "26.3").await.unwrap();
        store.mods[0].version_id = "l0".into();
        let updates = store.check_updates(&f.mr, "26.3").await.unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].1.id, "l1");
    }

    #[tokio::test]
    async fn defaults_install_fabric_api_always() {
        let f = fake().await;
        project(&f, "fabric-api", "f1", json!([]), true).await;
        project(&f, "sodium", "s1", json!([]), true).await;
        let mut store = ModStore::load(&f.paths);
        store.ensure_defaults(&f.mr, &f.paths, "26.3", &["sodium"]).await.unwrap();
        let slugs: Vec<&str> = store.mods.iter().map(|m| m.slug.as_str()).collect();
        assert!(slugs.contains(&"fabric-api") && slugs.contains(&"sodium"));
        assert_eq!(store.enabled_jars(&f.paths).len(), 2);
    }

    /// Talks to the real Modrinth API: `cargo test -- --ignored real_modrinth`.
    #[tokio::test]
    #[ignore]
    async fn real_modrinth_installs_modmenu_with_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths { home: dir.path().to_path_buf() };
        paths.ensure_dirs().unwrap();
        let mr = Modrinth::new(Http::new(), crate::modrinth::MODRINTH_API);
        let mut store = ModStore::load(&paths);
        let titles = store.install(&mr, &paths, "modmenu", "26.3").await.unwrap();
        println!("installed: {titles:?}");
        assert!(titles.len() >= 3, "{titles:?}");
        assert!(store.mods.iter().any(|m| m.slug == "fabric-api" && m.required));
        for jar in store.enabled_jars(&paths) {
            assert!(jar.is_file(), "{}", jar.display());
        }
    }

    #[tokio::test]
    async fn missing_for_lists_unavailable_mods() {
        let f = fake().await;
        project(&f, "lithium", "l1", json!([]), true).await;
        let mut store = ModStore::load(&f.paths);
        store.install(&f.mr, &f.paths, "lithium", "26.3").await.unwrap();
        Mock::given(method("GET")).and(path("/project/id-lithium/version"))
            .and(wiremock::matchers::query_param("game_versions", r#"["27.1"]"#))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([]))).with_priority(1).mount(&f.server).await;
        assert_eq!(store.missing_for(&f.mr, "27.1").await.unwrap(), vec!["LITHIUM".to_string()]);
    }
}
