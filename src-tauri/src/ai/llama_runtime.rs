use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Emitter};

/// Pinned llama.cpp Windows CPU build (includes llama-server.exe).
const LLAMA_RELEASE_TAG: &str = "b9909";
const LLAMA_ZIP_URL: &str =
    "https://github.com/ggml-org/llama.cpp/releases/download/b9909/llama-b9909-bin-win-cpu-x64.zip";

pub fn runtime_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hue")
        .join("llama-runtime")
}

pub fn bundled_server_path() -> PathBuf {
    runtime_dir().join("llama-server.exe")
}

/// Resolve llama-server: bundled cache → PATH → auto-download.
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

    download_and_install(app).await?;

    if bundled.is_file() {
        Ok(bundled)
    } else {
        Err("llama-server.exe introuvable après installation".into())
    }
}

fn find_on_path() -> Option<String> {
    for name in ["llama-server", "llama-server.exe"] {
        if let Ok(output) = Command::new("where").arg(name).output() {
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

async fn download_and_install(app: &AppHandle) -> Result<(), String> {
    let dir = runtime_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let zip_path = dir.join(format!("llama-{LLAMA_RELEASE_TAG}-win-cpu-x64.zip"));

    let client = reqwest::Client::builder()
        .user_agent("HUE/0.1 (Batpolygon)")
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(LLAMA_ZIP_URL)
        .send()
        .await
        .map_err(|e| format!("téléchargement : {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("téléchargement HTTP {}", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    let _ = app.emit(
        "hue-status",
        serde_json::json!({
            "message": format!(
                "llama-server téléchargé ({:.1} Mo), extraction…",
                bytes.len() as f64 / (1024.0 * 1024.0)
            )
        }),
    );
    let _ = total;

    {
        let mut f = fs::File::create(&zip_path).map_err(|e| e.to_string())?;
        f.write_all(&bytes).map_err(|e| e.to_string())?;
    }

    extract_zip(&zip_path, &dir)?;

    // Flatten: some zips put binaries in a subfolder — copy llama-server.exe (+ dlls) to root
    promote_server_binaries(&dir)?;

    let _ = fs::remove_file(&zip_path);
    let _ = app.emit(
        "hue-status",
        serde_json::json!({ "message": "llama-server prêt." }),
    );
    Ok(())
}

fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let zip = zip_path.to_string_lossy();
        let out = dest.to_string_lossy();
        let status = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
                    zip.replace('\'', "''"),
                    out.replace('\'', "''")
                ),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Expand-Archive a échoué".into());
        }
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = (zip_path, dest);
        Err("extraction zip non supportée sur cet OS".into())
    }
}

fn promote_server_binaries(dir: &Path) -> Result<(), String> {
    let target = dir.join("llama-server.exe");
    if target.is_file() {
        return Ok(());
    }

    // Search recursively for llama-server.exe
    let found = find_file_named(dir, "llama-server.exe")
        .ok_or_else(|| "llama-server.exe absent de l'archive".to_string())?;

    let parent = found.parent().unwrap_or(dir);
    // Copy all exe/dll from that folder into runtime root so deps resolve
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let p = entry.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let lower = name.to_lowercase();
            if lower.ends_with(".exe") || lower.ends_with(".dll") {
                let dest = dir.join(name);
                if !dest.exists() {
                    let _ = fs::copy(&p, &dest);
                }
            }
        }
    }

    if !target.is_file() {
        return Err("échec copie de llama-server.exe".into());
    }
    Ok(())
}

fn find_file_named(root: &Path, name: &str) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).ok()?;
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().and_then(|n| n.to_str()) == Some(name) {
                return Some(p);
            }
        }
    }
    None
}
