//! Small helpers for crash-safe JSON files.

use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::Path;

use crate::error::Result;

/// Reads a JSON file. A missing file yields the default. A corrupt file is renamed to
/// `<name>.bad` (so the user's data is not silently lost) and the default is returned.
pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    let Ok(text) = fs::read_to_string(path) else {
        return T::default();
    };
    match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("[widgetforge] {} is corrupt ({err}); keeping a copy as .bad", path.display());
            let mut bad = path.as_os_str().to_owned();
            bad.push(".bad");
            let _ = fs::rename(path, bad);
            T::default()
        }
    }
}

/// Writes JSON atomically: write a temp file next to the target, then rename over it.
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn roundtrip_and_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub/x.json");
        let mut m = BTreeMap::new();
        m.insert("a".to_string(), 1);
        save_json(&p, &m).unwrap();
        let back: BTreeMap<String, i32> = load_json(&p);
        assert_eq!(back, m);

        fs::write(&p, "{ not json").unwrap();
        let back: BTreeMap<String, i32> = load_json(&p);
        assert!(back.is_empty());
        assert!(dir.path().join("sub/x.json.bad").exists());
    }
}
