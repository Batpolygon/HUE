use tauri::WebviewWindow;

/// Overlay windows that must never steal keyboard focus (speech bubble, roaming pet).
pub fn make_non_activating(win: &WebviewWindow) {
    let _ = win.set_focusable(false);
    #[cfg(windows)]
    {
        apply_ws_ex_noactivate(win);
    }
}

/// Allow the main HUD to receive clicks / text input when the user opens dialogue.
pub fn make_focusable(win: &WebviewWindow) {
    let _ = win.set_focusable(true);
}

#[cfg(windows)]
fn apply_ws_ex_noactivate(win: &WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetWindowLongW, GWL_EXSTYLE, WS_EX_NOACTIVATE,
    };

    let Ok(raw) = win.hwnd() else {
        return;
    };
    unsafe {
        let hwnd = HWND(raw.0);
        let style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, style | WS_EX_NOACTIVATE.0 as i32);
    }
}

/// Show without activating (re-assert non-focusable first).
pub fn show_without_focus(win: &WebviewWindow) {
    make_non_activating(win);
    let _ = win.show();
}

/// Roaming: above every normal window (and usually above the taskbar).
pub fn set_above_all(win: &WebviewWindow) {
    let _ = win.set_always_on_top(true);
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        };
        if let Ok(raw) = win.hwnd() {
            unsafe {
                let _ = SetWindowPos(
                    HWND(raw.0),
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
    }
}

/// Peeked: still above normal apps, but tucked under the Windows taskbar of his own screen.
pub fn set_above_windows_below_taskbar(win: &WebviewWindow) {
    set_above_all(win);
    keep_below_taskbar(win);
}

/// Slip just under the taskbar of the monitor HUE is on. Cheap enough to call often: the shell
/// and other topmost apps reshuffle that band, so a one-off placement doesn't survive.
pub fn keep_below_taskbar(win: &WebviewWindow) {
    #[cfg(windows)]
    {
        use windows::core::w;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONEAREST};
        use windows::Win32::UI::WindowsAndMessaging::{
            FindWindowExW, FindWindowW, SetWindowPos, SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOOWNERZORDER, SWP_NOSIZE,
        };
        let Ok(raw) = win.hwnd() else {
            return;
        };
        unsafe {
            let hwnd = HWND(raw.0);
            let here = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            // Primary taskbar, then one Shell_SecondaryTrayWnd per extra screen.
            let mut tray = FindWindowW(w!("Shell_TrayWnd"), None).ok().filter(|t| !t.0.is_null());
            if tray.map_or(true, |t| MonitorFromWindow(t, MONITOR_DEFAULTTONEAREST) != here) {
                let mut after = HWND::default();
                while let Ok(t) = FindWindowExW(HWND::default(), after, w!("Shell_SecondaryTrayWnd"), None) {
                    if t.0.is_null() {
                        break;
                    }
                    if MonitorFromWindow(t, MONITOR_DEFAULTTONEAREST) == here {
                        tray = Some(t);
                        break;
                    }
                    after = t;
                }
            }
            if let Some(tray) = tray {
                // Inserting after a topmost window keeps us in the topmost band.
                let _ = SetWindowPos(
                    hwnd,
                    tray,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
                );
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = win;
    }
}
