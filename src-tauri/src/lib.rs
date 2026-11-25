mod ai;
mod brain;
mod commands;
mod config;
mod physics;
mod secrets;
mod startup;
mod state;
mod tray;
mod util;
mod watcher;
mod window_util;

use physics::{HueState, PhysicsBody, PET_WINDOW};
use state::AppState;
use tauri::{Manager, PhysicalPosition, PhysicalSize, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::pointer_event,
            commands::set_hud_open,
            commands::set_settings_open,
            commands::get_settings,
            commands::update_settings,
            commands::get_identity,
            commands::refresh_identity,
            commands::frontend_ready,
            commands::get_today,
            commands::submit_prompt,
            commands::analyze_file,
            commands::stt_status,
            commands::stt_start,
            commands::stt_stop,
            commands::stt_cancel,
            commands::confirm_answer,
            commands::clear_speaking,
            commands::show_speech_bubble,
            commands::hide_speech_bubble,
            commands::get_live_stats,
            commands::pet_hover,
            commands::get_memory,
            commands::forget_fact_index,
            commands::clear_conversation,
            commands::get_achievements,
            commands::list_documents,
            commands::forget_document,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // Never steal focus while roaming / speaking.
            if let Some(win) = handle.get_webview_window("main") {
                window_util::make_non_activating(&win);
                // Custom HUE icon (Explorer / Alt-Tab), not the Tauri default.
                if let Ok(icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png")) {
                    let _ = win.set_icon(icon);
                }
            }
            if let Some(win) = handle.get_webview_window("bubble") {
                window_util::make_non_activating(&win);
            }

            // Keep the Run key in sync with the saved preference (exe path may change).
            {
                let launch = handle.state::<AppState>().settings.lock().launch_at_startup;
                let _ = startup::set_launch_at_startup(launch);
            }

            tray::setup_tray(&handle).expect("tray icon");

            // Place pet + sync window immediately (avoid NaN desync / spawn squash).
            {
                let area = brain::initial_area();
                let state = handle.state::<AppState>();
                *state.cached_area.lock() = area.clone();
                let movement = state.settings.lock().movement_mode;
                let mut body = state.body.lock();
                body.set_size(PET_WINDOW, PET_WINDOW);
                body.x = area.x + area.width * 0.5 - PET_WINDOW * 0.5;
                body.squash_x = 1.0;
                body.squash_y = 1.0;
                match movement {
                    config::MovementMode::Taskbar => body.start_walking(&area),
                    config::MovementMode::Fullscreen => {
                        // Start mid-screen so we don't instantly "impact" the taskbar.
                        body.y = area.y + area.height * 0.45 - PET_WINDOW * 0.5;
                        body.start_roaming(&area);
                    }
                    config::MovementMode::Static => {
                        body.y = PhysicsBody::taskbar_y(&area, PET_WINDOW);
                        body.vx = 0.0;
                        body.vy = 0.0;
                        body.state = HueState::Idle;
                    }
                }
                let (x, y) = (body.x, body.y);
                drop(body);
                if let Some(win) = handle.get_webview_window("main") {
                    let _ = win.set_size(PhysicalSize::new(PET_WINDOW as u32, PET_WINDOW as u32));
                    let _ = win.set_position(PhysicalPosition::new(x as i32, y as i32));
                    window_util::make_non_activating(&win);
                    window_util::set_above_all(&win);
                }
            }

            brain::spawn_all(&handle);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building HUE");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            ai::llama_embed::shutdown();
            ai::whisper_stt::cancel();
            handle.state::<AppState>().memory.lock().save_if_needed(true);
        }
    });
}
