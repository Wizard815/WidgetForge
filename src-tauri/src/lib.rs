//! WidgetForge core. See docs/DESIGN.md for the architecture.

pub mod commands;
pub mod error;
pub mod layout;
pub mod manifest;
pub mod packages;
pub mod protocol;
pub mod registry;
pub mod settings;
pub mod state;
pub mod storage;
pub mod store;
pub mod system;
pub mod tray;
pub mod windows;

use std::path::PathBuf;
use std::time::Duration;
use tauri::{Emitter, Manager, RunEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as _};

use state::{AppState, Paths};

/// Passed by the OS login item: start with widgets only, no Hub.
pub const BACKGROUND_FLAG: &str = "--background";

fn builtin_dir(app: &tauri::App) -> PathBuf {
    if let Ok(res) = app.path().resource_dir() {
        let dir = res.join("widgets");
        if dir.is_dir() {
            return dir;
        }
    }
    // Development builds: use the repository folder.
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../widgets"))
}

/// A `.forgewidget` file passed on the command line (double-click) is offered to the Hub.
fn forward_package_args(app: &tauri::AppHandle, args: &[String]) {
    for a in args.iter().skip(1) {
        if a.ends_with(&format!(".{}", packages::EXTENSION)) {
            windows::show_hub(app);
            let _ = app.emit_to(windows::HUB, "forge://import-request", a.clone());
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let background = args.iter().any(|a| a == BACKGROUND_FLAG);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            windows::show_hub(app);
            forward_package_args(app, &args);
        }))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![BACKGROUND_FLAG])))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .register_uri_scheme_protocol("widget", protocol::handle)
        .setup(move |app| {
            let data = app.path().app_data_dir()?;
            let paths = Paths {
                config: app.path().app_config_dir()?,
                addons: data.join("addons"),
                storage: data.join("storage"),
                builtin: builtin_dir(app),
                data,
            };
            let state = AppState::load(paths);
            state.sampler.start();
            app.manage(state);

            let handle = app.handle().clone();
            tray::create(&handle)?;

            // Keep the stored setting in sync with the real OS login item.
            if let Ok(enabled) = handle.autolaunch().is_enabled() {
                let st = handle.state::<AppState>();
                st.settings.lock().unwrap().launch_on_startup = enabled;
            }

            // Open the widgets marked "start with WidgetForge".
            let startup: Vec<String> = {
                let st = handle.state::<AppState>();
                let layout = st.layout.lock().unwrap();
                layout.instances.iter().filter(|i| i.start_with_app).map(|i| i.id.clone()).collect()
            };
            for id in startup {
                if let Err(e) = windows::open(&handle, &id) {
                    eprintln!("[widgetforge] could not open instance {id}: {e}");
                }
            }

            let show_hub = !background && handle.state::<AppState>().settings.lock().unwrap().show_hub_on_launch;
            if show_hub {
                windows::show_hub(&handle);
            }
            forward_package_args(&handle, &args);

            // Save window moves at most once a second.
            let saver = handle.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                saver.state::<AppState>().save_layout_if_dirty();
            });
            Ok(())
        })
        .on_window_event(windows::on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::hub_state,
            commands::instance_add,
            commands::instance_update,
            commands::instance_remove,
            commands::instance_set_open,
            commands::widgets_set_hidden,
            commands::registry_reload,
            commands::settings_update,
            commands::addons_open_folder,
            commands::package_import,
            commands::package_export,
            commands::widget_uninstall,
            commands::app_quit,
            commands::widget_context,
            commands::widget_system,
            commands::widget_storage_all,
            commands::widget_storage_set,
            commands::widget_fetch,
            commands::widget_drawer,
            commands::widget_close,
            commands::widget_open_settings,
            commands::widget_set_layer,
            commands::widget_set_locked,
            commands::open_url,
        ])
        .build(tauri::generate_context!())
        .expect("error while building WidgetForge")
        .run(|_app, event| {
            // Closing every window must not quit: WidgetForge lives in the tray.
            if let RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
            }
        });
}
