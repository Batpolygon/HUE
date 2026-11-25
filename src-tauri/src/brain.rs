//! Background threads:
//! - physics (~30 Hz): moves the window, tracks hover-petting;
//! - heartbeat (4 Hz): notices system stalls (lag) and suspend/resume;
//! - brain (~1 Hz): events, mood, sleep, greetings, birthdays, achievements, chatter.

use crate::ai::achievements;
use crate::ai::calendar::{self, CalendarCtx};
use crate::ai::chatter::{Chatter, ChatterInput};
use crate::ai::lines::{self, LineCtx};
use crate::ai::llm;
use crate::ai::mood::MoodInputs;
use crate::ai::personality::IdentityCard;
use crate::commands::place_bubble;
use crate::config::{settings::parse_birthday, TalkFrequency};
use crate::physics::{BodySignal, HueState, WorkArea, PET_WINDOW};
use crate::state::{AppState, HUD_SIZE, SETTINGS_SIZE};
use crate::watcher::{self, os_events::OsWatcher, HardwareSnapshot, LiveStats, SystemEventKind};
use crate::window_util;
use chrono::{Datelike, NaiveDate, Timelike};
use rand::Rng;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};

pub fn spawn_all(app: &AppHandle) {
    spawn_physics(app.clone());
    spawn_heartbeat(app.clone());
    let a = app.clone();
    std::thread::spawn(move || Brain::new(a).run());
}

fn refresh_work_area() -> WorkArea {
    // OS work area (excludes taskbar) so HUE walks along the bar.
    watcher::stealth::desktop_work_area().unwrap_or(WorkArea {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1040.0,
    })
}

pub fn initial_area() -> WorkArea {
    refresh_work_area()
}

/// Work area of the monitor HUE's centre is on. Following the *pet* (not the cursor) keeps him
/// on his own screen: a hard throw that flings the cursor onto the other monitor used to make
/// him jump there for a few seconds, then back.
pub fn area_at(x: f64, y: f64) -> WorkArea {
    watcher::stealth::work_area_at(x, y).unwrap_or_else(refresh_work_area)
}

fn our_hwnd(app: &AppHandle) -> Option<isize> {
    app.get_webview_window("main").and_then(|w| {
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
    })
}

// ───────────────────────────── calendar ─────────────────────────────

pub fn today_payload(state: &AppState) -> Value {
    let events = state.today_events.lock().clone();
    let (decor_on, particles_on) = {
        let s = state.settings.lock();
        (s.seasonal_decor, s.particles)
    };
    json!({
        "events": events.iter().map(|e| json!({ "id": e.id, "name": e.name })).collect::<Vec<_>>(),
        "decor": if decor_on { calendar::decor_of(&events) } else { None },
        "particles": if decor_on && particles_on { calendar::particles_of(&events) } else { None },
    })
}

/// Recompute today's events (birthdays, seasons…) and push them to the UI.
pub fn refresh_today(app: &AppHandle) {
    let state = app.state::<AppState>();
    let today = chrono::Local::now().date_naive();
    let (user, me, pc_birth) = match state.identity.lock().as_ref() {
        Some(id) => (
            id.user_name.clone(),
            id.display_name.clone(),
            id.pc_birthday
                .as_deref()
                .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
        ),
        None => (String::from("toi"), String::from("HUE"), None),
    };
    let bday = parse_birthday(&state.settings.lock().user_birthday);
    let first_met = state
        .memory
        .lock()
        .first_met
        .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
        .map(|d| d.with_timezone(&chrono::Local).date_naive());
    let ctx = CalendarCtx {
        user: &user,
        me: &me,
        user_birthday: bday,
        pc_birth,
        first_met,
    };
    let events = calendar::events_for(today, &ctx);
    {
        let mut mem = state.memory.lock();
        for e in &events {
            if !mem.stats.seasonal.iter().any(|s| s == e.id) {
                mem.stats.seasonal.push(e.id.to_string());
                mem.mark();
            }
        }
    }
    *state.today_events.lock() = events;
    let _ = app.emit("hue-today", today_payload(&state));
}

// ───────────────────────────── physics ─────────────────────────────

fn spawn_physics(app: AppHandle) {
    std::thread::spawn(move || {
        // Body + window move at ~60 Hz (smooth throws); the webview only hears every other
        // frame (~30 Hz) so it never gets flooded.
        let tick = Duration::from_millis(16);
        let mut frame: u64 = 0;
        let mut last = Instant::now();
        let mut last_area_refresh = Instant::now();
        let mut last_x = f64::NAN;
        let mut last_y = f64::NAN;
        let mut last_w = 0u32;
        let mut last_h = 0u32;
        let mut last_peeking: Option<bool> = None;

        loop {
            let now = Instant::now();
            let dt = now.duration_since(last).as_secs_f64().min(0.08);
            last = now;

            frame = frame.wrapping_add(1);
            let emit_frame = frame % 2 == 0;
            let state = app.state::<AppState>();
            if last_area_refresh.elapsed() > Duration::from_millis(500) {
                let (cx, cy) = {
                    let b = state.body.lock();
                    (b.x + b.width * 0.5, b.y + b.height * 0.5)
                };
                *state.cached_area.lock() = area_at(cx, cy);
                last_area_refresh = Instant::now();
            }
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
                SETTINGS_SIZE
            } else if hud_open && !dragging && !peeking {
                HUD_SIZE
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
                        body.update_cursor_awareness(cx, cy, dt);
                        // Petting detection is tuned for ~30 Hz samples.
                        if !settings_open && emit_frame {
                            body.track_hover(cx, cy);
                        }
                    }
                }
                body.tick_payload()
            };

            // Keep the speech bubble glued above the pet.
            if payload.speaking {
                if let Some(bwin) = app.get_webview_window("bubble") {
                    let (bw, bh) = *state.bubble_size.lock();
                    let win_w = state.body.lock().width.max(PET_WINDOW);
                    let area = state.cached_area.lock().clone();
                    let (bx, by) = place_bubble(payload.x, payload.y, win_w, bw, bh, &area);
                    let _ = bwin.set_position(PhysicalPosition::new(bx as i32, by as i32));
                }
            }

            if let Some(win) = app.get_webview_window("main") {
                let (wu, hu) = (w as u32, h as u32);
                if wu != last_w || hu != last_h {
                    let _ = win.set_size(PhysicalSize::new(wu, hu));
                    last_w = wu;
                    last_h = hu;
                }
                // Above apps; when tucked away, directly below the taskbar.
                if last_peeking != Some(payload.peeking) {
                    if payload.peeking {
                        window_util::set_above_windows_below_taskbar(&win);
                    } else {
                        window_util::set_above_all(&win);
                    }
                    last_peeking = Some(payload.peeking);
                } else if payload.peeking && frame % 15 == 0 {
                    // Re-assert every ~250 ms: something else in the topmost band (Explorer, the
                    // bubble, other overlays) can lift him back over the taskbar.
                    window_util::keep_below_taskbar(&win);
                }
                let moved = last_x.is_nan()
                    || last_y.is_nan()
                    || (payload.x - last_x).abs() > 0.5
                    || (payload.y - last_y).abs() > 0.5;
                if !dragging && moved {
                    let _ = win.set_position(PhysicalPosition::new(payload.x as i32, payload.y as i32));
                }
                last_x = payload.x;
                last_y = payload.y;
            }

            if emit_frame {
                let _ = app.emit("hue-tick", &payload);
            }
            std::thread::sleep(tick);
        }
    });
}

// ───────────────────────────── heartbeat ─────────────────────────────

/// A 250 ms sleep that takes seconds means the whole machine stalled.
/// A huge jump means the PC was suspended.
fn spawn_heartbeat(app: AppHandle) {
    std::thread::spawn(move || {
        let start = Instant::now();
        let mut last_mono = Instant::now();
        let mut last_wall = SystemTime::now();
        let mut last_resume: Option<Instant> = None;
        loop {
            std::thread::sleep(Duration::from_millis(250));
            let mono = Instant::now();
            let wall = SystemTime::now();
            let mono_gap = mono.duration_since(last_mono).as_secs_f64();
            let wall_gap = wall
                .duration_since(last_wall)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            last_mono = mono;
            last_wall = wall;
            if start.elapsed() < Duration::from_secs(20) {
                continue;
            }
            let state = app.state::<AppState>();
            if mono_gap > 60.0 || wall_gap > 120.0 {
                last_resume = Some(mono);
                state
                    .resume_events
                    .lock()
                    .push((mono_gap.max(wall_gap) / 60.0).round() as u64);
            } else if mono_gap > 2.5 {
                // Ignore the shaky seconds right after a wake-up.
                let after_resume = last_resume
                    .map(|t| t.elapsed() < Duration::from_secs(20))
                    .unwrap_or(false);
                if !after_resume {
                    state.lag_events.lock().push(mono_gap);
                }
            }
        }
    });
}

// ───────────────────────────── brain ─────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Prio {
    /// Spontaneous / ambient: dropped when HUE can't speak.
    Low,
    /// Reactions to the user (pets, timers…): spoken even in mute mode.
    Direct,
    /// Greeting, birthdays, achievements: held until HUE is visible.
    Keep,
}

struct Brain {
    app: AppHandle,
    watcher: OsWatcher,
    chatter: Chatter,
    started: Instant,
    last_tick: Instant,
    launch: (bool, Option<i64>),
    greeted: bool,
    ready_at: Option<Instant>,
    day: NaiveDate,
    held: Vec<(String, &'static str)>,
    last_vitals: Instant,
    last_achievements: Instant,
    last_minute: Instant,
    last_pet_line: Instant,
    last_lag_line: Option<Instant>,
    recent_lag: Option<Instant>,
    hung: Option<(String, Instant, bool)>,
    sleep_started: Option<Instant>,
    was_sleeping: bool,
    next_thought: Instant,
    network_up: bool,
    last_net_check: Instant,
    identity_at: Instant,
}

impl Brain {
    fn new(app: AppHandle) -> Self {
        Self {
            app,
            watcher: OsWatcher::new(),
            chatter: Chatter::new(),
            started: Instant::now(),
            last_tick: Instant::now(),
            launch: (false, None),
            greeted: false,
            ready_at: None,
            day: chrono::Local::now().date_naive(),
            held: Vec::new(),
            last_vitals: Instant::now() - Duration::from_secs(10),
            last_achievements: Instant::now(),
            last_minute: Instant::now(),
            last_pet_line: Instant::now() - Duration::from_secs(60),
            last_lag_line: None,
            recent_lag: None,
            hung: None,
            sleep_started: None,
            was_sleeping: false,
            next_thought: Instant::now() + Duration::from_secs(12 * 60),
            network_up: true,
            last_net_check: Instant::now() - Duration::from_secs(60),
            identity_at: Instant::now(),
        }
    }

    fn state(&self) -> tauri::State<'_, AppState> {
        self.app.state::<AppState>()
    }

    fn identity(&mut self) -> IdentityCard {
        if let Some(id) = self.state().identity.lock().clone() {
            return id;
        }
        let (p, s) = {
            let st = self.state();
            let g = st.settings.lock();
            (g.title_prefix.clone(), g.title_suffix.clone())
        };
        let id = crate::ai::personality::build_identity(HardwareSnapshot::collect(), &p, &s);
        *self.state().identity.lock() = Some(id.clone());
        self.identity_at = Instant::now();
        let _ = self.app.emit("hue-identity", id.clone());
        id
    }

    fn run(mut self) {
        // Identity first: the shell / voice / personality depend on it.
        let id = self.identity();
        {
            let app = self.app.clone();
            let st = app.state::<AppState>();
            *st.mood.lock() = crate::ai::mood::Mood::new(&id.personality.axes);
            self.launch = st.memory.lock().on_launch();
        }
        refresh_today(&self.app);
        loop {
            self.tick();
            std::thread::sleep(Duration::from_millis(1000));
        }
    }

    fn line_ctx<'a>(&self, id: &'a IdentityCard, season: &'a str, mood: (f64, f64), peeking: bool) -> LineCtx<'a> {
        let now = chrono::Local::now();
        LineCtx {
            user: &id.user_name,
            me: &id.display_name,
            bucket: id.personality.primary_bucket(),
            hour: now.hour(),
            weekday: now.weekday(),
            season,
            valence: mood.0,
            energy: mood.1,
            peeking,
        }
    }

    fn fx(&self, kind: &str) {
        if self.state().settings.lock().particles {
            let _ = self.app.emit("hue-fx", json!({ "kind": kind }));
        }
    }

    fn face(&self, face: &'static str, ms: u64) {
        self.state().body.lock().set_face(face, ms);
    }

    /// Send a line to the UI (the frontend queues it if a bubble is already up).
    fn speak(&mut self, line: String, kind: &'static str, prio: Prio) {
        if line.trim().is_empty() {
            return;
        }
        let (concealed, ready, mute) = {
            let st = self.state();
            let c = *st.concealed.lock();
            let r = st.frontend_ready.load(Ordering::SeqCst);
            let m = st.settings.lock().talk_frequency == TalkFrequency::Mute;
            (c, r, m)
        };
        if concealed || !ready {
            if prio == Prio::Keep {
                self.held.push((line, kind));
            }
            return;
        }
        if mute && prio == Prio::Low {
            return;
        }
        self.chatter.note_spoke();
        let peeking = self.state().body.lock().peeking;
        let _ = self.app.emit(
            "hue-chatter",
            json!({ "message": line, "kind": kind, "peeking": peeking }),
        );
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64().clamp(0.0, 30.0);
        self.last_tick = now;

        let settings = self.state().settings.lock().clone();
        let id = self.identity();
        watcher::sysmon::tick();
        let live = watcher::live_stats::collect();
        let date = chrono::Local::now().date_naive();
        let hour = chrono::Local::now().hour();
        let season = calendar::season(date);

        // Day rollover (HUE left running overnight).
        if date != self.day {
            self.day = date;
            self.state().memory.lock().touch_day();
            refresh_today(&self.app);
        }

        self.update_conceal(&settings);
        let concealed = *self.state().concealed.lock();

        if self.last_net_check.elapsed() > Duration::from_secs(5) {
            self.network_up = watcher::os_events::has_default_route();
            self.last_net_check = Instant::now();
        }

        // Mood drives the body (speed, eyes).
        let (valence, energy, mood_label_state) = {
            let st = self.state();
            let (sleeping, dizzy) = {
                let b = st.body.lock();
                (b.sleeping, b.is_dizzy())
            };
            let mut mood = st.mood.lock();
            mood.update(
                dt,
                &MoodInputs {
                    stats: &live,
                    axes: &id.personality.axes,
                    hour,
                    sleeping,
                    dizzy,
                    network_up: self.network_up,
                    recent_lag: self
                        .recent_lag
                        .map(|t| t.elapsed() < Duration::from_secs(120))
                        .unwrap_or(false),
                },
            );
            let mut b = st.body.lock();
            b.valence = mood.valence;
            b.energy = mood.energy;
            b.speed_factor = mood.speed_factor();
            let st_mood = if b.is_dizzy() { HueState::Dizzy } else { b.state };
            (mood.valence, mood.energy, st_mood)
        };

        let peeking = self.state().body.lock().peeking;
        let c = self.line_ctx(&id, season, (valence, energy), peeking);
        let mut rng = rand::thread_rng();

        // Flush lines held while hidden.
        if !concealed && self.state().frontend_ready.load(Ordering::SeqCst) && !self.held.is_empty() {
            let held = std::mem::take(&mut self.held);
            for (l, k) in held {
                self.speak(l, k, Prio::Keep);
            }
        }

        self.greet(&c, &settings, &mut rng);
        self.celebrate_major(&mut rng);
        self.body_signals(&c, &mut rng);
        self.sleep_cycle(&c, &settings, concealed, &mut rng);
        self.stalls(&c, concealed, &mut rng);
        self.os_events(&c, &live, &settings, concealed, &mut rng);
        self.timers(&c, &mut rng);

        if self.last_vitals.elapsed() >= Duration::from_secs(3) {
            self.last_vitals = Instant::now();
            let mood = *self.state().mood.lock();
            let _ = self.app.emit(
                "hue-vitals",
                json!({ "stats": live, "mood": { "valence": mood.valence, "energy": mood.energy, "label": mood.label() }, "network_up": self.network_up }),
            );
        }

        if self.last_minute.elapsed() >= Duration::from_secs(60) {
            self.last_minute = Instant::now();
            let st = self.state();
            let mut m = st.memory.lock();
            m.stats.minutes_alive += 1;
            m.mark();
        }

        if self.greeted && self.last_achievements.elapsed() >= Duration::from_secs(5) {
            self.last_achievements = Instant::now();
            self.achievements(&c, &live, &id, &mut rng);
        }

        // Spontaneous chatter.
        let (interacting, speaking, sleeping, dizzy) = {
            let st = self.state();
            let hud = *st.hud_open.lock();
            let b = st.body.lock();
            (
                hud || b.state == HueState::Interacting || st.chat_busy.load(Ordering::SeqCst),
                b.speaking,
                b.sleeping,
                b.is_dizzy(),
            )
        };
        if self.greeted {
            let event_lines: Vec<String> = self
                .state()
                .today_events
                .lock()
                .iter()
                .filter(|e| !e.major)
                .flat_map(|e| e.lines.clone())
                .collect();
            let line = self.chatter.maybe_line(&ChatterInput {
                frequency: settings.talk_frequency,
                concealed,
                interacting,
                speaking,
                sleeping,
                mood_state: mood_label_state,
                dizzy,
                sociability: id.personality.axes.sociability,
                stats: &live,
                lines: &c,
                event_lines: &event_lines,
            });
            if !self.chatter.last_title().is_empty() {
                let t = self.chatter.last_title().to_string();
                self.state().memory.lock().set_last_app(t);
            }
            if let Some(l) = line {
                self.speak(l, "ambient", Prio::Low);
            } else if !concealed && !interacting && !speaking && !sleeping {
                self.maybe_thought(&settings, &id, &live);
            }
        }

        self.state().memory.lock().save_if_needed(false);
    }

    fn update_conceal(&mut self, settings: &crate::config::HueSettings) {
        let conceal = watcher::stealth::should_conceal(settings.stealth_auto, our_hwnd(&self.app));
        let st = self.state();
        let mut concealed = st.concealed.lock();
        if conceal && !*concealed {
            *concealed = true;
            *st.hud_open.lock() = false;
            *st.settings_open.lock() = false;
            st.body.lock().speaking = false;
            if let Some(win) = self.app.get_webview_window("main") {
                let _ = win.hide();
            }
            if let Some(win) = self.app.get_webview_window("bubble") {
                let _ = win.hide();
            }
        } else if !conceal && *concealed {
            *concealed = false;
            let area = st.cached_area.lock().clone();
            // Come back tucked in the corner — no full-size frame.
            {
                let mut body = st.body.lock();
                body.set_size(PET_WINDOW, PET_WINDOW);
                body.speaking = false;
                body.enter_peek(&area);
            }
            if let Some(win) = self.app.get_webview_window("main") {
                let (x, y) = {
                    let b = st.body.lock();
                    (b.x, b.y)
                };
                let _ = win.set_size(PhysicalSize::new(PET_WINDOW as u32, PET_WINDOW as u32));
                let _ = win.set_position(PhysicalPosition::new(x as i32, y as i32));
                window_util::show_without_focus(&win);
            }
        }
    }

    /// One greeting per launch, once the webview listens (fixes the silent boot).
    fn greet(&mut self, c: &LineCtx, settings: &crate::config::HueSettings, rng: &mut impl Rng) {
        if self.greeted || !self.state().frontend_ready.load(Ordering::SeqCst) {
            return;
        }
        let ready_at = *self.ready_at.get_or_insert_with(Instant::now);
        if ready_at.elapsed() < Duration::from_millis(1500) || *self.state().concealed.lock() {
            return;
        }
        self.greeted = true;
        let _ = settings;
        let today = self.day.format("%Y-%m-%d").to_string();
        let major = self
            .state()
            .today_events
            .lock()
            .iter()
            .find(|e| e.major && !e.lines.is_empty())
            .cloned();
        let line = match major {
            Some(ev) if self.state().memory.lock().celebrate_once(ev.id, &today) => {
                self.after_celebration(ev.id);
                ev.lines[rng.gen_range(0..ev.lines.len())].clone()
            }
            _ => lines::boot_greeting(c, self.launch.1, self.launch.0, rng),
        };
        if c.hour < 7 {
            let st = self.state();
            let mut m = st.memory.lock();
            m.stats.early_greets += 1;
            m.mark();
        }
        {
            let st = self.state();
            let mut b = st.body.lock();
            b.set_face("happy", 2500);
            b.start_micro("hop", 700);
        }
        self.speak(line, "greeting", Prio::Keep);
    }

    fn after_celebration(&self, id: &str) {
        if matches!(id, "user_birthday" | "pc_birthday") {
            let st = self.state();
            let mut m = st.memory.lock();
            m.stats.birthdays += 1;
            m.mark();
        }
        self.fx("confetti");
    }

    /// Birthdays & big days that start while HUE is already running.
    fn celebrate_major(&mut self, rng: &mut impl Rng) {
        if !self.greeted {
            return;
        }
        let today = self.day.format("%Y-%m-%d").to_string();
        let events: Vec<_> = self
            .state()
            .today_events
            .lock()
            .iter()
            .filter(|e| e.major && !e.lines.is_empty())
            .cloned()
            .collect();
        for ev in events {
            if self.state().memory.lock().celebrate_once(ev.id, &today) {
                self.after_celebration(ev.id);
                let l = ev.lines[rng.gen_range(0..ev.lines.len())].clone();
                self.speak(l, "event", Prio::Keep);
                break;
            }
        }
    }

    fn body_signals(&mut self, c: &LineCtx, rng: &mut impl Rng) {
        let signals = std::mem::take(&mut self.state().body.lock().signals);
        for sig in signals {
            match sig {
                BodySignal::Pet => {
                    {
                        let st = self.state();
                        st.mood.lock().nudge(0.25, 0.05);
                        let mut m = st.memory.lock();
                        m.stats.pets += 1;
                        m.mark();
                    }
                    self.fx("hearts");
                    if self.last_pet_line.elapsed() > Duration::from_secs(15) {
                        self.last_pet_line = Instant::now();
                        self.speak(lines::pet(c, rng), "pet", Prio::Direct);
                    }
                }
                BodySignal::DoubleClick => {
                    if rng.gen_bool(0.6) {
                        self.speak(lines::double_click(c, rng), "reaction", Prio::Direct);
                    }
                }
                BodySignal::SoftDrop => {
                    if rng.gen_bool(0.35) {
                        self.speak(lines::soft_drop(c, rng), "reaction", Prio::Direct);
                    }
                }
                BodySignal::Toss => {
                    let st = self.state();
                    st.mood.lock().nudge(-0.05, 0.1);
                    let mut m = st.memory.lock();
                    m.stats.tosses += 1;
                    m.mark();
                }
                BodySignal::Dizzy => {
                    {
                        let st = self.state();
                        st.mood.lock().nudge(-0.15, 0.0);
                        let mut m = st.memory.lock();
                        m.stats.dizzies += 1;
                        m.mark();
                    }
                    self.speak(lines::dizzy_announce(c, rng), "dizzy", Prio::Direct);
                }
                BodySignal::Peek => {
                    let st = self.state();
                    let mut m = st.memory.lock();
                    m.stats.peeks += 1;
                    m.mark();
                }
                BodySignal::WokenByClick => {
                    self.sleep_started = None;
                    self.was_sleeping = false;
                    self.speak(lines::woken_by_click(c, rng), "wake", Prio::Direct);
                }
            }
        }
    }

    fn sleep_cycle(
        &mut self,
        c: &LineCtx,
        settings: &crate::config::HueSettings,
        concealed: bool,
        rng: &mut impl Rng,
    ) {
        let idle = watcher::sysmon::user_idle_secs();
        let (sleeping, busy_body) = {
            let st = self.state();
            let b = st.body.lock();
            (
                b.sleeping,
                matches!(b.state, HueState::Dragging | HueState::Interacting | HueState::Tossed),
            )
        };
        // Woken some other way (pet, HUD, tool).
        if self.was_sleeping && !sleeping {
            self.was_sleeping = false;
            if let Some(t) = self.sleep_started.take() {
                let away = t.elapsed().as_secs() / 60;
                if away >= 3 && !concealed {
                    self.speak(lines::wake(c, away, rng), "wake", Prio::Direct);
                }
            }
            return;
        }
        if sleeping {
            self.was_sleeping = true;
            self.sleep_started.get_or_insert_with(Instant::now);
            if idle < 3 {
                // The user is back.
                self.state().body.lock().wake_up();
            }
            return;
        }
        if settings.sleep_after_min == 0 || concealed || busy_body {
            return;
        }
        let hud = *self.state().hud_open.lock();
        let busy = self.state().chat_busy.load(Ordering::SeqCst);
        // Nights make HUE drowsy twice as fast.
        let mut threshold = settings.sleep_after_min as u64 * 60;
        if c.hour < 6 {
            threshold /= 2;
        }
        if !hud && !busy && idle >= threshold.max(30) {
            self.state().body.lock().fall_asleep();
            self.was_sleeping = true;
            self.sleep_started = Some(Instant::now());
            {
                let st = self.state();
                let mut m = st.memory.lock();
                m.stats.naps += 1;
                m.mark();
            }
            if rng.gen_bool(0.6) {
                self.speak(lines::fall_asleep(c, rng), "sleep", Prio::Low);
            }
        }
    }

    /// Lag, frozen apps and wake-from-suspend.
    fn stalls(&mut self, c: &LineCtx, concealed: bool, rng: &mut impl Rng) {
        let lags = std::mem::take(&mut *self.state().lag_events.lock());
        let resumes = std::mem::take(&mut *self.state().resume_events.lock());

        if let Some(mins) = resumes.into_iter().max() {
            self.watcher = OsWatcher::new();
            self.state().body.lock().start_micro("stretch", 1400);
            self.face("sleepy", 2000);
            if !concealed && mins >= 1 {
                self.speak(lines::resume(c, mins, rng), "resume", Prio::Direct);
            }
        }

        if let Some(secs) = lags.into_iter().fold(None, |a: Option<f64>, x| Some(a.map_or(x, |y| y.max(x)))) {
            self.recent_lag = Some(Instant::now());
            {
                let st = self.state();
                st.mood.lock().nudge(-0.2, 0.0);
                let mut m = st.memory.lock();
                m.stats.lags += 1;
                m.mark();
            }
            let sleeping = self.state().body.lock().sleeping;
            let quiet = self
                .last_lag_line
                .map(|t| t.elapsed() < Duration::from_secs(90))
                .unwrap_or(false);
            if !sleeping && !concealed && !quiet {
                self.last_lag_line = Some(Instant::now());
                self.face("dead", 1800);
                self.fx("sweat");
                self.speak(lines::lag(c, secs, rng), "lag", Prio::Low);
            }
        }

        // Foreground app "Not responding" for 3 s+.
        let hung_now = watcher::sysmon::foreground_hung(our_hwnd(&self.app));
        match (hung_now, self.hung.take()) {
            (Some(name), Some((prev, since, announced))) if prev == name => {
                let mut announced = announced;
                if !announced && since.elapsed() >= Duration::from_secs(3) {
                    announced = true;
                    {
                        let st = self.state();
                        let mut m = st.memory.lock();
                        m.stats.lags += 1;
                        m.mark();
                    }
                    if !concealed {
                        self.face("wide", 2500);
                        self.fx("sweat");
                        self.speak(lines::app_hung(c, &name, rng), "lag", Prio::Low);
                    }
                }
                self.hung = Some((name, since, announced));
            }
            (Some(name), _) => self.hung = Some((name, Instant::now(), false)),
            (None, Some((prev, _, true))) => {
                if !concealed {
                    self.face("happy", 1800);
                    self.speak(lines::app_recovered(c, &prev, rng), "lag", Prio::Low);
                }
            }
            (None, _) => {}
        }
    }

    fn os_events(
        &mut self,
        c: &LineCtx,
        live: &LiveStats,
        settings: &crate::config::HueSettings,
        concealed: bool,
        rng: &mut impl Rng,
    ) {
        let cooldown = settings.talk_frequency.cooldown_secs().clamp(60, 600);
        let events = self.watcher.poll(live, cooldown);
        if concealed {
            return;
        }
        for ev in events {
            let (sleeping, peeking) = {
                let st = self.state();
                let b = st.body.lock();
                (b.sleeping, b.peeking)
            };
            let (face, state, fx, mv, me): (Option<&'static str>, Option<HueState>, Option<&str>, f64, f64) = match ev.kind {
                SystemEventKind::DiskFull => (Some("sad"), Some(HueState::Heavy), Some("sweat"), -0.2, 0.0),
                SystemEventKind::BatteryLow => (Some("sleepy"), Some(HueState::Tired), None, -0.1, -0.15),
                SystemEventKind::HotProcess => (Some("strain"), Some(HueState::Heavy), Some("sweat"), -0.05, 0.05),
                SystemEventKind::TempHigh => (Some("strain"), Some(HueState::Heavy), Some("steam"), -0.15, 0.0),
                SystemEventKind::DiskCleaned
                | SystemEventKind::AcPlugged
                | SystemEventKind::NetworkRestored
                | SystemEventKind::UsbConnect => (Some("happy"), Some(HueState::Joy), None, 0.15, 0.05),
                SystemEventKind::BandwidthHeavy => (Some("wide"), None, None, 0.0, 0.05),
                SystemEventKind::NetworkLost | SystemEventKind::UsbDisconnect => {
                    (Some("wide"), Some(HueState::Panic), Some("sweat"), -0.15, 0.05)
                }
                SystemEventKind::LongUptime | SystemEventKind::LateNight | SystemEventKind::AcUnplugged => {
                    (Some("sleepy"), Some(HueState::Tired), None, 0.0, -0.1)
                }
                _ => (None, None, None, 0.0, 0.0),
            };
            self.state().mood.lock().nudge(mv, me);
            if sleeping {
                continue;
            }
            if !peeking {
                let st = self.state();
                let mut b = st.body.lock();
                if let Some(s) = state {
                    b.set_expression(s);
                }
                if let Some(f) = face {
                    b.set_face(f, 2600);
                }
            }
            if let Some(f) = fx {
                self.fx(f);
            }
            let line = lines::event_line(ev.kind, &ev.payload, c, rng);
            self.speak(line, "event", Prio::Low);
            let _ = self.app.emit("hue-system", json!({ "kind": ev.kind, "payload": ev.payload }));
        }
    }

    fn timers(&mut self, c: &LineCtx, rng: &mut impl Rng) {
        let now = crate::util::now_unix();
        let due: Vec<_> = {
            let st = self.state();
            let mut t = st.timers.lock();
            let (done, keep): (Vec<_>, Vec<_>) = t.drain(..).partition(|x| x.due <= now);
            *t = keep;
            done
        };
        for t in due {
            {
                let st = self.state();
                let mut m = st.memory.lock();
                m.stats.timers_done += 1;
                m.mark();
                let mut b = st.body.lock();
                b.wake_up();
                b.set_face("wide", 2500);
                b.start_micro("hop", 700);
            }
            self.fx("ring");
            let line = lines::event_line(SystemEventKind::TimerDone, &json!({ "label": t.label }), c, rng);
            self.speak(line, "timer", Prio::Keep);
        }
    }

    fn achievements(&mut self, c: &LineCtx, live: &LiveStats, id: &IdentityCard, rng: &mut impl Rng) {
        let ctx = achievements::Ctx {
            uptime_hours: live.uptime_secs / 3600,
            pc_age_years: id.pc_age_years,
        };
        let unlocked = {
            let st = self.state();
            let mut m = st.memory.lock();
            let u = achievements::check(&mut m, &ctx);
            if !u.is_empty() {
                m.save_if_needed(true);
            }
            u
        };
        for a in unlocked {
            let _ = self.app.emit(
                "hue-achievement",
                json!({ "id": a.id, "name": a.name, "description": a.description, "title": a.title, "kind": a.kind }),
            );
            self.fx("confetti");
            self.face("happy", 2500);
            self.speak(lines::achievement(c, a.name, a.title, rng), "achievement", Prio::Keep);
        }
    }

    /// Now and then, a spontaneous thought from the LLM — only if one is already up
    /// (never boots the local model in the background).
    fn maybe_thought(&mut self, settings: &crate::config::HueSettings, id: &IdentityCard, live: &LiveStats) {
        if !settings.llm_thoughts
            || settings.talk_frequency == TalkFrequency::Mute
            || Instant::now() < self.next_thought
            || self.started.elapsed() < Duration::from_secs(5 * 60)
            || self.state().thought_busy.load(Ordering::SeqCst)
        {
            return;
        }
        let Some(target) = llm::target_if_ready(settings) else {
            return;
        };
        let mins = rand::thread_rng().gen_range(18..40);
        self.next_thought = Instant::now() + Duration::from_secs(mins * 60);
        self.chatter.note_spoke();
        self.state().thought_busy.store(true, Ordering::SeqCst);

        let mood = self.state().mood.lock().label();
        let events: Vec<String> = self.state().today_events.lock().iter().map(|e| e.name.clone()).collect();
        let include_app = target.local || settings.share_window_title;
        let app_hint = if include_app {
            let t = self.chatter.last_title();
            if t.is_empty() {
                String::new()
            } else {
                format!(" Fenêtre active : « {} ».", crate::util::truncate_chars(t, 60))
            }
        } else {
            String::new()
        };
        let facts = {
            let st = self.state();
            let m = st.memory.lock();
            m.facts.iter().rev().take(5).map(|f| f.text.clone()).collect::<Vec<_>>()
        };
        let now = chrono::Local::now();
        let ctx = format!(
            "Il est {}. Tu te sens {mood}. CPU {:.0} %, RAM {:.0} %.{}{}{}",
            now.format("%H:%M (%A)"),
            live.cpu_pct,
            live.ram_pct,
            if events.is_empty() { String::new() } else { format!(" Aujourd'hui : {}.", events.join(", ")) },
            app_hint,
            if facts.is_empty() { String::new() } else { format!(" Tu sais que : {}.", facts.join(" ; ")) },
        );
        let messages = vec![
            json!({ "role": "system", "content": id.system_prompt }),
            json!({ "role": "user", "content": format!(
                "{ctx}\n\nDis UNE pensée spontanée, à voix haute, comme un petit compagnon de bureau : \
une phrase courte (max 18 mots), drôle ou tendre, en lien avec le contexte. Pas de guillemets, pas d'émoji en excès."
            ) }),
        ];
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let res = llm::complete(&target, messages, 60).await;
            let st = app.state::<AppState>();
            st.thought_busy.store(false, Ordering::SeqCst);
            if let Ok(text) = res {
                let text = text.trim().trim_matches('"').trim().to_string();
                let blocked = *st.concealed.lock()
                    || *st.hud_open.lock()
                    || st.chat_busy.load(Ordering::SeqCst)
                    || st.body.lock().sleeping;
                if !text.is_empty() && !blocked {
                    let peeking = st.body.lock().peeking;
                    let _ = app.emit(
                        "hue-chatter",
                        json!({ "message": crate::util::truncate_chars(&text, 160), "kind": "thought", "peeking": peeking }),
                    );
                }
            }
        });
    }
}
