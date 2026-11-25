//! Dictation: native microphone capture (cpal) → 16 kHz mono WAV → whisper.cpp
//! (downloaded + verified on first use) or an OpenAI-compatible transcription API.

use crate::config::{HueSettings, SttEngine, SttModel};
use crate::util::download::{download_verified, extract_zip_flat};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const WHISPER_ZIP_URL: &str =
    "https://github.com/ggml-org/whisper.cpp/releases/download/b5907/whisper-bin-x64.zip";
const WHISPER_ZIP_SHA256: &str = "ecca07b277d943e9b9f4712856a144b857271a6b333104e511e1988e80db582b";

const MODEL_BASE_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base-q5_1.bin";
const MODEL_BASE_SHA256: &str = "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898";
const MODEL_SMALL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin";
const MODEL_SMALL_SHA256: &str = "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb";

const MAX_RECORD: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttStatus {
    pub available: bool,
    pub engine: String,
    pub ready: bool,
    pub recording: bool,
    pub message: String,
}

struct Recording {
    stop_tx: mpsc::Sender<()>,
    handle: std::thread::JoinHandle<Result<(Vec<f32>, u32), String>>,
    started: Instant,
}

static REC: Mutex<Option<Recording>> = Mutex::new(None);

fn runtime_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hue")
        .join("whisper-runtime")
}

fn cli_path() -> PathBuf {
    runtime_dir().join("whisper-cli.exe")
}

fn model_path(m: SttModel) -> PathBuf {
    runtime_dir().join(match m {
        SttModel::Base => "ggml-base-q5_1.bin",
        SttModel::Small => "ggml-small-q5_1.bin",
    })
}

pub fn status(settings: &HueSettings) -> SttStatus {
    let recording = REC.lock().is_some();
    match settings.stt_engine {
        SttEngine::Local => {
            let ready = cli_path().is_file() && model_path(settings.stt_model).is_file();
            SttStatus {
                available: cfg!(windows),
                engine: "local".into(),
                ready,
                recording,
                message: if ready {
                    "Whisper local prêt.".into()
                } else {
                    let size = match settings.stt_model {
                        SttModel::Base => "~70 Mo",
                        SttModel::Small => "~200 Mo",
                    };
                    format!("Au premier usage je télécharge whisper.cpp + le modèle ({size}).")
                },
            }
        }
        SttEngine::Api => SttStatus {
            available: !settings.api_key.trim().is_empty(),
            engine: "api".into(),
            ready: !settings.api_key.trim().is_empty(),
            recording,
            message: if settings.api_key.trim().is_empty() {
                "Dictée via API : configure une clé API dans ⚙.".into()
            } else {
                "Dictée via l'API (whisper-1).".into()
            },
        },
    }
}

/// Start capturing the default microphone. Emits `hue-mic-level` (0–1) for the UI.
pub fn start(app: AppHandle) -> Result<(), String> {
    let mut slot = REC.lock();
    if slot.is_some() {
        return Ok(());
    }
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    // cpal streams are not Send on every platform: own it on a dedicated thread.
    let handle = std::thread::spawn(move || -> Result<(Vec<f32>, u32), String> {
        let host = cpal::default_host();
        let device = match host.default_input_device() {
            Some(d) => d,
            None => {
                let _ = ready_tx.send(Err("Aucun micro détecté.".into()));
                return Err("Aucun micro détecté.".into());
            }
        };
        let config = match device.default_input_config() {
            Ok(c) => c,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("Micro indisponible : {e}")));
                return Err(e.to_string());
            }
        };
        let rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::with_capacity(rate as usize * 10)));
        let level: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));

        let push = {
            let samples = samples.clone();
            let level = level.clone();
            move |mono: &mut dyn Iterator<Item = f32>| {
                let mut s = samples.lock();
                let mut peak = 0.0f32;
                for v in mono {
                    peak = peak.max(v.abs());
                    s.push(v);
                }
                let mut l = level.lock();
                *l = (*l * 0.7).max(peak);
            }
        };
        let err_fn = |e| eprintln!("micro: {e}");
        let stream_cfg: cpal::StreamConfig = config.clone().into();
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                let push = push.clone();
                device.build_input_stream(
                    &stream_cfg,
                    move |data: &[f32], _: &_| {
                        let mut it = data.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32);
                        push(&mut it);
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let push = push.clone();
                device.build_input_stream(
                    &stream_cfg,
                    move |data: &[i16], _: &_| {
                        let mut it = data
                            .chunks(channels)
                            .map(|c| c.iter().map(|v| *v as f32 / 32768.0).sum::<f32>() / channels as f32);
                        push(&mut it);
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let push = push.clone();
                device.build_input_stream(
                    &stream_cfg,
                    move |data: &[u16], _: &_| {
                        let mut it = data.chunks(channels).map(|c| {
                            c.iter().map(|v| (*v as f32 - 32768.0) / 32768.0).sum::<f32>() / channels as f32
                        });
                        push(&mut it);
                    },
                    err_fn,
                    None,
                )
            }
            other => {
                let _ = ready_tx.send(Err(format!("Format micro non géré : {other:?}")));
                return Err("format".into());
            }
        };
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("Micro indisponible : {e}")));
                return Err(e.to_string());
            }
        };
        if let Err(e) = stream.play() {
            let _ = ready_tx.send(Err(format!("Micro indisponible : {e}")));
            return Err(e.to_string());
        }
        let _ = ready_tx.send(Ok(()));
        let started = Instant::now();
        loop {
            match stop_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let l = *level.lock();
                    let _ = app.emit("hue-mic-level", serde_json::json!({ "level": l.min(1.0) }));
                    if started.elapsed() > MAX_RECORD {
                        let _ = app.emit("hue-mic-autostop", ());
                        break;
                    }
                }
            }
        }
        drop(stream);
        let data = std::mem::take(&mut *samples.lock());
        Ok((data, rate))
    });

    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => {
            *slot = Some(Recording {
                stop_tx,
                handle,
                started: Instant::now(),
            });
            Ok(())
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err("Le micro ne répond pas.".into()),
    }
}

pub fn cancel() {
    if let Some(r) = REC.lock().take() {
        let _ = r.stop_tx.send(());
    }
}

/// Stop recording and return the transcription.
pub async fn stop_and_transcribe(app: &AppHandle, settings: &HueSettings) -> Result<String, String> {
    let rec = REC.lock().take().ok_or("Pas d'enregistrement en cours.")?;
    let _ = rec.stop_tx.send(());
    let (samples, rate) = tokio::task::spawn_blocking(move || rec.handle.join())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|_| "enregistrement interrompu".to_string())??;
    let _ = rec.started;
    let pcm = resample_to_16k(&samples, rate);
    if pcm.len() < 16_000 / 2 {
        return Err("Trop court — maintiens un peu plus longtemps.".into());
    }
    if pcm.iter().fold(0.0f32, |m, v| m.max(v.abs())) < 0.01 {
        return Err("Je n'ai rien entendu… le micro est peut-être coupé.".into());
    }
    let wav = encode_wav(&pcm);
    let text = match settings.stt_engine {
        SttEngine::Api => transcribe_api(settings, wav).await?,
        SttEngine::Local => transcribe_local(app, settings, wav).await?,
    };
    let cleaned = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('[') && !l.starts_with('('))
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() {
        return Err("Je n'ai pas compris, tu peux répéter ?".into());
    }
    Ok(cleaned)
}

fn resample_to_16k(input: &[f32], rate: u32) -> Vec<f32> {
    if rate == 16_000 || input.is_empty() {
        return input.to_vec();
    }
    let ratio = rate as f64 / 16_000.0;
    let out_len = (input.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        // Box-filter average over the source span (cheap anti-aliasing), then lerp.
        let start = (i as f64 * ratio) as usize;
        let end = (((i + 1) as f64 * ratio) as usize).min(input.len()).max(start + 1);
        let sum: f32 = input[start..end].iter().sum();
        out.push(sum / (end - start) as f32);
    }
    out
}

fn encode_wav(pcm: &[f32]) -> Vec<u8> {
    let data_len = (pcm.len() * 2) as u32;
    let mut w = Vec::with_capacity(44 + data_len as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data_len).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&16_000u32.to_le_bytes());
    w.extend_from_slice(&(16_000u32 * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data_len.to_le_bytes());
    // Normalise quiet microphones a little.
    let peak = pcm.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-4);
    let gain = (0.9 / peak).min(8.0);
    for v in pcm {
        let s = (v * gain).clamp(-1.0, 1.0);
        w.extend_from_slice(&((s * 32767.0) as i16).to_le_bytes());
    }
    w
}

async fn ensure_local(app: &AppHandle, model: SttModel) -> Result<(PathBuf, PathBuf), String> {
    let dir = runtime_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let cli = cli_path();
    if !cli.is_file() {
        let _ = app.emit("hue-status", serde_json::json!({ "message": "Téléchargement de whisper.cpp… une seule fois." }));
        let zip = dir.join("whisper-bin-x64.zip");
        download_verified(app, WHISPER_ZIP_URL, WHISPER_ZIP_SHA256, &zip, "whisper.cpp").await?;
        extract_zip_flat(&zip, &dir)?;
        let _ = std::fs::remove_file(&zip);
        if !cli.is_file() {
            return Err("whisper-cli.exe absent de l'archive".into());
        }
    }
    let mp = model_path(model);
    if !mp.is_file() {
        let (url, sha) = match model {
            SttModel::Base => (MODEL_BASE_URL, MODEL_BASE_SHA256),
            SttModel::Small => (MODEL_SMALL_URL, MODEL_SMALL_SHA256),
        };
        let _ = app.emit("hue-status", serde_json::json!({ "message": "Téléchargement du modèle Whisper…" }));
        download_verified(app, url, sha, &mp, "modèle Whisper").await?;
    }
    Ok((cli, mp))
}

async fn transcribe_local(app: &AppHandle, settings: &HueSettings, wav: Vec<u8>) -> Result<String, String> {
    let (cli, model) = ensure_local(app, settings.stt_model).await?;
    let tmp = std::env::temp_dir().join(format!("hue-dictee-{}.wav", std::process::id()));
    std::fs::write(&tmp, &wav).map_err(|e| e.to_string())?;
    let lang = if settings.stt_language.trim().is_empty() {
        "fr".to_string()
    } else {
        settings.stt_language.trim().to_string()
    };
    let threads = std::thread::available_parallelism()
        .map(|n| n.get().clamp(2, 8))
        .unwrap_or(4)
        .to_string();
    let dir = runtime_dir();
    let tmp2 = tmp.clone();
    let out = tokio::task::spawn_blocking(move || {
        crate::util::hidden_command(&cli.to_string_lossy())
            .current_dir(dir)
            .args([
                "-m",
                &model.to_string_lossy(),
                "-f",
                &tmp2.to_string_lossy(),
                "-l",
                &lang,
                "-t",
                &threads,
                "-nt",
                "-np",
            ])
            .output()
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("whisper-cli : {e}"))?;
    let _ = std::fs::remove_file(&tmp);
    if !out.status.success() {
        return Err(format!(
            "whisper-cli a échoué : {}",
            crate::util::truncate_chars(&String::from_utf8_lossy(&out.stderr), 200)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

async fn transcribe_api(settings: &HueSettings, wav: Vec<u8>) -> Result<String, String> {
    let url = format!("{}/audio/transcriptions", settings.api_endpoint.trim_end_matches('/'));
    let part = reqwest::multipart::Part::bytes(wav)
        .file_name("dictee.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;
    let mut form = reqwest::multipart::Form::new()
        .text("model", "whisper-1")
        .part("file", part);
    let lang = settings.stt_language.trim();
    if !lang.is_empty() && lang != "auto" {
        form = form.text("language", lang.to_string());
    }
    let resp = reqwest::Client::new()
        .post(&url)
        .bearer_auth(&settings.api_key)
        .multipart(form)
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let s = resp.status();
        let t = resp.text().await.unwrap_or_default();
        return Err(format!("Transcription API {s} : {}", crate::util::truncate_chars(&t, 200)));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(v["text"].as_str().unwrap_or("").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header() {
        let w = encode_wav(&vec![0.5; 16_000]);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        assert_eq!(w.len(), 44 + 32_000);
    }

    #[test]
    fn resample_length() {
        let v = resample_to_16k(&vec![0.1; 48_000], 48_000);
        assert_eq!(v.len(), 16_000);
    }
}
