mod activity;
mod clip;
mod discord;
mod local;
mod mail;
mod media;
mod model;
mod net;
mod presets;
mod protocol;
mod secrets;
mod shell;
mod system;
mod theme;
mod packages;
mod drawer;
mod registry;
mod builder;

use model::{Instance, Layout, WidgetInfo};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use system::SystemSource;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;

pub struct AppState {
    builtin_dir: PathBuf,
    addons_dir: PathBuf,
    layout_file: PathBuf,
    storage_dir: PathBuf,
    presets_dir: PathBuf,
    secrets: secrets::Secrets,
    widgets: Mutex<BTreeMap<String, WidgetInfo>>,
    errors: Mutex<Vec<String>>,
    layout: Mutex<Layout>,
    layout_dirty: AtomicBool,
    system: Mutex<Option<SystemSource>>,
    /// Per widget window: where we asked for it, when, and the offset the
    /// window manager added (so the nudge is not saved as a new position).
    placements: Mutex<HashMap<String, Placement>>,
    clipboard: Mutex<clip::ClipState>,
    shortcuts: Mutex<shell::Shortcuts>,
    /// .forgewidget files opened from outside, waiting for the Hub.
    pending_imports: Mutex<Vec<String>>,
    /// Drawers that are slid open right now.
    drawers_open: Mutex<std::collections::HashSet<String>>,
}

struct Placement {
    requested: (f64, f64),
    opened: Instant,
    offset: Option<(f64, f64)>,
}

impl AppState {
    pub(crate) fn save_layout_now(&self) {
        let layout = self.layout.lock().unwrap();
        if let Err(e) = model::write_json(&self.layout_file, &*layout) {
            eprintln!("could not save layout: {e}");
        }
        self.layout_dirty.store(false, Ordering::SeqCst);
    }

    fn storage_file(&self, instance_id: &str) -> PathBuf {
        self.storage_dir.join(format!("{instance_id}.json"))
    }

    /// App-wide settings (config/settings.json), next to the layout.
    pub(crate) fn settings_file(&self) -> PathBuf {
        self.layout_file.with_file_name("settings.json")
    }

    /// Removes an instance's secrets from the keychain (or file).
    fn forget_secrets(&self, inst: &Instance) {
        if let Ok(w) = self.widget(&inst.widget_id) {
            for key in w.secret_keys() {
                self.secrets.delete(&inst.id, &key);
            }
        }
    }

    pub(crate) fn instance(&self, id: &str) -> Option<Instance> {
        self.layout.lock().unwrap().instances.iter().find(|i| i.id == id).cloned()
    }

    pub(crate) fn widget(&self, id: &str) -> Result<WidgetInfo, String> {
        self.widgets.lock().unwrap().get(id).cloned().ok_or(format!("no widget `{id}`"))
    }
}

pub(crate) const WIDGET_PREFIX: &str = "w-";

fn instance_id_of(window: &WebviewWindow) -> Result<String, String> {
    window
        .label()
        .strip_prefix(WIDGET_PREFIX)
        .map(String::from)
        .ok_or_else(|| "only widget windows may call this".to_string())
}

/// The widget a call is for, for permission checks. A window may speak for its
/// own instance, or (a stack) for an instance inside it.
pub(crate) fn widget_of(
    window: &WebviewWindow,
    state: &AppState,
    instance: Option<String>,
) -> Result<(Instance, WidgetInfo), String> {
    let own = instance_id_of(window)?;
    let target = instance.unwrap_or_else(|| own.clone());
    let inst = state.instance(&target).ok_or("instance not found")?;
    if target != own && inst.parent.as_deref() != Some(own.as_str()) {
        return Err("this window cannot act for that widget".into());
    }
    let widget = state.widget(&inst.widget_id)?;
    Ok((inst, widget))
}

fn apply_layer(window: &WebviewWindow, layer: &str) {
    let _ = window.set_always_on_bottom(layer == "desktop");
    let _ = window.set_always_on_top(layer == "top");
}

pub(crate) fn open_window(app: &AppHandle, inst: &Instance) -> Result<(), String> {
    if inst.parent.is_some() {
        return Err("this widget is inside a stack".into());
    }
    let label = format!("{WIDGET_PREFIX}{}", inst.id);
    if let Some(w) = app.get_webview_window(&label) {
        w.show().map_err(|e| e.to_string())?;
        apply_layer(&w, &inst.layer);
        return Ok(());
    }
    let state = app.state::<AppState>();
    let widget = state.widget(&inst.widget_id)?;
    let (min_w, min_h) = widget.size("min_size", (80.0, 60.0));
    // A widget left on a monitor that is no longer connected comes back on screen.
    let (screens, primary) = shell::screens(app);
    let (x, y) = shell::fit_on_screens((inst.x, inst.y, inst.w, inst.h), &screens, primary);
    if (x, y) != (inst.x, inst.y) {
        if let Some(i) = state.layout.lock().unwrap().get_mut(&inst.id) {
            i.x = x;
            i.y = y;
        }
        state.layout_dirty.store(true, Ordering::SeqCst);
    }
    let inst = &Instance { x, y, ..inst.clone() };
    let name = widget.manifest["name"].as_str().unwrap_or("Widget").to_string();
    // A drawer opens closed: just its handle at the screen edge.
    let (px, py, pw, ph, mw, mh) = if drawer::is_drawer(inst) {
        state.drawers_open.lock().unwrap().remove(&inst.id);
        let r = drawer::closed_rect(app, inst);
        (r.0, r.1, r.2, r.3, drawer::HANDLE, drawer::HANDLE)
    } else {
        (inst.x, inst.y, inst.w, inst.h, min_w, min_h)
    };
    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("widget.html".into()))
        .title(name)
        .inner_size(pw, ph)
        .min_inner_size(mw, mh)
        .position(px, py)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .skip_taskbar(true)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;
    state.placements.lock().unwrap().insert(
        label,
        Placement { requested: (px, py), opened: Instant::now(), offset: None },
    );
    apply_layer(&window, &inst.layer);
    Ok(())
}

pub(crate) fn notify_hub(app: &AppHandle) {
    let _ = app.emit_to("hub", "layout-changed", ());
}

fn notify_widget(app: &AppHandle, inst: &Instance) {
    let _ = app.emit_to(format!("{WIDGET_PREFIX}{}", inst.id), "instance-changed", inst.clone());
    if let Some(parent) = &inst.parent {
        let _ = app.emit_to(format!("{WIDGET_PREFIX}{parent}"), "stack-changed", ());
    }
}

fn notify_stack(app: &AppHandle, stack_id: &str) {
    let _ = app.emit_to(format!("{WIDGET_PREFIX}{stack_id}"), "stack-changed", ());
}

pub(crate) fn show_hub(app: &AppHandle) {
    if let Some(hub) = app.get_webview_window("hub") {
        let _ = hub.show();
        let _ = hub.unminimize();
        let _ = hub.set_focus();
    }
}

// ---------- commands used by the Hub ----------

#[tauri::command]
fn list_widgets(state: State<AppState>) -> Value {
    let widgets: Vec<WidgetInfo> = state.widgets.lock().unwrap().values().cloned().collect();
    json!({ "widgets": widgets, "errors": *state.errors.lock().unwrap() })
}

#[tauri::command]
fn reload_widgets(state: State<AppState>) -> Value {
    rescan_widgets(&state)
}

/// Reads every widget folder again (after an add-on was added or saved).
pub(crate) fn rescan_widgets(state: &AppState) -> Value {
    let (widgets, errors) = model::scan(&state.builtin_dir, &state.addons_dir);
    *state.widgets.lock().unwrap() = widgets;
    *state.errors.lock().unwrap() = errors;
    let widgets: Vec<WidgetInfo> = state.widgets.lock().unwrap().values().cloned().collect();
    json!({ "widgets": widgets, "errors": *state.errors.lock().unwrap() })
}

#[tauri::command]
fn list_instances(state: State<AppState>) -> Vec<Instance> {
    state.layout.lock().unwrap().instances.clone()
}

#[tauri::command]
fn add_instance(app: AppHandle, state: State<AppState>, widget_id: String) -> Result<Instance, String> {
    let widget = state.widget(&widget_id)?;
    // A single-instance widget that is already placed is just shown again.
    if !widget.multi_instance() {
        let existing = state
            .layout
            .lock()
            .unwrap()
            .instances
            .iter()
            .find(|i| i.widget_id == widget_id)
            .cloned();
        if let Some(inst) = existing {
            return show_instance(app, state, inst.id.clone(), true).map(|_| inst);
        }
    }
    let (w, h) = widget.size("size", (240.0, 200.0));
    // Cascade new widgets down from the top-right corner of the main monitor.
    let (mut x, mut y) = (60.0, 60.0);
    if let Ok(Some(m)) = app.primary_monitor() {
        let scale = m.scale_factor();
        let size = m.size().to_logical::<f64>(scale);
        let count = state.layout.lock().unwrap().instances.len() as f64;
        x = size.width - w - 40.0 - (count % 6.0) * 30.0;
        y = 60.0 + (count % 6.0) * 30.0;
    }
    let inst = Instance {
        id: model::new_id(),
        widget_id,
        x,
        y,
        w,
        h,
        layer: widget.default_layer(),
        start_with_app: true,
        locked: false,
        open: true,
        settings: Map::new(),
        parent: None,
        style: Default::default(), mode: None, edge: None, pinned: false, drawer_at: None,
    };
    // A widget made to be a drawer (`window.mode: drawer`) starts as one.
    let mut inst = inst;
    if widget.manifest.pointer("/window/mode").and_then(Value::as_str) == Some("drawer") {
        inst.mode = Some("drawer".into());
        let edge = widget.manifest.pointer("/window/edge").and_then(Value::as_str).unwrap_or("right");
        inst.edge = Some(if drawer::EDGES.contains(&edge) { edge } else { "right" }.into());
        inst.layer = "top".into();
    }
    state.layout.lock().unwrap().instances.push(inst.clone());
    state.save_layout_now();
    open_window(&app, &inst)?;
    notify_hub(&app);
    Ok(inst)
}

#[tauri::command]
fn remove_instance(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(&format!("{WIDGET_PREFIX}{id}")) {
        let _ = w.destroy();
    }
    let removed = state.instance(&id);
    let parent = removed.as_ref().and_then(|i| i.parent.clone());
    if let Some(inst) = &removed {
        state.forget_secrets(inst);
    }
    {
        let mut layout = state.layout.lock().unwrap();
        layout.instances.retain(|i| i.id != id);
        // Widgets in a removed stack become hidden stand-alone widgets.
        for i in layout.instances.iter_mut().filter(|i| i.parent.as_deref() == Some(id.as_str())) {
            i.parent = None;
            i.open = false;
        }
    }
    let _ = std::fs::remove_file(state.storage_file(&id));
    state.save_layout_now();
    if let Some(p) = parent {
        notify_stack(&app, &p);
    }
    notify_hub(&app);
    Ok(())
}

/// Puts a widget into a stack (`stack_id`), or takes it out (`None`).
#[tauri::command]
fn move_to_stack(app: AppHandle, state: State<AppState>, id: String, stack_id: Option<String>) -> Result<(), String> {
    let inst = state.instance(&id).ok_or("instance not found")?;
    if state.widget(&inst.widget_id)?.is_stack() {
        return Err("a stack cannot go inside another stack".into());
    }
    if let Some(sid) = &stack_id {
        let stack = state.instance(sid).ok_or("stack not found")?;
        if !state.widget(&stack.widget_id)?.is_stack() {
            return Err("that widget is not a stack".into());
        }
    }
    let old_parent = inst.parent.clone();
    let updated = {
        let mut layout = state.layout.lock().unwrap();
        let i = layout.get_mut(&id).ok_or("instance not found")?;
        i.parent = stack_id.clone();
        if stack_id.is_none() {
            i.open = true;
        }
        let updated = i.clone();
        // Keep stack members together and in the order they were added.
        if let Some(sid) = &stack_id {
            let pos = layout.instances.iter().position(|x| x.id == id).unwrap();
            let item = layout.instances.remove(pos);
            let last = layout
                .instances
                .iter()
                .rposition(|x| x.parent.as_deref() == Some(sid.as_str()) || x.id == *sid)
                .map_or(layout.instances.len(), |p| p + 1);
            layout.instances.insert(last, item);
        }
        updated
    };
    if stack_id.is_some() {
        if let Some(w) = app.get_webview_window(&format!("{WIDGET_PREFIX}{id}")) {
            let _ = w.destroy();
        }
    } else {
        open_window(&app, &updated)?;
    }
    state.save_layout_now();
    for sid in [old_parent, stack_id].into_iter().flatten() {
        notify_stack(&app, &sid);
    }
    notify_hub(&app);
    Ok(())
}

/// Stores a secret setting (API key, token). Only "is set" is kept in settings.
#[tauri::command]
fn set_secret(app: AppHandle, state: State<AppState>, id: String, key: String, value: String) -> Result<(), String> {
    state.instance(&id).ok_or("instance not found")?;
    if value.is_empty() {
        state.secrets.delete(&id, &key);
    } else {
        state.secrets.set(&id, &key, &value)?;
    }
    let saved = !value.is_empty();
    let inst = {
        let mut layout = state.layout.lock().unwrap();
        let inst = layout.get_mut(&id).ok_or("instance not found")?;
        if saved {
            inst.settings.insert(key, Value::String("•••• saved".into()));
        } else {
            inst.settings.remove(&key);
        }
        inst.clone()
    };
    state.save_layout_now();
    notify_widget(&app, &inst);
    notify_hub(&app);
    Ok(())
}

#[tauri::command]
fn show_instance(app: AppHandle, state: State<AppState>, id: String, show: bool) -> Result<(), String> {
    let inst = {
        let mut layout = state.layout.lock().unwrap();
        let inst = layout.get_mut(&id).ok_or("instance not found")?;
        if inst.parent.is_some() {
            return Err("this widget is inside a stack; take it out of the stack first".into());
        }
        inst.open = show;
        inst.clone()
    };
    if show {
        open_window(&app, &inst)?;
    } else if let Some(w) = app.get_webview_window(&format!("{WIDGET_PREFIX}{id}")) {
        let _ = w.destroy();
    }
    state.save_layout_now();
    notify_hub(&app);
    Ok(())
}

/// Changes layer, start_with_app, locked or settings of an instance.
#[tauri::command]
fn update_instance(app: AppHandle, state: State<AppState>, id: String, patch: Value) -> Result<Instance, String> {
    let inst = {
        let mut layout = state.layout.lock().unwrap();
        let inst = layout.get_mut(&id).ok_or("instance not found")?;
        if let Some(layer) = patch.get("layer").and_then(Value::as_str) {
            if !["top", "normal", "desktop"].contains(&layer) {
                return Err(format!("unknown layer `{layer}`"));
            }
            inst.layer = layer.to_string();
        }
        if let Some(v) = patch.get("start_with_app").and_then(Value::as_bool) {
            inst.start_with_app = v;
        }
        if let Some(v) = patch.get("locked").and_then(Value::as_bool) {
            inst.locked = v;
        }
        // Shape: floating tile or edge drawer.
        let mut reshaped = false;
        if let Some(m) = patch.get("mode") {
            let m = m.as_str().filter(|m| *m == "drawer").map(String::from);
            reshaped |= m != inst.mode;
            inst.mode = m;
        }
        if let Some(e) = patch.get("edge").and_then(Value::as_str) {
            if !drawer::EDGES.contains(&e) {
                return Err(format!("unknown edge `{e}`"));
            }
            reshaped |= inst.edge.as_deref() != Some(e);
            inst.edge = Some(e.to_string());
        }
        if let Some(v) = patch.get("pinned").and_then(Value::as_bool) {
            inst.pinned = v;
        }
        if reshaped {
            inst.drawer_at.get_or_insert(0.5);
            if inst.mode.is_some() && inst.edge.is_none() {
                inst.edge = Some("right".into());
            }
        }
        if let Some(Value::Object(s)) = patch.get("style") {
            // null removes an override ("use the theme").
            for (k, v) in s {
                if v.is_null() {
                    inst.style.remove(k);
                } else {
                    inst.style.insert(k.clone(), v.clone());
                }
            }
        }
        if let Some(Value::Object(s)) = patch.get("settings") {
            for (k, v) in s {
                inst.settings.insert(k.clone(), v.clone());
            }
        }
        (inst.clone(), reshaped)
    };
    let (inst, reshaped) = inst;
    if let Some(w) = app.get_webview_window(&format!("{WIDGET_PREFIX}{id}")) {
        if reshaped {
            // A new shape needs a new window (different size limits and place).
            let _ = w.destroy();
            state.drawers_open.lock().unwrap().remove(&inst.id);
            if inst.open && inst.parent.is_none() {
                let app2 = app.clone();
                let inst2 = inst.clone();
                // Give the old window a moment to go before its label is reused.
                tauri::async_runtime::spawn(async move {
                    tokio_sleep(250).await;
                    let _ = open_window(&app2, &inst2);
                });
            }
        } else {
            apply_layer(&w, &inst.layer);
        }
    }
    state.save_layout_now();
    notify_widget(&app, &inst);
    notify_hub(&app);
    Ok(inst)
}

#[tauri::command]
fn open_hub(app: AppHandle, instance_id: Option<String>) {
    show_hub(&app);
    if let Some(id) = instance_id {
        let _ = app.emit_to("hub", "focus-instance", id);
    }
}

#[tauri::command]
fn open_addons_folder(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    std::fs::create_dir_all(&state.addons_dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(state.addons_dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn app_paths(state: State<AppState>) -> Value {
    json!({
        "builtin": state.builtin_dir, "addons": state.addons_dir,
        "layout": state.layout_file, "storage": state.storage_dir,
        "presets": state.presets_dir, "key_storage": state.secrets.backend_name(),
        "discord_client_id": discord::client_id(&state),
        // `--section=builder` opens the Hub on that page (shortcuts, testing).
        "start_section": std::env::args().find_map(|a| a.strip_prefix("--section=").map(String::from)),
    })
}

// ---------- commands used by widget windows (permission-checked) ----------

#[tauri::command]
fn widget_context(window: WebviewWindow, state: State<AppState>) -> Result<Value, String> {
    let (inst, widget) = widget_of(&window, &state, None)?;
    let mut children = Vec::new();
    if widget.is_stack() {
        let members: Vec<Instance> = state
            .layout
            .lock()
            .unwrap()
            .instances
            .iter()
            .filter(|i| i.parent.as_deref() == Some(inst.id.as_str()))
            .cloned()
            .collect();
        for c in members {
            if let Ok(w) = state.widget(&c.widget_id) {
                children.push(json!({ "manifest": w.manifest, "instance": c }));
            }
        }
    }
    Ok(json!({ "manifest": widget.manifest, "instance": inst, "children": children, "theme": theme::saved(&state) }))
}

#[tauri::command]
fn system_stats(window: WebviewWindow, state: State<AppState>, instance: Option<String>) -> Result<Value, String> {
    let (_, widget) = widget_of(&window, &state, instance)?;
    let allowed = widget.system_permissions();
    if allowed.is_empty() {
        return Err("this widget has no `system` permission".into());
    }
    let mut system = state.system.lock().unwrap();
    let source = system.as_mut().ok_or("system information is not available on this computer")?;
    Ok(source.snapshot(&allowed))
}

#[tauri::command]
fn storage_get(window: WebviewWindow, state: State<AppState>, instance: Option<String>, key: String) -> Result<Value, String> {
    let (inst, widget) = widget_of(&window, &state, instance)?;
    if !widget.has_storage() {
        return Err("this widget has no `storage` permission".into());
    }
    let data: Map<String, Value> = model::read_json(&state.storage_file(&inst.id));
    Ok(data.get(&key).cloned().unwrap_or(Value::Null))
}

#[tauri::command]
fn storage_set(
    window: WebviewWindow,
    state: State<AppState>,
    instance: Option<String>,
    key: String,
    value: Value,
) -> Result<(), String> {
    let (inst, widget) = widget_of(&window, &state, instance)?;
    if !widget.has_storage() {
        return Err("this widget has no `storage` permission".into());
    }
    let file = state.storage_file(&inst.id);
    let mut data: Map<String, Value> = model::read_json(&file);
    data.insert(key, value);
    if serde_json::to_vec(&data).map(|b| b.len()).unwrap_or(0) > model::STORAGE_LIMIT {
        return Err("storage is full (1 MB per widget)".into());
    }
    model::write_json(&file, &data)
}

fn blank_secrets(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        let Some(end) = rest[start..].find("]]").map(|e| e + start) else { break };
        out.push_str(&rest[..start]);
        rest = &rest[end + 2..];
    }
    out.push_str(rest);
    out
}

/// Hosts allowed by a setting: each address in it ("http://host:port/...",
/// one per line or comma, optionally "Name | address").
pub(crate) fn setting_hosts(value: &str) -> Vec<String> {
    value
        .split(['\n', ','])
        .filter_map(|line| {
            let addr = line.rsplit('|').next()?.trim();
            let u = url::Url::parse(addr).ok()?;
            let host = u.host_str()?.to_string();
            Some(match u.port() {
                Some(p) => format!("{host}:{p}"),
                None => host,
            })
        })
        .collect()
}

/// HTTP request for a widget, limited to its `permissions.network` hosts.
#[tauri::command]
async fn net_fetch(
    window: WebviewWindow,
    state: State<'_, AppState>,
    instance: Option<String>,
    url: String,
    method: Option<String>,
    headers: Option<Map<String, Value>>,
    body: Option<String>,
) -> Result<Value, String> {
    let (inst, widget) = widget_of(&window, &state, instance)?;
    // `setting:<key>` allows the host of a URL the user typed into that setting.
    let hosts: Vec<String> = widget
        .network_hosts()
        .into_iter()
        .filter_map(|h| match h.strip_prefix("setting:") {
            // A setting may hold one address, or a list (one per line, "Name | address").
            Some(key) => Some(setting_hosts(widget.setting(&inst, key)?.as_str()?)),
            None => Some(vec![h]),
        })
        .flatten()
        .collect();
    if hosts.is_empty() {
        return Err("this widget has no `network` permission (or its address setting is empty)".into());
    }
    // Put saved secrets into [[secret:name]] placeholders. A header whose
    // secret is not set is left out.
    // [[basic:user_setting:secret]] becomes HTTP basic sign-in ("user:password" in base64).
    let fill = |text: &str| -> Option<String> {
        let mut out = String::new();
        let mut rest = text;
        while let Some(start) = rest.find("[[") {
            let end = rest[start..].find("]]")? + start;
            let inner = &rest[start + 2..end];
            out.push_str(&rest[..start]);
            if let Some(key) = inner.strip_prefix("secret:") {
                out.push_str(&state.secrets.get(&inst.id, key)?);
            } else if let Some((user_key, secret_key)) = inner.strip_prefix("basic:").and_then(|k| k.split_once(':')) {
                use base64::Engine;
                let user = widget.setting(&inst, user_key).and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
                let pass = state.secrets.get(&inst.id, secret_key)?;
                out.push_str(&base64::engine::general_purpose::STANDARD.encode(format!("{}:{pass}", user.trim())));
            } else {
                out.push_str(&rest[start..end + 2]);
            }
            rest = &rest[end + 2..];
        }
        out.push_str(rest);
        Some(out)
    };
    let url = fill(&url).ok_or("a secret this request needs is not set")?;
    let headers = headers
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(k, v)| v.as_str().and_then(|v| fill(v)).map(|v| (k, v)))
        .collect();
    // In a body, a secret that is not set becomes empty text (an optional API key).
    let body = body.map(|b| fill(&b).unwrap_or_else(|| blank_secrets(&b)));
    let req = net::Request { url, method: method.unwrap_or_default(), headers, body };
    net::fetch(req, hosts).await
}

#[tauri::command]
async fn media_status(window: WebviewWindow, state: State<'_, AppState>, instance: Option<String>) -> Result<Value, String> {
    let (_, widget) = widget_of(&window, &state, instance)?;
    if !widget.permission("media") {
        return Err("this widget has no `media` permission".into());
    }
    tauri::async_runtime::spawn_blocking(media::status).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn media_control(
    window: WebviewWindow,
    state: State<'_, AppState>,
    instance: Option<String>,
    action: String,
) -> Result<(), String> {
    let (_, widget) = widget_of(&window, &state, instance)?;
    if !widget.permission("media") {
        return Err("this widget has no `media` permission".into());
    }
    tauri::async_runtime::spawn_blocking(move || media::control(&action)).await.map_err(|e| e.to_string())?
}

fn clipboard_allowed(window: &WebviewWindow, state: &AppState, instance: Option<String>) -> Result<(), String> {
    let (_, widget) = widget_of(window, state, instance)?;
    if !widget.permission("clipboard") {
        return Err("this widget has no `clipboard` permission".into());
    }
    Ok(())
}

#[tauri::command]
fn clipboard_history(window: WebviewWindow, state: State<AppState>, instance: Option<String>) -> Result<Value, String> {
    clipboard_allowed(&window, &state, instance)?;
    Ok(state.clipboard.lock().unwrap().poll())
}

#[tauri::command]
fn clipboard_copy(window: WebviewWindow, state: State<AppState>, instance: Option<String>, text: String) -> Result<(), String> {
    clipboard_allowed(&window, &state, instance)?;
    state.clipboard.lock().unwrap().copy(text)
}

#[tauri::command]
fn clipboard_clear(window: WebviewWindow, state: State<AppState>, instance: Option<String>) -> Result<(), String> {
    clipboard_allowed(&window, &state, instance)?;
    state.clipboard.lock().unwrap().clear();
    Ok(())
}

/// Opens a web link from the Hub (credits, help) in the default browser.
#[tauri::command]
fn hub_open_link(window: WebviewWindow, url: String) -> Result<(), String> {
    if window.label() != "hub" {
        return Err("only the Hub may call this".into());
    }
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("only web links can be opened".into());
    }
    window.app_handle().opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_url(window: WebviewWindow, url: String) -> Result<(), String> {
    instance_id_of(&window)?;
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("only web links can be opened".into());
    }
    window.app_handle().opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

async fn tokio_sleep(ms: u64) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(Duration::from_millis(ms))).await;
}

// ---------- screen time and email ----------

#[tauri::command]
fn screen_time(window: WebviewWindow, state: State<AppState>, instance: Option<String>) -> Result<Value, String> {
    let (_, widget) = widget_of(&window, &state, instance)?;
    if !widget.permission("activity") {
        return Err("this widget has no `activity` permission".into());
    }
    Ok(activity::report(state.storage_dir.join("screen-time.json")))
}

#[tauri::command]
fn screen_time_clear(window: WebviewWindow, state: State<AppState>, instance: Option<String>) -> Result<(), String> {
    let (_, widget) = widget_of(&window, &state, instance)?;
    if !widget.permission("activity") {
        return Err("this widget has no `activity` permission".into());
    }
    activity::clear();
    Ok(())
}

/// Unread mail over IMAP. Server, port and user come from settings; the
/// password is the `password` secret.
#[tauri::command]
async fn mail_check(window: WebviewWindow, state: State<'_, AppState>, instance: Option<String>) -> Result<Value, String> {
    let (inst, widget) = widget_of(&window, &state, instance)?;
    if !widget.permission("mail") {
        return Err("this widget has no `mail` permission".into());
    }
    let text = |k: &str| widget.setting(&inst, k).and_then(|v| v.as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty());
    let (Some(host), Some(user)) = (text("server"), text("user")) else { return Ok(json!({ "configured": false })) };
    let Some(password) = state.secrets.get(&inst.id, "password") else { return Ok(json!({ "configured": false })) };
    let port = widget.setting(&inst, "port").and_then(|v| v.as_u64()).unwrap_or(993) as u16;
    let account = mail::Account {
        host,
        port,
        user,
        password,
        folder: text("folder").unwrap_or_else(|| "INBOX".into()),
        preview: widget.setting(&inst, "preview").and_then(|v| v.as_u64()).unwrap_or(3).min(10) as usize,
    };
    tauri::async_runtime::spawn_blocking(move || mail::check(&account)).await.map_err(|e| e.to_string())?
}

// ---------- app setup ----------

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "hub", "Open WidgetForge", true, None::<&str>)?,
            &MenuItem::with_id(app, "show", "Show all widgets", true, None::<&str>)?,
            &MenuItem::with_id(app, "hide", "Hide all widgets", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?,
        ],
    )?;
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("WidgetForge")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "hub" => show_hub(app),
            "show" => shell::set_all_widgets(app, true),
            "hide" => shell::set_all_widgets(app, false),
            "quit" => {
                app.state::<AppState>().save_layout_now();
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

/// Remembers where a widget window was moved or resized to.
fn track_geometry(window: &tauri::Window, event: &WindowEvent) {
    let Some(id) = window.label().strip_prefix(WIDGET_PREFIX) else { return };
    let scale = window.scale_factor().unwrap_or(1.0);
    let state = window.state::<AppState>();
    // The first move right after opening is the window manager placing the
    // window; remember its offset instead of saving it.
    let mut offset = (0.0, 0.0);
    if let WindowEvent::Moved(p) = event {
        let p: LogicalPosition<f64> = p.to_logical(scale);
        let mut placements = state.placements.lock().unwrap();
        if let Some(pl) = placements.get_mut(window.label()) {
            match pl.offset {
                Some(o) => offset = o,
                None => {
                    let o = if pl.opened.elapsed() < Duration::from_millis(1500) {
                        (p.x - pl.requested.0, p.y - pl.requested.1)
                    } else {
                        (0.0, 0.0)
                    };
                    pl.offset = Some(o);
                    if pl.opened.elapsed() < Duration::from_millis(1500) {
                        return;
                    }
                }
            }
        }
    }
    let mut layout = state.layout.lock().unwrap();
    let Some(inst) = layout.get_mut(id) else { return };
    // A drawer's place comes from its edge; only an open drawer's size is kept.
    if drawer::is_drawer(inst) {
        if let WindowEvent::Resized(sz) = event {
            let sz: LogicalSize<f64> = sz.to_logical(scale);
            if state.drawers_open.lock().unwrap().contains(id) && sz.width > 60.0 && sz.height > 60.0 {
                inst.w = sz.width;
                inst.h = sz.height;
                state.layout_dirty.store(true, Ordering::SeqCst);
            }
        }
        return;
    }
    match event {
        WindowEvent::Moved(p) => {
            let p: LogicalPosition<f64> = p.to_logical(scale);
            inst.x = p.x - offset.0;
            inst.y = p.y - offset.1;
        }
        WindowEvent::Resized(s) => {
            let s: LogicalSize<f64> = s.to_logical(scale);
            if s.width < 1.0 || s.height < 1.0 {
                return;
            }
            inst.w = s.width;
            inst.h = s.height;
        }
        _ => return,
    }
    state.layout_dirty.store(true, Ordering::SeqCst);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // Double-clicking a .forgewidget while WidgetForge runs offers to install it.
            let found = packages::packages_in_args(&args);
            if found.is_empty() {
                show_hub(app);
            }
            for p in found {
                packages::offer_import(app, p);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| shell::on_shortcut(app, shortcut, event))
                .build(),
        )
        .register_uri_scheme_protocol("widget", |ctx, request| protocol::handle(ctx.app_handle(), request))
        .setup(|app| {
            let paths = app.path();
            let builtin_dir = paths.resource_dir()?.join("widgets");
            let data_dir = paths.app_data_dir()?;
            let config_dir = paths.app_config_dir()?;
            let addons_dir = data_dir.join("addons");
            let storage_dir = data_dir.join("storage");
            let secrets_dir = data_dir.join("secrets");
            let presets_dir = data_dir.join("presets");
            std::fs::create_dir_all(&presets_dir)?;
            std::fs::create_dir_all(&addons_dir)?;
            std::fs::create_dir_all(&storage_dir)?;
            std::fs::create_dir_all(&secrets_dir)?;
            let layout_file = config_dir.join("layout.json");

            let (widgets, errors) = model::scan(&builtin_dir, &addons_dir);
            for e in &errors {
                eprintln!("widget error: {e}");
            }
            let layout: Layout = model::read_json(&layout_file);
            app.manage(AppState {
                builtin_dir,
                addons_dir,
                layout_file,
                storage_dir,
                presets_dir,
                secrets: secrets::Secrets::new(secrets_dir),
                widgets: Mutex::new(widgets),
                errors: Mutex::new(errors),
                layout: Mutex::new(layout),
                layout_dirty: AtomicBool::new(false),
                placements: Mutex::new(HashMap::new()),
                clipboard: Mutex::new(clip::ClipState::new()),
                shortcuts: Mutex::new(shell::Shortcuts::default()),
                pending_imports: Mutex::new(Vec::new()),
                drawers_open: Mutex::new(Default::default()),
                system: Mutex::new(SystemSource::new().map_err(|e| eprintln!("system info: {e}")).ok()),
            });

            build_tray(app.handle())?;

            // Global keyboard shortcuts from Settings (show/hide widgets, open the Hub).
            let (toggle, hub) = shell::saved_shortcuts(&app.state::<AppState>());
            if let Err(e) = shell::apply_shortcuts(app.handle(), &toggle, &hub) {
                eprintln!("shortcuts: {e}");
            }

            // Started by double-clicking a .forgewidget: offer to install it.
            for p in packages::packages_in_args(&std::env::args().collect::<Vec<_>>()) {
                packages::offer_import(app.handle(), p);
            }

            // Open the widgets marked "Start with WidgetForge".
            let state = app.state::<AppState>();
            let startup: Vec<Instance> = state
                .layout
                .lock()
                .unwrap()
                .instances
                .iter()
                .filter(|i| i.start_with_app && i.open && i.parent.is_none())
                .cloned()
                .collect();
            for inst in startup {
                if let Err(e) = open_window(app.handle(), &inst) {
                    eprintln!("could not open {}: {e}", inst.widget_id);
                }
            }

            // `--background` (sign-in launch) starts in the tray without the Hub.
            if !std::env::args().any(|a| a == "--background") {
                show_hub(app.handle());
            }

            // Save moved/resized positions at most once a second.
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                let state = handle.state::<AppState>();
                if state.layout_dirty.load(Ordering::SeqCst) {
                    state.save_layout_now();
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "hub" {
                // Closing the Hub hides it to the tray.
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
                return;
            }
            track_geometry(window, event);
        })
        .invoke_handler(tauri::generate_handler![
            list_widgets,
            reload_widgets,
            list_instances,
            add_instance,
            remove_instance,
            show_instance,
            update_instance,
            move_to_stack,
            set_secret,
            open_hub,
            open_addons_folder,
            app_paths,
            widget_context,
            system_stats,
            storage_get,
            storage_set,
            net_fetch,
            media_status,
            media_control,
            clipboard_history,
            clipboard_copy,
            clipboard_clear,
            open_url,
            hub_open_link,
            presets::list_presets,
            presets::save_preset,
            presets::apply_preset,
            presets::delete_preset,
            presets::open_presets_folder,
            discord::discord_login,
            discord::discord_status,
            discord::discord_logout,
            discord::set_discord_client_id,
            discord::discord_api,
            shell::get_app_settings,
            shell::set_shortcuts,
            shell::set_autostart,
            local::folder_list,
            local::net_latency,
            local::audio_status,
            local::audio_set,
            local::docker_list,
            local::docker_action,
            local::run_command,
            local::open_folder_item,
            local::tail_file,
            local::wake_on_lan,
            local::minecraft_status,
            local::notify,
            local::speed_test,
            screen_time,
            screen_time_clear,
            mail_check,
            theme::get_theme,
            theme::set_theme,
            builder::builder_save,
            builder::builder_sample_system,
            builder::builder_fetch,
            builder::builder_run,
            packages::export_widget,
            packages::reveal_file,
            packages::inspect_package,
            packages::install_package,
            packages::pending_imports,
            drawer::drawer_set,
            drawer::drawer_slide,
            registry::registry_index,
            registry::set_registry_url,
            registry::registry_download,
        ])
        .run(tauri::generate_context!())
        .expect("error while running WidgetForge");
}
