//! One OpenAI-compatible client for both backends (remote API or local llama-server),
//! with streaming and a tool-calling loop.

use crate::config::{HueSettings, LlmBackend};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone)]
pub struct LlmTarget {
    pub endpoint: String,
    pub key: String,
    pub model: String,
    pub local: bool,
}

/// Why no target: a friendly message for the bubble.
pub enum NoTarget {
    Message(String),
}

pub async fn resolve_target(app: &AppHandle, settings: &HueSettings) -> Result<Result<LlmTarget, NoTarget>, String> {
    match settings.llm_backend {
        LlmBackend::Api => {
            if settings.api_key.trim().is_empty() {
                return Ok(Err(NoTarget::Message(
                    "Hmm… mon lien API n'est pas configuré. Branche une clé dans ⚙, ou passe en local GGUF.".into(),
                )));
            }
            Ok(Ok(LlmTarget {
                endpoint: settings.api_endpoint.trim_end_matches('/').to_string(),
                key: settings.api_key.clone(),
                model: settings.api_model.clone(),
                local: false,
            }))
        }
        LlmBackend::Local => {
            let path = settings.gguf_path.trim();
            if path.is_empty() {
                return Ok(Err(NoTarget::Message(
                    "Mode local : indique le chemin d'un fichier .gguf dans ⚙ pour que je puisse réfléchir.".into(),
                )));
            }
            let gguf = PathBuf::from(path);
            if !gguf.is_file() {
                return Err(format!("Fichier GGUF introuvable : {path}"));
            }
            let base = crate::ai::llama_embed::ensure_llama_server(app, &gguf).await?;
            Ok(Ok(LlmTarget {
                endpoint: format!("{base}/v1"),
                key: "local".into(),
                model: gguf
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("local-gguf")
                    .to_string(),
                local: true,
            }))
        }
    }
}

/// A target usable right now without side effects (no server start) — for spontaneous thoughts.
pub fn target_if_ready(settings: &HueSettings) -> Option<LlmTarget> {
    match settings.llm_backend {
        LlmBackend::Api if !settings.api_key.trim().is_empty() => Some(LlmTarget {
            endpoint: settings.api_endpoint.trim_end_matches('/').to_string(),
            key: settings.api_key.clone(),
            model: settings.api_model.clone(),
            local: false,
        }),
        LlmBackend::Local => crate::ai::llama_embed::running_base_url().map(|base| LlmTarget {
            endpoint: format!("{base}/v1"),
            key: "local".into(),
            model: "local".into(),
            local: true,
        }),
        _ => None,
    }
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
}

pub async fn stream_fake(app: &AppHandle, text: &str) {
    for word in text.split_whitespace() {
        let _ = app.emit("hue-chat", json!({ "delta": format!("{word} ") }));
        tokio::time::sleep(std::time::Duration::from_millis(45)).await;
    }
    let _ = app.emit("hue-chat", json!({ "done": true }));
}

#[derive(Default, Debug, Clone)]
struct ToolCallAcc {
    id: String,
    name: String,
    args: String,
}

/// Streams the reply into `hue-chat`. Runs tools when the model asks for them.
pub async fn chat_stream(
    app: &AppHandle,
    target: &LlmTarget,
    mut messages: Vec<Value>,
    use_tools: bool,
) -> Result<String, String> {
    let http = client()?;
    let url = format!("{}/chat/completions", target.endpoint);
    let mut tools_enabled = use_tools;
    let mut final_text = String::new();

    for round in 0..5 {
        let mut body = json!({
            "model": target.model,
            "stream": true,
            "messages": messages,
        });
        if tools_enabled {
            body["tools"] = crate::ai::tools::specs();
            body["tool_choice"] = json!("auto");
        }
        let resp = http
            .post(&url)
            .bearer_auth(&target.key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            // Some providers / models reject tools: retry once without them.
            if tools_enabled && status.is_client_error() && round == 0 {
                tools_enabled = false;
                continue;
            }
            return Err(format!("API {status}: {}", crate::util::truncate_chars(&text, 300)));
        }

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut text = String::new();
        let mut calls: Vec<ToolCallAcc> = Vec::new();
        let mut finished = false;

        'outer: while let Some(item) = stream.next().await {
            let chunk = item.map_err(|e| e.to_string())?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer.drain(..=pos);
                if line.is_empty() || line.starts_with(':') {
                    continue;
                }
                let data = line.strip_prefix("data:").map(str::trim).unwrap_or(&line);
                if data == "[DONE]" {
                    finished = true;
                    break 'outer;
                }
                let Ok(v) = serde_json::from_str::<Value>(data) else {
                    continue;
                };
                let Some(choice) = v["choices"].get(0) else {
                    continue;
                };
                let delta = &choice["delta"];
                if let Some(t) = delta["content"].as_str() {
                    if !t.is_empty() {
                        text.push_str(t);
                        let _ = app.emit("hue-chat", json!({ "delta": t }));
                    }
                }
                if let Some(tc) = delta["tool_calls"].as_array() {
                    for c in tc {
                        let idx = c["index"].as_u64().unwrap_or(calls.len() as u64) as usize;
                        while calls.len() <= idx {
                            calls.push(ToolCallAcc::default());
                        }
                        let acc = &mut calls[idx];
                        if let Some(id) = c["id"].as_str() {
                            acc.id = id.to_string();
                        }
                        if let Some(n) = c["function"]["name"].as_str() {
                            acc.name.push_str(n);
                        }
                        if let Some(a) = c["function"]["arguments"].as_str() {
                            acc.args.push_str(a);
                        }
                    }
                }
            }
        }
        let _ = finished;

        final_text.push_str(&text);
        let calls: Vec<ToolCallAcc> = calls.into_iter().filter(|c| !c.name.is_empty()).collect();
        if calls.is_empty() {
            let _ = app.emit("hue-chat", json!({ "done": true }));
            return Ok(final_text);
        }

        // Model asked for tools: record the call, execute, feed results back.
        let tool_calls: Vec<Value> = calls
            .iter()
            .enumerate()
            .map(|(i, c)| {
                json!({
                    "id": if c.id.is_empty() { format!("call_{round}_{i}") } else { c.id.clone() },
                    "type": "function",
                    "function": { "name": c.name, "arguments": if c.args.is_empty() { "{}".to_string() } else { c.args.clone() } }
                })
            })
            .collect();
        messages.push(json!({
            "role": "assistant",
            "content": if text.is_empty() { Value::Null } else { Value::String(text.clone()) },
            "tool_calls": tool_calls,
        }));
        for (i, c) in calls.iter().enumerate() {
            let _ = app.emit(
                "hue-chat",
                json!({ "tool": crate::ai::tools::label(&c.name) }),
            );
            let args: Value = serde_json::from_str(&c.args).unwrap_or(json!({}));
            let result = crate::ai::tools::execute(app, &c.name, &args).await;
            messages.push(json!({
                "role": "tool",
                "tool_call_id": tool_calls[i]["id"],
                "content": result,
            }));
        }
        if !final_text.is_empty() {
            let _ = app.emit("hue-chat", json!({ "delta": " " }));
            final_text.push(' ');
        }
    }
    let _ = app.emit("hue-chat", json!({ "done": true }));
    Ok(final_text)
}

/// Non-streaming single completion (thoughts, internal use).
pub async fn complete(target: &LlmTarget, messages: Vec<Value>, max_tokens: u32) -> Result<String, String> {
    let http = client()?;
    let url = format!("{}/chat/completions", target.endpoint);
    let body = json!({
        "model": target.model,
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": 0.9,
    });
    let resp = http
        .post(&url)
        .bearer_auth(&target.key)
        .timeout(std::time::Duration::from_secs(90))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("API {}", resp.status()));
    }
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(v["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string())
}
