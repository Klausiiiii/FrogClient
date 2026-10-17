use crate::fsutil::write_atomic;
use crate::settings::LauncherSettings;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AccountKind {
    Microsoft { owns_game: bool },
    Offline,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
    /// Minecraft UUID without dashes; also the instance directory name.
    pub id: String,
    pub name: String,
    pub kind: AccountKind,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherFile {
    pub accounts: Vec<Account>,
    pub selected: Vec<String>,
    pub settings: LauncherSettings,
}

pub fn load_launcher_file(path: &Path) -> LauncherFile {
    let Ok(text) = std::fs::read_to_string(path) else { return LauncherFile::default() };
    match serde_json::from_str(&text) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("launcher.json is unreadable ({e}), using defaults; backup: launcher.json.bak");
            let _ = std::fs::copy(path, path.with_extension("json.bak"));
            LauncherFile::default()
        }
    }
}

pub fn save_launcher_file(path: &Path, file: &LauncherFile) -> io::Result<()> {
    let json = serde_json::to_vec_pretty(file).map_err(io::Error::other)?;
    write_atomic(path, &json)
}

/// Offline accounts need a Microsoft account that owns the game (lifted in `dev` builds).
pub fn can_add_offline(accounts: &[Account]) -> bool {
    cfg!(feature = "dev") || accounts.iter().any(|a| a.kind == AccountKind::Microsoft { owns_game: true })
}

/// Where Microsoft refresh tokens live. Never a file.
pub trait TokenStore: Send + Sync {
    fn get(&self, id: &str) -> Option<String>;
    fn set(&self, id: &str, token: &str) -> Result<(), String>;
    fn delete(&self, id: &str);
}

/// OS credential store (Windows Credential Manager).
pub struct KeyringStore;

const KEYRING_SERVICE: &str = "FrogClient";

impl TokenStore for KeyringStore {
    fn get(&self, id: &str) -> Option<String> {
        keyring::Entry::new(KEYRING_SERVICE, id).ok()?.get_password().ok()
    }
    fn set(&self, id: &str, token: &str) -> Result<(), String> {
        keyring::Entry::new(KEYRING_SERVICE, id)
            .and_then(|e| e.set_password(token))
            .map_err(|e| e.to_string())
    }
    fn delete(&self, id: &str) {
        if let Ok(e) = keyring::Entry::new(KEYRING_SERVICE, id) {
            let _ = e.delete_credential();
        }
    }
}

#[derive(Default)]
pub struct MemoryStore(std::sync::Mutex<std::collections::HashMap<String, String>>);

impl TokenStore for MemoryStore {
    fn get(&self, id: &str) -> Option<String> {
        self.0.lock().unwrap().get(id).cloned()
    }
    fn set(&self, id: &str, token: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(id.into(), token.into());
        Ok(())
    }
    fn delete(&self, id: &str) {
        self.0.lock().unwrap().remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msa(owns: bool) -> Account {
        Account { id: "a".into(), name: "A".into(), kind: AccountKind::Microsoft { owns_game: owns } }
    }

    #[cfg(not(feature = "dev"))]
    #[test]
    fn offline_gated_without_owned_msa() {
        assert!(!can_add_offline(&[]));
        assert!(!can_add_offline(&[msa(false)]));
        assert!(can_add_offline(&[msa(true)]));
    }

    #[cfg(feature = "dev")]
    #[test]
    fn dev_build_allows_offline() {
        assert!(can_add_offline(&[]));
    }

    #[test]
    fn launcher_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("launcher.json");
        let f = LauncherFile {
            accounts: vec![msa(true), Account { id: "b".into(), name: "B".into(), kind: AccountKind::Offline }],
            selected: vec!["b".into()],
            settings: LauncherSettings { heap_mb: 4096, ..Default::default() },
        };
        save_launcher_file(&path, &f).unwrap();
        assert_eq!(load_launcher_file(&path), f);
        assert!(!std::fs::read_to_string(&path).unwrap().to_lowercase().contains("token"));
    }

    #[test]
    fn corrupt_launcher_file_defaults_and_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("launcher.json");
        std::fs::write(&path, "{kaputt").unwrap();
        assert_eq!(load_launcher_file(&path), LauncherFile::default());
        assert_eq!(std::fs::read_to_string(dir.path().join("launcher.json.bak")).unwrap(), "{kaputt");
    }

    #[test]
    fn missing_file_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_launcher_file(&dir.path().join("nope.json")), LauncherFile::default());
    }

    #[test]
    fn defaults() {
        let s = LauncherSettings::default();
        assert_eq!(s.heap_mb, 3072);
        assert_eq!(s.loader_version, "0.19.5");
        assert!(!s.minimize_on_launch);
    }

    #[test]
    fn memory_store_roundtrip() {
        let s = MemoryStore::default();
        s.set("a", "t").unwrap();
        assert_eq!(s.get("a").as_deref(), Some("t"));
        s.delete("a");
        assert_eq!(s.get("a"), None);
    }
}
