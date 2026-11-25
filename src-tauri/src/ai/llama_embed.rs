use crate::ai::llama_runtime::{resolve_llama_server, runtime_dir};
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::OnceLock;
use std::time::Duration;
use tauri::AppHandle;

static LLAMA_SERVER: OnceLock<Mutex<Option<ServerState>>> = OnceLock::new();
/// Serialises concurrent first starts (chat + file drop at the same time).
static START_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const PORT: u16 = 18765;
/// Room for a document excerpt + history; ~512 Mo of KV cache for an 8B model.
const CONTEXT: &str = "4096";

struct ServerState {
    child: Child,
    gguf: PathBuf,
    base_url: String,
}

fn server_slot() -> &'static Mutex<Option<ServerState>> {
    LLAMA_SERVER.get_or_init(|| Mutex::new(None))
}

/// Base URL if a server is already up (never starts one).
pub fn running_base_url() -> Option<String> {
    let mut slot = server_slot().lock();
    let state = slot.as_mut()?;
    match state.child.try_wait() {
        Ok(None) => Some(state.base_url.clone()),
        _ => {
            *slot = None;
            None
        }
    }
}

pub fn shutdown() {
    if let Some(mut s) = server_slot().lock().take() {
        let _ = s.child.kill();
    }
}

pub async fn ensure_llama_server(app: &AppHandle, gguf: &Path) -> Result<String, String> {
    let _guard = START_LOCK.lock().await;
    {
        let mut slot = server_slot().lock();
        if let Some(state) = slot.as_mut() {
            if state.gguf == gguf {
                match state.child.try_wait() {
                    Ok(None) => return Ok(state.base_url.clone()),
                    _ => {
                        let _ = state.child.kill();
                        *slot = None;
                    }
                }
            } else {
                let _ = state.child.kill();
                *slot = None;
            }
        }
    }

    let bin = resolve_llama_server(app).await?;
    let work_dir = bin
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(runtime_dir);

    let base_url = format!("http://127.0.0.1:{PORT}");
    let mut cmd = crate::util::hidden_command(&bin.to_string_lossy());
    cmd.current_dir(&work_dir)
        .args([
            "-m",
            gguf.to_str().ok_or("chemin GGUF invalide")?,
            "--port",
            &PORT.to_string(),
            "--host",
            "127.0.0.1",
            "-c",
            CONTEXT,
            // Chat templates with tool-calling support.
            "--jinja",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("échec lancement `{}` : {e}", bin.display()))?;

    let client = reqwest::Client::new();
    let mut ready = false;
    for _ in 0..120 {
        tokio::time::sleep(Duration::from_millis(300)).await;
        if matches!(child.try_wait(), Ok(Some(_))) {
            return Err(
                "llama-server s'est arrêté immédiatement (GGUF invalide ou dépendance manquante)".into(),
            );
        }
        if client
            .get(format!("{base_url}/health"))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
        {
            ready = true;
            break;
        }
    }

    if !ready {
        let _ = child.kill();
        return Err("llama-server ne répond pas (timeout au démarrage)".into());
    }

    *server_slot().lock() = Some(ServerState {
        child,
        gguf: gguf.to_path_buf(),
        base_url: base_url.clone(),
    });

    Ok(base_url)
}
