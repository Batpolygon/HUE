
const CAPTURE_PROCESSES: &[&str] = &[
    "obs64.exe",
    "obs32.exe",
    "obs.exe",
    "streamlabs obs.exe",
    "streamlabs obs",
    "xsplit.core.exe",
    "xsplit.core",
];

const GAME_PROCESSES: &[&str] = &[
    "minecraft.exe",
    "minecraftlauncher.exe",
    "valorant.exe",
    "valorant-win64-shipping.exe",
    "leagueclient.exe",
    "league of legends.exe",
    "fortniteclient-win64-shipping.exe",
    "cs2.exe",
    "csgo.exe",
    "overwatch.exe",
    "dota2.exe",
    "eldenring.exe",
    "gta5.exe",
    "rdr2.exe",
    "wow.exe",
    "ffxiv_dx11.exe",
    "destiny2.exe",
    "apex_legends.exe",
    "r5apex.exe",
    "rocketleague.exe",
    "pathofexile.exe",
    "pathofexile_x64.exe",
    "cyberpunk2077.exe",
    "witcher3.exe",
    "hl2.exe",
    "gameoverlayui.exe",
];

/// OBS / capture tools — hide while they run (even in background).
fn capture_tool_running() -> bool {
    super::sysmon::any_process_named(CAPTURE_PROCESSES)
}

/// Foreground is a known game process (not merely "a game is installed somewhere").
fn foreground_is_known_game(exclude_hwnd: Option<isize>) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId,
        };

        unsafe {
            let fg = GetForegroundWindow();
            if fg.0.is_null() {
                return false;
            }
            if let Some(ex) = exclude_hwnd {
                if fg.0 as isize == ex {
                    return false;
                }
            }
            let mut pid = 0u32;
            let _ = GetWindowThreadProcessId(HWND(fg.0), Some(&mut pid));
            if pid == 0 {
                return false;
            }
            if let Some(name) = super::sysmon::process_name(pid) {
                return GAME_PROCESSES.iter().any(|p| name == *p);
            }
        }
        false
    }
    #[cfg(not(windows))]
    {
        let _ = exclude_hwnd;
        false
    }
}

/// Desktop / shell surfaces look "fullscreen" but must never hide HUE.
#[cfg(windows)]
fn is_desktop_or_shell(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;

    let mut buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    if len <= 0 {
        return false;
    }
    let class = String::from_utf16_lossy(&buf[..len as usize]);
    matches!(
        class.as_str(),
        "Progman"
            | "WorkerW"
            | "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "DV2ControlHost"
            | "ForegroundStaging"
            | "ApplicationFrameWindow" // sometimes empty desktop focus quirks
    ) || class.starts_with("Windows.UI.")
}

/// Strict fullscreen only: covers the whole monitor (over the taskbar).
/// Maximized apps sit in the work area and must NOT trigger this.
/// Desktop (Progman/WorkerW) must NOT trigger this.
pub fn foreground_is_fullscreen(exclude_hwnd: Option<isize>) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowRect, IsIconic, IsWindowVisible,
        };

        unsafe {
            let fg = GetForegroundWindow();
            if fg.0.is_null() {
                return false;
            }
            if let Some(ex) = exclude_hwnd {
                if fg.0 as isize == ex {
                    return false;
                }
            }
            // Clicking the desktop focuses Progman/WorkerW — full monitor size, not a game.
            if is_desktop_or_shell(fg) {
                return false;
            }
            if !IsWindowVisible(fg).as_bool() || IsIconic(fg).as_bool() {
                return false;
            }

            let mut wr = RECT::default();
            if GetWindowRect(fg, &mut wr).is_err() {
                return false;
            }

            let monitor = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                rcMonitor: RECT::default(),
                rcWork: RECT::default(),
                dwFlags: 0,
            };
            if !GetMonitorInfoW(monitor, &mut mi).as_bool() {
                return false;
            }

            let mon = mi.rcMonitor;
            let work = mi.rcWork;
            let mon_w = mon.right - mon.left;
            let mon_h = mon.bottom - mon.top;
            let win_w = wr.right - wr.left;
            let win_h = wr.bottom - wr.top;

            // Maximized window ≈ work area (leaves taskbar). Not fullscreen.
            let taskbar = (mon.bottom - work.bottom)
                .abs()
                .max((work.top - mon.top).abs())
                .max((mon.right - work.right).abs())
                .max((work.left - mon.left).abs());
            if taskbar > 24 {
                let in_work = wr.left >= work.left - 8
                    && wr.top >= work.top - 8
                    && wr.right <= work.right + 8
                    && wr.bottom <= work.bottom + 8;
                if in_work {
                    return false;
                }
            }

            // True fullscreen: essentially equal to the full monitor rect.
            win_w >= mon_w - 4
                && win_h >= mon_h - 4
                && wr.left <= mon.left + 4
                && wr.top <= mon.top + 4
                && wr.right >= mon.right - 4
                && wr.bottom >= mon.bottom - 4
        }
    }
    #[cfg(not(windows))]
    {
        let _ = exclude_hwnd;
        false
    }
}

/// Only when discrete mode is ON: hide for real fullscreen / focused games / OBS.
pub fn should_conceal(stealth_auto: bool, exclude_hwnd: Option<isize>) -> bool {
    if !stealth_auto {
        return false;
    }
    foreground_is_fullscreen(exclude_hwnd)
        || foreground_is_known_game(exclude_hwnd)
        || capture_tool_running()
}

/// Desktop work area (excludes taskbar) for the monitor under the cursor / primary.
pub fn desktop_work_area() -> Option<crate::physics::WorkArea> {
    let (x, y) = cursor_screen_pos().unwrap_or((0.0, 0.0));
    work_area_at(x, y)
}

/// Work area (excludes taskbar) of the monitor containing `(x, y)` — the nearest one if the
/// point is off every screen.
pub fn work_area_at(x: f64, y: f64) -> Option<crate::physics::WorkArea> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{POINT, RECT};
        use windows::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        };

        unsafe {
            let pt = POINT {
                x: x.round() as i32,
                y: y.round() as i32,
            };
            let monitor = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                rcMonitor: RECT::default(),
                rcWork: RECT::default(),
                dwFlags: 0,
            };
            if !GetMonitorInfoW(monitor, &mut mi).as_bool() {
                return None;
            }
            let w = mi.rcWork;
            Some(crate::physics::WorkArea {
                x: w.left as f64,
                y: w.top as f64,
                width: (w.right - w.left) as f64,
                height: (w.bottom - w.top) as f64,
            })
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (x, y);
        None
    }
}

pub fn cursor_screen_pos() -> Option<(f64, f64)> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        unsafe {
            let mut pt = POINT::default();
            if GetCursorPos(&mut pt).is_ok() {
                return Some((pt.x as f64, pt.y as f64));
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        None
    }
}
