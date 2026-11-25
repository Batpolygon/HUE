use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TalkFrequency {
    Mute,
    Discrete,
    Normal,
    Chatty,
    Hyperactive,
}

impl TalkFrequency {
    pub fn cooldown_secs(self) -> u64 {
        match self {
            TalkFrequency::Mute => u64::MAX,
            TalkFrequency::Discrete => 180,
            TalkFrequency::Normal => 60,
            TalkFrequency::Chatty => 25,
            TalkFrequency::Hyperactive => 8,
        }
    }

    pub fn from_label(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "mute" | "muet" => Some(Self::Mute),
            "discrete" | "discret" => Some(Self::Discrete),
            "normal" => Some(Self::Normal),
            "chatty" | "bavard" => Some(Self::Chatty),
            "hyperactive" | "hyperactif" => Some(Self::Hyperactive),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmBackend {
    Local,
    Api,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MovementMode {
    /// Patrol the whole work area (bounce on edges).
    Fullscreen,
    /// Walk left/right along the taskbar.
    #[default]
    Taskbar,
    /// Stay where placed (no autonomous move).
    Static,
}

impl MovementMode {
    pub fn from_label(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "fullscreen" | "ecran" | "écran" => Some(Self::Fullscreen),
            "taskbar" | "barre" => Some(Self::Taskbar),
            "static" | "statique" | "immobile" => Some(Self::Static),
            _ => None,
        }
    }
}

/// What the peek (rangé) face screen cycles through when idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PeekDisplayMode {
    /// Eyes → clock → PC stats.
    #[default]
    All,
    /// CPU / RAM / disk / GPU / battery only.
    SpecOnly,
    /// Analog/digital clock only.
    ClockOnly,
    /// Classic eyes only.
    EyesOnly,
}

/// Speech-to-text engine for dictation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SttEngine {
    /// whisper.cpp downloaded on first use, 100 % local.
    #[default]
    Local,
    /// OpenAI-compatible `/audio/transcriptions` on the configured API endpoint.
    Api,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SttModel {
    /// ~60 Mo, fast.
    #[default]
    Base,
    /// ~190 Mo, more accurate.
    Small,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HueSettings {
    pub talk_frequency: TalkFrequency,
    pub stealth_auto: bool,
    #[serde(default)]
    pub movement_mode: MovementMode,
    #[serde(default)]
    pub peek_display: PeekDisplayMode,
    pub llm_backend: LlmBackend,
    pub gguf_path: String,
    pub api_endpoint: String,
    /// Kept in the Windows Credential Manager — always empty in settings.json.
    #[serde(default)]
    pub api_key: String,
    pub api_model: String,
    pub animalese_pitch: f32,
    pub animalese_speed: f32,
    #[serde(default = "default_true")]
    pub animalese_enabled: bool,
    /// Launch HUE when the user signs into Windows.
    #[serde(default)]
    pub launch_at_startup: bool,

    /// User birthday: "MM-DD" or "YYYY-MM-DD" (empty = unknown).
    #[serde(default)]
    pub user_birthday: String,
    /// Selected unlocked title prefix (empty = none).
    #[serde(default)]
    pub title_prefix: String,
    /// Selected unlocked title suffix (empty = none).
    #[serde(default)]
    pub title_suffix: String,

    #[serde(default)]
    pub stt_engine: SttEngine,
    #[serde(default)]
    pub stt_model: SttModel,
    /// "fr", "en", … or "auto".
    #[serde(default = "default_lang")]
    pub stt_language: String,

    /// Occasionally let the LLM write a spontaneous thought.
    #[serde(default = "default_true")]
    pub llm_thoughts: bool,
    /// Send the foreground window title to a *remote* API (local GGUF always may use it).
    #[serde(default)]
    pub share_window_title: bool,
    /// Fall asleep after N minutes without keyboard/mouse input (0 = never).
    #[serde(default = "default_sleep")]
    pub sleep_after_min: u32,
    #[serde(default = "default_true")]
    pub seasonal_decor: bool,
    #[serde(default = "default_true")]
    pub particles: bool,
}

fn default_true() -> bool {
    true
}

fn default_lang() -> String {
    "fr".into()
}

fn default_sleep() -> u32 {
    5
}

impl Default for HueSettings {
    fn default() -> Self {
        Self {
            talk_frequency: TalkFrequency::Normal,
            stealth_auto: true,
            movement_mode: MovementMode::Taskbar,
            peek_display: PeekDisplayMode::All,
            llm_backend: LlmBackend::Local,
            gguf_path: String::new(),
            api_endpoint: "https://api.openai.com/v1".into(),
            api_key: String::new(),
            api_model: "gpt-4o-mini".into(),
            animalese_pitch: 1.0,
            animalese_speed: 1.0,
            animalese_enabled: true,
            launch_at_startup: false,
            user_birthday: String::new(),
            title_prefix: String::new(),
            title_suffix: String::new(),
            stt_engine: SttEngine::Local,
            stt_model: SttModel::Base,
            stt_language: default_lang(),
            llm_thoughts: true,
            share_window_title: false,
            sleep_after_min: default_sleep(),
            seasonal_decor: true,
            particles: true,
        }
    }
}

impl HueSettings {
    pub fn config_dir() -> PathBuf {
        let base = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("hue");
        let _ = fs::create_dir_all(&base);
        base
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let mut settings: HueSettings = match fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        };
        if !settings.api_key.trim().is_empty() {
            // Legacy plaintext key → move it into the Credential Manager.
            if crate::secrets::store_api_key(&settings.api_key).is_ok() {
                let _ = settings.save();
            }
        } else {
            settings.api_key = crate::secrets::load_api_key().unwrap_or_default();
        }
        settings
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut on_disk = self.clone();
        match crate::secrets::store_api_key(&self.api_key) {
            Ok(()) => on_disk.api_key.clear(),
            // No secure store on this OS: keep the old behaviour rather than losing the key.
            Err(_) => {}
        }
        let raw = serde_json::to_string_pretty(&on_disk).map_err(|e| e.to_string())?;
        crate::util::write_atomic(&path, raw.as_bytes())
    }

    /// Parsed birthday (month, day).
    pub fn user_birthday_md(&self) -> Option<(u32, u32)> {
        parse_birthday(&self.user_birthday).map(|(_, m, d)| (m, d))
    }
}

/// Accepts "MM-DD", "YYYY-MM-DD", "DD/MM" and "DD/MM/YYYY".
pub fn parse_birthday(raw: &str) -> Option<(Option<i32>, u32, u32)> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    let (y, m, d) = if t.contains('/') {
        let p: Vec<&str> = t.split('/').collect();
        match p.as_slice() {
            [d, m] => (None, m.parse().ok()?, d.parse().ok()?),
            [d, m, y] => (Some(y.parse().ok()?), m.parse().ok()?, d.parse().ok()?),
            _ => return None,
        }
    } else {
        let p: Vec<&str> = t.split('-').collect();
        match p.as_slice() {
            [m, d] => (None, m.parse().ok()?, d.parse().ok()?),
            [y, m, d] => (Some(y.parse().ok()?), m.parse().ok()?, d.parse().ok()?),
            _ => return None,
        }
    };
    // Real calendar check (2024 is a leap year, so 29/02 stays valid without a year).
    if chrono::NaiveDate::from_ymd_opt(y.unwrap_or(2024), m, d).is_none() {
        return None;
    }
    if let Some(year) = y {
        if !(1900..=2100).contains(&year) {
            return None;
        }
    }
    Some((y, m, d))
}

/// Canonical storage form: "MM-DD" or "YYYY-MM-DD" ("" when empty or invalid).
pub fn normalize_birthday(raw: &str) -> String {
    match parse_birthday(raw) {
        Some((Some(y), m, d)) => format!("{y:04}-{m:02}-{d:02}"),
        Some((None, m, d)) => format!("{m:02}-{d:02}"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn birthday_formats() {
        assert_eq!(parse_birthday("03-14"), Some((None, 3, 14)));
        assert_eq!(parse_birthday("1995-12-01"), Some((Some(1995), 12, 1)));
        assert_eq!(parse_birthday("14/03"), Some((None, 3, 14)));
        assert_eq!(parse_birthday("14/03/2001"), Some((Some(2001), 3, 14)));
        assert_eq!(parse_birthday("13-40"), None);
        assert_eq!(parse_birthday("31/02"), None);
        assert_eq!(parse_birthday("29/02"), Some((None, 2, 29)));
        assert_eq!(parse_birthday("29/02/2023"), None);
        assert_eq!(parse_birthday(""), None);
        assert_eq!(normalize_birthday("4/7/1990"), "1990-07-04");
        assert_eq!(normalize_birthday("14/03"), "03-14");
    }

    #[test]
    fn old_settings_still_load() {
        let raw = r#"{"talk_frequency":"normal","stealth_auto":true,"llm_backend":"api",
            "gguf_path":"","api_endpoint":"x","api_key":"","api_model":"m",
            "animalese_pitch":1.0,"animalese_speed":1.0}"#;
        let s: HueSettings = serde_json::from_str(raw).unwrap();
        assert_eq!(s.sleep_after_min, 5);
        assert!(s.llm_thoughts);
        assert_eq!(s.stt_language, "fr");
    }
}
