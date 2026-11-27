//! Secret settings (API keys, tokens).
//!
//! Stored in the system's password store: Windows Credential Manager, macOS
//! Keychain, or the Secret Service on Linux (GNOME Keyring, KWallet). Only if no
//! store is available do they fall back to an owner-only file, and the Hub says so.
//! Widget code never sees a secret: the core puts it into requests.

use serde_json::{Map, Value};
use std::path::PathBuf;

const SERVICE: &str = "WidgetForge";

#[derive(Clone, Copy, PartialEq)]
pub enum Backend {
    Keychain,
    File,
}

pub struct Secrets {
    dir: PathBuf,
    pub backend: Backend,
}

fn entry(instance: &str, key: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, &format!("{instance}/{key}"))
}

impl Secrets {
    /// Uses the system store if a harmless read works (a missing entry is fine).
    pub fn new(dir: PathBuf) -> Self {
        let works = match entry("probe", "probe").and_then(|e| e.get_password()) {
            Ok(_) | Err(keyring::Error::NoEntry) => true,
            Err(e) => {
                eprintln!("system password store not available, using a file: {e}");
                false
            }
        };
        let s = Self { dir, backend: if works { Backend::Keychain } else { Backend::File } };
        if works {
            s.migrate_files();
        }
        s
    }

    #[cfg(test)]
    pub fn file_only(dir: PathBuf) -> Self {
        Self { dir, backend: Backend::File }
    }

    pub fn backend_name(&self) -> &'static str {
        match self.backend {
            Backend::File => "a file only your user account can read (no system password store was found)",
            Backend::Keychain if cfg!(target_os = "windows") => "Windows Credential Manager",
            Backend::Keychain if cfg!(target_os = "macos") => "the macOS Keychain",
            Backend::Keychain => "the system keyring (Secret Service)",
        }
    }

    fn file(&self, instance: &str) -> PathBuf {
        self.dir.join(format!("{instance}.json"))
    }

    fn read_file(&self, instance: &str) -> Map<String, Value> {
        crate::model::read_json(&self.file(instance))
    }

    fn write_file(&self, instance: &str, map: &Map<String, Value>) -> Result<(), String> {
        let file = self.file(instance);
        if map.is_empty() {
            let _ = std::fs::remove_file(&file);
            return Ok(());
        }
        crate::model::write_json(&file, map)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn get(&self, instance: &str, key: &str) -> Option<String> {
        if self.backend == Backend::Keychain {
            if let Ok(v) = entry(instance, key).and_then(|e| e.get_password()) {
                return Some(v);
            }
        }
        self.read_file(instance).get(key)?.as_str().map(String::from)
    }

    pub fn set(&self, instance: &str, key: &str, value: &str) -> Result<(), String> {
        match self.backend {
            Backend::Keychain => entry(instance, key)
                .and_then(|e| e.set_password(value))
                .map_err(|e| format!("could not save to {}: {e}", self.backend_name())),
            Backend::File => {
                let mut map = self.read_file(instance);
                map.insert(key.into(), Value::String(value.into()));
                self.write_file(instance, &map)
            }
        }
    }

    pub fn delete(&self, instance: &str, key: &str) {
        if self.backend == Backend::Keychain {
            let _ = entry(instance, key).and_then(|e| e.delete_credential());
        }
        let mut map = self.read_file(instance);
        if map.remove(key).is_some() {
            let _ = self.write_file(instance, &map);
        }
    }

    /// Moves secrets saved in files by earlier versions into the system store.
    fn migrate_files(&self) {
        let Ok(dir) = std::fs::read_dir(&self.dir) else { return };
        for f in dir.flatten() {
            let path = f.path();
            let Some(instance) = path.file_stem().and_then(|s| s.to_str()).map(String::from) else { continue };
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let map = self.read_file(&instance);
            let mut left = Map::new();
            for (k, v) in map {
                let ok = v.as_str().is_some_and(|s| self.set(&instance, &k, s).is_ok());
                if !ok {
                    left.insert(k, v);
                }
            }
            let _ = self.write_file(&instance, &left);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("widgetforge-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn file_fallback_round_trip() {
        let s = Secrets { dir: temp_dir("file"), backend: Backend::File };
        s.set("inst1", "api_key", "abc123").unwrap();
        assert_eq!(s.get("inst1", "api_key").as_deref(), Some("abc123"));
        assert_eq!(s.get("inst2", "api_key"), None);
        s.delete("inst1", "api_key");
        assert_eq!(s.get("inst1", "api_key"), None);
        assert!(!s.file("inst1").exists(), "empty secrets file is removed");
    }

    /// Uses the real system keychain. Run with: cargo test -- --ignored keychain
    #[test]
    #[ignore]
    fn keychain_round_trip_and_migration() {
        let dir = temp_dir("keychain");
        // An old-style file secret is moved into the keychain on start.
        crate::model::write_json(&dir.join("wf-test-inst.json"), &serde_json::json!({ "old": "from-file" })).unwrap();
        let s = Secrets::new(dir.clone());
        assert!(s.backend == Backend::Keychain, "no system keychain available");
        assert_eq!(s.get("wf-test-inst", "old").as_deref(), Some("from-file"));
        assert!(!dir.join("wf-test-inst.json").exists(), "file removed after migration");
        s.set("wf-test-inst", "api_key", "secret-value").unwrap();
        assert_eq!(s.get("wf-test-inst", "api_key").as_deref(), Some("secret-value"));
        s.delete("wf-test-inst", "api_key");
        s.delete("wf-test-inst", "old");
        assert_eq!(s.get("wf-test-inst", "api_key"), None);
        assert_eq!(s.get("wf-test-inst", "old"), None);
    }
}
