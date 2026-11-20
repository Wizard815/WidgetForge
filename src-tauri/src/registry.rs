//! Finds widget packages in the built-in and add-ons folders.

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::manifest::{self, Issue, Manifest};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Builtin,
    Addon,
}

#[derive(Debug, Clone, Serialize)]
pub struct WidgetEntry {
    pub manifest: Manifest,
    #[serde(skip)]
    pub dir: PathBuf,
    pub origin: Origin,
    pub warnings: Vec<Issue>,
}

/// A folder that looked like a widget but could not be loaded.
#[derive(Debug, Clone, Serialize)]
pub struct LoadError {
    pub folder: String,
    pub issues: Vec<Issue>,
}

#[derive(Debug, Default, Clone)]
pub struct Registry {
    pub widgets: BTreeMap<String, WidgetEntry>,
    pub errors: Vec<LoadError>,
}

impl Registry {
    /// Scans `builtin` first, then `addons`. An add-on cannot replace a built-in widget.
    pub fn scan(builtin: &Path, addons: &Path) -> Registry {
        let mut reg = Registry::default();
        reg.scan_dir(builtin, Origin::Builtin);
        reg.scan_dir(addons, Origin::Addon);
        reg
    }

    fn scan_dir(&mut self, root: &Path, origin: Origin) {
        let Ok(read) = std::fs::read_dir(root) else { return };
        let mut dirs: Vec<PathBuf> = read
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && p.join(manifest::MANIFEST_FILE).is_file())
            .collect();
        dirs.sort();
        for dir in dirs {
            let folder = dir.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let (m, issues) = manifest::load_dir(&dir);
            let m = match m {
                Some(m) if !manifest::has_errors(&issues) => m,
                _ => {
                    self.errors.push(LoadError { folder, issues });
                    continue;
                }
            };
            if let Some(existing) = self.widgets.get(&m.id) {
                let what = match existing.origin {
                    Origin::Builtin => "a built-in widget",
                    Origin::Addon => "another add-on",
                };
                self.errors.push(LoadError {
                    folder,
                    issues: vec![Issue {
                        level: manifest::Level::Error,
                        path: "id".into(),
                        message: format!("id '{}' is already used by {what}", m.id),
                    }],
                });
                continue;
            }
            self.widgets.insert(m.id.clone(), WidgetEntry { manifest: m, dir, origin, warnings: issues });
        }
    }

    pub fn get(&self, id: &str) -> Option<&WidgetEntry> {
        self.widgets.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_widget(root: &Path, folder: &str, id: &str) {
        let d = root.join(folder);
        fs::create_dir_all(&d).unwrap();
        fs::write(
            d.join("widget.yaml"),
            format!("schema: 1\nid: {id}\nname: X\nversion: 1.0.0\nui:\n  blocks:\n    - {{ type: text, text: hi }}\n"),
        )
        .unwrap();
    }

    #[test]
    fn builtin_widgets_are_valid() {
        let builtin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../widgets");
        let reg = Registry::scan(&builtin, Path::new("/nonexistent"));
        assert!(reg.errors.is_empty(), "{:#?}", reg.errors);
        for (id, e) in &reg.widgets {
            assert!(e.warnings.is_empty(), "{id}: {:#?}", e.warnings);
        }
        assert!(reg.widgets.len() >= 6);
    }

    #[test]
    fn scans_and_rejects_conflicts() {
        let tmp = tempfile::tempdir().unwrap();
        let builtin = tmp.path().join("builtin");
        let addons = tmp.path().join("addons");
        write_widget(&builtin, "a", "forge.a");
        write_widget(&addons, "b", "me.b");
        write_widget(&addons, "evil", "forge.a");
        fs::create_dir_all(addons.join("broken")).unwrap();
        fs::write(addons.join("broken/widget.yaml"), "schema: 1\nid: nope\n").unwrap();
        fs::create_dir_all(addons.join("not-a-widget")).unwrap();

        let reg = Registry::scan(&builtin, &addons);
        assert_eq!(reg.widgets.len(), 2);
        assert_eq!(reg.get("forge.a").unwrap().origin, Origin::Builtin);
        assert_eq!(reg.get("me.b").unwrap().origin, Origin::Addon);
        let failed: Vec<_> = reg.errors.iter().map(|e| e.folder.as_str()).collect();
        assert_eq!(failed, vec!["broken", "evil"]);
    }
}
