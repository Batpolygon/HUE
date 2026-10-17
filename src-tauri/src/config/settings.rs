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
    pub api_key: String,
    pub api_model: String,
    pub animalese_pitch: f32,
    pub animalese_speed: f32,
    #[serde(default = "default_true")]
    pub animalese_enabled: bool,
    /// Launch HUE when the user signs into Windows.
    #[serde(default)]
    pub launch_at_startup: bool,
}

fn default_true() -> bool {
    true
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
        }
    }
}

impl HueSettings {
    pub fn config_path() -> PathBuf {
        let base = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("hue");
        let _ = fs::create_dir_all(&base);
        base.join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        match fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let raw = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, raw).map_err(|e| e.to_string())
    }
}
