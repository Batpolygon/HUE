use crate::ai::llama_runtime::{resolve_llama_server, runtime_dir};
use crate::config::HueSettings;
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;
use tauri::AppHandle;

static LLAMA_SERVER: OnceLock<Mutex<Option<ServerState>>> = OnceLock::new();

struct ServerState {
    child: Child,
    gguf: PathBuf,
    base_url: String,
}

fn server_slot() -> &'static Mutex<Option<ServerState>> {
    LLAMA_SERVER.get_or_init(|| Mutex::new(None))
}

/// Local GGUF : auto-download llama-server if needed, then stream via OpenAI-compatible API.
pub async fn stream_chat(
    app: AppHandle,
    settings: HueSettings,
    system_prompt: String,
    user_message: String,
    history: Vec<serde_json::Value>,
) -> Result<String, String> {
    let path = settings.gguf_path.trim().to_string();
    if path.is_empty() {
        let reply = format!(
            "Mode local : indique le chemin d'un fichier .gguf dans ⚙. Tu as dit : « {user_message} »."
        );
        crate::ai::api_fallback::stream_fake(&app, &reply).await;
        return Ok(reply);
    }

    let gguf = PathBuf::from(&path);
    if !gguf.is_file() {
        return Err(format!("Fichier GGUF introuvable : {path}"));
    }

    let base_url = ensure_llama_server(&app, &gguf).await?;

    let mut api_settings = settings.clone();
    api_settings.api_endpoint = format!("{base_url}/v1");
    api_settings.api_key = "local".into();
    api_settings.api_model = gguf
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("local-gguf")
        .to_string();

    crate::ai::api_fallback::stream_chat(app, api_settings, system_prompt, user_message, history)
        .await
}

async fn ensure_llama_server(app: &AppHandle, gguf: &Path) -> Result<String, String> {
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

    let port = 18765u16;
    let base_url = format!("http://127.0.0.1:{port}");
    let mut cmd = Command::new(&bin);
    cmd.current_dir(&work_dir)
        .args([
            "-m",
            gguf.to_str().ok_or("chemin GGUF invalide")?,
            "--port",
            &port.to_string(),
            "--host",
            "127.0.0.1",
            "-c",
            "2048",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    // Hide the console window that llama-server.exe would otherwise pop open.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("échec lancement `{}` : {e}", bin.display()))?;

    let client = reqwest::Client::new();
    let mut ready = false;
    for _ in 0..80 {
        tokio::time::sleep(Duration::from_millis(300)).await;
        if matches!(child.try_wait(), Ok(Some(_))) {
            return Err(
                "llama-server s'est arrêté immédiatement (GGUF invalide ou dépendance manquante)"
                    .into(),
            );
        }
        if client
            .get(format!("{base_url}/health"))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
            || client
                .get(format!("{base_url}/v1/models"))
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
