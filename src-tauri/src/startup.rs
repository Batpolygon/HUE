/// Enable/disable HUE in the current-user Windows startup list (HKCU Run).
pub fn set_launch_at_startup(enabled: bool) -> Result<(), String> {
    // Dev builds (`tauri dev`) must never repoint the user's real startup entry.
    if cfg!(debug_assertions) {
        return Ok(());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const RUN_VALUE: &str = "HUE";
        const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

        if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let quoted = format!("\"{}\"", exe.to_string_lossy());
            let status = Command::new("reg")
                .args([
                    "add",
                    RUN_KEY,
                    "/v",
                    RUN_VALUE,
                    "/t",
                    "REG_SZ",
                    "/d",
                    &quoted,
                    "/f",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map_err(|e| format!("reg add : {e}"))?;
            if !status.success() {
                return Err("Impossible d'activer le démarrage automatique".into());
            }
        } else {
            let _ = Command::new("reg")
                .args(["delete", RUN_KEY, "/v", RUN_VALUE, "/f"])
                .creation_flags(CREATE_NO_WINDOW)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Err("Démarrage auto non supporté sur cet OS".into())
    }
}
