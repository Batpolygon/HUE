//! Persistent memory (`%APPDATA%\hue\memory.json`): recent conversation, long-term
//! facts about the user, lifetime counters (for achievements) and calendar markers.

use crate::config::HueSettings;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const MAX_TURNS: usize = 12;
const MAX_FACTS: usize = 80;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Fact {
    pub text: String,
    /// Unix seconds.
    pub added: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Stats {
    pub pets: u32,
    pub chats: u32,
    pub files_read: u32,
    pub tosses: u32,
    pub dizzies: u32,
    pub lags: u32,
    pub naps: u32,
    pub peeks: u32,
    pub timers_done: u32,
    pub tools_used: u32,
    pub voice_uses: u32,
    pub launches: u32,
    pub night_chats: u32,
    pub early_greets: u32,
    pub minutes_alive: u32,
    pub days_seen: u32,
    pub streak_days: u32,
    pub best_streak: u32,
    pub birthdays: u32,
    pub seasonal: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PersistentMemory {
    pub turns: VecDeque<(String, String)>,
    pub facts: Vec<Fact>,
    pub last_app: String,
    pub first_met: Option<i64>,
    pub last_seen: Option<i64>,
    /// "YYYY-MM-DD" of the last day counted in `days_seen`.
    pub last_day: String,
    pub stats: Stats,
    /// achievement id → unlocked at (unix).
    pub achievements: BTreeMap<String, i64>,
    /// event id → "YYYY" (or day) already celebrated, to say it once.
    pub celebrated: BTreeMap<String, String>,
    #[serde(skip)]
    dirty: bool,
    #[serde(skip)]
    last_save: Option<Instant>,
}

impl PersistentMemory {
    pub fn path() -> PathBuf {
        HueSettings::config_dir().join("memory.json")
    }

    pub fn load() -> Self {
        fs::read_to_string(Self::path())
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn mark(&mut self) {
        self.dirty = true;
    }

    /// Debounced save (call often; writes at most every 20 s unless forced).
    pub fn save_if_needed(&mut self, force: bool) {
        if !self.dirty {
            return;
        }
        let due = self
            .last_save
            .map(|t| t.elapsed() > Duration::from_secs(20))
            .unwrap_or(true);
        if !(force || due) {
            return;
        }
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            if crate::util::write_atomic(&Self::path(), raw.as_bytes()).is_ok() {
                self.dirty = false;
                self.last_save = Some(Instant::now());
            }
        }
    }

    /// Called once per launch. Returns (first launch ever, days since last seen).
    pub fn on_launch(&mut self) -> (bool, Option<i64>) {
        let now = crate::util::now_unix();
        let first = self.first_met.is_none();
        if first {
            self.first_met = Some(now);
        }
        let days_away = self.last_seen.map(|t| (now - t) / 86_400);
        self.stats.launches += 1;
        self.touch_day();
        self.last_seen = Some(now);
        self.mark();
        (first, days_away)
    }

    /// Counts a new calendar day and the consecutive-day streak.
    pub fn touch_day(&mut self) {
        let today = chrono::Local::now().date_naive();
        let key = today.format("%Y-%m-%d").to_string();
        if self.last_day == key {
            return;
        }
        let yesterday = (today - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
        self.stats.streak_days = if self.last_day == yesterday {
            self.stats.streak_days + 1
        } else {
            1
        };
        self.stats.best_streak = self.stats.best_streak.max(self.stats.streak_days);
        self.stats.days_seen += 1;
        self.last_day = key;
        self.mark();
    }

    pub fn set_last_app(&mut self, title: String) {
        let t = title.trim().to_string();
        if !t.is_empty() && t != self.last_app {
            self.last_app = t;
        }
    }

    pub fn push_turn(&mut self, user: String, assistant: String) {
        if user.trim().is_empty() && assistant.trim().is_empty() {
            return;
        }
        self.turns.push_back((
            crate::util::truncate_chars(&user, 600),
            crate::util::truncate_chars(&assistant, 800),
        ));
        while self.turns.len() > MAX_TURNS {
            self.turns.pop_front();
        }
        self.mark();
    }

    pub fn remember(&mut self, fact: &str) -> bool {
        let f = fact.trim();
        if f.is_empty() {
            return false;
        }
        if self.facts.iter().any(|x| x.text.eq_ignore_ascii_case(f)) {
            return false;
        }
        self.facts.push(Fact {
            text: crate::util::truncate_chars(f, 240),
            added: crate::util::now_unix(),
        });
        if self.facts.len() > MAX_FACTS {
            self.facts.remove(0);
        }
        self.mark();
        true
    }

    /// Removes facts containing `query` (case-insensitive). Returns how many.
    pub fn forget(&mut self, query: &str) -> usize {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return 0;
        }
        let before = self.facts.len();
        self.facts.retain(|f| !f.text.to_lowercase().contains(&q));
        let n = before - self.facts.len();
        if n > 0 {
            self.mark();
        }
        n
    }

    pub fn forget_index(&mut self, idx: usize) {
        if idx < self.facts.len() {
            self.facts.remove(idx);
            self.mark();
        }
    }

    pub fn clear_conversation(&mut self) {
        self.turns.clear();
        self.mark();
    }

    /// OpenAI-style prior turns (user/assistant), excluding the current user message.
    pub fn history_messages(&self) -> Vec<Value> {
        let mut out = Vec::with_capacity(self.turns.len() * 2);
        for (u, a) in &self.turns {
            if !u.trim().is_empty() {
                out.push(json!({ "role": "user", "content": u }));
            }
            if !a.trim().is_empty() {
                out.push(json!({ "role": "assistant", "content": a }));
            }
        }
        out
    }

    /// Long-term context for the system prompt.
    pub fn prompt_addon(&self, include_app: bool) -> String {
        let mut out = String::new();
        if let Some(first) = self.first_met {
            let days = (crate::util::now_unix() - first) / 86_400;
            out.push_str(&format!(
                "Vous vous connaissez depuis {days} jour(s) ; {} jour(s) passés ensemble, série actuelle {}.\n",
                self.stats.days_seen, self.stats.streak_days
            ));
        }
        if !self.facts.is_empty() {
            out.push_str("Ce que tu sais de l'utilisateur (souvenirs persistants) :\n");
            for f in self.facts.iter().rev().take(25) {
                out.push_str("- ");
                out.push_str(&f.text);
                out.push('\n');
            }
        }
        if include_app && !self.last_app.is_empty() {
            out.push_str(&format!(
                "Dernière application au premier plan remarquée : « {} ».\n",
                crate::util::truncate_chars(&self.last_app, 60)
            ));
        }
        out
    }

    /// True once per key+period; marks it.
    pub fn celebrate_once(&mut self, key: &str, period: &str) -> bool {
        if self.celebrated.get(key).map(|p| p == period).unwrap_or(false) {
            return false;
        }
        self.celebrated.insert(key.to_string(), period.to_string());
        self.mark();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facts_dedupe_and_forget() {
        let mut m = PersistentMemory::default();
        assert!(m.remember("Aime les chats"));
        assert!(!m.remember("aime les chats"));
        assert!(m.remember("Travaille sur HUE"));
        assert_eq!(m.forget("chats"), 1);
        assert_eq!(m.facts.len(), 1);
    }

    #[test]
    fn launch_counts_days() {
        let mut m = PersistentMemory::default();
        let (first, away) = m.on_launch();
        assert!(first);
        assert_eq!(away, None);
        assert_eq!(m.stats.days_seen, 1);
        let (first2, away2) = m.on_launch();
        assert!(!first2);
        assert_eq!(away2, Some(0));
        assert_eq!(m.stats.days_seen, 1, "same day counted once");
    }

    #[test]
    fn roundtrip_json() {
        let mut m = PersistentMemory::default();
        m.push_turn("salut".into(), "coucou".into());
        m.stats.pets = 3;
        let raw = serde_json::to_string(&m).unwrap();
        let back: PersistentMemory = serde_json::from_str(&raw).unwrap();
        assert_eq!(back.turns.len(), 1);
        assert_eq!(back.stats.pets, 3);
    }
}
