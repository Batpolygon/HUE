//! Shared, persistent process view. One `System` lives here and is refreshed on a
//! slow cadence — fresh `System::new()` instances report 0 % CPU per process and
//! are expensive to rebuild every frame.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

const REFRESH_EVERY: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProcInfo {
    pub name: String,
    /// Share of the whole machine (0–100), not of one core.
    pub cpu: f32,
    pub ram_mb: u64,
}

struct Inner {
    sys: System,
    last: Option<Instant>,
    /// Two refreshes are needed before CPU numbers mean anything.
    primed: bool,
    names: HashMap<u32, String>,
    cores: f32,
}

static MON: Mutex<Option<Inner>> = Mutex::new(None);

const IGNORED: &[&str] = &[
    "system idle process",
    "idle",
    "system",
    "hue.exe",
    "hue",
    "llama-server.exe",
    "whisper-cli.exe",
    "memory compression",
    "registry",
];

/// Refresh the process table if it is older than the cadence. Cheap otherwise.
pub fn tick() {
    let mut g = MON.lock();
    let inner = g.get_or_insert_with(|| {
        let sys = System::new();
        let cores = std::thread::available_parallelism()
            .map(|n| n.get() as f32)
            .unwrap_or(4.0);
        Inner {
            sys,
            last: None,
            primed: false,
            names: HashMap::new(),
            cores,
        }
    });
    if inner.last.map(|t| t.elapsed() < REFRESH_EVERY).unwrap_or(false) {
        return;
    }
    let was_refreshed = inner.last.is_some();
    inner.sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cpu().with_memory(),
    );
    inner.last = Some(Instant::now());
    inner.primed = was_refreshed;
    inner.names = inner
        .sys
        .processes()
        .iter()
        .map(|(pid, p)| (pid.as_u32(), p.name().to_string_lossy().to_lowercase()))
        .collect();
}

pub fn top_processes(n: usize, by_ram: bool) -> Vec<ProcInfo> {
    let g = MON.lock();
    let Some(inner) = g.as_ref() else {
        return Vec::new();
    };
    let mut list: Vec<ProcInfo> = inner
        .sys
        .processes()
        .values()
        .filter_map(|p| {
            let name = p.name().to_string_lossy().to_string();
            if IGNORED.contains(&name.to_lowercase().as_str()) {
                return None;
            }
            Some(ProcInfo {
                name,
                cpu: if inner.primed {
                    p.cpu_usage() / inner.cores
                } else {
                    0.0
                },
                ram_mb: p.memory() / (1024 * 1024),
            })
        })
        .collect();
    if by_ram {
        list.sort_by(|a, b| b.ram_mb.cmp(&a.ram_mb));
    } else {
        list.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
    }
    // Merge multi-process apps (chrome.exe × 30) into one entry.
    let mut merged: Vec<ProcInfo> = Vec::new();
    for p in list {
        if let Some(m) = merged.iter_mut().find(|m| m.name.eq_ignore_ascii_case(&p.name)) {
            m.cpu += p.cpu;
            m.ram_mb += p.ram_mb;
        } else {
            merged.push(p);
        }
    }
    if by_ram {
        merged.sort_by(|a, b| b.ram_mb.cmp(&a.ram_mb));
    } else {
        merged.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
    }
    merged.truncate(n);
    merged
}

pub fn top_cpu_process() -> Option<ProcInfo> {
    let primed = MON.lock().as_ref().map(|i| i.primed).unwrap_or(false);
    if !primed {
        return None;
    }
    top_processes(1, false).into_iter().next()
}

pub fn any_process_named(names: &[&str]) -> bool {
    let g = MON.lock();
    g.as_ref()
        .map(|i| i.names.values().any(|n| names.iter().any(|x| n == x)))
        .unwrap_or(false)
}

pub fn process_name(pid: u32) -> Option<String> {
    let mut g = MON.lock();
    let inner = g.as_mut()?;
    if let Some(n) = inner.names.get(&pid) {
        return Some(n.clone());
    }
    // Brand-new process (just launched game): refresh that pid only.
    let p = Pid::from_u32(pid);
    inner
        .sys
        .refresh_processes(ProcessesToUpdate::Some(&[p]), true);
    let name = inner.sys.process(p)?.name().to_string_lossy().to_lowercase();
    inner.names.insert(pid, name.clone());
    Some(name)
}

/// Seconds since the last keyboard / mouse input anywhere on the system.
pub fn user_idle_secs() -> u64 {
    #[cfg(windows)]
    {
        use windows::Win32::System::SystemInformation::GetTickCount;
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
        unsafe {
            let mut info = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            if GetLastInputInfo(&mut info).as_bool() {
                let now = GetTickCount();
                return (now.wrapping_sub(info.dwTime) / 1000) as u64;
            }
        }
    }
    0
}

/// Foreground window that stopped pumping messages ("Ne répond pas").
pub fn foreground_hung(exclude_hwnd: Option<isize>) -> Option<String> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsHungAppWindow};
        unsafe {
            let fg = GetForegroundWindow();
            if fg.0.is_null() {
                return None;
            }
            if exclude_hwnd == Some(fg.0 as isize) {
                return None;
            }
            if IsHungAppWindow(fg).as_bool() {
                let title = crate::ai::chatter::foreground_window_title().unwrap_or_default();
                let title = title.replace(" (Ne répond pas)", "").replace(" (Not Responding)", "");
                return Some(if title.trim().is_empty() {
                    "une fenêtre".into()
                } else {
                    title.trim().to_string()
                });
            }
        }
    }
    let _ = exclude_hwnd;
    None
}
