//! Shared app state, owned by Tauri and reachable from every command.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock};
use std::time::Duration;

use crate::error::{Error, Result};
use crate::layout::Layout;
use crate::registry::Registry;
use crate::settings::AppSettings;
use crate::storage::Storage;
use crate::store;
use crate::system::Sampler;

#[derive(Debug, Clone)]
pub struct Paths {
    pub config: PathBuf,
    pub data: PathBuf,
    pub addons: PathBuf,
    pub storage: PathBuf,
    pub builtin: PathBuf,
}

impl Paths {
    pub fn layout_file(&self) -> PathBuf {
        self.config.join("layout.json")
    }
    pub fn settings_file(&self) -> PathBuf {
        self.config.join("settings.json")
    }
}

pub struct AppState {
    pub paths: Paths,
    pub registry: RwLock<Registry>,
    pub layout: Mutex<Layout>,
    pub settings: Mutex<AppSettings>,
    pub storage: Storage,
    pub sampler: Sampler,
    pub http: reqwest::Client,
    layout_dirty: AtomicBool,
}

impl AppState {
    pub fn load(paths: Paths) -> AppState {
        for dir in [&paths.config, &paths.data, &paths.addons, &paths.storage] {
            let _ = std::fs::create_dir_all(dir);
        }
        let registry = Registry::scan(&paths.builtin, &paths.addons);
        for e in &registry.errors {
            for i in &e.issues {
                eprintln!("[widgetforge] widget folder '{}': {i}", e.folder);
            }
        }
        let layout: Layout = store::load_json(&paths.layout_file());
        let settings: AppSettings = store::load_json(&paths.settings_file());
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            // Redirects could leave the widget's allowed hosts, so they are not followed.
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("WidgetForge/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("http client");
        AppState {
            storage: Storage::new(&paths.storage),
            registry: RwLock::new(registry),
            layout: Mutex::new(layout),
            settings: Mutex::new(settings),
            sampler: Sampler::new(),
            http,
            layout_dirty: AtomicBool::new(false),
            paths,
        }
    }

    pub fn reload_registry(&self) {
        *self.registry.write().unwrap() = Registry::scan(&self.paths.builtin, &self.paths.addons);
    }

    /// Ask for the layout to be saved soon (window moves fire many events).
    pub fn mark_layout_dirty(&self) {
        self.layout_dirty.store(true, Ordering::Relaxed);
    }

    pub fn save_layout_now(&self) -> Result<()> {
        self.layout_dirty.store(false, Ordering::Relaxed);
        let layout = self.layout.lock().unwrap().clone();
        store::save_json(&self.paths.layout_file(), &layout)
    }

    pub fn save_layout_if_dirty(&self) {
        if self.layout_dirty.swap(false, Ordering::Relaxed) {
            if let Err(e) = self.save_layout_now() {
                eprintln!("[widgetforge] saving layout failed: {e}");
            }
        }
    }

    pub fn save_settings(&self) -> Result<()> {
        let s = self.settings.lock().unwrap().clone();
        store::save_json(&self.paths.settings_file(), &s)
    }

    pub fn widget_dir(&self, widget_id: &str) -> Result<PathBuf> {
        self.registry
            .read()
            .unwrap()
            .get(widget_id)
            .map(|e| e.dir.clone())
            .ok_or_else(|| Error::NotFound(format!("widget {widget_id}")))
    }
}
