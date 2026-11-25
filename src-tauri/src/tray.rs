use crate::physics::{HueState, PhysicsBody, PET_WINDOW};
use crate::window_util;
use crate::state::AppState;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show_i = MenuItem::with_id(app, "show", "Afficher HUE", true, None::<&str>)?;
    let peek_i = MenuItem::with_id(app, "peek", "Ranger", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let restart_i = MenuItem::with_id(app, "restart", "Redémarrer", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_i, &peek_i, &sep, &restart_i, &quit_i])?;

    // Prefer the bundled app icon (same custom HUE mascot as the .exe).
    let icon = Image::from_bytes(include_bytes!("../icons/icon.png"))
        .or_else(|_| Image::from_bytes(include_bytes!("../icons/hue-tray.png")))
        .or_else(|_| {
            app.default_window_icon()
                .cloned()
                .ok_or_else(|| tauri::Error::FailedToReceiveMessage)
        })?;

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .tooltip("HUE v1.0 — Made by Batpolygon")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_hue(app),
            "peek" => peek_hue(app),
            "restart" => app.restart(),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn show_hue(app: &AppHandle) {
    let state = app.state::<AppState>();
    *state.concealed.lock() = false;
    *state.hud_open.lock() = false;
    *state.settings_open.lock() = false;
    let area = state.cached_area.lock().clone();
    {
        let mut body = state.body.lock();
        body.set_size(PET_WINDOW, PET_WINDOW);
        body.speaking = false;
        body.peeking = false;
        let (x, y) = PhysicsBody::emerge_position(&area, PET_WINDOW, PET_WINDOW);
        body.x = x;
        body.y = y;
        body.vx = 0.0;
        body.vy = 0.0;
        body.state = HueState::Idle;
    }
    if let Some(win) = app.get_webview_window("main") {
        let body = state.body.lock();
        let _ = win.set_size(tauri::PhysicalSize::new(
            PET_WINDOW as u32,
            PET_WINDOW as u32,
        ));
        let _ = win.set_position(tauri::PhysicalPosition::new(body.x as i32, body.y as i32));
        drop(body);
        window_util::show_without_focus(&win);
    }
    let _ = app.emit("hue-hud", serde_json::json!({ "open": false }));
}

pub fn peek_hue(app: &AppHandle) {
    let state = app.state::<AppState>();
    *state.hud_open.lock() = false;
    *state.settings_open.lock() = false;
    let area = state.cached_area.lock().clone();
    {
        let mut body = state.body.lock();
        body.set_size(PET_WINDOW, PET_WINDOW);
        body.speaking = false;
        body.enter_peek(&area);
    }
    if let Some(win) = app.get_webview_window("bubble") {
        let _ = win.hide();
    }
    if let Some(win) = app.get_webview_window("main") {
        let body = state.body.lock();
        let _ = win.set_size(tauri::PhysicalSize::new(
            PET_WINDOW as u32,
            PET_WINDOW as u32,
        ));
        let _ = win.set_position(tauri::PhysicalPosition::new(body.x as i32, body.y as i32));
        drop(body);
        window_util::show_without_focus(&win);
        window_util::make_non_activating(&win);
    }
    let _ = app.emit("hue-hud", serde_json::json!({ "open": false }));
}
