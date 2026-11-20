//! Widget windows: creation, layers, drawers, and saving their geometry.

use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Monitor, WebviewUrl, WebviewWindow, WindowEvent};

use crate::error::{Error, Result};
use crate::layout::Instance;
use crate::manifest::{Edge, Layer, Mode};
use crate::state::AppState;

pub const HUB: &str = "hub";
const PREFIX: &str = "w-";
/// Width of the strip a collapsed drawer leaves on screen.
pub const DRAWER_HANDLE: f64 = 22.0;

pub fn label(instance_id: &str) -> String {
    format!("{PREFIX}{instance_id}")
}

pub fn instance_id(label: &str) -> Option<&str> {
    label.strip_prefix(PREFIX)
}

pub fn is_open(app: &AppHandle, instance_id: &str) -> bool {
    app.get_webview_window(&label(instance_id)).is_some()
}

pub fn show_hub(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(HUB) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// A rectangle in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

fn work_area(m: &Monitor) -> Rect {
    let s = m.scale_factor();
    let wa = m.work_area();
    Rect {
        x: wa.position.x as f64 / s,
        y: wa.position.y as f64 / s,
        w: wa.size.width as f64 / s,
        h: wa.size.height as f64 / s,
    }
}

/// The monitor an instance belongs on: its saved monitor, else the one containing its
/// position, else the primary one.
fn monitor_for(app: &AppHandle, inst: &Instance) -> Option<Monitor> {
    let monitors = app.available_monitors().ok()?;
    if let Some(name) = &inst.monitor {
        if let Some(m) = monitors.iter().find(|m| m.name().map(|n| n == name).unwrap_or(false)) {
            return Some(m.clone());
        }
    }
    let cx = inst.x + inst.w.min(40.0);
    let cy = inst.y + inst.h.min(40.0);
    monitors
        .iter()
        .find(|m| work_area(m).contains(cx, cy))
        .cloned()
        .or_else(|| app.primary_monitor().ok().flatten())
        .or_else(|| monitors.first().cloned())
}

pub fn primary_work_area(app: &AppHandle) -> Rect {
    app.primary_monitor()
        .ok()
        .flatten()
        .map(|m| work_area(&m))
        .unwrap_or(Rect { x: 0.0, y: 0.0, w: 1280.0, h: 720.0 })
}

/// Clamp a tile so at least part of it stays reachable on its monitor.
fn clamp_tile(inst: &Instance, wa: Rect) -> (f64, f64) {
    let keep = 48.0;
    let x = inst.x.clamp(wa.x - inst.w + keep, wa.x + wa.w - keep);
    let y = inst.y.clamp(wa.y, wa.y + wa.h - keep);
    (x, y)
}

/// Where a drawer sits when open or collapsed. Its position along the edge is taken from
/// the saved x (top/bottom edges) or y (left/right edges).
pub fn drawer_position(inst: &Instance, wa: Rect, open: bool) -> (f64, f64) {
    let along_y = inst.y.clamp(wa.y, (wa.y + wa.h - inst.h).max(wa.y));
    let along_x = inst.x.clamp(wa.x, (wa.x + wa.w - inst.w).max(wa.x));
    match (inst.edge, open) {
        (Edge::Right, true) => (wa.x + wa.w - inst.w, along_y),
        (Edge::Right, false) => (wa.x + wa.w - DRAWER_HANDLE, along_y),
        (Edge::Left, true) => (wa.x, along_y),
        (Edge::Left, false) => (wa.x - inst.w + DRAWER_HANDLE, along_y),
        (Edge::Top, true) => (along_x, wa.y),
        (Edge::Top, false) => (along_x, wa.y - inst.h + DRAWER_HANDLE),
        (Edge::Bottom, true) => (along_x, wa.y + wa.h - inst.h),
        (Edge::Bottom, false) => (along_x, wa.y + wa.h - DRAWER_HANDLE),
    }
}

fn apply_layer(w: &WebviewWindow, layer: Layer) -> Result<()> {
    match layer {
        Layer::Top => {
            w.set_always_on_bottom(false)?;
            w.set_always_on_top(true)?;
        }
        Layer::Normal => {
            w.set_always_on_top(false)?;
            w.set_always_on_bottom(false)?;
        }
        Layer::Desktop => {
            w.set_always_on_top(false)?;
            w.set_always_on_bottom(true)?;
        }
    }
    Ok(())
}

/// Opens (or re-shows) the window of an instance.
pub fn open(app: &AppHandle, instance_id: &str) -> Result<()> {
    let state = app.state::<AppState>();
    let inst = state
        .layout
        .lock()
        .unwrap()
        .get(instance_id)
        .cloned()
        .ok_or_else(|| Error::NotFound(format!("instance {instance_id}")))?;
    if let Some(w) = app.get_webview_window(&label(instance_id)) {
        w.show()?;
        return Ok(());
    }
    let (resizable, min_size, name) = {
        let reg = state.registry.read().unwrap();
        let entry = reg.get(&inst.widget_id).ok_or_else(|| Error::NotFound(format!("widget {}", inst.widget_id)))?;
        (entry.manifest.window.resizable, entry.manifest.window.min_size, entry.manifest.name.clone())
    };

    let wa = monitor_for(app, &inst).map(|m| work_area(&m)).unwrap_or_else(|| primary_work_area(app));
    let (x, y) = match inst.mode {
        Mode::Tile => clamp_tile(&inst, wa),
        Mode::Drawer => drawer_position(&inst, wa, false),
    };

    let hidden = state.settings.lock().unwrap().widgets_hidden;
    let mut b = tauri::WebviewWindowBuilder::new(app, label(instance_id), WebviewUrl::App("widget.html".into()))
        .title(name)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(resizable && inst.mode == Mode::Tile)
        .skip_taskbar(!inst.show_in_taskbar)
        .inner_size(inst.w, inst.h)
        .position(x, y)
        .focused(false)
        .visible(!hidden)
        .always_on_top(inst.layer == Layer::Top)
        .always_on_bottom(inst.layer == Layer::Desktop);
    if let Some([mw, mh]) = min_size {
        b = b.min_inner_size(mw as f64, mh as f64);
    }
    b.build()?;
    Ok(())
}

pub fn close(app: &AppHandle, instance_id: &str) {
    if let Some(w) = app.get_webview_window(&label(instance_id)) {
        let _ = w.destroy();
    }
}

/// Applies changed instance options to a live window. Shape changes need a new window.
pub fn apply(app: &AppHandle, before: &Instance, after: &Instance) -> Result<()> {
    let Some(w) = app.get_webview_window(&label(&after.id)) else { return Ok(()) };
    if before.mode != after.mode || before.edge != after.edge {
        w.destroy()?;
        return open(app, &after.id);
    }
    if before.layer != after.layer {
        apply_layer(&w, after.layer)?;
    }
    if before.show_in_taskbar != after.show_in_taskbar {
        w.set_skip_taskbar(!after.show_in_taskbar)?;
    }
    Ok(())
}

/// Slides a drawer open or closed.
pub fn set_drawer(app: &AppHandle, instance_id: &str, open: bool) -> Result<()> {
    let state = app.state::<AppState>();
    let inst = state.layout.lock().unwrap().get(instance_id).cloned().ok_or_else(|| Error::NotFound("instance".into()))?;
    if inst.mode != Mode::Drawer {
        return Ok(());
    }
    let w = app.get_webview_window(&label(instance_id)).ok_or_else(|| Error::NotFound("window".into()))?;
    let wa = monitor_for(app, &inst).map(|m| work_area(&m)).unwrap_or_else(|| primary_work_area(app));
    let from = drawer_position(&inst, wa, !open);
    let to = drawer_position(&inst, wa, open);
    if open {
        let _ = w.set_focus();
    }
    std::thread::spawn(move || {
        const STEPS: u32 = 8;
        for i in 1..=STEPS {
            let t = i as f64 / STEPS as f64;
            let ease = 1.0 - (1.0 - t).powi(3);
            let x = from.0 + (to.0 - from.0) * ease;
            let y = from.1 + (to.1 - from.1) * ease;
            let _ = w.set_position(LogicalPosition::new(x, y));
            std::thread::sleep(std::time::Duration::from_millis(14));
        }
    });
    Ok(())
}

/// Hides or shows every widget window (tray toggle).
pub fn set_all_hidden(app: &AppHandle, hidden: bool) {
    app.state::<AppState>().settings.lock().unwrap().widgets_hidden = hidden;
    for (lbl, w) in app.webview_windows() {
        if instance_id(&lbl).is_some() {
            let _ = if hidden { w.hide() } else { w.show() };
        }
    }
}

/// Keeps the layout in sync when the user drags or resizes a tile.
pub fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    let lbl = window.label().to_string();
    if lbl == HUB {
        if let WindowEvent::CloseRequested { api, .. } = event {
            // Closing the Hub hides it; WidgetForge keeps running in the tray.
            api.prevent_close();
            let _ = window.hide();
        }
        return;
    }
    let Some(id) = instance_id(&lbl) else { return };
    let app = window.app_handle();
    let state = app.state::<AppState>();
    let scale = window.scale_factor().unwrap_or(1.0);
    match event {
        WindowEvent::Moved(pos) => {
            let monitor = window.current_monitor().ok().flatten().and_then(|m| m.name().cloned());
            let mut layout = state.layout.lock().unwrap();
            if let Some(inst) = layout.get_mut(id) {
                if inst.mode == Mode::Tile {
                    let p = pos.to_logical::<f64>(scale);
                    inst.x = p.x.round();
                    inst.y = p.y.round();
                    if monitor.is_some() {
                        inst.monitor = monitor;
                    }
                    state.mark_layout_dirty();
                }
            }
        }
        WindowEvent::Resized(size) => {
            if size.width == 0 || size.height == 0 {
                return; // minimised
            }
            let mut layout = state.layout.lock().unwrap();
            if let Some(inst) = layout.get_mut(id) {
                if inst.mode == Mode::Tile {
                    let s: LogicalSize<f64> = size.to_logical(scale);
                    inst.w = s.width.round();
                    inst.h = s.height.round();
                    state.mark_layout_dirty();
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest;

    fn inst(edge: Edge) -> Instance {
        let (m, _) = manifest::parse(
            "schema: 1\nid: forge.d\nname: D\nversion: 1.0.0\nwindow: { mode: drawer, size: [300, 400] }\nui:\n  blocks: [ { type: text, text: x } ]\n",
        );
        let mut i = Instance::new("i".into(), &m.unwrap(), 0.0, 100.0);
        i.edge = edge;
        i
    }

    #[test]
    fn drawer_positions() {
        let wa = Rect { x: 0.0, y: 0.0, w: 1920.0, h: 1040.0 };
        assert_eq!(drawer_position(&inst(Edge::Right), wa, true), (1620.0, 100.0));
        assert_eq!(drawer_position(&inst(Edge::Right), wa, false), (1920.0 - DRAWER_HANDLE, 100.0));
        assert_eq!(drawer_position(&inst(Edge::Left), wa, false), (-300.0 + DRAWER_HANDLE, 100.0));
        assert_eq!(drawer_position(&inst(Edge::Bottom), wa, true), (0.0, 640.0));
        let mut i = inst(Edge::Right);
        i.y = 5000.0;
        assert_eq!(drawer_position(&i, wa, true).1, 640.0, "clamped inside the work area");
    }

    #[test]
    fn labels() {
        assert_eq!(label("abc"), "w-abc");
        assert_eq!(instance_id("w-abc"), Some("abc"));
        assert_eq!(instance_id("hub"), None);
    }
}
