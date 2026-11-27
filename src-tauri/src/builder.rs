//! The visual widget builder (Hub → Builder): saves a widget the user made as
//! an add-on, and gives the builder's live preview sample data. Every command
//! here is for the Hub window only.

use crate::AppState;
use serde_json::{json, Value};
use std::time::Duration;
use tauri::{State, WebviewWindow};

/// Ids the builder may write: two or more lowercase parts, not starting with
/// "forge." (those are the built-in widgets).
pub fn valid_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    parts.len() >= 2
        && !id.starts_with("forge.")
        && id.len() <= 64
        && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
}

/// A manifest the builder made must not ship a command to run (only the
/// user's own setting may hold one) — the same rule as hand-written widgets.
fn check_no_default_commands(m: &Value) -> Result<(), String> {
    for s in m["settings"].as_array().into_iter().flatten() {
        if s["type"] == "command" && s.get("default").is_some_and(|d| !d.is_null() && d != "") {
            return Err("a command setting may not have a default".into());
        }
    }
    Ok(())
}

/// Saves `manifest` as add-ons/<id>/widget.yaml and reloads the widget list.
/// `replace` must be true to overwrite an add-on that already exists.
#[tauri::command]
pub fn builder_save(window: WebviewWindow, state: State<AppState>, manifest: Value, replace: bool) -> Result<Value, String> {
    crate::shell::hub_only(&window)?;
    let id = crate::model::validate(&manifest)?;
    if !valid_id(&id) {
        return Err("choose an id like my.cpu-card (lowercase, with a dot; not starting with forge.)".into());
    }
    check_no_default_commands(&manifest)?;
    let dir = state.addons_dir.join(&id);
    let file = dir.join("widget.yaml");
    if file.exists() && !replace {
        return Err(format!("an add-on called {id} already exists"));
    }
    let yaml = serde_yaml::to_string(&manifest).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let tmp = dir.join("widget.yaml.tmp");
    std::fs::write(&tmp, format!("# Made with the WidgetForge builder. You can edit it by hand too.\n{yaml}")).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &file).map_err(|e| e.to_string())?;
    Ok(crate::rescan_widgets(&state))
}

/// Everything the system source can report, for the builder's value picker.
#[tauri::command]
pub fn builder_sample_system(window: WebviewWindow, state: State<AppState>) -> Result<Value, String> {
    crate::shell::hub_only(&window)?;
    let mut guard = state.system.lock().unwrap();
    if guard.is_none() {
        *guard = Some(crate::system::SystemSource::new()?);
    }
    let all: Vec<String> = ["cpu", "memory", "disk", "network", "os", "processes", "gpu", "temperature", "battery", "disks", "sensors", "interfaces"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    Ok(guard.as_mut().unwrap().snapshot(&all))
}

/// Fetches an address the user typed into the builder, to show what it returns.
#[tauri::command]
pub async fn builder_fetch(window: WebviewWindow, url: String, headers: Option<serde_json::Map<String, Value>>) -> Result<Value, String> {
    crate::shell::hub_only(&window)?;
    let u = url::Url::parse(url.trim()).map_err(|e| format!("bad address: {e}"))?;
    let host = match (u.host_str(), u.port()) {
        (Some(h), Some(p)) => format!("{h}:{p}"),
        (Some(h), None) => h.to_string(),
        _ => return Err("the address has no host".into()),
    };
    let headers = headers
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(k, v)| v.as_str().map(|v| (k, v.to_string())))
        .filter(|(_, v)| !v.contains("[[")) // no saved secrets in the builder
        .collect();
    let req = crate::net::Request { url: url.trim().to_string(), method: String::new(), headers, body: None };
    crate::net::fetch(req, vec![host]).await
}

/// Runs a command the user typed into the builder, once, to show its output.
#[tauri::command]
pub async fn builder_run(window: WebviewWindow, command: String) -> Result<Value, String> {
    crate::shell::hub_only(&window)?;
    let cmd = command.trim().to_string();
    if cmd.is_empty() {
        return Ok(json!({ "configured": false }));
    }
    tauri::async_runtime::spawn_blocking(move || crate::local::run_shell(&cmd, Duration::from_secs(15)))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        assert!(valid_id("my.cpu-card"));
        assert!(valid_id("sam.home.nas2"));
        assert!(!valid_id("forge.clock"));
        assert!(!valid_id("nodot"));
        assert!(!valid_id("My.Widget"));
        assert!(!valid_id("my..x"));
        assert!(!valid_id("../evil.x"));
    }

    #[test]
    fn saved_yaml_loads_back() {
        let m = json!({
            "schema": 1, "id": "my.cpu-card", "name": "CPU card", "version": "1.0.0",
            "permissions": { "system": ["cpu"] },
            "data": { "pc": { "source": "system", "interval": 2000 } },
            "ui": { "blocks": [{ "type": "stat", "label": "CPU", "value": "{{ pc.cpu.usage | fixed(0) }}%" }] }
        });
        let dir = std::env::temp_dir().join(format!("wf-builder-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let yaml = serde_yaml::to_string(&m).unwrap();
        std::fs::write(dir.join("widget.yaml"), format!("# Made with the WidgetForge builder.\n{yaml}")).unwrap();
        let (id, back) = crate::model::load_widget(&dir).unwrap();
        assert_eq!(id, "my.cpu-card");
        assert_eq!(back, m);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn commands_cannot_ship() {
        let ok = json!({ "settings": [{ "key": "command", "type": "command" }] });
        assert!(check_no_default_commands(&ok).is_ok());
        let bad = json!({ "settings": [{ "key": "command", "type": "command", "default": "rm -rf ~" }] });
        assert!(check_no_default_commands(&bad).is_err());
    }
}
