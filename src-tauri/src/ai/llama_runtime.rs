use crate::util::download::{download_verified, extract_zip_flat};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

/// Pinned llama.cpp Windows CPU build (includes llama-server.exe).
const LLAMA_RELEASE_TAG: &str = "b9909";
const LLAMA_ZIP_URL: &str =
    "https://github.com/ggml-org/llama.cpp/releases/download/b9909/llama-b9909-bin-win-cpu-x64.zip";
/// SHA-256 published by GitHub for that exact asset.
const LLAMA_ZIP_SHA256: &str = "2b8b62913f4f4b4d0b61ac31398c82840120792ab82ceea452731b4c95be353d";

pub fn runtime_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hue")
        .join("llama-runtime")
}

pub fn bundled_server_path() -> PathBuf {
    runtime_dir().join("llama-server.exe")
}

/// Resolve llama-server: bundled cache → PATH → verified auto-download.
pub async fn resolve_llama_server(app: &AppHandle) -> Result<PathBuf, String> {
    let bundled = bundled_server_path();
    if bundled.is_file() {
        return Ok(bundled);
    }

    if let Some(p) = find_on_path() {
        return Ok(PathBuf::from(p));
    }

    let _ = app.emit(
        "hue-status",
        serde_json::json!({
            "message": "Téléchargement de llama-server (llama.cpp)… Une seule fois."
        }),
    );

    let dir = runtime_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let zip_path = dir.join(format!("llama-{LLAMA_RELEASE_TAG}-win-cpu-x64.zip"));
    download_verified(app, LLAMA_ZIP_URL, LLAMA_ZIP_SHA256, &zip_path, "llama-server").await?;
    extract_zip_flat(&zip_path, &dir)?;
    let _ = fs::remove_file(&zip_path);

    if bundled.is_file() {
        let _ = app.emit(
            "hue-status",
            serde_json::json!({ "message": "llama-server prêt." }),
        );
        Ok(bundled)
    } else {
        Err("llama-server.exe introuvable après installation".into())
    }
}

fn find_on_path() -> Option<String> {
    for name in ["llama-server", "llama-server.exe"] {
        if let Ok(output) = crate::util::hidden_command("where").arg(name).output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = text.lines().next() {
                    let p = line.trim();
                    if !p.is_empty() {
                        return Some(p.to_string());
                    }
                }
            }
        }
    }
    None
}
