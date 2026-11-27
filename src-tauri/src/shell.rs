//! Desktop behaviour: start at sign-in, global keyboard shortcuts, showing and
//! hiding all widgets, and keeping widgets on a screen that is connected.

use crate::{show_hub, AppState, WIDGET_PREFIX};
use serde_json::{json, Map, Value};
use std::str::FromStr;
use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

pub const DEFAULT_TOGGLE: &str = "Ctrl+Alt+W";

#[derive(Default)]
pub struct Shortcuts {
    toggle: Option<Shortcut>,
    hub: Option<Shortcut>,
    pub error: Option<String>,
}

// ---------- show / hide all ----------

pub fn set_all_widgets(app: &AppHandle, show: bool) {
    for (label, w) in app.webview_windows() {
        if label.starts_with(WIDGET_PREFIX) {
            let _ = if show { w.show() } else { w.hide() };
        }
    }
}

pub fn toggle_widgets(app: &AppHandle) {
    let any_visible = app
        .webview_windows()
        .iter()
        .any(|(l, w)| l.starts_with(WIDGET_PREFIX) && w.is_visible().unwrap_or(false));
    set_all_widgets(app, !any_visible);
}

// ---------- shortcuts ----------

fn parse(text: &str) -> Result<Option<Shortcut>, String> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    Shortcut::from_str(t).map(Some).map_err(|e| format!("“{t}” is not a shortcut: {e}"))
}

pub fn on_shortcut(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    let state = app.state::<AppState>();
    let (toggle, hub) = {
        let s = state.shortcuts.lock().unwrap();
        (s.toggle == Some(*shortcut), s.hub == Some(*shortcut))
    };
    if toggle {
        toggle_widgets(app);
    } else if hub {
        show_hub(app);
    }
}

/// Registers the two shortcuts (empty text = none). Keeps going if one is taken.
pub fn apply_shortcuts(app: &AppHandle, toggle: &str, hub: &str) -> Result<(), String> {
    let toggle_sc = parse(toggle)?;
    let hub_sc = parse(hub)?;
    if toggle_sc.is_some() && toggle_sc == hub_sc {
        return Err("the two shortcuts must be different".into());
    }
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut errors = Vec::new();
    for (sc, text) in [(toggle_sc, toggle), (hub_sc, hub)] {
        if let Some(sc) = sc {
            if let Err(e) = gs.register(sc) {
                errors.push(format!("{} could not be used (another program may have it): {e}", text.trim()));
            }
        }
    }
    let state = app.state::<AppState>();
    let mut s = state.shortcuts.lock().unwrap();
    s.toggle = toggle_sc;
    s.hub = hub_sc;
    for e in &errors {
        eprintln!("shortcut: {e}");
    }
    if errors.is_empty() {
        eprintln!("shortcuts registered: toggle={:?} hub={:?}", toggle.trim(), hub.trim());
    }
    s.error = if errors.is_empty() { None } else { Some(errors.join("; ")) };
    Ok(())
}

pub(crate) fn read_settings(state: &AppState) -> Map<String, Value> {
    crate::model::read_json(&state.settings_file())
}

/// The saved shortcuts, with the defaults for a first start.
pub fn saved_shortcuts(state: &AppState) -> (String, String) {
    let s = read_settings(state);
    let get = |k: &str, d: &str| s.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
    (get("shortcut_toggle", DEFAULT_TOGGLE), get("shortcut_hub", ""))
}

// ---------- Hub commands ----------

pub(crate) fn hub_only(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == "hub" {
        Ok(())
    } else {
        Err("only the Hub may change this".into())
    }
}

#[tauri::command]
pub fn get_app_settings(app: AppHandle, state: State<AppState>) -> Value {
    let (toggle, hub) = saved_shortcuts(&state);
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    json!({
        "autostart": autostart,
        "shortcut_toggle": toggle,
        "shortcut_hub": hub,
        "shortcut_error": state.shortcuts.lock().unwrap().error,
    })
}

#[tauri::command]
pub fn set_shortcuts(app: AppHandle, window: WebviewWindow, state: State<AppState>, toggle: String, hub: String) -> Result<Value, String> {
    hub_only(&window)?;
    apply_shortcuts(&app, &toggle, &hub)?;
    let mut s = read_settings(&state);
    s.insert("shortcut_toggle".into(), json!(toggle.trim()));
    s.insert("shortcut_hub".into(), json!(hub.trim()));
    crate::model::write_json(&state.settings_file(), &s)?;
    Ok(get_app_settings(app, state))
}

/// Start at sign-in: launches with `--background` (widgets only, Hub in the tray).
#[tauri::command]
pub fn set_autostart(app: AppHandle, window: WebviewWindow, enabled: bool) -> Result<bool, String> {
    hub_only(&window)?;
    let al = app.autolaunch();
    let r = if enabled { al.enable() } else { al.disable() };
    r.map_err(|e| e.to_string())?;
    al.is_enabled().map_err(|e| e.to_string())
}

// ---------- screens ----------

/// A rectangle in logical pixels: x, y, width, height.
pub type Rect = (f64, f64, f64, f64);

/// Keeps a widget on a connected screen. If less than 40×40 of it shows on any
/// screen (a monitor was unplugged), it moves to the top-right of the main
/// screen's usable area.
pub fn fit_on_screens(widget: Rect, screens: &[Rect], primary: usize) -> (f64, f64) {
    let (x, y, w, h) = widget;
    let visible = screens.iter().any(|&(sx, sy, sw, sh)| {
        let ox = (x + w).min(sx + sw) - x.max(sx);
        let oy = (y + h).min(sy + sh) - y.max(sy);
        ox >= 40.0 && oy >= 40.0
    });
    if visible || screens.is_empty() {
        return (x, y);
    }
    let (sx, sy, sw, sh) = screens[primary.min(screens.len() - 1)];
    let nx = (sx + sw - w - 40.0).max(sx);
    let ny = (sy + 60.0).min((sy + sh - h).max(sy));
    (nx, ny)
}

/// The usable area of every connected screen, and which one is the main one.
pub fn screens(app: &AppHandle) -> (Vec<Rect>, usize) {
    let monitors = app.available_monitors().unwrap_or_default();
    let primary_name = app.primary_monitor().ok().flatten().and_then(|m| m.name().cloned());
    let mut primary = 0;
    let rects = monitors
        .iter()
        .enumerate()
        .map(|(i, m)| {
            if m.name().cloned() == primary_name && primary_name.is_some() {
                primary = i;
            }
            let scale = m.scale_factor();
            let area = m.work_area();
            let p = area.position.to_logical::<f64>(scale);
            let s = area.size.to_logical::<f64>(scale);
            (p.x, p.y, s.width, s.height)
        })
        .collect();
    (rects, primary)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEFT: Rect = (0.0, 0.0, 1920.0, 1040.0);
    const RIGHT: Rect = (1920.0, 0.0, 2560.0, 1400.0);

    #[test]
    fn widget_on_a_connected_screen_stays() {
        assert_eq!(fit_on_screens((100.0, 100.0, 280.0, 300.0), &[LEFT, RIGHT], 0), (100.0, 100.0));
        assert_eq!(fit_on_screens((3000.0, 500.0, 280.0, 300.0), &[LEFT, RIGHT], 0), (3000.0, 500.0));
    }

    #[test]
    fn widget_on_an_unplugged_screen_moves_to_the_main_one() {
        // The right-hand monitor is gone.
        assert_eq!(fit_on_screens((3000.0, 500.0, 280.0, 300.0), &[LEFT], 0), (1600.0, 60.0));
        // Mostly off the edge: only 20 px visible.
        assert_eq!(fit_on_screens((1900.0, 100.0, 280.0, 300.0), &[LEFT], 0), (1600.0, 60.0));
    }

    #[test]
    fn shortcut_text_is_parsed() {
        assert!(parse("Ctrl+Alt+W").unwrap().is_some());
        assert!(parse("ctrl+shift+F9").unwrap().is_some());
        assert!(parse("").unwrap().is_none());
        assert!(parse("Ctrl+Banana").is_err());
    }
}
