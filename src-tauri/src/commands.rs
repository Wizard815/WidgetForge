//! Commands callable from the webviews. Hub commands only work from the Hub window;
//! widget commands act on the calling widget's own instance and check its permissions.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_opener::OpenerExt as _;

use crate::error::{Error, Result};
use crate::layout::{Instance, InstancePatch};
use crate::manifest::{Issue, Layer, Manifest};
use crate::packages::{self, ImportPreview};
use crate::registry::{LoadError, Origin, WidgetEntry};
use crate::settings::{AppSettings, SettingsPatch};
use crate::state::AppState;
use crate::system;
use crate::windows;

pub const EVENT_HUB_CHANGED: &str = "forge://changed";
pub const EVENT_INSTANCE: &str = "forge://instance";

fn require_hub(w: &WebviewWindow) -> Result<()> {
    if w.label() == windows::HUB {
        Ok(())
    } else {
        Err(Error::Permission("only the Hub can do this".into()))
    }
}

fn notify_hub(app: &AppHandle) {
    let _ = app.emit_to(windows::HUB, EVENT_HUB_CHANGED, ());
}

/// The instance and widget behind a calling widget window.
fn caller(w: &WebviewWindow, state: &AppState) -> Result<(Instance, WidgetEntry)> {
    let id = windows::instance_id(w.label()).ok_or_else(|| Error::Permission("not a widget window".into()))?;
    let inst = state.layout.lock().unwrap().get(id).cloned().ok_or_else(|| Error::NotFound("instance".into()))?;
    let entry = state
        .registry
        .read()
        .unwrap()
        .get(&inst.widget_id)
        .cloned()
        .ok_or_else(|| Error::NotFound(format!("widget {}", inst.widget_id)))?;
    Ok((inst, entry))
}

// ───────────────────────────── Hub ─────────────────────────────

#[derive(Serialize)]
pub struct WidgetInfo {
    pub manifest: Manifest,
    pub origin: Origin,
    pub warnings: Vec<Issue>,
    pub instances: usize,
}

#[derive(Serialize)]
pub struct InstanceInfo {
    #[serde(flatten)]
    pub instance: Instance,
    pub open: bool,
    pub missing: bool,
}

#[derive(Serialize)]
pub struct HubState {
    pub widgets: Vec<WidgetInfo>,
    pub errors: Vec<LoadError>,
    pub instances: Vec<InstanceInfo>,
    pub settings: AppSettings,
    pub addons_dir: String,
    pub version: &'static str,
    pub platform: &'static str,
}

#[tauri::command]
pub fn hub_state(app: AppHandle, window: WebviewWindow, state: tauri::State<AppState>) -> Result<HubState> {
    require_hub(&window)?;
    let reg = state.registry.read().unwrap();
    let layout = state.layout.lock().unwrap();
    let widgets = reg
        .widgets
        .values()
        .map(|e| WidgetInfo {
            manifest: e.manifest.clone(),
            origin: e.origin,
            warnings: e.warnings.clone(),
            instances: layout.count_of(&e.manifest.id),
        })
        .collect();
    let instances = layout
        .instances
        .iter()
        .map(|i| InstanceInfo {
            instance: i.clone(),
            open: windows::is_open(&app, &i.id),
            missing: reg.get(&i.widget_id).is_none(),
        })
        .collect();
    Ok(HubState {
        widgets,
        errors: reg.errors.clone(),
        instances,
        settings: state.settings.lock().unwrap().clone(),
        addons_dir: state.paths.addons.to_string_lossy().into_owned(),
        version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
    })
}

#[tauri::command]
pub fn instance_add(app: AppHandle, window: WebviewWindow, state: tauri::State<AppState>, widget_id: String) -> Result<Instance> {
    require_hub(&window)?;
    let manifest = state
        .registry
        .read()
        .unwrap()
        .get(&widget_id)
        .map(|e| e.manifest.clone())
        .ok_or_else(|| Error::NotFound(format!("widget {widget_id}")))?;
    let inst = {
        let mut layout = state.layout.lock().unwrap();
        if !manifest.window.multi_instance && layout.count_of(&widget_id) > 0 {
            return Err(Error::msg(format!("{} can only be added once", manifest.name)));
        }
        let wa = windows::primary_work_area(&app);
        let (x, y) = layout.next_position(wa.x, wa.y, wa.w, manifest.window.size[0] as f64);
        let inst = Instance::new(uuid::Uuid::new_v4().simple().to_string(), &manifest, x, y);
        layout.instances.push(inst.clone());
        inst
    };
    state.save_layout_now()?;
    windows::open(&app, &inst.id)?;
    notify_hub(&app);
    Ok(inst)
}

#[tauri::command]
pub fn instance_update(
    app: AppHandle,
    window: WebviewWindow,
    state: tauri::State<AppState>,
    id: String,
    patch: InstancePatch,
) -> Result<Instance> {
    require_hub(&window)?;
    let (before, after) = {
        let mut layout = state.layout.lock().unwrap();
        let inst = layout.get_mut(&id).ok_or_else(|| Error::NotFound("instance".into()))?;
        let before = inst.clone();
        patch.apply(inst);
        (before, inst.clone())
    };
    state.save_layout_now()?;
    windows::apply(&app, &before, &after)?;
    let _ = app.emit_to(windows::label(&id), EVENT_INSTANCE, ());
    notify_hub(&app);
    Ok(after)
}

#[tauri::command]
pub fn instance_remove(app: AppHandle, window: WebviewWindow, state: tauri::State<AppState>, id: String) -> Result<()> {
    require_hub(&window)?;
    windows::close(&app, &id);
    state.layout.lock().unwrap().remove(&id);
    state.storage.delete_instance(&id);
    state.save_layout_now()?;
    notify_hub(&app);
    Ok(())
}

#[tauri::command]
pub fn instance_set_open(app: AppHandle, window: WebviewWindow, id: String, open: bool) -> Result<()> {
    require_hub(&window)?;
    if open {
        windows::open(&app, &id)?;
    } else {
        windows::close(&app, &id);
    }
    notify_hub(&app);
    Ok(())
}

#[tauri::command]
pub fn widgets_set_hidden(app: AppHandle, window: WebviewWindow, hidden: bool) -> Result<()> {
    require_hub(&window)?;
    windows::set_all_hidden(&app, hidden);
    notify_hub(&app);
    Ok(())
}

#[tauri::command]
pub fn registry_reload(app: AppHandle, window: WebviewWindow, state: tauri::State<AppState>) -> Result<()> {
    require_hub(&window)?;
    state.reload_registry();
    // Reload open widgets so they pick up manifest changes.
    for (lbl, w) in app.webview_windows() {
        if windows::instance_id(&lbl).is_some() {
            let _ = w.eval("location.reload()");
        }
    }
    notify_hub(&app);
    Ok(())
}

#[tauri::command]
pub fn settings_update(
    app: AppHandle,
    window: WebviewWindow,
    state: tauri::State<AppState>,
    patch: SettingsPatch,
) -> Result<AppSettings> {
    require_hub(&window)?;
    if let Some(on) = patch.launch_on_startup {
        let al = app.autolaunch();
        let res = if on { al.enable() } else { al.disable() };
        res.map_err(|e| Error::msg(format!("could not change the startup setting: {e}")))?;
    }
    let s = {
        let mut s = state.settings.lock().unwrap();
        patch.apply(&mut s);
        s.clone()
    };
    state.save_settings()?;
    notify_hub(&app);
    Ok(s)
}

#[tauri::command]
pub fn addons_open_folder(app: AppHandle, window: WebviewWindow, state: tauri::State<AppState>) -> Result<()> {
    require_hub(&window)?;
    app.opener()
        .open_path(state.paths.addons.to_string_lossy(), None::<&str>)
        .map_err(|e| Error::msg(e.to_string()))
}

#[tauri::command]
pub fn package_import(
    app: AppHandle,
    window: WebviewWindow,
    state: tauri::State<AppState>,
    path: PathBuf,
    install: bool,
) -> Result<ImportPreview> {
    require_hub(&window)?;
    let preview = {
        let reg = state.registry.read().unwrap();
        packages::import(&path, &state.paths.addons, &reg, install)?
    };
    if install {
        state.reload_registry();
        // Reload windows of the updated widget.
        let ids: Vec<String> = state
            .layout
            .lock()
            .unwrap()
            .instances
            .iter()
            .filter(|i| i.widget_id == preview.manifest.id)
            .map(|i| i.id.clone())
            .collect();
        for id in ids {
            if let Some(w) = app.get_webview_window(&windows::label(&id)) {
                let _ = w.eval("location.reload()");
            }
        }
        notify_hub(&app);
    }
    Ok(preview)
}

#[tauri::command]
pub fn package_export(window: WebviewWindow, state: tauri::State<AppState>, widget_id: String, dest: PathBuf) -> Result<()> {
    require_hub(&window)?;
    let dir = state.widget_dir(&widget_id)?;
    packages::export(&dir, &dest)
}

#[tauri::command]
pub fn widget_uninstall(app: AppHandle, window: WebviewWindow, state: tauri::State<AppState>, widget_id: String) -> Result<()> {
    require_hub(&window)?;
    let dir = {
        let reg = state.registry.read().unwrap();
        let e = reg.get(&widget_id).ok_or_else(|| Error::NotFound(format!("widget {widget_id}")))?;
        if e.origin != Origin::Addon {
            return Err(Error::Permission("built-in widgets cannot be uninstalled".into()));
        }
        e.dir.clone()
    };
    let removed: Vec<Instance> = {
        let mut layout = state.layout.lock().unwrap();
        let (gone, keep): (Vec<_>, Vec<_>) = layout.instances.drain(..).partition(|i| i.widget_id == widget_id);
        layout.instances = keep;
        gone
    };
    for i in &removed {
        windows::close(&app, &i.id);
        state.storage.delete_instance(&i.id);
    }
    state.save_layout_now()?;
    packages::uninstall(&state.paths.addons, &dir)?;
    state.reload_registry();
    notify_hub(&app);
    Ok(())
}

#[tauri::command]
pub fn app_quit(app: AppHandle, window: WebviewWindow) -> Result<()> {
    require_hub(&window)?;
    crate::tray::quit_app(&app);
    Ok(())
}

// ───────────────────────────── Widgets ─────────────────────────────

#[derive(Serialize)]
pub struct WidgetContext {
    pub instance: Instance,
    pub manifest: Manifest,
    pub settings: Map<String, Value>,
    pub origin: Origin,
    pub platform: &'static str,
}

#[tauri::command]
pub fn widget_context(window: WebviewWindow, state: tauri::State<AppState>) -> Result<WidgetContext> {
    let (inst, entry) = caller(&window, &state)?;
    Ok(WidgetContext {
        settings: inst.effective_settings(&entry.manifest),
        instance: inst,
        manifest: entry.manifest,
        origin: entry.origin,
        platform: std::env::consts::OS,
    })
}

#[tauri::command]
pub fn widget_system(window: WebviewWindow, state: tauri::State<AppState>) -> Result<Value> {
    let (_, entry) = caller(&window, &state)?;
    let allowed = &entry.manifest.permissions.system;
    if allowed.is_empty() {
        return Err(Error::Permission("this widget has no system permissions".into()));
    }
    Ok(system::filter(&state.sampler.snapshot(), allowed))
}

fn require_storage(entry: &WidgetEntry) -> Result<()> {
    if entry.manifest.permissions.storage {
        Ok(())
    } else {
        Err(Error::Permission("this widget has no storage permission".into()))
    }
}

#[tauri::command]
pub fn widget_storage_all(window: WebviewWindow, state: tauri::State<AppState>) -> Result<Map<String, Value>> {
    let (inst, entry) = caller(&window, &state)?;
    require_storage(&entry)?;
    Ok(state.storage.all(&inst.id))
}

#[tauri::command]
pub fn widget_storage_set(window: WebviewWindow, state: tauri::State<AppState>, key: String, value: Value) -> Result<()> {
    let (inst, entry) = caller(&window, &state)?;
    require_storage(&entry)?;
    state.storage.set(&inst.id, &key, value)
}

#[derive(Deserialize)]
pub struct FetchRequest {
    pub url: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Serialize)]
pub struct FetchResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: String,
}

const MAX_FETCH: usize = 2 * 1024 * 1024;

#[tauri::command]
pub async fn widget_fetch(window: WebviewWindow, app: AppHandle, req: FetchRequest) -> Result<FetchResponse> {
    let state = app.state::<AppState>();
    let (_, entry) = caller(&window, &state)?;
    let url = reqwest::Url::parse(&req.url).map_err(|e| Error::msg(format!("bad url: {e}")))?;
    if url.scheme() != "https" {
        return Err(Error::Permission("only https:// URLs are allowed".into()));
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if !entry.manifest.permissions.network.iter().any(|h| h.eq_ignore_ascii_case(&host)) {
        return Err(Error::Permission(format!("{host} is not in this widget's permissions.network")));
    }
    let method = match req.method.as_deref().unwrap_or("GET").to_ascii_uppercase().as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "DELETE" => reqwest::Method::DELETE,
        other => return Err(Error::msg(format!("method {other} not supported"))),
    };
    let mut r = state.http.request(method, url);
    for (k, v) in &req.headers {
        r = r.header(k, v);
    }
    if let Some(b) = req.body {
        r = r.body(b);
    }
    let mut resp = r.send().await?;
    let status = resp.status().as_u16();
    let content_type = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).map(String::from);
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        body.extend_from_slice(&chunk);
        if body.len() > MAX_FETCH {
            return Err(Error::msg("response is larger than 2 MB"));
        }
    }
    Ok(FetchResponse { status, content_type, body: String::from_utf8_lossy(&body).into_owned() })
}

#[tauri::command]
pub fn widget_drawer(app: AppHandle, window: WebviewWindow, open: bool) -> Result<()> {
    let id = windows::instance_id(window.label()).ok_or_else(|| Error::Permission("not a widget".into()))?;
    windows::set_drawer(&app, id, open)
}

#[tauri::command]
pub fn widget_close(app: AppHandle, window: WebviewWindow) -> Result<()> {
    let id = windows::instance_id(window.label()).ok_or_else(|| Error::Permission("not a widget".into()))?;
    windows::close(&app, id);
    notify_hub(&app);
    Ok(())
}

#[tauri::command]
pub fn widget_open_settings(app: AppHandle, window: WebviewWindow) -> Result<()> {
    let id = windows::instance_id(window.label()).ok_or_else(|| Error::Permission("not a widget".into()))?;
    windows::show_hub(&app);
    let _ = app.emit_to(windows::HUB, "forge://focus-instance", id.to_string());
    Ok(())
}

#[tauri::command]
pub fn widget_set_layer(app: AppHandle, window: WebviewWindow, state: tauri::State<AppState>, layer: Layer) -> Result<()> {
    let id = windows::instance_id(window.label()).ok_or_else(|| Error::Permission("not a widget".into()))?.to_string();
    let (before, after) = {
        let mut layout = state.layout.lock().unwrap();
        let inst = layout.get_mut(&id).ok_or_else(|| Error::NotFound("instance".into()))?;
        let before = inst.clone();
        inst.layer = layer;
        (before, inst.clone())
    };
    state.save_layout_now()?;
    windows::apply(&app, &before, &after)?;
    let _ = app.emit_to(windows::label(&id), EVENT_INSTANCE, ());
    notify_hub(&app);
    Ok(())
}

#[tauri::command]
pub fn widget_set_locked(app: AppHandle, window: WebviewWindow, state: tauri::State<AppState>, locked: bool) -> Result<()> {
    let id = windows::instance_id(window.label()).ok_or_else(|| Error::Permission("not a widget".into()))?.to_string();
    if let Some(i) = state.layout.lock().unwrap().get_mut(&id) {
        i.locked = locked;
    }
    state.save_layout_now()?;
    let _ = app.emit_to(windows::label(&id), EVENT_INSTANCE, ());
    notify_hub(&app);
    Ok(())
}

/// Opens an http(s) link in the default browser (button blocks, links in widgets).
#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> Result<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(Error::Permission("only http(s) links can be opened".into()));
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| Error::msg(e.to_string()))
}
