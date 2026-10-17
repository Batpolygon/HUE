use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttStatus {
    pub available: bool,
    pub message: String,
}

/// Optional STT module — stubbed for V1 first cut.
pub fn status() -> SttStatus {
    SttStatus {
        available: false,
        message: "STT Whisper arrive bientôt — utilise le champ texte pour l'instant.".into(),
    }
}
