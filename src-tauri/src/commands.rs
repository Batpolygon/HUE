use crate::ai::achievements::{self, TitleKind};
use crate::ai::documents::{self, Document};
use crate::ai::llm::{self, NoTarget};
use crate::ai::personality::{build_identity, compose_title, IdentityCard};
use crate::config::HueSettings;
use crate::physics::{HueState, PhysicsBody, PointerPhase, WorkArea, PET_WINDOW};
use crate::state::{AppState, SETTINGS_SIZE};
use crate::watcher::HardwareSnapshot;
use crate::{watcher, window_util};
use chrono::Timelike;
use serde::Deserialize;
use serde_json::json;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State};

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

#[derive(Debug, Deserialize)]
pub struct PointerInput {
    pub phase: PointerPhase,
    pub screen_x: f64,
    pub screen_y: f64,
}

#[tauri::command]
pub fn pointer_event(app: AppHandle, state: State<AppState>, input: PointerInput) {
    if *state.concealed.lock() {
        return;
    }
    // On release, HUE belongs to the screen he was dropped on (not the cached one from before
    // the drag, nor wherever the flick sent the cursor).
    let area = if matches!(input.phase, PointerPhase::Up) {
        let (cx, cy) = {
            let b = state.body.lock();
            (b.x + b.width.max(PET_WINDOW) * 0.5, b.y + b.height.max(PET_WINDOW) * 0.5)
        };
        let a = crate::brain::area_at(cx, cy);
        *state.cached_area.lock() = a.clone();
        a
    } else {
        state.cached_area.lock().clone()
    };
    let hud_was_open = *state.hud_open.lock();

    // Absolute cursor from OS — window-relative coords break as soon as the window moves.
    let (sx, sy) =
        watcher::stealth::cursor_screen_pos().unwrap_or((input.screen_x, input.screen_y));

    let (x, y, want_hud, peeking) = {
        let mut body = state.body.lock();
        body.on_pointer(input.phase, sx, sy, now_ms(), &area, hud_was_open, false);
        (body.x, body.y, body.want_hud, body.peeking)
    };

    if let Some(open) = want_hud {
        *state.hud_open.lock() = open;
        if !open {
            *state.settings_open.lock() = false;
        }
        let _ = app.emit("hue-hud", json!({ "open": open }));
    }
    if peeking {
        *state.hud_open.lock() = false;
        *state.settings_open.lock() = false;
        let _ = app.emit("hue-hud", json!({ "open": false }));
    }

    if let Some(win) = app.get_webview_window("main") {
        if matches!(input.phase, PointerPhase::Down | PointerPhase::Move) {
            let _ = win.set_size(PhysicalSize::new(PET_WINDOW as u32, PET_WINDOW as u32));
        }
        let _ = win.set_position(PhysicalPosition::new(x as i32, y as i32));
    }
}

#[tauri::command]
pub fn set_hud_open(app: AppHandle, state: State<AppState>, open: bool) {
    *state.hud_open.lock() = open;
    let mut body = state.body.lock();
    if open {
        body.wake_up();
        body.peeking = false;
        body.hud_should_close = false;
        body.state = HueState::Interacting;
        body.facing_user = true;
        body.vx = 0.0;
        body.vy = 0.0;
        body.squash_x = 1.0;
        body.squash_y = 1.0;
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
pub fn set_settings_open(app: AppHandle, state: State<AppState>, open: bool) {
    *state.settings_open.lock() = open;
    if open {
        *state.hud_open.lock() = true;
        let area = state.cached_area.lock().clone();
        let mut body = state.body.lock();
        body.peeking = false;
        body.state = HueState::Interacting;
        let (w, h) = SETTINGS_SIZE;
        let (x, y) = PhysicsBody::emerge_position(&area, w, h);
        body.x = x.min(area.x + area.width - w);
        body.y = y.min(area.y + area.height - h).max(area.y);
        if let Some(win) = app.get_webview_window("main") {
            window_util::make_focusable(&win);
        }
    }
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> HueSettings {
    state.settings.lock().clone()
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<AppState>,
    mut settings: HueSettings,
) -> Result<HueSettings, String> {
    {
        let mem = state.memory.lock();
        if !achievements::title_allowed(&mem, TitleKind::Prefix, &settings.title_prefix) {
            settings.title_prefix.clear();
        }
        if !achievements::title_allowed(&mem, TitleKind::Suffix, &settings.title_suffix) {
            settings.title_suffix.clear();
        }
    }
    if !settings.user_birthday.trim().is_empty() {
        let norm = crate::config::settings::normalize_birthday(&settings.user_birthday);
        if norm.is_empty() {
            return Err("Date d'anniversaire invalide (format JJ/MM ou JJ/MM/AAAA).".into());
        }
        settings.user_birthday = norm;
    }
    let launch_changed = state.settings.lock().launch_at_startup != settings.launch_at_startup;
    if launch_changed {
        crate::startup::set_launch_at_startup(settings.launch_at_startup)?;
    }
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
    refresh_title(&app);
    crate::brain::refresh_today(&app);
    Ok(settings)
}

/// Recompute the titled name and push the identity to the UI.
pub fn refresh_title(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (p, s) = {
        let st = state.settings.lock();
        (st.title_prefix.clone(), st.title_suffix.clone())
    };
    let id = {
        let mut guard = state.identity.lock();
        let Some(id) = guard.as_mut() else { return };
        id.full_title = compose_title(&p, &id.display_name, &s);
        id.system_prompt = id
            .system_prompt
            .lines()
            .map(|l| {
                if l.contains("Ton titre complet") {
                    format!(
                        "« {} » (hostname : {}). Ton titre complet : « {} ».",
                        id.display_name, id.hostname, id.full_title
                    )
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        id.clone()
    };
    let _ = app.emit("hue-identity", id);
}

fn ensure_identity(state: &AppState) -> IdentityCard {
    if let Some(id) = state.identity.lock().clone() {
        return id;
    }
    let (p, s) = {
        let st = state.settings.lock();
        (st.title_prefix.clone(), st.title_suffix.clone())
    };
    let id = build_identity(HardwareSnapshot::collect(), &p, &s);
    *state.identity.lock() = Some(id.clone());
    id
}

#[tauri::command]
pub async fn get_identity(app: AppHandle) -> Result<IdentityCard, String> {
    tauri::async_runtime::spawn_blocking(move || ensure_identity(&app.state::<AppState>()))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn refresh_identity(app: AppHandle) -> Result<IdentityCard, String> {
    let a = app.clone();
    let id = tauri::async_runtime::spawn_blocking(move || {
        let state = a.state::<AppState>();
        *state.identity.lock() = None;
        ensure_identity(&state)
    })
    .await
    .map_err(|e| e.to_string())?;
    let _ = app.emit("hue-identity", id.clone());
    Ok(id)
}

#[tauri::command]
pub fn frontend_ready(state: State<AppState>) {
    state.frontend_ready.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn get_today(state: State<AppState>) -> serde_json::Value {
    crate::brain::today_payload(&state)
}

#[tauri::command]
pub fn stt_status(state: State<AppState>) -> crate::ai::whisper_stt::SttStatus {
    crate::ai::whisper_stt::status(&state.settings.lock())
}

#[tauri::command]
pub fn stt_start(app: AppHandle) -> Result<(), String> {
    crate::ai::whisper_stt::start(app)
}

#[tauri::command]
pub fn stt_cancel() {
    crate::ai::whisper_stt::cancel();
}

#[tauri::command]
pub async fn stt_stop(app: AppHandle) -> Result<String, String> {
    let settings = app.state::<AppState>().settings.lock().clone();
    let text = crate::ai::whisper_stt::stop_and_transcribe(&app, &settings).await?;
    let state = app.state::<AppState>();
    let mut m = state.memory.lock();
    m.stats.voice_uses += 1;
    m.mark();
    Ok(text)
}

#[tauri::command]
pub fn confirm_answer(state: State<AppState>, id: u64, yes: bool) {
    if let Some(tx) = state.pending_confirms.lock().remove(&id) {
        let _ = tx.send(yes);
    }
}

#[tauri::command]
pub fn clear_speaking(app: AppHandle, state: State<AppState>) {
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
    let cols = 34.0_f64;
    let lines = (chars / cols).ceil().clamp(1.0, 5.0);
    let bw = (chars * 6.2 + 28.0).clamp(120.0, 250.0);
    let bh = (12.0 + lines * 15.0 + 8.0).clamp(34.0, 96.0);
    (bw, bh)
}

const BUBBLE_EDGE: f64 = 6.0;

/// Centered above the pet, but kept inside the pet's own screen (tucked in the bottom-right
/// corner, a centered bubble would spill onto the next monitor).
pub fn place_bubble(x: f64, y: f64, win_w: f64, bw: f64, bh: f64, area: &WorkArea) -> (f64, f64) {
    let min_x = area.x + BUBBLE_EDGE;
    let max_x = (area.x + area.width - bw - BUBBLE_EDGE).max(min_x);
    let bx = (x + win_w * 0.5 - bw * 0.5).clamp(min_x, max_x);
    let min_y = area.y;
    let max_y = (area.y + area.height - bh).max(min_y);
    let by = (y - bh - BUBBLE_GAP).clamp(min_y, max_y);
    (bx, by)
}

/// Show dialogue in a separate window above the pet — pet window stays untouched.
#[tauri::command]
pub fn show_speech_bubble(
    app: AppHandle,
    state: State<AppState>,
    text: String,
    kind: Option<String>,
) -> Result<(), String> {
    let (x, y, win_w) = {
        let mut body = state.body.lock();
        body.speaking = true;
        (body.x, body.y, body.width.max(PET_WINDOW))
    };
    let (bw, bh) = estimate_bubble_size(&text);
    *state.bubble_size.lock() = (bw, bh);
    let area = state.cached_area.lock().clone();
    let (bx, by) = place_bubble(x, y, win_w, bw, bh, &area);
    let win = app
        .get_webview_window("bubble")
        .ok_or_else(|| "bubble window missing".to_string())?;
    let _ = win.set_size(PhysicalSize::new(bw as u32, bh as u32));
    let _ = win.set_position(PhysicalPosition::new(bx as i32, by as i32));
    let _ = win.set_always_on_top(true);
    window_util::show_without_focus(&win);
    win.emit("hue-bubble", json!({ "text": text, "kind": kind.unwrap_or_default() }))
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn hide_speech_bubble(app: AppHandle, state: State<AppState>) {
    state.body.lock().speaking = false;
    if let Some(win) = app.get_webview_window("bubble") {
        let _ = win.emit("hue-bubble-clear", ());
        let _ = win.hide();
    }
}

#[tauri::command]
pub fn get_live_stats() -> watcher::LiveStats {
    watcher::live_stats::collect()
}

#[tauri::command]
pub fn pet_hover(state: State<AppState>) {
    state.body.lock().hover_hop();
}

#[tauri::command]
pub fn get_memory(state: State<AppState>) -> serde_json::Value {
    let m = state.memory.lock();
    json!({
        "facts": m.facts,
        "stats": m.stats,
        "first_met": m.first_met,
        "turns": m.turns.len(),
    })
}

#[tauri::command]
pub fn forget_fact_index(state: State<AppState>, index: usize) {
    let mut m = state.memory.lock();
    m.forget_index(index);
    m.save_if_needed(true);
}

#[tauri::command]
pub fn clear_conversation(state: State<AppState>) {
    let mut m = state.memory.lock();
    m.clear_conversation();
    m.save_if_needed(true);
}

#[tauri::command]
pub fn get_achievements(state: State<AppState>) -> Vec<achievements::AchievementView> {
    achievements::views(&state.memory.lock())
}

#[tauri::command]
pub fn list_documents(state: State<AppState>) -> Vec<documents::DocSummary> {
    state.docs.lock().list()
}

#[tauri::command]
pub fn forget_document(state: State<AppState>, name: String) -> bool {
    state.docs.lock().remove(&name)
}

/// Live context appended to the persona for each reply.
fn live_context(state: &AppState, settings: &HueSettings, local: bool) -> String {
    let s = watcher::live_stats::collect();
    let mood = *state.mood.lock();
    let now = chrono::Local::now();
    let events: Vec<String> = state.today_events.lock().iter().map(|e| e.name.clone()).collect();
    let mut out = format!(
        "\n\nMAINTENANT : {} {}. Humeur : {}. CPU {:.0} %, RAM {:.0} %, disque {:.0} %, GPU {:.0} %{}{}.",
        now.format("%A %d/%m/%Y"),
        now.format("%H:%M"),
        mood.label(),
        s.cpu_pct,
        s.ram_pct,
        s.disk_pct,
        s.gpu_pct,
        s.cpu_temp.map(|t| format!(", CPU {t:.0} °C")).unwrap_or_default(),
        s.battery_pct
            .map(|b| format!(", batterie {b:.0} %{}", if s.on_ac == Some(true) { " (secteur)" } else { "" }))
            .unwrap_or_default(),
    );
    if !events.is_empty() {
        out.push_str(&format!(" Aujourd'hui : {}.", events.join(", ")));
    }
    if let Some((m, d)) = settings.user_birthday_md() {
        out.push_str(&format!(" Anniversaire de l'utilisateur : {d:02}/{m:02}."));
    }
    let mem = state.memory.lock();
    let addon = mem.prompt_addon(local || settings.share_window_title);
    if !addon.is_empty() {
        out.push_str("\n\n");
        out.push_str(&addon);
    }
    out
}

struct BusyGuard<'a>(&'a AppState);
impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.chat_busy.store(false, Ordering::SeqCst);
    }
}

#[tauri::command]
pub async fn submit_prompt(app: AppHandle, message: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.chat_busy.store(true, Ordering::SeqCst);
    let _busy = BusyGuard(&state);
    let settings = state.settings.lock().clone();
    let a2 = app.clone();
    let identity = tauri::async_runtime::spawn_blocking(move || ensure_identity(&a2.state::<AppState>()))
        .await
        .map_err(|e| e.to_string())?;
    {
        let mut body = state.body.lock();
        body.touch_interaction();
        body.wake_up();
    }

    let target = match llm::resolve_target(&app, &settings).await? {
        Ok(t) => t,
        Err(NoTarget::Message(m)) => {
            llm::stream_fake(&app, &m).await;
            return Ok(());
        }
    };

    let dizzy = state.body.lock().is_dizzy();
    let mut system = identity.system_prompt.clone();
    system.push_str(&live_context(&state, &settings, target.local));
    let rag = state
        .docs
        .lock()
        .rag_context(&message, if target.local { 2500 } else { 6000 });
    if !rag.is_empty() {
        system.push_str("\n\n");
        system.push_str(&rag);
    }
    if dizzy {
        system.push_str(
            "\n\nÉTAT ACTUEL : tu es étourdi (l'utilisateur t'a secoué). Tes idées zigzaguent, \
tu mélanges un peu les mots, tu es affectueux mais confus. Réponds court, avec humour.",
        );
    }
    let history = state.memory.lock().history_messages();
    let mut messages = vec![json!({ "role": "system", "content": system })];
    messages.extend(history);
    messages.push(json!({ "role": "user", "content": message }));

    let reply = llm::chat_stream(&app, &target, messages, true).await?;

    let hour = chrono::Local::now().hour();
    let mut mem = state.memory.lock();
    mem.push_turn(message, reply);
    mem.stats.chats += 1;
    if (2..5).contains(&hour) {
        mem.stats.night_chats += 1;
    }
    mem.save_if_needed(false);
    Ok(())
}

/// A file was dropped on the pet: read it, index it, and have the LLM summarise + react.
#[tauri::command]
pub async fn analyze_file(app: AppHandle, path: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.chat_busy.store(true, Ordering::SeqCst);
    let _busy = BusyGuard(&state);
    let p = std::path::PathBuf::from(&path);
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    {
        let mut body = state.body.lock();
        body.wake_up();
        body.set_face("wide", 2000);
        body.touch_interaction();
    }
    {
        let id = ensure_identity(&state);
        let mood = *state.mood.lock();
        let now = chrono::Local::now();
        let c = crate::ai::lines::LineCtx {
            user: &id.user_name,
            me: &id.display_name,
            bucket: id.personality.primary_bucket(),
            hour: now.hour(),
            weekday: chrono::Datelike::weekday(&now),
            season: crate::ai::calendar::season(now.date_naive()),
            valence: mood.valence,
            energy: mood.energy,
            peeking: false,
        };
        let line = crate::ai::lines::file_received(&c, &name, &mut rand::thread_rng());
        let _ = app.emit("hue-chat", json!({ "tool": line }));
    }

    let p2 = p.clone();
    let extracted = tauri::async_runtime::spawn_blocking(move || documents::extract(&p2))
        .await
        .map_err(|e| e.to_string())??;
    let chunks = documents::chunk(&extracted.text);
    let size_bytes = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
    if !chunks.is_empty() {
        state.docs.lock().add(Document {
            name: name.clone(),
            path: path.clone(),
            kind: extracted.kind.clone(),
            size_bytes,
            added: crate::util::now_unix(),
            chunks: chunks.clone(),
            summary: String::new(),
        });
    }

    let settings = state.settings.lock().clone();
    let target = match llm::resolve_target(&app, &settings).await? {
        Ok(t) => t,
        Err(NoTarget::Message(m)) => {
            let msg = if chunks.is_empty() {
                format!("J'ai regardé « {name} » ({}). {m}", extracted.meta)
            } else {
                format!(
                    "J'ai lu et rangé « {name} » ({} morceaux) — je pourrai le résumer et répondre dessus dès que mon LLM sera configuré. {m}",
                    chunks.len()
                )
            };
            llm::stream_fake(&app, &msg).await;
            bump_files(&state);
            return Ok(());
        }
    };

    let identity = ensure_identity(&state);
    let budget = if target.local { 5000 } else { 14000 };
    let user_msg = if chunks.is_empty() {
        format!(
            "{} vient de te glisser un fichier dont tu ne peux pas lire le contenu.\nNom : « {name} »\nInfos : {}\n\nDis en 2-3 phrases ce que tu en déduis (type, usage probable) et ce que tu en penses, avec ta personnalité.",
            identity.user_name, extracted.meta
        )
    } else {
        let ctx = documents::representative_context(&chunks, budget);
        let partial = if ctx.chars().count() < extracted.text.chars().count() {
            " (extraits représentatifs : le document est long)"
        } else {
            ""
        };
        format!(
            "{} vient de te glisser le fichier « {name} » ({}).\nContenu{partial} :\n<<<\n{ctx}\n>>>\n\n\
Réponds en français :\n1) Un résumé clair en 3 à 5 puces courtes.\n2) Ce que TU en penses, avec ta personnalité (1-2 phrases).\nMaximum 150 mots.",
            identity.user_name, extracted.meta
        )
    };
    let messages = vec![
        json!({ "role": "system", "content": identity.system_prompt }),
        json!({ "role": "user", "content": user_msg }),
    ];
    let reply = llm::chat_stream(&app, &target, messages, false).await?;
    if !chunks.is_empty() {
        state.docs.lock().set_summary(&path, &reply);
    }
    state
        .memory
        .lock()
        .push_turn(format!("[fichier glissé : {name}]"), reply);
    bump_files(&state);
    Ok(())
}

fn bump_files(state: &AppState) {
    let mut m = state.memory.lock();
    m.stats.files_read += 1;
    m.save_if_needed(false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bubble_stays_on_the_pets_screen() {
        // Left screen 0..1920, taskbar 48 px; pet tucked in its bottom-right corner.
        let area = WorkArea { x: 0.0, y: 0.0, width: 1920.0, height: 1032.0 };
        let (px, py) = PhysicsBody::peek_position(&area, PET_WINDOW, PET_WINDOW);
        let (bw, bh) = (250.0, 60.0);
        let (bx, by) = place_bubble(px, py, PET_WINDOW, bw, bh, &area);
        assert!(bx + bw <= area.x + area.width, "bubble spills right: {bx}+{bw}");
        assert!(bx >= area.x);
        assert!(by >= area.y && by + bh <= area.y + area.height);

        // Same on a right-hand screen (negative-free offsets).
        let right = WorkArea { x: 1920.0, ..area };
        let (bx, _) = place_bubble(right.x - 40.0, 500.0, PET_WINDOW, bw, bh, &right);
        assert!(bx >= right.x, "bubble spills left onto the other screen: {bx}");

        // Mid-screen: still centered on the pet.
        let (bx, _) = place_bubble(800.0, 500.0, PET_WINDOW, bw, bh, &area);
        assert_eq!(bx, 800.0 + PET_WINDOW * 0.5 - bw * 0.5);
    }
}
