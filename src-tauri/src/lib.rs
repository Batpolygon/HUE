mod ai;
mod config;
mod physics;
mod startup;
mod tray;
mod watcher;
mod window_util;

use ai::memory::ShortMemory;
use ai::personality::{build_identity, IdentityCard};
use config::{HueSettings, LlmBackend};
use parking_lot::Mutex;
use physics::{HueState, PhysicsBody, PointerPhase, WorkArea, PET_WINDOW};
use serde::Deserialize;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State};
use watcher::os_events::OsWatcher;
use watcher::{HardwareSnapshot, SystemEventKind};

pub struct AppState {
    pub body: Mutex<PhysicsBody>,
    pub settings: Mutex<HueSettings>,
    pub identity: Mutex<Option<IdentityCard>>,
    pub hud_open: Mutex<bool>,
    pub settings_open: Mutex<bool>,
    pub cached_area: Mutex<WorkArea>,
    /// Fully hidden (fullscreen video / game) — not the corner peek.
    pub concealed: Mutex<bool>,
    /// Current speech-bubble window size (tight to text).
    pub bubble_size: Mutex<(f64, f64)>,
    pub memory: Mutex<ShortMemory>,
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn refresh_work_area(_app: &AppHandle) -> WorkArea {
    // Prefer OS work area (excludes taskbar) so HUE walks along the bar.
    if let Some(wa) = watcher::stealth::desktop_work_area() {
        return wa;
    }
    WorkArea {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1040.0,
    }
}

#[derive(Debug, Deserialize)]
pub struct PointerInput {
    pub phase: PointerPhase,
    pub screen_x: f64,
    pub screen_y: f64,
}

#[tauri::command]
fn pointer_event(app: AppHandle, state: State<AppState>, input: PointerInput) {
    if *state.concealed.lock() {
        return;
    }
    let stealth = state.settings.lock().stealth_auto;
    let area = state.cached_area.lock().clone();
    let hud_was_open = *state.hud_open.lock();

    // Absolute cursor from OS — window-relative coords break as soon as the window moves.
    let (sx, sy) =
        watcher::stealth::cursor_screen_pos().unwrap_or((input.screen_x, input.screen_y));

    let (x, y, want_hud, peeking) = {
        let mut body = state.body.lock();
        body.on_pointer(
            input.phase,
            sx,
            sy,
            now_ms(),
            &area,
            hud_was_open,
            stealth,
        );
        (body.x, body.y, body.want_hud, body.peeking)
    };

    if let Some(open) = want_hud {
        *state.hud_open.lock() = open;
        if !open {
            *state.settings_open.lock() = false;
        }
        let _ = app.emit("hue-hud", serde_json::json!({ "open": open }));
    }
    if peeking {
        *state.hud_open.lock() = false;
        *state.settings_open.lock() = false;
        let _ = app.emit("hue-hud", serde_json::json!({ "open": false }));
    }

    if let Some(win) = app.get_webview_window("main") {
        if matches!(input.phase, PointerPhase::Down | PointerPhase::Move) {
            let _ = win.set_size(PhysicalSize::new(PET_WINDOW as u32, PET_WINDOW as u32));
        }
        if matches!(
            input.phase,
            PointerPhase::Down | PointerPhase::Move | PointerPhase::Up
        ) {
            let _ = win.set_position(PhysicalPosition::new(x as i32, y as i32));
        }
    }
}

#[tauri::command]
fn set_hud_open(app: AppHandle, state: State<AppState>, open: bool) {
    *state.hud_open.lock() = open;
    let area = state.cached_area.lock().clone();
    let mut body = state.body.lock();
    if open {
        body.peeking = false;
        body.hud_should_close = false;
        body.state = HueState::Interacting;
        body.facing_user = true;
        body.vx = 0.0;
        body.vy = 0.0;
        body.squash_x = 1.0;
        body.squash_y = 1.0;
        let _ = area;
        if let Some(win) = app.get_webview_window("main") {
            window_util::make_focusable(&win);
        }
    } else if body.state == HueState::Interacting {
        body.state = HueState::Idle;
        body.facing_user = false;
        body.squash_x = 1.0;
        body.squash_y = 1.0;
        if let Some(win) = app.get_webview_window("main") {
            window_util::make_non_activating(&win);
        }
    }
}

#[tauri::command]
fn set_settings_open(app: AppHandle, state: State<AppState>, open: bool) {
    *state.settings_open.lock() = open;
    if open {
        *state.hud_open.lock() = true;
        let area = state.cached_area.lock().clone();
        let mut body = state.body.lock();
        body.peeking = false;
        body.state = HueState::Interacting;
        let (x, y) = PhysicsBody::emerge_position(&area, 400.0, 520.0);
        body.x = x.min(area.x + area.width - 400.0);
        body.y = y.min(area.y + area.height - 520.0).max(area.y);
        if let Some(win) = app.get_webview_window("main") {
            window_util::make_focusable(&win);
        }
    }
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> HueSettings {
    state.settings.lock().clone()
}

#[tauri::command]
fn update_settings(state: State<AppState>, settings: HueSettings) -> Result<HueSettings, String> {
    startup::set_launch_at_startup(settings.launch_at_startup)?;
    settings.save()?;
    // Discrete mode only controls fullscreen hide — does not force peek.
    if !settings.stealth_auto {
        let mut body = state.body.lock();
        if body.peeking {
            body.peeking = false;
            body.state = HueState::Idle;
        }
    }
    *state.settings.lock() = settings.clone();
    Ok(settings)
}

#[tauri::command]
fn get_identity(state: State<AppState>) -> IdentityCard {
    if let Some(id) = state.identity.lock().clone() {
        return id;
    }
    let hw = HardwareSnapshot::collect();
    let id = build_identity(hw);
    *state.identity.lock() = Some(id.clone());
    id
}

#[tauri::command]
fn refresh_identity(state: State<AppState>) -> IdentityCard {
    let hw = HardwareSnapshot::collect();
    let id = build_identity(hw);
    *state.identity.lock() = Some(id.clone());
    id
}

#[tauri::command]
fn stt_status() -> ai::whisper_stt::SttStatus {
    ai::whisper_stt::status()
}

#[tauri::command]
fn clear_speaking(app: AppHandle, state: State<AppState>) {
    state.body.lock().speaking = false;
    if let Some(win) = app.get_webview_window("bubble") {
        let _ = win.hide();
        let _ = win.emit("hue-bubble-clear", ());
    }
}

const BUBBLE_GAP: f64 = 2.0;

/// Compact bubble sized to the text — sits snug above the pet window, centered on it.
fn estimate_bubble_size(text: &str) -> (f64, f64) {
    let chars = text.chars().count().max(1) as f64;
    let cols = 26.0_f64;
    let lines = (chars / cols).ceil().clamp(1.0, 4.0);
    let bw = (chars * 6.2 + 28.0).clamp(120.0, 220.0);
    let bh = (10.0 + lines * 15.0 + 10.0).clamp(34.0, 68.0);
    (bw, bh)
}

fn place_bubble(x: f64, y: f64, win_w: f64, bw: f64, bh: f64) -> (f64, f64) {
    // Center on the pet window; pet head sits near the top → bubble just above.
    let bx = x + win_w * 0.5 - bw * 0.5;
    let by = y - bh - BUBBLE_GAP;
    (bx, by)
}

/// Show dialogue in a separate window above the pet — pet window stays untouched.
#[tauri::command]
fn show_speech_bubble(app: AppHandle, state: State<AppState>, text: String) -> Result<(), String> {
    let (x, y, win_w) = {
        let mut body = state.body.lock();
        body.speaking = true;
        (body.x, body.y, body.width.max(PET_WINDOW))
    };

    let (bw, bh) = estimate_bubble_size(&text);
    *state.bubble_size.lock() = (bw, bh);
    let (bx, by) = place_bubble(x, y, win_w, bw, bh);

    let win = app
        .get_webview_window("bubble")
        .ok_or_else(|| "bubble window missing".to_string())?;
    let _ = win.set_size(PhysicalSize::new(bw as u32, bh as u32));
    let _ = win.set_position(PhysicalPosition::new(bx as i32, by as i32));
    let _ = win.set_always_on_top(true);
    window_util::show_without_focus(&win);
    win.emit("hue-bubble", serde_json::json!({ "text": text }))
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_live_stats() -> watcher::LiveStats {
    watcher::live_stats::collect()
}

#[tauri::command]
fn hide_speech_bubble(app: AppHandle, state: State<AppState>) {
    state.body.lock().speaking = false;
    if let Some(win) = app.get_webview_window("bubble") {
        let _ = win.emit("hue-bubble-clear", ());
        let _ = win.hide();
    }
}

#[tauri::command]
fn pet_hover(state: State<AppState>) {
    state.body.lock().hover_hop();
}

#[tauri::command]
async fn submit_prompt(
    app: AppHandle,
    state: State<'_, AppState>,
    message: String,
) -> Result<(), String> {
    let settings = state.settings.lock().clone();
    let identity = {
        if let Some(id) = state.identity.lock().clone() {
            id
        } else {
            let live = HardwareSnapshot::collect();
            let id = build_identity(live);
            *state.identity.lock() = Some(id.clone());
            id
        }
    };

    {
        let mut body = state.body.lock();
        body.touch_interaction();
    }
    let (memory_addon, history) = {
        let mem = state.memory.lock();
        (mem.prompt_addon(), mem.history_messages())
    };
    let dizzy = state.body.lock().is_dizzy();
    let mut system = identity.system_prompt.clone();
    if !memory_addon.is_empty() {
        system.push_str("\n\n");
        system.push_str(&memory_addon);
    }
    if dizzy {
        system.push_str(
            "\n\nÉTAT ACTUEL : tu es DIZZY (l'utilisateur t'a secoué). \
Tu te sens étourdi, tes idées zigzaguent, tu mélanges un peu les mots, \
tu es affectueux mais confus. Réponds court, un peu lost, avec humour. \
Ne sors pas complètement du personnage.",
        );
    }

    let reply = match settings.llm_backend {
        LlmBackend::Api => {
            ai::api_fallback::stream_chat(app, settings, system, message.clone(), history).await?
        }
        LlmBackend::Local => {
            ai::llama_embed::stream_chat(app, settings, system, message.clone(), history).await?
        }
    };

    state.memory.lock().push_turn(message, reply);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            body: Mutex::new(PhysicsBody::new(0.0, 0.0, PET_WINDOW, PET_WINDOW)),
            settings: Mutex::new(HueSettings::load()),
            identity: Mutex::new(None),
            hud_open: Mutex::new(false),
            settings_open: Mutex::new(false),
            cached_area: Mutex::new(WorkArea {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            }),
            concealed: Mutex::new(false),
            bubble_size: Mutex::new((160.0, 40.0)),
            memory: Mutex::new(ShortMemory::new()),
        })
        .invoke_handler(tauri::generate_handler![
            pointer_event,
            set_hud_open,
            set_settings_open,
            get_settings,
            update_settings,
            get_identity,
            refresh_identity,
            submit_prompt,
            stt_status,
            clear_speaking,
            show_speech_bubble,
            hide_speech_bubble,
            get_live_stats,
            pet_hover,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // Never steal focus while roaming / speaking.
            if let Some(win) = handle.get_webview_window("main") {
                window_util::make_non_activating(&win);
                // Force custom HUE icon (Explorer / Alt-Tab), not the Tauri default.
                if let Ok(icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))
                {
                    let _ = win.set_icon(icon);
                }
            }
            if let Some(win) = handle.get_webview_window("bubble") {
                window_util::make_non_activating(&win);
            }

            // Keep Windows Run key in sync with saved preference (exe path may change).
            {
                let launch = handle.state::<AppState>().settings.lock().launch_at_startup;
                let _ = startup::set_launch_at_startup(launch);
            }

            tray::setup_tray(&handle).expect("tray icon");

            // Place pet + sync window immediately (avoid NaN desync / spawn squash).
            {
                let area = refresh_work_area(&handle);
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

            let app_physics = handle.clone();
            std::thread::spawn(move || {
                let tick = Duration::from_millis(33); // ~30 Hz — avoids freezing the webview
                let mut last = Instant::now();
                let mut last_area_refresh = Instant::now();
                let mut last_x = f64::NAN;
                let mut last_y = f64::NAN;
                let mut last_w = 0u32;
                let mut last_h = 0u32;
                let mut last_peeking: Option<bool> = None;
                let mut emit_counter = 0u8;

                loop {
                    let now = Instant::now();
                    let dt = now.duration_since(last).as_secs_f64().min(0.08);
                    last = now;

                    if last_area_refresh.elapsed() > Duration::from_secs(2) {
                        let area = refresh_work_area(&app_physics);
                        *app_physics.state::<AppState>().cached_area.lock() = area;
                        last_area_refresh = Instant::now();
                    }

                    let state = app_physics.state::<AppState>();
                    if *state.concealed.lock() {
                        std::thread::sleep(tick);
                        continue;
                    }
                    let hud_open = *state.hud_open.lock();
                    let settings_open = *state.settings_open.lock();
                    let movement = state.settings.lock().movement_mode;
                    let area = state.cached_area.lock().clone();
                    let (dragging, peeking) = {
                        let b = state.body.lock();
                        (b.state == HueState::Dragging, b.peeking)
                    };

                    let (w, h) = if settings_open {
                        (400.0, 520.0)
                    } else if hud_open && !dragging && !peeking {
                        // Pet full size + compact reply (~½) + input
                        (300.0, 280.0)
                    } else {
                        (PET_WINDOW, PET_WINDOW)
                    };

                    let cursor = watcher::stealth::cursor_screen_pos();
                    let payload = {
                        let mut body = state.body.lock();
                        body.set_size(w, h);
                        if !dragging {
                            body.step(dt, &area, movement, cursor);
                            if let Some((cx, cy)) = cursor {
                                body.update_cursor_awareness(cx, cy);
                            }
                        }
                        body.tick_payload()
                    };

                    // Keep speech bubble glued above the pet (incl. dizzy wobble while peeked).
                    if payload.speaking {
                        if let Some(bwin) = app_physics.get_webview_window("bubble") {
                            let st = app_physics.state::<AppState>();
                            let (bw, bh) = *st.bubble_size.lock();
                            let win_w = st.body.lock().width.max(PET_WINDOW);
                            let (bx, by) = place_bubble(payload.x, payload.y, win_w, bw, bh);
                            let _ = bwin.set_size(PhysicalSize::new(bw as u32, bh as u32));
                            let _ = bwin
                                .set_position(PhysicalPosition::new(bx as i32, by as i32));
                            let _ = bwin.set_always_on_top(true);
                        }
                    }

                    if let Some(win) = app_physics.get_webview_window("main") {
                        let wu = w as u32;
                        let hu = h as u32;
                        if wu != last_w || hu != last_h {
                            let _ = win.set_size(PhysicalSize::new(wu, hu));
                            last_w = wu;
                            last_h = hu;
                        }
                        // Keep HUE above apps and the desktop. When tucked away, place it
                        // directly below the taskbar in the topmost band.
                        if last_peeking != Some(payload.peeking) {
                            if payload.peeking {
                                window_util::set_above_windows_below_taskbar(&win);
                            } else {
                                window_util::set_above_all(&win);
                            }
                            last_peeking = Some(payload.peeking);
                        }
                        // NaN on first frames must still move the window (else pet walks "invisible").
                        let moved = last_x.is_nan()
                            || last_y.is_nan()
                            || (payload.x - last_x).abs() > 0.5
                            || (payload.y - last_y).abs() > 0.5;
                        if !dragging && moved {
                            let _ = win
                                .set_position(PhysicalPosition::new(payload.x as i32, payload.y as i32));
                            last_x = payload.x;
                            last_y = payload.y;
                        } else if dragging {
                            last_x = payload.x;
                            last_y = payload.y;
                        }
                    }

                    emit_counter = emit_counter.wrapping_add(1);
                    if emit_counter % 1 == 0 {
                        let _ = app_physics.emit("hue-tick", &payload);
                    }

                    std::thread::sleep(tick);
                }
            });

            let app_watch = handle.clone();
            std::thread::spawn(move || {
                let snap = HardwareSnapshot::collect();
                let identity = build_identity(snap.clone());
                {
                    let state = app_watch.state::<AppState>();
                    *state.identity.lock() = Some(identity.clone());
                }
                let _ = app_watch.emit("hue-identity", identity);

                let mut watcher = OsWatcher::new(snap);
                let mut chatter = ai::chatter::Chatter::new();
                let mut removable = {
                    let disks = sysinfo::Disks::new_with_refreshed_list();
                    disks.list().iter().filter(|d| d.is_removable()).count()
                };

                loop {
                    let settings = {
                        let state = app_watch.state::<AppState>();
                        let cloned = state.settings.lock().clone();
                        cloned
                    };
                    let cooldown = settings.talk_frequency.cooldown_secs();

                    let concealed_early = *app_watch.state::<AppState>().concealed.lock();
                    let mut events = watcher.poll(cooldown);
                    events.extend(watcher.poll_usb(&mut removable, cooldown));

                    // Silent while fully hidden (fullscreen). Peek/roam may speak.
                    if !concealed_early {
                        for ev in events {
                            {
                                let state = app_watch.state::<AppState>();
                                let mut body = state.body.lock();
                                if !body.peeking {
                                    match ev.kind {
                                        SystemEventKind::DiskFull
                                        | SystemEventKind::BatteryLow
                                        | SystemEventKind::HotProcess => {
                                            body.set_expression(HueState::Heavy);
                                        }
                                        SystemEventKind::DiskCleaned
                                        | SystemEventKind::AcPlugged
                                        | SystemEventKind::NetworkRestored
                                        | SystemEventKind::BandwidthHeavy
                                        | SystemEventKind::UsbConnect => {
                                            body.set_expression(HueState::Joy);
                                        }
                                        SystemEventKind::NetworkLost
                                        | SystemEventKind::UsbDisconnect => {
                                            body.set_expression(HueState::Panic);
                                        }
                                        SystemEventKind::LongUptime
                                        | SystemEventKind::LateNight
                                        | SystemEventKind::AcUnplugged => {
                                            body.set_expression(HueState::Tired);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            let _ = app_watch.emit("hue-system", ev);
                        }
                    }

                    // Fullscreen video / game / OBS → fully hide. Otherwise peek if discrete.
                    let our_hwnd = app_watch.get_webview_window("main").and_then(|w| {
                        w.hwnd().ok().map(|h| {
                            #[cfg(windows)]
                            {
                                h.0 as isize
                            }
                            #[cfg(not(windows))]
                            {
                                let _ = h;
                                0isize
                            }
                        })
                    });
                    let conceal = watcher::stealth::should_conceal(settings.stealth_auto, our_hwnd);
                    {
                        let state = app_watch.state::<AppState>();
                        let mut concealed = state.concealed.lock();
                        if conceal && !*concealed {
                            *concealed = true;
                            *state.hud_open.lock() = false;
                            *state.settings_open.lock() = false;
                            state.body.lock().speaking = false;
                            if let Some(win) = app_watch.get_webview_window("main") {
                                let _ = win.hide();
                            }
                            if let Some(win) = app_watch.get_webview_window("bubble") {
                                let _ = win.hide();
                            }
                        } else if !conceal && *concealed {
                            *concealed = false;
                            let area = state.cached_area.lock().clone();
                            // Directement ranger en bas à droite — pas de frame "déployé".
                            {
                                let mut body = state.body.lock();
                                body.set_size(PET_WINDOW, PET_WINDOW);
                                body.speaking = false;
                                body.enter_peek(&area);
                            }
                            if let Some(win) = app_watch.get_webview_window("main") {
                                let body = state.body.lock();
                                let _ = win.set_size(PhysicalSize::new(
                                    PET_WINDOW as u32,
                                    PET_WINDOW as u32,
                                ));
                                let _ = win.set_position(PhysicalPosition::new(
                                    body.x as i32,
                                    body.y as i32,
                                ));
                                drop(body);
                                window_util::show_without_focus(&win);
                            }
                        }
                    }

                    // Spontaneous chatter (mute while fully concealed / fullscreen)
                    let stats = watcher::live_stats::collect();
                    let (
                        concealed_now,
                        peeking,
                        interacting,
                        mood,
                        personality,
                        user_name,
                        display_name,
                        dizzy,
                        force_dizzy,
                        force_kind,
                    ) = {
                        let state = app_watch.state::<AppState>();
                        let concealed_now = *state.concealed.lock();
                        let hud = *state.hud_open.lock();
                        let mut body = state.body.lock();
                        let peeking = body.peeking;
                        let mood = if body.is_dizzy() {
                            HueState::Dizzy
                        } else {
                            body.state
                        };
                        let dizzy = body.is_dizzy();
                        let force_dizzy = body.dizzy_just_started;
                        let force_kind = body.want_chatter_kind.take();
                        let interacting = body.state == HueState::Interacting || hud;
                        // Keep the flag if we cannot speak yet (HUD / concealed).
                        drop(body);
                        let (personality, user_name, display_name) =
                            match state.identity.lock().as_ref() {
                                Some(id) => (
                                    id.personality.clone(),
                                    id.user_name.clone(),
                                    id.display_name.clone(),
                                ),
                                None => {
                                    let hw = HardwareSnapshot::collect();
                                    let id = build_identity(hw);
                                    let p = id.personality.clone();
                                    let u = id.user_name.clone();
                                    let d = id.display_name.clone();
                                    *state.identity.lock() = Some(id);
                                    (p, u, d)
                                }
                            };
                        (
                            concealed_now,
                            peeking,
                            interacting,
                            mood,
                            personality,
                            user_name,
                            display_name,
                            dizzy,
                            force_dizzy,
                            force_kind,
                        )
                    };
                    let force_line = force_kind.map(|kind| {
                        let mut rng = rand::thread_rng();
                        ai::chatter::pet_line(
                            kind,
                            &user_name,
                            personality.primary_bucket(),
                            &mut rng,
                        )
                    });
                    if let Some(line) = chatter.maybe_line(
                        settings.talk_frequency,
                        peeking,
                        concealed_now,
                        interacting,
                        &personality,
                        &user_name,
                        &display_name,
                        mood,
                        dizzy,
                        force_dizzy,
                        Some(&stats),
                        force_line,
                    ) {
                        {
                            let state = app_watch.state::<AppState>();
                            let mut body = state.body.lock();
                            body.speaking = true;
                            if force_dizzy {
                                body.dizzy_just_started = false;
                            }
                            drop(body);
                            if !chatter.last_title().is_empty() {
                                state
                                    .memory
                                    .lock()
                                    .set_last_app(chatter.last_title().to_string());
                            }
                        }
                        let _ = app_watch.emit(
                            "hue-chatter",
                            serde_json::json!({
                                "message": line,
                                "peeking": peeking,
                            }),
                        );
                    } else if !chatter.last_title().is_empty() {
                        app_watch
                            .state::<AppState>()
                            .memory
                            .lock()
                            .set_last_app(chatter.last_title().to_string());
                    }

                    std::thread::sleep(Duration::from_millis(800));
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running HUE");
}
