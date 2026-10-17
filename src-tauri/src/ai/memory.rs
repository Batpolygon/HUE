use serde_json::{json, Value};
use std::collections::VecDeque;

const MAX_TURNS: usize = 8;

#[derive(Debug, Clone, Default)]
pub struct ShortMemory {
    /// Recent (user, assistant) pairs for the LLM.
    turns: VecDeque<(String, String)>,
    /// Last foreground window title HUE noticed.
    pub last_app: String,
}

impl ShortMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_last_app(&mut self, title: String) {
        let t = title.trim().to_string();
        if !t.is_empty() {
            self.last_app = t;
        }
    }

    pub fn push_turn(&mut self, user: String, assistant: String) {
        if user.trim().is_empty() && assistant.trim().is_empty() {
            return;
        }
        self.turns.push_back((user, assistant));
        while self.turns.len() > MAX_TURNS {
            self.turns.pop_front();
        }
    }

    /// OpenAI-style prior turns (user/assistant), excluding the current user message.
    pub fn history_messages(&self) -> Vec<Value> {
        let mut out = Vec::with_capacity(self.turns.len() * 2);
        for (u, a) in &self.turns {
            if !u.trim().is_empty() {
                out.push(json!({ "role": "user", "content": truncate(u, 400) }));
            }
            if !a.trim().is_empty() {
                out.push(json!({ "role": "assistant", "content": truncate(a, 500) }));
            }
        }
        out
    }

    /// Extra system context (app focus) — conversation history goes as real messages.
    pub fn prompt_addon(&self) -> String {
        if self.last_app.is_empty() {
            return String::new();
        }
        format!(
            "Dernière application au premier plan remarquée : « {} ».",
            truncate(&self.last_app, 60)
        )
    }
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let mut out: String = t.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}
