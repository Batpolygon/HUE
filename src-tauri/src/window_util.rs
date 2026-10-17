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

/// Peeked: still above normal apps, but tucked under the Windows taskbar.
pub fn set_above_windows_below_taskbar(win: &WebviewWindow) {
    let _ = win.set_always_on_top(true);
    #[cfg(windows)]
    {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            FindWindowW, SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        };
        if let Ok(raw) = win.hwnd() {
            unsafe {
                let hwnd = HWND(raw.0);
                // Join the topmost band first so we stay above regular windows.
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
                // Then sit just below the primary taskbar in that band.
                let tray = FindWindowW(
                    PCWSTR::from_raw(windows::core::w!("Shell_TrayWnd").as_ptr()),
                    PCWSTR::null(),
                );
                if let Ok(tray) = tray {
                    if !tray.0.is_null() {
                    let _ = SetWindowPos(
                        hwnd,
                        tray,
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
    }
    #[cfg(not(windows))]
    {
        let _ = win;
    }
}
