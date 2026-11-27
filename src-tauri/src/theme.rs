//! The app-wide look of every widget: background, opacity, corners, text,
//! font and accent. Saved in settings.json; widgets restyle at once when it changes.

use crate::AppState;
use serde_json::{json, Map, Value};
use tauri::{AppHandle, Emitter, State, WebviewWindow};

/// Keys a theme may have, with what each must be.
fn clean(input: &Map<String, Value>) -> Map<String, Value> {
    let mut out = Map::new();
    let color = |v: &Value| v.as_str().filter(|s| s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())).map(|s| json!(s));
    for (k, v) in input {
        let ok = match k.as_str() {
            "preset" => v.as_str().filter(|s| s.len() < 40).map(|s| json!(s)),
            "background" | "text" | "accent" => color(v),
            "opacity" => v.as_f64().map(|f| json!(f.clamp(0.0, 1.0))),
            "radius" => v.as_f64().map(|f| json!(f.clamp(0.0, 40.0))),
            "scale" => v.as_f64().map(|f| json!(f.clamp(0.6, 2.0))),
            "border" | "shadow" => v.as_bool().map(|b| json!(b)),
            "font" => v.as_str().filter(|s| ["system", "rounded", "serif", "mono", "condensed"].contains(s)).map(|s| json!(s)),
            "accent_mode" => v.as_str().filter(|s| ["widget", "theme"].contains(s)).map(|s| json!(s)),
            _ => None,
        };
        if let Some(v) = ok {
            out.insert(k.clone(), v);
        }
    }
    out
}

pub fn saved(state: &AppState) -> Value {
    let s = crate::shell::read_settings(state);
    s.get("theme").cloned().unwrap_or_else(|| json!({}))
}

#[tauri::command]
pub fn get_theme(state: State<AppState>) -> Value {
    saved(&state)
}

#[tauri::command]
pub fn set_theme(app: AppHandle, window: WebviewWindow, state: State<AppState>, theme: Map<String, Value>) -> Result<Value, String> {
    crate::shell::hub_only(&window)?;
    let theme = Value::Object(clean(&theme));
    let mut s = crate::shell::read_settings(&state);
    s.insert("theme".into(), theme.clone());
    crate::model::write_json(&state.settings_file(), &s)?;
    let _ = app.emit("theme-changed", &theme);
    Ok(theme)
}

#[cfg(test)]
mod tests {
    #[test]
    fn bad_values_are_dropped() {
        let input = serde_json::json!({
            "background": "#101520", "text": "red", "opacity": 3.0, "font": "comic", "radius": 10,
            "accent_mode": "theme", "evil": "<script>", "scale": 0.1
        });
        let out = super::clean(input.as_object().unwrap());
        assert_eq!(out["background"], "#101520");
        assert!(out.get("text").is_none());
        assert_eq!(out["opacity"], 1.0);
        assert!(out.get("font").is_none());
        assert!(out.get("evil").is_none());
        assert_eq!(out["scale"], 0.6);
        assert_eq!(out["accent_mode"], "theme");
    }
}
