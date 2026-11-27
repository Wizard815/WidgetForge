//! Edge drawers: a widget docked to a screen edge. Closed, only a small handle
//! shows; it slides open on hover or click and closes when the pointer leaves
//! (unless pinned). The handle can be dragged along the edge.

use crate::model::Instance;
use crate::shell::Rect;
use crate::AppState;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, LogicalPosition, LogicalSize, State, WebviewWindow};

/// Thickness and length of the closed handle (logical pixels).
pub const HANDLE: f64 = 18.0;
pub const HANDLE_LEN: f64 = 110.0;
pub const EDGES: [&str; 4] = ["left", "right", "top", "bottom"];

pub fn is_drawer(inst: &Instance) -> bool {
    inst.mode.as_deref() == Some("drawer")
}

/// The screen a drawer belongs to: the one under the widget's last floating
/// position, else the main one.
pub fn screen_for(inst: &Instance, screens: &[Rect], primary: usize) -> Rect {
    let (cx, cy) = (inst.x + inst.w / 2.0, inst.y + inst.h / 2.0);
    screens
        .iter()
        .find(|(x, y, w, h)| cx >= *x && cx < x + w && cy >= *y && cy < y + h)
        .or(screens.get(primary))
        .or(screens.first())
        .copied()
        .unwrap_or((0.0, 0.0, 1920.0, 1080.0))
}

/// Where the drawer's window goes, open or closed. `at` is the middle of the
/// drawer along the edge, 0.0 (top/left) to 1.0 (bottom/right).
pub fn drawer_rect(area: Rect, edge: &str, at: f64, w: f64, h: f64, open: bool) -> Rect {
    let (ax, ay, aw, ah) = area;
    let at = at.clamp(0.0, 1.0);
    let along = |len: f64, start: f64, size: f64| (start + at * size - len / 2.0).clamp(start, (start + size - len).max(start));
    match edge {
        "left" | "right" => {
            let (ww, hh) = if open { (w.min(aw), h.min(ah)) } else { (HANDLE, HANDLE_LEN.min(ah)) };
            let x = if edge == "left" { ax } else { ax + aw - ww };
            (x, along(hh, ay, ah), ww, hh)
        }
        _ => {
            let (ww, hh) = if open { (w.min(aw), h.min(ah)) } else { (HANDLE_LEN.min(aw), HANDLE) };
            let y = if edge == "top" { ay } else { ay + ah - hh };
            (along(ww, ax, aw), y, ww, hh)
        }
    }
}

fn rect_now(app: &AppHandle, inst: &Instance, open: bool) -> Rect {
    let (screens, primary) = crate::shell::screens(app);
    let area = screen_for(inst, &screens, primary);
    drawer_rect(area, inst.edge.as_deref().unwrap_or("right"), inst.drawer_at.unwrap_or(0.5), inst.w, inst.h, open)
}

fn place(window: &WebviewWindow, r: Rect) {
    let _ = window.set_size(LogicalSize::new(r.2.max(1.0), r.3.max(1.0)));
    let _ = window.set_position(LogicalPosition::new(r.0, r.1));
}

pub fn closed_rect(app: &AppHandle, inst: &Instance) -> Rect {
    rect_now(app, inst, false)
}

fn own_instance(window: &WebviewWindow, state: &AppState) -> Result<Instance, String> {
    let id = window.label().strip_prefix(crate::WIDGET_PREFIX).ok_or("not a widget window")?;
    let inst = state.instance(id).ok_or("instance not found")?;
    if !is_drawer(&inst) {
        return Err("this widget is not a drawer".into());
    }
    Ok(inst)
}

/// Slides a drawer open or closed.
#[tauri::command]
pub async fn drawer_set(window: WebviewWindow, app: AppHandle, state: State<'_, AppState>, open: bool) -> Result<bool, String> {
    let inst = own_instance(&window, &state)?;
    {
        let mut set = state.drawers_open.lock().unwrap();
        let was = set.contains(&inst.id);
        if was == open {
            return Ok(open);
        }
        if open {
            set.insert(inst.id.clone());
        } else {
            set.remove(&inst.id);
        }
    }
    let from = rect_now(&app, &inst, !open);
    let to = rect_now(&app, &inst, open);
    tauri::async_runtime::spawn_blocking(move || {
        // A short slide: a few steps, eased.
        const STEPS: u32 = 7;
        for i in 1..=STEPS {
            let t = i as f64 / STEPS as f64;
            let e = 1.0 - (1.0 - t).powi(3);
            let lerp = |a: f64, b: f64| a + (b - a) * e;
            place(&window, (lerp(from.0, to.0), lerp(from.1, to.1), lerp(from.2, to.2), lerp(from.3, to.3)));
            std::thread::sleep(Duration::from_millis(14));
        }
        if open {
            let _ = window.set_focus();
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(open)
}

/// Moves a drawer along its edge by `delta` logical pixels (dragging the handle).
#[tauri::command]
pub fn drawer_slide(window: WebviewWindow, app: AppHandle, state: State<AppState>, delta: f64) -> Result<(), String> {
    let inst = own_instance(&window, &state)?;
    let (screens, primary) = crate::shell::screens(&app);
    let area = screen_for(&inst, &screens, primary);
    let len = match inst.edge.as_deref().unwrap_or("right") {
        "left" | "right" => area.3,
        _ => area.2,
    };
    let at = (inst.drawer_at.unwrap_or(0.5) + delta / len.max(1.0)).clamp(0.0, 1.0);
    let updated = {
        let mut layout = state.layout.lock().unwrap();
        let i = layout.get_mut(&inst.id).ok_or("instance not found")?;
        i.drawer_at = Some(at);
        i.clone()
    };
    state.layout_dirty.store(true, Ordering::SeqCst);
    let open = state.drawers_open.lock().unwrap().contains(&inst.id);
    place(&window, rect_now(&app, &updated, open));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const AREA: Rect = (0.0, 0.0, 1920.0, 1040.0);

    #[test]
    fn closed_handles_sit_on_their_edge() {
        assert_eq!(drawer_rect(AREA, "right", 0.5, 300.0, 400.0, false), (1920.0 - HANDLE, 520.0 - 55.0, HANDLE, HANDLE_LEN));
        assert_eq!(drawer_rect(AREA, "left", 0.5, 300.0, 400.0, false).0, 0.0);
        assert_eq!(drawer_rect(AREA, "top", 0.25, 300.0, 400.0, false), (480.0 - 55.0, 0.0, HANDLE_LEN, HANDLE));
        assert_eq!(drawer_rect(AREA, "bottom", 0.5, 300.0, 400.0, false).1, 1040.0 - HANDLE);
    }

    #[test]
    fn open_drawers_stay_on_screen() {
        assert_eq!(drawer_rect(AREA, "right", 0.5, 300.0, 400.0, true), (1620.0, 320.0, 300.0, 400.0));
        // Near the bottom: pushed up so all of it shows.
        assert_eq!(drawer_rect(AREA, "right", 1.0, 300.0, 400.0, true).1, 640.0);
        // Bigger than the screen: shrunk to fit.
        assert_eq!(drawer_rect(AREA, "left", 0.5, 300.0, 5000.0, true), (0.0, 0.0, 300.0, 1040.0));
        // A second screen to the right.
        let second = (1920.0, 0.0, 1280.0, 1024.0);
        assert_eq!(drawer_rect(second, "left", 0.5, 300.0, 400.0, true).0, 1920.0);
    }

    #[test]
    fn drawers_pick_the_screen_they_were_on() {
        let screens = [(0.0, 0.0, 1920.0, 1040.0), (1920.0, 0.0, 1280.0, 1024.0)];
        let mut inst: Instance = serde_json::from_value(serde_json::json!({
            "id": "x", "widget_id": "w", "x": 2500, "y": 100, "w": 200, "h": 200, "layer": "top"
        }))
        .unwrap();
        assert_eq!(screen_for(&inst, &screens, 0), screens[1]);
        inst.x = 9000.0;
        assert_eq!(screen_for(&inst, &screens, 0), screens[0]);
    }
}
