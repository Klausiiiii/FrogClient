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
/// Windows limits one credential to 2560 bytes; longer tokens are split into numbered parts.
const CHUNK: usize = 2000;

fn entry(name: &str) -> Option<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, name).ok()
}

fn read(name: &str) -> Option<String> {
    String::from_utf8(entry(name)?.get_secret().ok()?).ok()
}

fn write(name: &str, value: &str) -> Result<(), String> {
    entry(name).ok_or("credential store unavailable")?.set_secret(value.as_bytes()).map_err(|e| e.to_string())
}

fn remove(name: &str) {
    if let Some(e) = entry(name) {
        let _ = e.delete_credential();
    }
}

impl TokenStore for KeyringStore {
    fn get(&self, id: &str) -> Option<String> {
        let head = read(id)?;
        let Some(n) = head.strip_prefix("chunks:").and_then(|n| n.parse::<usize>().ok()) else { return Some(head) };
        (0..n).map(|i| read(&format!("{id}#{i}"))).collect()
    }
    fn set(&self, id: &str, token: &str) -> Result<(), String> {
        self.delete(id);
        if token.len() <= CHUNK {
            return write(id, token);
        }
        let parts: Vec<&str> = token.as_bytes().chunks(CHUNK).map(|c| std::str::from_utf8(c).map_err(|e| e.to_string())).collect::<Result<_, _>>()?;
        for (i, part) in parts.iter().enumerate() {
            write(&format!("{id}#{i}"), part)?;
        }
        write(id, &format!("chunks:{}", parts.len()))
    }
    fn delete(&self, id: &str) {
        if let Some(n) = read(id).and_then(|h| h.strip_prefix("chunks:").and_then(|n| n.parse::<usize>().ok())) {
            for i in 0..n {
                remove(&format!("{id}#{i}"));
            }
        }
        remove(id);
    }
}

#[cfg(test)]
#[derive(Default)]
pub struct MemoryStore(std::sync::Mutex<std::collections::HashMap<String, String>>);

#[cfg(test)]
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

    /// Uses the real Windows Credential Manager: `cargo test -- --ignored keyring_store`.
    #[test]
    #[ignore]
    fn keyring_store_holds_long_refresh_token() {
        let token: String = "M.C5_BAY.0.U.-".chars().cycle().take(6000).collect();
        let store = KeyringStore;
        store.set("frogclient-selftest", &token).expect("store long token");
        let back = store.get("frogclient-selftest");
        store.delete("frogclient-selftest");
        assert_eq!(back.as_deref(), Some(token.as_str()));
        assert_eq!(store.get("frogclient-selftest"), None);
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
