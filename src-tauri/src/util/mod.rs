pub mod download;

use std::fs;
use std::path::Path;

/// Write to a temp file then rename — a crash never leaves a half-written JSON.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

pub fn now_unix() -> i64 {
    chrono::Local::now().timestamp()
}

/// Truncate on a char boundary, adding an ellipsis.
pub fn truncate_chars(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let mut out: String = t.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(windows)]
pub fn hidden_command(program: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut c = std::process::Command::new(program);
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

#[cfg(not(windows))]
pub fn hidden_command(program: &str) -> std::process::Command {
    std::process::Command::new(program)
}
