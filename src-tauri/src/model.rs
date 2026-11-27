//! Widget registry, desktop layout and per-instance storage.

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// One installed widget package.
#[derive(Clone, Serialize)]
pub struct WidgetInfo {
    pub id: String,
    /// "built-in" or "local"
    pub source: String,
    #[serde(skip)]
    pub dir: PathBuf,
    pub manifest: Value,
}

impl WidgetInfo {
    /// The `permissions.system` list, e.g. ["cpu", "memory"].
    pub fn system_permissions(&self) -> Vec<String> {
        self.manifest
            .pointer("/permissions/system")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    }

    /// The `permissions.network` list: hostnames, optionally with a port.
    pub fn network_hosts(&self) -> Vec<String> {
        self.manifest
            .pointer("/permissions/network")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    }

    /// A yes/no permission such as `permissions.clipboard` or `permissions.media`.
    pub fn permission(&self, name: &str) -> bool {
        self.manifest.pointer(&format!("/permissions/{name}")).and_then(Value::as_bool) == Some(true)
    }

    /// Keys of settings with `type: secret`.
    pub fn secret_keys(&self) -> Vec<String> {
        self.manifest
            .get("settings")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter(|s| s.get("type").and_then(Value::as_str) == Some("secret"))
                    .filter_map(|s| s.get("key").and_then(Value::as_str).map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// A stack widget shows other widgets inside its window.
    pub fn is_stack(&self) -> bool {
        self.manifest.pointer("/ui/stack").and_then(Value::as_bool) == Some(true)
    }

    /// A setting's value for an instance, falling back to the manifest default.
    pub fn setting(&self, inst: &Instance, key: &str) -> Option<Value> {
        if let Some(v) = inst.settings.get(key) {
            return Some(v.clone());
        }
        self.manifest
            .get("settings")?
            .as_array()?
            .iter()
            .find(|s| s.get("key").and_then(Value::as_str) == Some(key))
            .and_then(|s| s.get("default").cloned())
    }

    pub fn has_storage(&self) -> bool {
        self.manifest.pointer("/permissions/storage").and_then(Value::as_bool) == Some(true)
    }

    pub fn multi_instance(&self) -> bool {
        self.manifest.pointer("/window/multi_instance").and_then(Value::as_bool) == Some(true)
    }

    pub fn size(&self, key: &str, fallback: (f64, f64)) -> (f64, f64) {
        let p = format!("/window/{key}");
        match self.manifest.pointer(&p).and_then(Value::as_array) {
            Some(a) if a.len() == 2 => (
                a[0].as_f64().unwrap_or(fallback.0),
                a[1].as_f64().unwrap_or(fallback.1),
            ),
            _ => fallback,
        }
    }

    pub fn default_layer(&self) -> String {
        self.manifest
            .pointer("/window/layer")
            .and_then(Value::as_str)
            .unwrap_or("desktop")
            .to_string()
    }
}

/// Checks the parts of a manifest the core depends on. Returns the widget id.
pub fn validate(m: &Value) -> Result<String, String> {
    if m.get("schema").and_then(Value::as_u64) != Some(1) {
        return Err("`schema` must be 1".into());
    }
    let id = m.get("id").and_then(Value::as_str).ok_or("missing `id`")?;
    let parts: Vec<&str> = id.split('.').collect();
    let part_ok = |p: &&str| {
        !p.is_empty()
            && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    };
    if parts.len() < 2 || !parts.iter().all(part_ok) {
        return Err(format!("bad id `{id}`: use lowercase words separated by dots"));
    }
    for key in ["name", "version"] {
        if m.get(key).and_then(Value::as_str).is_none() {
            return Err(format!("missing `{key}`"));
        }
    }
    let ui = m.get("ui").ok_or("missing `ui`")?;
    let is_stack = ui.get("stack").and_then(Value::as_bool) == Some(true);
    if ui.get("blocks").is_none() && ui.get("entry").is_none() && !is_stack {
        return Err("`ui` needs `blocks`, `entry` or `stack: true`".into());
    }
    Ok(id.to_string())
}

pub fn load_widget(dir: &Path) -> Result<(String, Value), String> {
    let text = fs::read_to_string(dir.join("widget.yaml")).map_err(|e| e.to_string())?;
    let manifest: Value = serde_yaml::from_str(&text).map_err(|e| e.to_string())?;
    let id = validate(&manifest)?;
    Ok((id, manifest))
}

/// Reads every widget folder. Local add-ons override built-ins with the same id.
pub fn scan(builtin: &Path, addons: &Path) -> (BTreeMap<String, WidgetInfo>, Vec<String>) {
    let mut widgets = BTreeMap::new();
    let mut errors = Vec::new();
    for (root, source) in [(builtin, "built-in"), (addons, "local")] {
        let Ok(entries) = fs::read_dir(root) else { continue };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.join("widget.yaml").is_file() {
                continue;
            }
            match load_widget(&dir) {
                Ok((id, manifest)) => {
                    widgets.insert(
                        id.clone(),
                        WidgetInfo { id, source: source.into(), dir, manifest },
                    );
                }
                Err(e) => errors.push(format!("{}: {e}", dir.display())),
            }
        }
    }
    (widgets, errors)
}

/// One placed copy of a widget.
#[derive(Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub widget_id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub layer: String,
    #[serde(default)]
    pub start_with_app: bool,
    #[serde(default)]
    pub locked: bool,
    /// Whether its window is showing.
    #[serde(default)]
    pub open: bool,
    #[serde(default)]
    pub settings: Map<String, Value>,
    /// The stack this widget is shown in, if any. It then has no window of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// This widget's own look, over the app theme: frame, opacity, accent.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub style: Map<String, Value>,
    /// "drawer" docks it to a screen edge; none = a floating tile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Drawer edge: left, right, top or bottom.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge: Option<String>,
    /// Drawer stays open.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
    /// Where along the edge the drawer sits, 0.0 to 1.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawer_at: Option<f64>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Layout {
    pub instances: Vec<Instance>,
}

impl Layout {
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Instance> {
        self.instances.iter_mut().find(|i| i.id == id)
    }
}

pub fn new_id() -> String {
    let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("{:x}", n % 0xffff_ffff_ffff)
}

/// Reads a JSON file. A missing file gives the default; a corrupt one is renamed
/// `*.bad` and replaced by the default instead of crashing.
pub fn read_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    let Ok(text) = fs::read_to_string(path) else { return T::default() };
    match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => {
            let _ = fs::rename(path, path.with_extension("json.bad"));
            T::default()
        }
    }
}

/// Writes a JSON file atomically: temp file, then rename.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

pub const STORAGE_LIMIT: usize = 1024 * 1024;

#[cfg(test)]
mod tests {
    #[test]
    fn every_built_in_widget_loads() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../widgets");
        let (widgets, errors) = super::scan(&dir, std::path::Path::new("/nonexistent"));
        assert!(errors.is_empty(), "widget errors: {errors:#?}");
        assert!(widgets.len() >= 18, "only {} widgets found", widgets.len());
    }
}
