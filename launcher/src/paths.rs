use std::io;
use std::path::PathBuf;

/// On-disk layout of the launcher's data directory (spec 6.2).
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub home: PathBuf,
}

impl AppPaths {
    /// `FROGCLIENT_HOME` wins (tests/development), otherwise the OS config dir
    /// (`%APPDATA%\FrogClient` on Windows).
    pub fn resolve() -> io::Result<AppPaths> {
        if let Some(home) = std::env::var_os("FROGCLIENT_HOME").filter(|v| !v.is_empty()) {
            return Ok(AppPaths { home: PathBuf::from(home) });
        }
        let base = directories::BaseDirs::new()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home directory"))?;
        Ok(AppPaths { home: base.config_dir().join("FrogClient") })
    }

    pub fn ensure_dirs(&self) -> io::Result<()> {
        for dir in [
            self.versions(),
            self.libraries(),
            self.assets(),
            self.runtimes(),
            self.mods(),
            self.config(),
            self.shared_gamedata(),
            self.instances(),
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    fn shared(&self) -> PathBuf {
        self.home.join("shared")
    }
    pub fn versions(&self) -> PathBuf {
        self.shared().join("versions")
    }
    pub fn libraries(&self) -> PathBuf {
        self.shared().join("libraries")
    }
    pub fn assets(&self) -> PathBuf {
        self.shared().join("assets")
    }
    pub fn runtimes(&self) -> PathBuf {
        self.shared().join("runtimes")
    }
    pub fn mods(&self) -> PathBuf {
        self.home.join("mods")
    }
    pub fn mods_json(&self) -> PathBuf {
        self.mods().join("mods.json")
    }
    pub fn config(&self) -> PathBuf {
        self.home.join("config")
    }
    pub fn shared_gamedata(&self) -> PathBuf {
        self.home.join("shared-gamedata")
    }
    pub fn instances(&self) -> PathBuf {
        self.home.join("instances")
    }
    pub fn instance(&self, id: &str) -> PathBuf {
        self.instances().join(id)
    }
    pub fn launcher_json(&self) -> PathBuf {
        self.home.join("launcher.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_spec() {
        let p = AppPaths { home: PathBuf::from("H") };
        assert_eq!(p.versions(), PathBuf::from("H/shared/versions"));
        assert_eq!(p.libraries(), PathBuf::from("H/shared/libraries"));
        assert_eq!(p.assets(), PathBuf::from("H/shared/assets"));
        assert_eq!(p.runtimes(), PathBuf::from("H/shared/runtimes"));
        assert_eq!(p.mods(), PathBuf::from("H/mods"));
        assert_eq!(p.mods_json(), PathBuf::from("H/mods/mods.json"));
        assert_eq!(p.config(), PathBuf::from("H/config"));
        assert_eq!(p.shared_gamedata(), PathBuf::from("H/shared-gamedata"));
        assert_eq!(p.instance("abc"), PathBuf::from("H/instances/abc"));
        assert_eq!(p.launcher_json(), PathBuf::from("H/launcher.json"));
    }

    #[test]
    fn env_override_wins() {
        let dir = tempfile::tempdir().unwrap();
        // SAFETY: only this test touches FROGCLIENT_HOME.
        unsafe { std::env::set_var("FROGCLIENT_HOME", dir.path()) };
        let p = AppPaths::resolve().unwrap();
        unsafe { std::env::remove_var("FROGCLIENT_HOME") };
        assert_eq!(p.home, dir.path());
    }

    #[test]
    fn ensure_dirs_creates_all() {
        let dir = tempfile::tempdir().unwrap();
        let p = AppPaths { home: dir.path().to_path_buf() };
        p.ensure_dirs().unwrap();
        for d in [
            p.versions(),
            p.libraries(),
            p.assets(),
            p.runtimes(),
            p.mods(),
            p.config(),
            p.shared_gamedata(),
            p.instances(),
        ] {
            assert!(d.is_dir(), "{} missing", d.display());
        }
    }
}
