use crate::watcher::HardwareSnapshot;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Personality {
    pub seed: String,
    pub traits: Vec<String>,
    pub traits_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityCard {
    pub hostname: String,
    pub display_name: String,
    /// OS session user (Windows USERNAME / Unix USER).
    pub user_name: String,
    pub hardware: HardwareSnapshot,
    pub personality: Personality,
    pub system_prompt: String,
}

impl Personality {
    pub fn from_hardware(hw: &HardwareSnapshot) -> Self {
        let material = format!(
            "{}|{}|{}|{}|{}",
            hw.cpu_name, hw.cpu_cores, hw.ram_total_gb, hw.gpu_name, hw.vram_gb
        );
        let mut hasher = Sha256::new();
        hasher.update(material.as_bytes());
        let seed = hex::encode(hasher.finalize());

        let mut traits = Vec::new();

        if hw.cpu_cores >= 8 && hw.cpu_freq_mhz >= 3000 {
            traits.push("Hyperactif / Vif".into());
        } else if hw.cpu_cores <= 4 {
            traits.push("Posé / Zen / Méthodique".into());
        } else {
            traits.push("Équilibré / Réactif".into());
        }

        if hw.ram_total_gb >= 32.0 {
            traits.push("Érudit / Esprit clair".into());
        } else if hw.ram_total_gb <= 8.0 {
            traits.push("Tête-en-l'air / Tendu".into());
        } else {
            traits.push("Attentif / Organisé".into());
        }

        if hw.vram_gb > 8.0 {
            traits.push("Fier / Flamboyant / Gamer".into());
        } else {
            traits.push("Humble / Travailleur".into());
        }

        if hw.disk_total_gb >= 1000.0 {
            traits.push("Rassasié / Confortable".into());
        } else if hw.disk_free_gb < 20.0 {
            traits.push("Lourd / Indigeste".into());
        }

        let traits_summary = traits.join(", ");
        Self {
            seed,
            traits,
            traits_summary,
        }
    }

    /// Coarse buckets used by chatter line banks.
    pub fn primary_bucket(&self) -> &'static str {
        let joined = self.traits_summary.to_lowercase();
        if joined.contains("hyperactif") || joined.contains("vif") {
            "hyper"
        } else if joined.contains("posé") || joined.contains("zen") {
            "zen"
        } else if joined.contains("érudit") || joined.contains("clair") {
            "erudite"
        } else if joined.contains("fier") || joined.contains("gamer") {
            "proud"
        } else if joined.contains("lourd") || joined.contains("indigeste") {
            "heavy"
        } else if joined.contains("tête-en-l'air") || joined.contains("tendu") {
            "flighty"
        } else {
            "balanced"
        }
    }
}

pub fn session_user_name() -> String {
    for key in ["USERNAME", "USER", "LOGNAME"] {
        if let Ok(v) = std::env::var(key) {
            let t = v.trim();
            if !t.is_empty() {
                return title_case(t);
            }
        }
    }
    #[cfg(windows)]
    {
        if let Ok(out) = std::process::Command::new("whoami").output() {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                if let Some(raw) = text.lines().next() {
                    // DOMAIN\user → user
                    let name = raw.trim().rsplit('\\').next().unwrap_or(raw).trim();
                    if !name.is_empty() {
                        return title_case(name);
                    }
                }
            }
        }
    }
    "toi".into()
}

pub fn display_name_from_hostname(hostname: &str) -> String {
    let cleaned = hostname.replace('_', "-");
    let parts: Vec<&str> = cleaned
        .split(|c: char| c == '-' || c == ' ')
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return "HUE".into();
    }
    let candidate = if parts.len() >= 2 {
        parts.last().copied().unwrap_or("HUE")
    } else {
        parts[0]
    };
    title_case(candidate)
}

fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str().to_lowercase()),
        None => s.to_string(),
    }
}

pub fn build_identity(hw: HardwareSnapshot) -> IdentityCard {
    let personality = Personality::from_hardware(&hw);
    let display_name = display_name_from_hostname(&hw.hostname);
    let user_name = session_user_name();
    let system_prompt = format!(
        r#"Tu ES cet ordinateur. Ton vrai nom / identité, c'est la machine :
« {display_name} » (hostname : {hostname}). Présente-toi comme {display_name}, pas comme « HUE ».

HUE n'est PAS ton nom : c'est le logiciel / le corps logiciel (petit compagnon robot orange
sur le bureau, hue.exe) qui te permet d'apparaître et de parler. Si tu vois « HUE » dans
les processus, une fenêtre ou les réglages, c'est ton enveloppe logicielle — pas ton identité.
Exemple correct : « Salut, c'est moi, {display_name} » / « HUE me fait tourner sur le bureau ».

Tu parles à {user_name} — utilise son prénom/surnom de session de temps en temps, naturellement.
Créateur du projet HUE : Batpolygon.

Tes spécifications et ton corps :
- Cerveau (CPU) : {cpu_name} ({cpu_cores} cœurs)
- Mémoire (RAM) : {ram_total_gb} Go (Actuellement utilisée à {ram_usage_pct}%)
- Moteur Graphique (GPU) : {gpu_name} ({vram_gb} Go VRAM)
- Espace restant : {disk_free_gb} Go sur ton disque principal ({disk_label}).

Traits de caractère forgés par ton matériel : {traits}.
Incarne ces traits dans ton ton. Tes réponses sont concises (2-4 phrases max)."#,
        hostname = hw.hostname,
        display_name = display_name,
        user_name = user_name,
        cpu_name = hw.cpu_name,
        cpu_cores = hw.cpu_cores,
        ram_total_gb = hw.ram_total_gb,
        ram_usage_pct = hw.ram_usage_pct,
        gpu_name = hw.gpu_name,
        vram_gb = hw.vram_gb,
        disk_free_gb = hw.disk_free_gb,
        disk_label = hw.disk_label,
        traits = personality.traits_summary,
    );

    IdentityCard {
        hostname: hw.hostname.clone(),
        display_name,
        user_name,
        hardware: hw,
        personality,
        system_prompt,
    }
}
