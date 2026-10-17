use crate::config::HueSettings;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::json;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Deserialize)]
struct ChatChunk {
    choices: Option<Vec<Choice>>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    delta: Option<Delta>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
}

/// Returns the full assistant reply (for short-term memory).
pub async fn stream_chat(
    app: AppHandle,
    settings: HueSettings,
    system_prompt: String,
    user_message: String,
    history: Vec<serde_json::Value>,
) -> Result<String, String> {
    if settings.api_key.trim().is_empty() {
        let reply = format!(
            "Hmm… mon lien API n'est pas configuré. Tu as dit : « {} ». Branche une clé dans ⚙️, ou passe en local GGUF.",
            user_message
        );
        stream_fake(&app, &reply).await;
        return Ok(reply);
    }

    let url = format!(
        "{}/chat/completions",
        settings.api_endpoint.trim_end_matches('/')
    );

    let mut messages = Vec::with_capacity(2 + history.len());
    messages.push(json!({ "role": "system", "content": system_prompt }));
    messages.extend(history);
    messages.push(json!({ "role": "user", "content": user_message }));

    let client = reqwest::Client::new();
    let body = json!({
        "model": settings.api_model,
        "stream": true,
        "messages": messages
    });

    let response = client
        .post(&url)
        .bearer_auth(&settings.api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("API {status}: {text}"));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut full = String::new();

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| e.to_string())?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            buffer = buffer[pos + 1..].to_string();
            if line.is_empty() || line.starts_with(':') {
                continue;
            }
            let data = line.strip_prefix("data: ").unwrap_or(&line);
            if data.trim() == "[DONE]" {
                let _ = app.emit("hue-chat", json!({ "done": true }));
                return Ok(full);
            }
            if let Ok(parsed) = serde_json::from_str::<ChatChunk>(data) {
                if let Some(choices) = parsed.choices {
                    for choice in choices {
                        if let Some(delta) = choice.delta.and_then(|d| d.content) {
                            full.push_str(&delta);
                            let _ = app.emit("hue-chat", json!({ "delta": delta }));
                        }
                    }
                }
            }
        }
    }

    let _ = app.emit("hue-chat", json!({ "done": true }));
    Ok(full)
}

pub async fn stream_fake(app: &AppHandle, text: &str) {
    for word in text.split_whitespace() {
        let _ = app.emit("hue-chat", json!({ "delta": format!("{word} ") }));
        tokio::time::sleep(std::time::Duration::from_millis(45)).await;
    }
    let _ = app.emit("hue-chat", json!({ "done": true }));
}
