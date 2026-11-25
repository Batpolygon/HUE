//! Verified downloads (SHA-256) and safe zip extraction for helper runtimes.

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

fn status(app: &AppHandle, message: String) {
    let _ = app.emit("hue-status", serde_json::json!({ "message": message }));
}

/// Download `url` to `dest`, verifying the SHA-256. Emits progress to the pet.
pub async fn download_verified(
    app: &AppHandle,
    url: &str,
    sha256_hex: &str,
    dest: &Path,
    label: &str,
) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let client = reqwest::Client::builder()
        .user_agent("HUE-desktop-pet")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("téléchargement {label} : {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("téléchargement {label} : HTTP {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);
    let part = dest.with_extension("part");
    let mut file = fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut done: u64 = 0;
    let mut last_decile = 0u64;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("téléchargement {label} : {e}"))?;
        hasher.update(&chunk);
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        done += chunk.len() as u64;
        if total > 0 {
            let decile = done * 10 / total;
            if decile > last_decile && decile < 10 && decile % 3 == 0 {
                last_decile = decile;
                status(app, format!("{label} : {}0 %…", decile));
            }
        }
    }
    drop(file);
    let got = hex::encode(hasher.finalize());
    if !got.eq_ignore_ascii_case(sha256_hex) {
        let _ = fs::remove_file(&part);
        return Err(format!(
            "{label} : empreinte SHA-256 invalide (fichier corrompu ou modifié) — rien n'a été installé"
        ));
    }
    fs::rename(&part, dest).map_err(|e| e.to_string())?;
    Ok(())
}

/// Extract every file of `zip_path` into `dest`, flattening directories
/// (runtime zips put the exe + dlls in a subfolder).
pub fn extract_zip_flat(zip_path: &Path, dest: &Path) -> Result<Vec<PathBuf>, String> {
    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let mut written = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        // `enclosed_name` rejects absolute paths and `..` traversal.
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let Some(name) = rel.file_name() else {
            continue;
        };
        let out_path = dest.join(name);
        let mut out = fs::File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        written.push(out_path);
    }
    Ok(written)
}
