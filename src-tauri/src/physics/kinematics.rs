use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const FRICTION: f64 = 0.985;
const BOUNCE: f64 = 0.72;
const STOP_EPS: f64 = 8.0;
const GRAVITY: f64 = 0.0;
/// Soft spring-back after an edge hit (higher = longer visible squash).
const SQUASH_DECAY: f64 = 0.92;
const CLICK_MAX_MS: u128 = 250;
const CLICK_MAX_DIST: f64 = 8.0;
/// Visible strip above the taskbar when peeked — full face screen must stay readable.
pub const PEEK_VISIBLE_PX: f64 = 118.0;
const PEEK_MARGIN_RIGHT: f64 = 12.0;
/// How close to the bottom-right corner triggers auto-peek on release.
const CORNER_SNAP_PX: f64 = 96.0;
pub const PET_WINDOW: f64 = 168.0;
const WALK_SPEED: f64 = 55.0;
/// Dizzy lasts 30–60s; recovers faster while tucked.
const DIZZY_SECS_MIN: u64 = 30;
const DIZZY_SECS_MAX: u64 = 60;
/// Extra recovery multiplier while peeked (≈3× total while tucked).
const DIZZY_PEEK_RECOVERY: f64 = 2.0;
/// Min drag speed (px/s) to count as a shake swing.
const SHAKE_SPEED: f64 = 520.0;
/// Direction flips needed within the shake window.
const SHAKE_HITS: u8 = 5;
const SHAKE_WINDOW: Duration = Duration::from_millis(1600);
/// After this idle time, HUE seeks the cursor.
const IDLE_SEEK_AFTER: Duration = Duration::from_secs(50);
const SEEK_COOLDOWN: Duration = Duration::from_secs(40);
const SEEK_SPEED: f64 = 110.0;
/// Soft place instead of toss when release speed is below this (px/s).
const SOFT_DROP_SPEED: f64 = 380.0;
/// Gentle stroke speed band for petting (px/s).
const PET_SPEED_MAX: f64 = 320.0;
const PET_STROKE_PX: f64 = 70.0;
const DOUBLE_CLICK_MS: u128 = 420;
const MICRO_IDLE_MIN: Duration = Duration::from_secs(12);
const MICRO_IDLE_MAX: Duration = Duration::from_secs(28);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HueState {
    Idle,
    Walking,
    Dragging,
    Tossed,
    Interacting,
    Peeking,
    Heavy,
    Tired,
    Panic,
    Joy,
    Dizzy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerPhase {
    Down,
    Move,
    Up,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickPayload {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub squash_x: f64,
    pub squash_y: f64,
    pub state: HueState,
    pub facing_user: bool,
    pub peeking: bool,
    pub speaking: bool,
    pub hud_should_close: bool,
    /// Soft eye look direction (−1…1) toward the cursor.
    pub gaze_x: f64,
    pub gaze_y: f64,
    /// True while HUE is dizzy (also reflected in `state` when free to move).
    pub dizzy: bool,
    /// Horizontal facing for walk/mirror (−1 left, +1 right).
    pub walk_dir: f64,
}

#[derive(Debug, Clone)]
pub struct WorkArea {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug)]
pub struct PhysicsBody {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub width: f64,
    pub height: f64,
    pub squash_x: f64,
    pub squash_y: f64,
    pub state: HueState,
    pub facing_user: bool,
    pub peeking: bool,
    /// Spontaneous speech (bubble above head while peeked / roaming).
    pub speaking: bool,
    /// Frontend should tuck dialogue / settings.
    pub hud_should_close: bool,
    /// Soft eye look (−1…1) toward the cursor.
    pub gaze_x: f64,
    pub gaze_y: f64,
    /// After a pointer Up: Some(true)=open HUD, Some(false)=close HUD.
    pub want_hud: Option<bool>,
    /// Just entered dizzy — chatter can announce it once.
    pub dizzy_just_started: bool,
    /// Forced chatter cue: "pet" | "double" | "soft_drop".
    pub want_chatter_kind: Option<&'static str>,
    walk_dir: f64,
    drag_offset_x: f64,
    drag_offset_y: f64,
    pointer_down_ms: Option<u128>,
    pointer_down_x: f64,
    pointer_down_y: f64,
    last_pointer_x: f64,
    last_pointer_y: f64,
    last_pointer_ms: u128,
    moved_while_down: bool,
    was_peeking_on_down: bool,
    hud_open_on_down: bool,
    /// Hold Joy/Tired/etc. before resuming walk.
    expression_until: Option<Instant>,
    dizzy_until: Option<Instant>,
    shake_hits: u8,
    shake_window_start: Option<Instant>,
    last_shake_dir_x: f64,
    last_shake_dir_y: f64,
    wobble_t: f64,
    last_interaction: Instant,
    next_seek_at: Instant,
    seeking: bool,
    pet_stroke_px: f64,
    pet_triggered: bool,
    last_short_click_ms: u128,
    next_micro_at: Instant,
}

impl PhysicsBody {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            width,
            height,
            squash_x: 1.0,
            squash_y: 1.0,
            state: HueState::Idle,
            facing_user: false,
            peeking: false,
            speaking: false,
            hud_should_close: false,
            gaze_x: 0.0,
            gaze_y: 0.0,
            want_hud: None,
            dizzy_just_started: false,
            want_chatter_kind: None,
            walk_dir: 1.0,
            drag_offset_x: 0.0,
            drag_offset_y: 0.0,
            pointer_down_ms: None,
            pointer_down_x: 0.0,
            pointer_down_y: 0.0,
            last_pointer_x: 0.0,
            last_pointer_y: 0.0,
            last_pointer_ms: 0,
            moved_while_down: false,
            was_peeking_on_down: false,
            hud_open_on_down: false,
            expression_until: None,
            dizzy_until: None,
            shake_hits: 0,
            shake_window_start: None,
            last_shake_dir_x: 0.0,
            last_shake_dir_y: 0.0,
            wobble_t: 0.0,
            last_interaction: Instant::now(),
            next_seek_at: Instant::now() + IDLE_SEEK_AFTER,
            seeking: false,
            pet_stroke_px: 0.0,
            pet_triggered: false,
            last_short_click_ms: 0,
            next_micro_at: Instant::now()
                + Duration::from_millis(
                    rand::thread_rng().gen_range(
                        MICRO_IDLE_MIN.as_millis() as u64..=MICRO_IDLE_MAX.as_millis() as u64,
                    ),
                ),
        }
    }

    pub fn touch_interaction(&mut self) {
        self.last_interaction = Instant::now();
        self.next_seek_at = Instant::now() + IDLE_SEEK_AFTER;
        self.seeking = false;
    }

    pub fn is_dizzy(&self) -> bool {
        self.dizzy_until
            .map(|until| Instant::now() < until)
            .unwrap_or(false)
    }

    pub fn tick_payload(&self) -> TickPayload {
        let dizzy = self.is_dizzy();
        // Keep Peeking/Dragging/Interacting as the interaction state; frontend uses `dizzy`.
        let state = if dizzy
            && !matches!(
                self.state,
                HueState::Dragging | HueState::Interacting | HueState::Peeking
            )
        {
            HueState::Dizzy
        } else {
            self.state
        };
        TickPayload {
            x: self.x,
            y: self.y,
            vx: self.vx,
            vy: self.vy,
            squash_x: self.squash_x,
            squash_y: self.squash_y,
            state,
            facing_user: self.facing_user,
            peeking: self.peeking,
            speaking: self.speaking,
            hud_should_close: self.hud_should_close
                || matches!(self.state, HueState::Dragging | HueState::Tossed | HueState::Peeking),
            gaze_x: self.gaze_x,
            gaze_y: self.gaze_y,
            dizzy,
            walk_dir: if self.walk_dir.abs() < 0.1 {
                if self.vx >= 0.0 {
                    1.0
                } else {
                    -1.0
                }
            } else {
                self.walk_dir.signum()
            },
        }
    }

    pub fn set_size(&mut self, width: f64, height: f64) {
        self.width = width;
        self.height = height;
    }

    /// Skull peek: window top sits `PEEK_VISIBLE_PX` above the screen bottom.
    /// Speech bubble uses a separate window — never resize/move the pet for dialogue.
    pub fn peek_position(area: &WorkArea, width: f64, _height: f64) -> (f64, f64) {
        let x = area.x + area.width - width - PEEK_MARGIN_RIGHT;
        let y = area.y + area.height - PEEK_VISIBLE_PX;
        (x.max(area.x), y)
    }

    /// Y so the pet rests on the work-area bottom (above the taskbar).
    pub fn taskbar_y(area: &WorkArea, height: f64) -> f64 {
        area.y + area.height - height
    }

    pub fn emerge_position(area: &WorkArea, width: f64, height: f64) -> (f64, f64) {
        let x = area.x + area.width - width - PEEK_MARGIN_RIGHT;
        let y = area.y + area.height - height - 24.0;
        (
            x.max(area.x),
            y.clamp(area.y, (area.y + area.height - height).max(area.y)),
        )
    }

    pub fn near_dock_corner(area: &WorkArea, x: f64, y: f64, width: f64, height: f64) -> bool {
        let right = area.x + area.width;
        let bottom = area.y + area.height;
        let cx = x + width * 0.5;
        let cy = y + height * 0.5;
        cx >= right - CORNER_SNAP_PX - width * 0.25 && cy >= bottom - CORNER_SNAP_PX - height * 0.25
    }

    pub fn enter_peek(&mut self, area: &WorkArea) {
        self.width = PET_WINDOW;
        self.height = PET_WINDOW;
        let (x, y) = Self::peek_position(area, self.width, self.height);
        self.x = x;
        self.y = y;
        self.vx = 0.0;
        self.vy = 0.0;
        self.peeking = true;
        self.state = HueState::Peeking;
        self.facing_user = false;
        self.squash_x = 1.0;
        self.squash_y = 1.0;
        self.hud_should_close = true;
    }

    pub fn start_walking(&mut self, area: &WorkArea) {
        if self.peeking || matches!(self.state, HueState::Dragging | HueState::Interacting) {
            return;
        }
        self.state = HueState::Walking;
        self.y = Self::taskbar_y(area, self.height);
        if self.vx.abs() < 10.0 {
            self.vx = WALK_SPEED * self.walk_dir;
        }
        self.vy = 0.0;
        // Resting on the taskbar is not an impact — keep a clean silhouette at spawn.
        self.squash_x = 1.0;
        self.squash_y = 1.0;
    }

    pub fn start_roaming(&mut self, area: &WorkArea) {
        if self.peeking || matches!(self.state, HueState::Dragging | HueState::Interacting) {
            return;
        }
        self.state = HueState::Walking;
        if self.vx.abs() < 10.0 && self.vy.abs() < 10.0 {
            self.vx = WALK_SPEED * self.walk_dir;
            self.vy = WALK_SPEED * 0.35;
        }
        self.squash_x = 1.0;
        self.squash_y = 1.0;
        self.clamp_inside(area);
    }

    pub fn on_pointer(
        &mut self,
        phase: PointerPhase,
        screen_x: f64,
        screen_y: f64,
        now_ms: u128,
        area: &WorkArea,
        hud_open: bool,
        _stealth_auto: bool,
    ) {
        self.want_hud = None;
        self.touch_interaction();
        match phase {
            PointerPhase::Down => {
                self.width = PET_WINDOW;
                self.height = PET_WINDOW;
                self.was_peeking_on_down = self.peeking;
                self.hud_open_on_down = hud_open;
                // Don't close HUD yet — short click may toggle. Drag will close on Move.

                if self.peeking {
                    // Pull fully on-screen under cursor (ready to talk or drag)
                    self.peeking = false;
                    self.x = screen_x - self.width * 0.5;
                    self.y = screen_y - self.height * 0.5;
                    self.clamp_inside(area);
                }

                self.pointer_down_ms = Some(now_ms);
                self.pointer_down_x = screen_x;
                self.pointer_down_y = screen_y;
                self.last_pointer_x = screen_x;
                self.last_pointer_y = screen_y;
                self.last_pointer_ms = now_ms;
                self.moved_while_down = false;
                self.pet_stroke_px = 0.0;
                self.pet_triggered = false;
                self.drag_offset_x = screen_x - self.x;
                self.drag_offset_y = screen_y - self.y;
                self.vx = 0.0;
                self.vy = 0.0;
                self.state = HueState::Dragging;
                self.facing_user = true;
                self.squash_x = 1.06;
                self.squash_y = 0.94;
                self.shake_hits = 0;
                self.shake_window_start = None;
                self.last_shake_dir_x = 0.0;
                self.last_shake_dir_y = 0.0;
            }
            PointerPhase::Move => {
                if self.pointer_down_ms.is_none() {
                    return;
                }
                let dx = screen_x - self.pointer_down_x;
                let dy = screen_y - self.pointer_down_y;
                if (dx * dx + dy * dy).sqrt() > CLICK_MAX_DIST {
                    self.moved_while_down = true;
                    self.hud_should_close = true;
                    self.want_hud = Some(false);
                }
                let dt = (now_ms.saturating_sub(self.last_pointer_ms) as f64).max(1.0) / 1000.0;
                let nx = screen_x - self.drag_offset_x;
                let ny = screen_y - self.drag_offset_y;
                self.vx = (nx - self.x) / dt;
                self.vy = (ny - self.y) / dt;
                let step = ((nx - self.x).powi(2) + (ny - self.y).powi(2)).sqrt();
                let speed = (self.vx * self.vx + self.vy * self.vy).sqrt();
                // Gentle stroke = pet (not a hard shake / throw wind-up).
                if speed > 20.0 && speed < PET_SPEED_MAX {
                    self.pet_stroke_px += step;
                    if !self.pet_triggered && self.pet_stroke_px >= PET_STROKE_PX {
                        self.pet_triggered = true;
                        self.want_chatter_kind = Some("pet");
                        self.hover_hop();
                    }
                }
                self.x = nx;
                self.y = ny;
                self.last_pointer_x = screen_x;
                self.last_pointer_y = screen_y;
                self.last_pointer_ms = now_ms;
                self.state = HueState::Dragging;
                self.peeking = false;
                self.note_shake_sample();
            }
            PointerPhase::Up => {
                let down_ms = self.pointer_down_ms.take();
                let duration = down_ms.map(|t| now_ms.saturating_sub(t)).unwrap_or(0);
                let dist = ((screen_x - self.pointer_down_x).powi(2)
                    + (screen_y - self.pointer_down_y).powi(2))
                    .sqrt();

                let short_click =
                    !self.moved_while_down && duration <= CLICK_MAX_MS && dist <= CLICK_MAX_DIST;

                if short_click {
                    self.vx = 0.0;
                    self.vy = 0.0;
                    self.squash_x = 1.0;
                    self.squash_y = 1.0;

                    let is_double = self.last_short_click_ms > 0
                        && now_ms.saturating_sub(self.last_short_click_ms) <= DOUBLE_CLICK_MS;
                    self.last_short_click_ms = now_ms;

                    if is_double {
                        // Dedicated hop — do not toggle HUD.
                        self.peeking = false;
                        self.facing_user = true;
                        self.want_hud = None;
                        self.hud_should_close = false;
                        self.want_chatter_kind = Some("double");
                        self.state = HueState::Idle;
                        self.hover_hop();
                        self.squash_x = 1.15;
                        self.squash_y = 0.85;
                        if !self.is_dizzy() {
                            self.set_expression(HueState::Joy);
                        }
                        self.clamp_inside(area);
                        return;
                    }

                    // Dialogue already open + simple click → ranger
                    if self.hud_open_on_down && !self.was_peeking_on_down {
                        self.state = HueState::Idle;
                        self.facing_user = false;
                        self.peeking = false;
                        self.hud_should_close = true;
                        self.want_hud = Some(false);
                        self.clamp_inside(area);
                        return;
                    }

                    // Caché (peek) ou HUD fermé → sortir + ouvrir la boîte de dialogue
                    if self.was_peeking_on_down {
                        let (x, y) = Self::emerge_position(area, self.width, self.height);
                        self.x = x;
                        self.y = y;
                    } else {
                        self.clamp_inside(area);
                    }
                    self.peeking = false;
                    self.state = HueState::Interacting;
                    self.facing_user = true;
                    self.hud_should_close = false;
                    self.want_hud = Some(true);
                    return;
                }

                // Dropped in the bottom-right dock zone → tuck automatically
                if Self::near_dock_corner(area, self.x, self.y, self.width, self.height) {
                    self.enter_peek(area);
                    self.want_hud = Some(false);
                    return;
                }

                let speed = (self.vx * self.vx + self.vy * self.vy).sqrt();
                self.hud_should_close = true;
                self.want_hud = Some(false);
                self.peeking = false;

                if speed < SOFT_DROP_SPEED || self.pet_triggered {
                    // Soft place — settle without tossing.
                    self.vx = 0.0;
                    self.vy = 0.0;
                    self.state = HueState::Idle;
                    self.facing_user = true;
                    self.squash_x = 1.05;
                    self.squash_y = 0.95;
                    if self.want_chatter_kind.is_none() {
                        self.want_chatter_kind = Some("soft_drop");
                    }
                    if self.pet_triggered && !self.is_dizzy() {
                        self.set_expression(HueState::Joy);
                    }
                    self.clamp_inside(area);
                    return;
                }

                // Throw / release elsewhere
                self.vx = self.vx.clamp(-2400.0, 2400.0);
                self.vy = self.vy.clamp(-2400.0, 2400.0);
                self.state = HueState::Tossed;
                self.facing_user = false;
                self.clamp_inside(area);
            }
        }
    }

    pub fn step(
        &mut self,
        dt: f64,
        area: &WorkArea,
        movement: crate::config::MovementMode,
        cursor: Option<(f64, f64)>,
    ) {
        use crate::config::MovementMode;

        self.squash_x = 1.0 + (self.squash_x - 1.0) * SQUASH_DECAY;
        self.squash_y = 1.0 + (self.squash_y - 1.0) * SQUASH_DECAY;
        self.wobble_t += dt;

        // Faster dizzy recovery while tucked behind the taskbar.
        if self.peeking && self.is_dizzy() {
            if let Some(until) = self.dizzy_until {
                let remaining = until.saturating_duration_since(Instant::now());
                let cut = Duration::from_secs_f64((dt * DIZZY_PEEK_RECOVERY).max(0.0));
                let left = remaining.saturating_sub(cut);
                self.dizzy_until = if left.is_zero() {
                    None
                } else {
                    Some(Instant::now() + left)
                };
            }
        }

        // Expire dizzy timer
        if let Some(until) = self.dizzy_until {
            if Instant::now() >= until {
                self.dizzy_until = None;
                if self.state == HueState::Dizzy {
                    self.state = HueState::Idle;
                }
            }
        }

        if self.peeking {
            self.width = PET_WINDOW;
            self.height = PET_WINDOW;
            let (bx, by) = Self::peek_position(area, self.width, self.height);
            self.vx = 0.0;
            self.vy = 0.0;
            self.state = HueState::Peeking;
            self.hud_should_close = true;
            if self.is_dizzy() {
                // Soft wobble so speech bubble / face move with the dizzy pet.
                let wobble = (self.wobble_t * 5.5).sin();
                self.x = bx + wobble * 14.0;
                self.y = by + (self.wobble_t * 4.2).cos() * 4.0;
                let spin = self.wobble_t * 4.2;
                self.gaze_x = spin.sin() * 0.85;
                self.gaze_y = (spin * 1.3).cos() * 0.55;
                self.squash_x = 1.0 + (self.wobble_t * 6.0).sin() * 0.05;
                self.squash_y = 1.0 + (self.wobble_t * 6.0).cos() * 0.05;
            } else {
                self.x = bx;
                self.y = by;
            }
            return;
        }

        if self.state == HueState::Dragging || self.state == HueState::Interacting {
            self.clamp_inside(area);
            return;
        }

        let min_x = area.x;
        let max_x = area.x + area.width - self.width;
        let min_y = area.y;
        let bar_y = Self::taskbar_y(area, self.height);
        let max_y = bar_y;
        let dizzy = self.is_dizzy();

        // Dizzy overrides short mood expressions and drives a wobbly walk.
        if dizzy {
            self.expression_until = None;
            self.seeking = false;
            self.state = HueState::Dizzy;
            self.apply_dizzy_motion(dt, area, movement, min_x, max_x, min_y, max_y, bar_y);
            return;
        }

        // Tiny idle life between seeks.
        self.maybe_micro_idle();

        // Boredom: after inactivity, stroll toward the cursor.
        self.maybe_start_seek(movement, cursor);
        if self.seeking {
            if let Some((cx, cy)) = cursor {
                if self.drive_toward_cursor(dt, area, movement, cx, cy, min_x, max_x, min_y, max_y, bar_y)
                {
                    return;
                }
            } else {
                self.seeking = false;
            }
        }

        // Hold mood expressions a few seconds so they remain visible.
        if matches!(
            self.state,
            HueState::Joy | HueState::Panic | HueState::Heavy | HueState::Tired
        ) {
            if let Some(until) = self.expression_until {
                if Instant::now() < until {
                    self.vx = 0.0;
                    self.vy = 0.0;
                    self.clamp_inside(area);
                    return;
                }
            }
            self.expression_until = None;
            self.state = HueState::Idle;
        }

        // Resume autonomous motion after idle / expression
        if matches!(self.state, HueState::Idle | HueState::Dizzy) {
            match movement {
                MovementMode::Taskbar => self.start_walking(area),
                MovementMode::Fullscreen => self.start_roaming(area),
                MovementMode::Static => {
                    self.vx = 0.0;
                    self.vy = 0.0;
                    self.state = HueState::Idle;
                }
            }
        }

        if self.state == HueState::Walking {
            match movement {
                MovementMode::Static => {
                    self.vx = 0.0;
                    self.vy = 0.0;
                    self.state = HueState::Idle;
                    return;
                }
                MovementMode::Taskbar => {
                    self.y = bar_y;
                    self.vy = 0.0;
                    if self.vx.abs() < 20.0 {
                        self.vx = WALK_SPEED * self.walk_dir;
                    }
                    self.x += self.vx * dt;
                    if self.x <= min_x {
                        self.x = min_x;
                        self.walk_dir = 1.0;
                        self.vx = WALK_SPEED;
                        self.apply_edge_squash(true);
                    } else if self.x >= max_x {
                        self.x = max_x;
                        self.walk_dir = -1.0;
                        self.vx = -WALK_SPEED;
                        self.apply_edge_squash(true);
                    }
                    self.facing_user = false;
                    return;
                }
                MovementMode::Fullscreen => {
                    if self.vx.abs() < 15.0 && self.vy.abs() < 15.0 {
                        self.vx = WALK_SPEED * self.walk_dir;
                        self.vy = WALK_SPEED * 0.4;
                    }
                    self.x += self.vx * dt;
                    self.y += self.vy * dt;
                    if self.x <= min_x {
                        self.x = min_x;
                        self.vx = self.vx.abs();
                        self.walk_dir = 1.0;
                        self.apply_edge_squash(true);
                    } else if self.x >= max_x {
                        self.x = max_x;
                        self.vx = -self.vx.abs();
                        self.walk_dir = -1.0;
                        self.apply_edge_squash(true);
                    }
                    if self.y <= min_y {
                        self.y = min_y;
                        let impact = self.vy.abs();
                        self.vy = self.vy.abs();
                        if impact > 25.0 {
                            self.apply_edge_squash(false);
                        }
                    } else if self.y >= max_y {
                        self.y = max_y;
                        let impact = self.vy.abs();
                        self.vy = -self.vy.abs();
                        // Ignore soft contact with the taskbar line (spawn / settle).
                        if impact > 25.0 {
                            self.apply_edge_squash(false);
                        }
                    }
                    self.facing_user = false;
                    return;
                }
            }
        }

        // Ballistic toss
        self.vy += GRAVITY * dt;
        self.x += self.vx * dt;
        self.y += self.vy * dt;

        let mut bounced = false;
        if self.x < min_x {
            self.x = min_x;
            let impact = self.vx.abs();
            self.vx = -self.vx * BOUNCE;
            if impact > 40.0 {
                self.apply_edge_squash(true);
            }
            bounced = true;
        } else if self.x > max_x {
            self.x = max_x;
            let impact = self.vx.abs();
            self.vx = -self.vx * BOUNCE;
            if impact > 40.0 {
                self.apply_edge_squash(true);
            }
            bounced = true;
        }

        if self.y < min_y {
            self.y = min_y;
            let impact = self.vy.abs();
            self.vy = -self.vy * BOUNCE;
            if impact > 40.0 {
                self.apply_edge_squash(false);
            }
            bounced = true;
        } else if self.y > max_y {
            self.y = max_y;
            let impact = self.vy.abs();
            self.vy = -self.vy * BOUNCE;
            if impact > 40.0 {
                self.apply_edge_squash(false);
            }
            bounced = true;
        }

        self.vx *= FRICTION;
        self.vy *= FRICTION;

        let speed = (self.vx * self.vx + self.vy * self.vy).sqrt();
        if speed < STOP_EPS {
            self.vx = 0.0;
            self.vy = 0.0;
            if Self::near_dock_corner(area, self.x, self.y, self.width, self.height) {
                self.enter_peek(area);
            } else {
                match movement {
                    MovementMode::Taskbar => self.start_walking(area),
                    MovementMode::Fullscreen => self.start_roaming(area),
                    MovementMode::Static => self.state = HueState::Idle,
                }
            }
        } else if bounced {
            self.state = HueState::Tossed;
        }

        self.clamp_inside(area);
    }

    /// Squash/stretch into the wall: horizontal edge → compress X / stretch Y.
    fn apply_edge_squash(&mut self, against_vertical_wall: bool) {
        if against_vertical_wall {
            self.squash_x = 0.52;
            self.squash_y = 1.42;
        } else {
            self.squash_x = 1.42;
            self.squash_y = 0.52;
        }
    }

    fn clamp_inside(&mut self, area: &WorkArea) {
        let max_x = (area.x + area.width - self.width).max(area.x);
        let max_y = (area.y + area.height - self.height).max(area.y);
        self.x = self.x.clamp(area.x, max_x);
        self.y = self.y.clamp(area.y, max_y);
    }

    pub fn set_expression(&mut self, state: HueState) {
        if self.is_dizzy() {
            return;
        }
        if self.state != HueState::Dragging && !self.peeking {
            self.state = state;
            self.expression_until = Some(Instant::now() + Duration::from_millis(3200));
            self.vx = 0.0;
            self.vy = 0.0;
        }
    }

    fn note_shake_sample(&mut self) {
        let speed = (self.vx * self.vx + self.vy * self.vy).sqrt();
        if speed < SHAKE_SPEED {
            return;
        }
        let dir_x = if self.vx.abs() > 80.0 {
            self.vx.signum()
        } else {
            0.0
        };
        let dir_y = if self.vy.abs() > 80.0 {
            self.vy.signum()
        } else {
            0.0
        };

        let mut hit = false;
        if dir_x != 0.0 && self.last_shake_dir_x != 0.0 && dir_x != self.last_shake_dir_x {
            hit = true;
        }
        if dir_y != 0.0 && self.last_shake_dir_y != 0.0 && dir_y != self.last_shake_dir_y {
            hit = true;
        }
        if dir_x != 0.0 {
            self.last_shake_dir_x = dir_x;
        }
        if dir_y != 0.0 {
            self.last_shake_dir_y = dir_y;
        }
        if !hit {
            return;
        }

        let now = Instant::now();
        match self.shake_window_start {
            Some(start) if now.duration_since(start) <= SHAKE_WINDOW => {
                self.shake_hits = self.shake_hits.saturating_add(1);
            }
            _ => {
                self.shake_window_start = Some(now);
                self.shake_hits = 1;
            }
        }

        if self.shake_hits >= SHAKE_HITS {
            self.enter_dizzy();
        }
    }

    fn enter_dizzy(&mut self) {
        let secs = rand::thread_rng().gen_range(DIZZY_SECS_MIN..=DIZZY_SECS_MAX);
        self.dizzy_until = Some(Instant::now() + Duration::from_secs(secs));
        self.dizzy_just_started = true;
        self.seeking = false;
        self.shake_hits = 0;
        self.shake_window_start = None;
        self.last_shake_dir_x = 0.0;
        self.last_shake_dir_y = 0.0;
        self.expression_until = None;
        if !matches!(
            self.state,
            HueState::Dragging | HueState::Interacting | HueState::Peeking
        ) {
            self.state = HueState::Dizzy;
        }
        // Visible jolt when it kicks in
        self.squash_x = 1.2;
        self.squash_y = 0.82;
        self.gaze_x = 0.0;
        self.gaze_y = 0.0;
    }

    fn schedule_next_micro(&mut self) {
        let ms = rand::thread_rng()
            .gen_range(MICRO_IDLE_MIN.as_millis() as u64..=MICRO_IDLE_MAX.as_millis() as u64);
        self.next_micro_at = Instant::now() + Duration::from_millis(ms);
    }

    fn maybe_micro_idle(&mut self) {
        if self.seeking || self.is_dizzy() {
            return;
        }
        if !matches!(self.state, HueState::Idle | HueState::Walking) {
            return;
        }
        if Instant::now() < self.next_micro_at {
            return;
        }
        self.schedule_next_micro();
        let mut rng = rand::thread_rng();
        // Never randomize gaze here — it fights cursor tracking and looks glitchy from afar.
        match rng.gen_range(0..2) {
            0 => self.hover_hop(),
            _ => {
                if matches!(self.state, HueState::Idle) {
                    self.set_expression(HueState::Joy);
                } else {
                    self.hover_hop();
                }
            }
        }
    }

    fn maybe_start_seek(
        &mut self,
        movement: crate::config::MovementMode,
        cursor: Option<(f64, f64)>,
    ) {
        use crate::config::MovementMode;
        if self.seeking || cursor.is_none() {
            return;
        }
        if matches!(movement, MovementMode::Static) {
            // Static: tiny hop reaction only, no chase.
            if Instant::now() >= self.next_seek_at {
                self.hover_hop();
                self.facing_user = true;
                self.next_seek_at = Instant::now() + SEEK_COOLDOWN;
                self.last_interaction = Instant::now();
            }
            return;
        }
        if Instant::now() >= self.next_seek_at
            && self.last_interaction.elapsed() >= IDLE_SEEK_AFTER
        {
            self.seeking = true;
            self.expression_until = None;
            self.state = HueState::Walking;
            self.squash_x = 1.1;
            self.squash_y = 0.92;
        }
    }

    /// Returns true if this frame fully handled motion (caller should skip normal walk).
    fn drive_toward_cursor(
        &mut self,
        dt: f64,
        area: &WorkArea,
        movement: crate::config::MovementMode,
        cx: f64,
        cy: f64,
        min_x: f64,
        max_x: f64,
        min_y: f64,
        max_y: f64,
        bar_y: f64,
    ) -> bool {
        use crate::config::MovementMode;

        let target_x = (cx - self.width * 0.5).clamp(min_x, max_x);
        let target_y = match movement {
            MovementMode::Taskbar => bar_y,
            MovementMode::Fullscreen => (cy - self.height * 0.4).clamp(min_y, max_y),
            MovementMode::Static => self.y,
        };

        let dx = target_x - self.x;
        let dy = target_y - self.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 22.0 {
            self.x = target_x;
            self.y = target_y;
            self.vx = 0.0;
            self.vy = 0.0;
            self.seeking = false;
            self.facing_user = true;
            self.hover_hop();
            self.next_seek_at = Instant::now() + SEEK_COOLDOWN;
            self.last_interaction = Instant::now();
            self.state = HueState::Idle;
            self.clamp_inside(area);
            return true;
        }

        let speed = SEEK_SPEED;
        self.vx = (dx / dist) * speed;
        self.vy = (dy / dist) * speed;
        if matches!(movement, MovementMode::Taskbar) {
            self.y = bar_y;
            self.vy = 0.0;
            self.vx = dx.signum() * speed;
            self.x += self.vx * dt;
        } else {
            self.x += self.vx * dt;
            self.y += self.vy * dt;
        }
        if dx.abs() > 8.0 {
            self.walk_dir = dx.signum();
        }
        self.state = HueState::Walking;
        self.facing_user = true;
        self.clamp_inside(area);
        true
    }

    fn apply_dizzy_motion(
        &mut self,
        dt: f64,
        area: &WorkArea,
        movement: crate::config::MovementMode,
        min_x: f64,
        max_x: f64,
        min_y: f64,
        max_y: f64,
        bar_y: f64,
    ) {
        use crate::config::MovementMode;

        // Spinning gaze + soft squash pulse
        let spin = self.wobble_t * 4.2;
        self.gaze_x = spin.sin() * 0.85;
        self.gaze_y = (spin * 1.3).cos() * 0.55;
        self.squash_x = 1.0 + (self.wobble_t * 6.0).sin() * 0.06;
        self.squash_y = 1.0 + (self.wobble_t * 6.0).cos() * 0.06;
        self.facing_user = false;

        let wobble = (self.wobble_t * 5.5).sin();
        match movement {
            MovementMode::Static => {
                self.vx = 0.0;
                self.vy = 0.0;
                // Tiny in-place sway
                self.x += wobble * 18.0 * dt;
                self.clamp_inside(area);
            }
            MovementMode::Taskbar => {
                self.y = bar_y;
                self.vy = 0.0;
                let speed = WALK_SPEED * (0.55 + wobble.abs() * 0.45);
                if self.vx.abs() < 12.0 {
                    self.vx = speed * self.walk_dir;
                }
                // Occasional random flip while dizzy
                if (self.wobble_t * 10.0).sin().abs() < 0.04 {
                    self.walk_dir = -self.walk_dir;
                    self.vx = speed * self.walk_dir;
                }
                self.x += self.vx * dt + wobble * 40.0 * dt;
                if self.x <= min_x {
                    self.x = min_x;
                    self.walk_dir = 1.0;
                    self.vx = speed;
                    self.apply_edge_squash(true);
                } else if self.x >= max_x {
                    self.x = max_x;
                    self.walk_dir = -1.0;
                    self.vx = -speed;
                    self.apply_edge_squash(true);
                }
            }
            MovementMode::Fullscreen => {
                let speed = WALK_SPEED * 0.7;
                if self.vx.abs() < 10.0 && self.vy.abs() < 10.0 {
                    self.vx = speed * self.walk_dir;
                    self.vy = speed * 0.35;
                }
                self.vx += wobble * 90.0 * dt;
                self.vy += (self.wobble_t * 4.1).cos() * 70.0 * dt;
                self.x += self.vx * dt;
                self.y += self.vy * dt;
                if self.x <= min_x {
                    self.x = min_x;
                    self.vx = self.vx.abs();
                    self.walk_dir = 1.0;
                    self.apply_edge_squash(true);
                } else if self.x >= max_x {
                    self.x = max_x;
                    self.vx = -self.vx.abs();
                    self.walk_dir = -1.0;
                    self.apply_edge_squash(true);
                }
                if self.y <= min_y {
                    self.y = min_y;
                    self.vy = self.vy.abs();
                } else if self.y >= max_y {
                    self.y = max_y;
                    self.vy = -self.vy.abs();
                }
            }
        }
        self.clamp_inside(area);
    }

    /// Soft look-toward the cursor (eyes follow slightly).
    pub fn update_cursor_awareness(&mut self, cursor_x: f64, cursor_y: f64) {
        if matches!(self.state, HueState::Dragging) || self.is_dizzy() {
            return;
        }
        // Aim from approximate eye height inside the pet window.
        let cx = self.x + self.width * 0.5;
        let cy = self.y + self.height * 0.38;
        let dx = cursor_x - cx;
        let dy = cursor_y - cy;
        let dist = (dx * dx + dy * dy).sqrt().max(1.0);
        // Fade out with distance — no residual glance from across the screen.
        let strength = if dist > 420.0 {
            0.0
        } else {
            ((1.0 - dist / 420.0) * 1.15).clamp(0.0, 1.0)
        };
        let target_x = (dx / 110.0).clamp(-1.0, 1.0) * strength;
        let target_y = (dy / 130.0).clamp(-1.0, 1.0) * strength * 0.7;
        // Smooth so the gaze doesn't jitter.
        let blend = if strength > 0.35 { 0.28 } else { 0.14 };
        self.gaze_x += (target_x - self.gaze_x) * blend;
        self.gaze_y += (target_y - self.gaze_y) * blend;
        // Face the user when close — do NOT flip walk_dir (mirrors eyes / glitches gaze).
        self.facing_user = dist < 260.0 && !self.peeking && strength > 0.2;
    }

    /// Tiny hop when hovered / petted (frontend-driven via command).
    pub fn hover_hop(&mut self) {
        if self.peeking {
            return;
        }
        self.squash_x = 1.08;
        self.squash_y = 0.9;
    }
}
