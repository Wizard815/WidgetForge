//! Desk presets: saved sets of widgets with their places, layers, stacks and
//! settings. Each is a `.forgedesk` file (JSON) in the presets folder, so it can
//! be copied to another computer. Secrets are never written into a preset.

use crate::model::{self, Instance};
use crate::{notify_hub, open_window, AppState, WIDGET_PREFIX};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

const EXT: &str = "forgedesk";
const AUTO_NAME: &str = "Before last switch";

#[derive(Serialize, Deserialize)]
struct Preset {
    format: String,
    version: u32,
    name: String,
    saved_at: u64,
    instances: Vec<Instance>,
}

/// File name from a preset name: letters, digits, spaces, - and _ only.
fn file_for(state: &AppState, name: &str) -> Result<PathBuf, String> {
    let clean: String = name
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .take(60)
        .collect();
    if clean.trim().is_empty() {
        return Err("give the preset a name (letters and numbers)".into());
    }
    Ok(state.presets_dir.join(format!("{}.{EXT}", clean.trim())))
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The current desktop as a preset, with secret settings removed.
fn snapshot(state: &AppState, name: &str) -> Preset {
    let mut instances = state.layout.lock().unwrap().instances.clone();
    for inst in &mut instances {
        if let Ok(w) = state.widget(&inst.widget_id) {
            for key in w.secret_keys() {
                inst.settings.remove(&key);
            }
        }
    }
    Preset { format: "forgedesk".into(), version: 1, name: name.trim().into(), saved_at: now(), instances }
}

fn write(state: &AppState, preset: &Preset) -> Result<(), String> {
    model::write_json(&file_for(state, &preset.name)?, preset)
}

#[tauri::command]
pub fn list_presets(state: State<AppState>) -> Value {
    let mut presets = Vec::new();
    let mut errors = Vec::new();
    if let Ok(dir) = std::fs::read_dir(&state.presets_dir) {
        for f in dir.flatten() {
            let path = f.path();
            if path.extension().and_then(|e| e.to_str()) != Some(EXT) {
                continue;
            }
            let parsed = std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| serde_json::from_str::<Preset>(&t).map_err(|e| e.to_string()));
            match parsed {
                Ok(p) => {
                    let missing: Vec<String> = p
                        .instances
                        .iter()
                        .filter(|i| state.widget(&i.widget_id).is_err())
                        .map(|i| i.widget_id.clone())
                        .collect();
                    presets.push(json!({
                        "name": p.name,
                        "file": path.file_name().map(|n| n.to_string_lossy().into_owned()),
                        "saved_at": p.saved_at,
                        "widgets": p.instances.len(),
                        "auto": p.name == AUTO_NAME,
                        "missing": missing,
                    }));
                }
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }
    }
    presets.sort_by_key(|p| std::cmp::Reverse(p["saved_at"].as_u64().unwrap_or(0)));
    json!({ "presets": presets, "errors": errors })
}

#[tauri::command]
pub fn save_preset(state: State<AppState>, name: String) -> Result<(), String> {
    write(&state, &snapshot(&state, &name))
}

/// Replaces the desktop with a preset. The current desktop is saved first as
/// "Before last switch", so a switch can be undone.
#[tauri::command]
pub fn apply_preset(app: AppHandle, state: State<AppState>, file: String) -> Result<(), String> {
    if file.contains(['/', '\\']) || !file.ends_with(&format!(".{EXT}")) {
        return Err("bad preset file name".into());
    }
    let text = std::fs::read_to_string(state.presets_dir.join(&file)).map_err(|e| e.to_string())?;
    let mut preset: Preset = serde_json::from_str(&text).map_err(|e| format!("not a preset file: {e}"))?;
    if preset.name != AUTO_NAME {
        write(&state, &snapshot(&state, AUTO_NAME))?;
    }

    // Close every widget window.
    for (label, w) in app.webview_windows() {
        if label.starts_with(WIDGET_PREFIX) {
            let _ = w.destroy();
        }
    }

    // Widgets whose package is not installed are left out. Saved keys for the
    // same widgets on this computer are still in the keychain: mark them set.
    preset.instances.retain(|i| state.widget(&i.widget_id).is_ok());
    let ids: Vec<String> = preset.instances.iter().map(|i| i.id.clone()).collect();
    for inst in &mut preset.instances {
        if inst.parent.as_ref().is_some_and(|p| !ids.contains(p)) {
            inst.parent = None;
        }
        let widget = state.widget(&inst.widget_id)?;
        for key in widget.secret_keys() {
            if state.secrets.get(&inst.id, &key).is_some() {
                inst.settings.insert(key, Value::String("•••• saved".into()));
            }
        }
    }
    let open: Vec<Instance> = preset
        .instances
        .iter()
        .filter(|i| i.open && i.parent.is_none())
        .cloned()
        .collect();
    state.layout.lock().unwrap().instances = preset.instances;
    state.save_layout_now();
    for inst in open {
        if let Err(e) = open_window(&app, &inst) {
            eprintln!("could not open {}: {e}", inst.widget_id);
        }
    }
    notify_hub(&app);
    Ok(())
}

#[tauri::command]
pub fn delete_preset(state: State<AppState>, file: String) -> Result<(), String> {
    if file.contains(['/', '\\']) || !file.ends_with(&format!(".{EXT}")) {
        return Err("bad preset file name".into());
    }
    std::fs::remove_file(state.presets_dir.join(file)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_presets_folder(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    app.opener()
        .open_path(state.presets_dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Layout;
    use serde_json::Map;
    use std::collections::HashMap;
    use std::sync::{atomic::AtomicBool, Mutex};

    fn state(tmp: &std::path::Path) -> AppState {
        let builtin = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../widgets");
        let (widgets, _) = model::scan(&builtin, &tmp.join("addons"));
        let mut settings = Map::new();
        settings.insert("provider".into(), json!("anthropic"));
        settings.insert("api_key".into(), json!("•••• saved"));
        let inst = Instance {
            id: "abc".into(), widget_id: "forge.llm-usage".into(), x: 10.0, y: 20.0, w: 280.0, h: 170.0,
            layer: "top".into(), start_with_app: true, locked: false, open: true, settings, parent: None, style: Default::default(), mode: None, edge: None, pinned: false, drawer_at: None,
        };
        std::fs::create_dir_all(tmp.join("presets")).unwrap();
        AppState {
            builtin_dir: builtin,
            addons_dir: tmp.join("addons"),
            layout_file: tmp.join("layout.json"),
            storage_dir: tmp.join("storage"),
            presets_dir: tmp.join("presets"),
            secrets: crate::secrets::Secrets::file_only(tmp.join("secrets")),
            widgets: Mutex::new(widgets),
            errors: Mutex::new(vec![]),
            layout: Mutex::new(Layout { instances: vec![inst] }),
            layout_dirty: AtomicBool::new(false),
            system: Mutex::new(None),
            placements: Mutex::new(HashMap::new()),
            clipboard: Mutex::new(crate::clip::ClipState::new()),
            shortcuts: Mutex::new(crate::shell::Shortcuts::default()),
            pending_imports: Mutex::new(vec![]),
            drawers_open: Mutex::new(Default::default()),
        }
    }

    #[test]
    fn saved_preset_keeps_layout_but_never_secrets() {
        let tmp = std::env::temp_dir().join(format!("widgetforge-preset-{}", std::process::id()));
        let st = state(&tmp);
        let p = snapshot(&st, "  Work desk ");
        write(&st, &p).unwrap();
        let path = tmp.join("presets/Work desk.forgedesk");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("api_key"), "secret setting leaked into preset: {text}");
        let back: Preset = serde_json::from_str(&text).unwrap();
        assert_eq!(back.name, "Work desk");
        assert_eq!(back.instances[0].settings["provider"], "anthropic");
        assert_eq!((back.instances[0].x, back.instances[0].layer.as_str()), (10.0, "top"));
        // Names can't escape the presets folder.
        assert_eq!(file_for(&st, "../../etc/x").unwrap(), tmp.join("presets/etcx.forgedesk"));
        assert!(file_for(&st, "///").is_err());
    }
}
