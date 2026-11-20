//! System tray icon and menu.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::state::AppState;
use crate::windows;

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open WidgetForge", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "Show widgets", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Hide widgets", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit WidgetForge", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &sep, &show, &hide, &sep, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("WidgetForge")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => windows::show_hub(app),
            "show" => windows::set_all_hidden(app, false),
            "hide" => windows::set_all_hidden(app, true),
            "quit" => quit_app(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                windows::show_hub(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

pub fn quit_app(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Err(e) = state.save_layout_now() {
        eprintln!("[widgetforge] saving layout on quit failed: {e}");
    }
    app.exit(0);
}
