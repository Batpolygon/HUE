use crate::ai::calendar::DayEvent;
use crate::ai::documents::DocStore;
use crate::ai::memory::PersistentMemory;
use crate::ai::mood::Mood;
use crate::ai::personality::IdentityCard;
use crate::config::HueSettings;
use crate::physics::{PhysicsBody, WorkArea, PET_WINDOW};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

/// Main window size while the chat HUD is open.
pub const HUD_SIZE: (f64, f64) = (320.0, 372.0);
/// Main window size while the settings modal is open.
pub const SETTINGS_SIZE: (f64, f64) = (420.0, 560.0);

#[derive(Debug, Clone)]
pub struct Timer {
    /// Unix seconds.
    pub due: i64,
    pub label: String,
}

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
    pub memory: Mutex<PersistentMemory>,
    pub docs: Mutex<DocStore>,
    pub mood: Mutex<Mood>,
    pub timers: Mutex<Vec<Timer>>,
    pub pending_confirms: Mutex<HashMap<u64, tokio::sync::oneshot::Sender<bool>>>,
    pub confirm_seq: Mutex<u64>,
    pub today_events: Mutex<Vec<DayEvent>>,
    /// The main webview registered its listeners (safe to greet).
    pub frontend_ready: AtomicBool,
    /// A chat reply / file analysis is streaming into the HUD.
    pub chat_busy: AtomicBool,
    /// Stall durations (s) noticed by the heartbeat thread.
    pub lag_events: Mutex<Vec<f64>>,
    /// Minutes spent suspended (wall clock jumped).
    pub resume_events: Mutex<Vec<u64>>,
    pub thought_busy: AtomicBool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            body: Mutex::new(PhysicsBody::new(0.0, 0.0, PET_WINDOW, PET_WINDOW)),
            settings: Mutex::new(HueSettings::load()),
            identity: Mutex::new(None),
            hud_open: Mutex::new(false),
            settings_open: Mutex::new(false),
            cached_area: Mutex::new(WorkArea {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1040.0,
            }),
            concealed: Mutex::new(false),
            bubble_size: Mutex::new((160.0, 40.0)),
            memory: Mutex::new(PersistentMemory::load()),
            docs: Mutex::new(DocStore::load()),
            mood: Mutex::new(Mood::default()),
            timers: Mutex::new(Vec::new()),
            pending_confirms: Mutex::new(HashMap::new()),
            confirm_seq: Mutex::new(0),
            today_events: Mutex::new(Vec::new()),
            frontend_ready: AtomicBool::new(false),
            chat_busy: AtomicBool::new(false),
            lag_events: Mutex::new(Vec::new()),
            resume_events: Mutex::new(Vec::new()),
            thought_busy: AtomicBool::new(false),
        }
    }
}
