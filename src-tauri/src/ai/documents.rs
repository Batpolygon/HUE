//! Files dropped on the pet: text extraction (plain text / code / PDF / Office),
//! chunking, and a small persistent BM25 index used for retrieval (RAG) in later chats.

use crate::config::HueSettings;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const CHUNK_CHARS: usize = 900;
const CHUNK_OVERLAP: usize = 150;
const MAX_TEXT_CHARS: usize = 400_000;
const MAX_DOCS: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub size_bytes: u64,
    pub added: i64,
    pub chunks: Vec<String>,
    #[serde(default)]
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocSummary {
    pub name: String,
    pub kind: String,
    pub added: i64,
    pub chunks: usize,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    pub doc: String,
    pub chunk: usize,
    pub score: f64,
    pub text: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DocStore {
    pub docs: Vec<Document>,
}

pub struct Extracted {
    pub kind: String,
    pub text: String,
    /// Extra facts for the prompt when there is little/no text (images, binaries).
    pub meta: String,
}

const TEXT_EXT: &[&str] = &[
    "txt", "md", "markdown", "rst", "log", "csv", "tsv", "json", "jsonl", "xml", "html", "htm", "yaml",
    "yml", "toml", "ini", "cfg", "conf", "env", "rs", "ts", "tsx", "js", "jsx", "mjs", "py", "rb",
    "go", "java", "kt", "kts", "swift", "c", "h", "cpp", "hpp", "cc", "cs", "php", "lua", "sh", "bash",
    "ps1", "bat", "cmd", "sql", "css", "scss", "vue", "svelte", "tex", "srt", "vtt", "gd", "zig", "dart",
    "r", "m", "pl", "gradle", "cmake", "makefile", "dockerfile",
];
const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "svg", "tiff"];

pub fn extract(path: &Path) -> Result<Extracted, String> {
    let meta = fs::metadata(path).map_err(|e| format!("lecture impossible : {e}"))?;
    if meta.is_dir() {
        return Ok(Extracted {
            kind: "dossier".into(),
            text: String::new(),
            meta: folder_meta(path),
        });
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let name_lower = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    let size = meta.len();

    let (kind, text) = if TEXT_EXT.contains(&ext.as_str())
        || name_lower == "makefile"
        || name_lower == "dockerfile"
        || name_lower.starts_with("readme")
    {
        ("texte", read_text(path, 4 * 1024 * 1024)?)
    } else if ext == "pdf" {
        ("pdf", pdf_text(path)?)
    } else if ext == "docx" {
        ("word", office_text(path, &["word/document.xml"], "w:p")?)
    } else if ext == "odt" {
        ("odt", office_text(path, &["content.xml"], "text:p")?)
    } else if ext == "pptx" {
        ("powerpoint", pptx_text(path)?)
    } else if ext == "xlsx" {
        ("excel", office_text(path, &["xl/sharedStrings.xml"], "si")?)
    } else if IMAGE_EXT.contains(&ext.as_str()) {
        let dims = image_dims(path).map(|(w, h)| format!("{w}×{h} px")).unwrap_or_default();
        return Ok(Extracted {
            kind: "image".into(),
            text: String::new(),
            meta: format!("Image {ext}, {}, {dims}", human_size(size)),
        });
    } else if looks_like_text(path) {
        ("texte", read_text(path, 4 * 1024 * 1024)?)
    } else {
        return Ok(Extracted {
            kind: "binaire".into(),
            text: String::new(),
            meta: format!(
                "Fichier binaire .{ext}, {} — contenu non lisible directement.",
                human_size(size)
            ),
        });
    };
    let mut text = normalize_ws(&text);
    if text.chars().count() > MAX_TEXT_CHARS {
        text = text.chars().take(MAX_TEXT_CHARS).collect();
    }
    Ok(Extracted {
        kind: kind.into(),
        meta: format!("{kind}, {}, {} caractères lus", human_size(size), text.chars().count()),
        text,
    })
}

pub fn human_size(b: u64) -> String {
    let f = b as f64;
    if f >= 1e9 {
        format!("{:.1} Go", f / 1e9)
    } else if f >= 1e6 {
        format!("{:.1} Mo", f / 1e6)
    } else if f >= 1e3 {
        format!("{:.0} Ko", f / 1e3)
    } else {
        format!("{b} o")
    }
}

fn folder_meta(path: &Path) -> String {
    let mut files = 0usize;
    let mut dirs = 0usize;
    let mut exts: HashMap<String, usize> = HashMap::new();
    if let Ok(rd) = fs::read_dir(path) {
        for e in rd.flatten().take(2000) {
            let p = e.path();
            if p.is_dir() {
                dirs += 1;
            } else {
                files += 1;
                if let Some(x) = p.extension().and_then(|x| x.to_str()) {
                    *exts.entry(x.to_lowercase()).or_default() += 1;
                }
            }
        }
    }
    let mut top: Vec<_> = exts.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1));
    let top: Vec<String> = top.iter().take(5).map(|(e, n)| format!(".{e} ×{n}")).collect();
    format!("Dossier : {files} fichier(s), {dirs} sous-dossier(s). Types : {}", top.join(", "))
}

fn read_text(path: &Path, max: u64) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    f.by_ref().take(max).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    // UTF-16 LE with BOM (PowerShell / Notepad exports).
    if buf.len() >= 2 && buf[0] == 0xFF && buf[1] == 0xFE {
        let u16s: Vec<u16> = buf[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return Ok(String::from_utf16_lossy(&u16s));
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn looks_like_text(path: &Path) -> bool {
    let Ok(mut f) = fs::File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 4096];
    let n = f.read(&mut buf).unwrap_or(0);
    if n == 0 {
        return false;
    }
    let sample = &buf[..n];
    !sample.contains(&0) && std::str::from_utf8(sample).is_ok()
}

fn pdf_text(path: &Path) -> Result<String, String> {
    let doc = lopdf::Document::load(path).map_err(|e| format!("PDF illisible : {e}"))?;
    let pages: Vec<u32> = doc.get_pages().keys().copied().take(300).collect();
    let mut out = String::new();
    for p in pages {
        if let Ok(t) = doc.extract_text(&[p]) {
            out.push_str(&t);
            out.push('\n');
        }
        if out.len() > MAX_TEXT_CHARS * 2 {
            break;
        }
    }
    if out.trim().is_empty() {
        return Err("PDF sans texte extractible (scan ou image ?)".into());
    }
    Ok(out)
}

fn zip_entry(path: &Path, name: &str) -> Result<String, String> {
    let f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| format!("archive Office illisible : {e}"))?;
    let mut entry = z.by_name(name).map_err(|e| e.to_string())?;
    let mut s = String::new();
    entry
        .by_ref()
        .take(32 * 1024 * 1024)
        .read_to_string(&mut s)
        .map_err(|e| e.to_string())?;
    Ok(s)
}

/// Strip XML tags, turning closing `</{para_tag}>` into line breaks.
fn xml_to_text(xml: &str, para_tag: &str) -> String {
    let close = format!("</{para_tag}>");
    let xml = xml.replace(&close, "\n");
    let mut out = String::with_capacity(xml.len() / 3);
    let mut in_tag = false;
    for ch in xml.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn office_text(path: &Path, entries: &[&str], para: &str) -> Result<String, String> {
    let mut out = String::new();
    for e in entries {
        out.push_str(&xml_to_text(&zip_entry(path, e)?, para));
    }
    Ok(out)
}

fn pptx_text(path: &Path) -> Result<String, String> {
    let f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut names: Vec<String> = (0..z.len())
        .filter_map(|i| z.by_index(i).ok().map(|e| e.name().to_string()))
        .filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))
        .collect();
    names.sort_by_key(|n| {
        n.trim_start_matches("ppt/slides/slide")
            .trim_end_matches(".xml")
            .parse::<u32>()
            .unwrap_or(0)
    });
    let mut out = String::new();
    for (i, n) in names.iter().enumerate() {
        let xml = zip_entry(path, n)?;
        out.push_str(&format!("[Diapo {}] ", i + 1));
        out.push_str(&xml_to_text(&xml, "a:p"));
        out.push('\n');
    }
    Ok(out)
}

fn image_dims(path: &Path) -> Option<(u32, u32)> {
    let mut f = fs::File::open(path).ok()?;
    let mut b = [0u8; 32];
    f.read_exact(&mut b).ok()?;
    if &b[1..4] == b"PNG" {
        let w = u32::from_be_bytes([b[16], b[17], b[18], b[19]]);
        let h = u32::from_be_bytes([b[20], b[21], b[22], b[23]]);
        return Some((w, h));
    }
    if &b[0..3] == b"GIF" {
        return Some((u16::from_le_bytes([b[6], b[7]]) as u32, u16::from_le_bytes([b[8], b[9]]) as u32));
    }
    if &b[0..2] == b"BM" {
        let w = i32::from_le_bytes([b[18], b[19], b[20], b[21]]);
        let h = i32::from_le_bytes([b[22], b[23], b[24], b[25]]);
        return Some((w.unsigned_abs(), h.unsigned_abs()));
    }
    None
}

fn normalize_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank_run = 0;
    for line in s.lines() {
        let l = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if l.is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(&l);
        out.push('\n');
    }
    out.trim().to_string()
}

pub fn chunk(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let mut end = (start + CHUNK_CHARS).min(chars.len());
        // Prefer to cut on a paragraph / sentence boundary.
        if end < chars.len() {
            let window_start = start + CHUNK_CHARS / 2;
            if let Some(pos) = (window_start..end).rev().find(|&i| chars[i] == '\n') {
                end = pos + 1;
            } else if let Some(pos) = (window_start..end).rev().find(|&i| chars[i] == '.' || chars[i] == '!' || chars[i] == '?') {
                end = pos + 1;
            }
        }
        let piece: String = chars[start..end].iter().collect();
        if !piece.trim().is_empty() {
            out.push(piece.trim().to_string());
        }
        if end >= chars.len() {
            break;
        }
        start = end.saturating_sub(CHUNK_OVERLAP).max(start + 1);
    }
    out
}

const STOP: &[&str] = &[
    "le", "la", "les", "un", "une", "des", "de", "du", "et", "ou", "en", "au", "aux", "a", "à", "est",
    "que", "qui", "pour", "dans", "sur", "par", "pas", "plus", "ce", "ces", "se", "sa", "son", "ses",
    "il", "elle", "on", "nous", "vous", "ils", "the", "of", "and", "to", "in", "is", "it", "that",
    "for", "on", "with", "as", "this", "be", "are", "was", "an", "or", "by", "at", "je", "tu", "ne",
    "y", "l", "d", "s", "c", "qu", "j", "n", "m", "t", "mais", "avec", "être", "avoir", "fait", "sont",
];

pub fn tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.chars().count() >= 2 && !STOP.contains(t))
        .map(|t| t.to_string())
        .collect()
}

/// BM25 over a list of chunks.
pub fn bm25(chunks: &[String], query: &str) -> Vec<(usize, f64)> {
    let q = tokens(query);
    if q.is_empty() || chunks.is_empty() {
        return Vec::new();
    }
    let docs: Vec<Vec<String>> = chunks.iter().map(|c| tokens(c)).collect();
    let n = docs.len() as f64;
    let avgdl = docs.iter().map(|d| d.len()).sum::<usize>() as f64 / n.max(1.0);
    let mut df: HashMap<&str, usize> = HashMap::new();
    for d in &docs {
        let mut seen: Vec<&str> = d.iter().map(|s| s.as_str()).collect();
        seen.sort();
        seen.dedup();
        for t in seen {
            *df.entry(t).or_default() += 1;
        }
    }
    let (k1, b) = (1.4, 0.75);
    let mut scores: Vec<(usize, f64)> = docs
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let dl = d.len() as f64;
            let mut s = 0.0;
            for term in &q {
                let f = d.iter().filter(|t| *t == term).count() as f64;
                if f == 0.0 {
                    continue;
                }
                let dfi = *df.get(term.as_str()).unwrap_or(&0) as f64;
                let idf = ((n - dfi + 0.5) / (dfi + 0.5) + 1.0).ln();
                s += idf * (f * (k1 + 1.0)) / (f + k1 * (1.0 - b + b * dl / avgdl.max(1.0)));
            }
            (i, s)
        })
        .filter(|(_, s)| *s > 0.0)
        .collect();
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scores
}

/// Most characteristic terms of a document (for picking representative chunks).
fn key_terms(chunks: &[String], n: usize) -> String {
    let mut tf: HashMap<String, usize> = HashMap::new();
    for c in chunks {
        for t in tokens(c) {
            if t.chars().count() >= 4 && !t.chars().all(|c| c.is_ascii_digit()) {
                *tf.entry(t).or_default() += 1;
            }
        }
    }
    let mut v: Vec<_> = tf.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v.into_iter().take(n).map(|(t, _)| t).collect::<Vec<_>>().join(" ")
}

/// Fit a whole document into `budget` characters: beginning, end, the most
/// representative chunks, and evenly spaced samples — in reading order.
pub fn representative_context(chunks: &[String], budget: usize) -> String {
    let total: usize = chunks.iter().map(|c| c.chars().count()).sum();
    if total <= budget {
        return chunks.join("\n");
    }
    let mut picked: Vec<usize> = vec![0];
    if chunks.len() > 1 {
        picked.push(1.min(chunks.len() - 1));
        picked.push(chunks.len() - 1);
    }
    let terms = key_terms(chunks, 12);
    for (i, _) in bm25(chunks, &terms).into_iter().take(6) {
        picked.push(i);
    }
    let step = (chunks.len() / 8).max(1);
    for i in (0..chunks.len()).step_by(step) {
        picked.push(i);
    }
    let mut out_idx: Vec<usize> = Vec::new();
    let mut used = 0usize;
    for i in picked {
        if out_idx.contains(&i) {
            continue;
        }
        let len = chunks[i].chars().count() + 8;
        if used + len > budget {
            continue;
        }
        used += len;
        out_idx.push(i);
    }
    out_idx.sort_unstable();
    let mut out = String::new();
    let mut prev: Option<usize> = None;
    for i in out_idx {
        if let Some(p) = prev {
            if i > p + 1 {
                out.push_str("\n[…]\n");
            }
        }
        out.push_str(&chunks[i]);
        out.push('\n');
        prev = Some(i);
    }
    out
}

impl DocStore {
    fn path() -> PathBuf {
        HueSettings::config_dir().join("documents.json")
    }

    pub fn load() -> Self {
        fs::read_to_string(Self::path())
            .ok()
            .and_then(|r| serde_json::from_str(&r).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(raw) = serde_json::to_string(self) {
            let _ = crate::util::write_atomic(&Self::path(), raw.as_bytes());
        }
    }

    pub fn add(&mut self, doc: Document) {
        self.docs.retain(|d| d.path != doc.path);
        self.docs.push(doc);
        while self.docs.len() > MAX_DOCS {
            self.docs.remove(0);
        }
        self.save();
    }

    pub fn set_summary(&mut self, path: &str, summary: &str) {
        if let Some(d) = self.docs.iter_mut().find(|d| d.path == path) {
            d.summary = crate::util::truncate_chars(summary, 1200);
            self.save();
        }
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.docs.len();
        self.docs.retain(|d| d.name != name);
        let changed = before != self.docs.len();
        if changed {
            self.save();
        }
        changed
    }

    pub fn list(&self) -> Vec<DocSummary> {
        self.docs
            .iter()
            .rev()
            .map(|d| DocSummary {
                name: d.name.clone(),
                kind: d.kind.clone(),
                added: d.added,
                chunks: d.chunks.len(),
                summary: d.summary.clone(),
            })
            .collect()
    }

    /// Top-k chunks over every stored document.
    pub fn search(&self, query: &str, k: usize) -> Vec<Hit> {
        let mut all: Vec<Hit> = Vec::new();
        for d in &self.docs {
            // Document names count as a strong signal ("dans mon CV…").
            let name_bonus = if tokens(&d.name).iter().any(|t| tokens(query).contains(t)) {
                1.5
            } else {
                0.0
            };
            for (i, s) in bm25(&d.chunks, query).into_iter().take(k) {
                all.push(Hit {
                    doc: d.name.clone(),
                    chunk: i,
                    score: s + name_bonus,
                    text: d.chunks[i].clone(),
                });
            }
        }
        all.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        all.truncate(k);
        all
    }

    /// RAG block for the system prompt (empty when nothing relevant).
    pub fn rag_context(&self, query: &str, budget: usize) -> String {
        let hits = self.search(query, 4);
        let hits: Vec<&Hit> = hits.iter().filter(|h| h.score >= 2.0).collect();
        if hits.is_empty() {
            return String::new();
        }
        let mut out = String::from(
            "Extraits pertinents de documents que l'utilisateur t'a fait lire (cite le nom du fichier si tu t'en sers) :\n",
        );
        for h in hits {
            let piece = format!("[{} §{}] {}\n", h.doc, h.chunk + 1, h.text);
            if out.len() + piece.len() > budget {
                break;
            }
            out.push_str(&piece);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunking_overlaps_and_covers() {
        let text = (0..200).map(|i| format!("Phrase numéro {i}. ")).collect::<String>();
        let c = chunk(&text);
        assert!(c.len() > 3);
        assert!(c.iter().all(|x| x.chars().count() <= CHUNK_CHARS));
        assert!(c.last().unwrap().contains("199"));
    }

    #[test]
    fn bm25_finds_relevant_chunk() {
        let chunks = vec![
            "La recette de la tarte aux pommes demande du beurre.".to_string(),
            "Le moteur Rust compile le projet Tauri rapidement.".to_string(),
            "Les chats dorment beaucoup pendant la journée.".to_string(),
        ];
        let r = bm25(&chunks, "comment compiler avec Rust ?");
        assert_eq!(r[0].0, 1);
    }

    #[test]
    fn representative_fits_budget() {
        let chunks: Vec<String> = (0..60).map(|i| format!("Section {i} ").repeat(80)).collect();
        let ctx = representative_context(&chunks, 6000);
        assert!(ctx.chars().count() <= 6000 + 200);
        assert!(ctx.contains("Section 0"));
        assert!(ctx.contains("Section 59"));
    }

    #[test]
    fn xml_stripping() {
        let t = xml_to_text("<w:p><w:r><w:t>Bonjour &amp; salut</w:t></w:r></w:p><w:p><w:t>Ligne 2</w:t></w:p>", "w:p");
        assert_eq!(t.trim(), "Bonjour & salut\nLigne 2");
    }
}
