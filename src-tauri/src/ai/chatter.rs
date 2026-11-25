use rand::Rng;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use crate::ai::lines::{self, LineCtx};
use crate::config::TalkFrequency;
use crate::physics::HueState;
use crate::watcher::live_stats::LiveStats;

/// Everything chatter needs for one decision.
pub struct ChatterInput<'a> {
    pub frequency: TalkFrequency,
    pub concealed: bool,
    pub interacting: bool,
    pub speaking: bool,
    pub sleeping: bool,
    pub mood_state: HueState,
    pub dizzy: bool,
    pub sociability: f64,
    pub stats: &'a LiveStats,
    pub lines: &'a LineCtx<'a>,
    /// Seasonal lines available today.
    pub event_lines: &'a [String],
}

/// Spontaneous lines + reactions to the foreground window title.
pub struct Chatter {
    last_spoke: Instant,
    last_title: String,
    recent: VecDeque<String>,
    last_stats_kind: HashMap<&'static str, Instant>,
    last_event_line: Instant,
}

impl Chatter {
    pub fn new() -> Self {
        Self {
            last_spoke: Instant::now() - Duration::from_secs(30),
            last_title: String::new(),
            recent: VecDeque::with_capacity(32),
            last_stats_kind: HashMap::new(),
            last_event_line: Instant::now() - Duration::from_secs(3600),
        }
    }

    pub fn note_spoke(&mut self) {
        self.last_spoke = Instant::now();
    }

    pub fn cooldown(&self, frequency: TalkFrequency, sociability: f64, dizzy: bool) -> Duration {
        let base = frequency.cooldown_secs().min(600) as f64;
        // Sociable machines talk up to ~35 % more, shy ones ~35 % less.
        let factor = (1.35 - sociability * 0.7).clamp(0.6, 1.4);
        let mut secs = base * factor;
        if dizzy {
            secs = (secs / 2.0).max(8.0);
        }
        Duration::from_secs_f64(secs)
    }

    pub fn ready(&self, frequency: TalkFrequency, sociability: f64, dizzy: bool) -> bool {
        frequency != TalkFrequency::Mute && self.last_spoke.elapsed() >= self.cooldown(frequency, sociability, dizzy)
    }

    pub fn maybe_line(&mut self, inp: &ChatterInput) -> Option<String> {
        if inp.concealed || inp.interacting || inp.speaking || inp.sleeping {
            return None;
        }
        if !self.ready(inp.frequency, inp.sociability, inp.dizzy) {
            return None;
        }
        let mut rng = rand::thread_rng();
        let c = inp.lines;

        if inp.dizzy && rng.gen_bool(0.72) {
            return self.commit(lines::dizzy_line(c, &mut rng));
        }

        if let Some(line) = self.stats_line(inp.stats, c, &mut rng) {
            return self.commit(line);
        }

        if !inp.event_lines.is_empty()
            && self.last_event_line.elapsed() > Duration::from_secs(25 * 60)
            && rng.gen_bool(0.5)
        {
            self.last_event_line = Instant::now();
            let l = inp.event_lines[rng.gen_range(0..inp.event_lines.len())].clone();
            return self.commit(l);
        }

        let title = foreground_window_title().unwrap_or_default();
        let title_trim = title.trim().to_string();
        if !title_trim.is_empty() && title_trim != self.last_title && rng.gen_bool(0.5) {
            self.last_title = title_trim.clone();
            return self.commit(lines::title_reaction(&title_trim, c, &mut rng));
        }
        if !title_trim.is_empty() {
            self.last_title = title_trim;
        }

        let mood_key = match inp.mood_state {
            HueState::Heavy => "heavy",
            HueState::Tired => "tired",
            HueState::Panic => "panic",
            HueState::Joy => "joy",
            HueState::Dizzy => "dizzy",
            _ => "",
        };
        if !mood_key.is_empty() && rng.gen_bool(0.4) {
            if let Some(line) = lines::mood_line(c, mood_key, &mut rng) {
                return self.commit(line);
            }
        }

        if rng.gen_bool(0.45) || c.peeking {
            return self.commit(lines::ambient(c, &mut rng));
        }
        None
    }

    pub fn last_title(&self) -> &str {
        &self.last_title
    }

    fn stats_line(&mut self, stats: &LiveStats, c: &LineCtx, rng: &mut impl Rng) -> Option<String> {
        let now = Instant::now();
        let ok = |key: &'static str, map: &mut HashMap<&'static str, Instant>| {
            let free = map
                .get(key)
                .map(|t| now.duration_since(*t) >= Duration::from_secs(120))
                .unwrap_or(true);
            if free {
                map.insert(key, now);
            }
            free
        };
        let temp = stats.cpu_temp.unwrap_or(0.0).max(stats.gpu_temp.unwrap_or(0.0));
        if temp >= 80.0 && rng.gen_bool(0.5) && ok("temp", &mut self.last_stats_kind) {
            return Some(lines::stats_line("temp", temp, c, rng));
        }
        if stats.cpu_pct >= 85.0 && rng.gen_bool(0.55) && ok("cpu", &mut self.last_stats_kind) {
            return Some(lines::stats_line("cpu", stats.cpu_pct, c, rng));
        }
        if stats.ram_pct >= 88.0 && rng.gen_bool(0.5) && ok("ram", &mut self.last_stats_kind) {
            return Some(lines::stats_line("ram", stats.ram_pct, c, rng));
        }
        if stats.disk_pct >= 92.0 && rng.gen_bool(0.3) && ok("disk", &mut self.last_stats_kind) {
            return Some(lines::stats_line("disk", stats.disk_pct, c, rng));
        }
        if stats.gpu_pct >= 80.0 && rng.gen_bool(0.4) && ok("gpu", &mut self.last_stats_kind) {
            return Some(lines::stats_line("gpu", stats.gpu_pct, c, rng));
        }
        None
    }

    fn commit(&mut self, line: String) -> Option<String> {
        if self.recent.iter().any(|r| r == &line) {
            return None;
        }
        self.recent.push_back(line.clone());
        while self.recent.len() > 32 {
            self.recent.pop_front();
        }
        self.last_spoke = Instant::now();
        Some(line)
    }
}

pub fn foreground_window_title() -> Option<String> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
        };
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }
            let len = GetWindowTextLengthW(hwnd);
            if len <= 0 {
                return None;
            }
            let mut buf = vec![0u16; (len + 1) as usize];
            let read = GetWindowTextW(hwnd, &mut buf);
            if read <= 0 {
                return None;
            }
            Some(String::from_utf16_lossy(&buf[..read as usize]))
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}
