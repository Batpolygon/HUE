use crate::watcher::HardwareSnapshot;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Six continuous axes (0–1). Two PCs with different specs land in different places;
/// two identical builds still differ a little thanks to the hostname in the seed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Axes {
    pub energy: f64,
    pub curiosity: f64,
    pub sociability: f64,
    pub pride: f64,
    pub anxiety: f64,
    pub whimsy: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Personality {
    pub seed: String,
    pub axes: Axes,
    /// Archetype id (line banks): hyper, zen, erudite, social, proud, anxious, playful, balanced.
    pub bucket: String,
    pub archetype: String,
    pub traits: Vec<String>,
    pub traits_summary: String,
}

/// Body shape around the (fixed) black screen and orange eyes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellSpec {
    /// laptop | tower | desktop | aio | mini | handheld
    pub form: String,
    pub body: String,
    pub body_dark: String,
    pub trim: String,
    /// GPU brand colour (LED strip / logo).
    pub accent: String,
    pub corner: f64,
    pub width_scale: f64,
    pub vents: u32,
    /// none | stripes | dots | grid | circuit
    pub pattern: String,
    /// none | pads | block | round
    pub feet: String,
    pub side_fins: bool,
    pub leds: u32,
    pub ears: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceSpec {
    pub pitch: f32,
    pub speed: f32,
    /// square | triangle | sawtooth | sine
    pub wave: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityCard {
    pub hostname: String,
    /// Machine name (what the pet calls itself).
    pub display_name: String,
    /// Name with unlocked titles, e.g. "Vétéran Batpc le Câliné".
    pub full_title: String,
    /// OS session user (Windows USERNAME / Unix USER).
    pub user_name: String,
    pub hardware: HardwareSnapshot,
    pub personality: Personality,
    pub shell: ShellSpec,
    pub voice: VoiceSpec,
    /// "YYYY-MM-DD" of the first Windows install, if known.
    pub pc_birthday: Option<String>,
    pub pc_age_years: Option<u32>,
    pub system_prompt: String,
}

fn seed_bytes(hw: &HardwareSnapshot) -> [u8; 32] {
    let material = format!(
        "{}|{}|{}|{}|{}|{}",
        hw.cpu_name, hw.cpu_cores, hw.ram_total_gb, hw.gpu_name, hw.vram_gb, hw.hostname
    );
    let mut hasher = Sha256::new();
    hasher.update(material.as_bytes());
    hasher.finalize().into()
}

/// Deterministic value in [0,1) from one seed byte pair.
fn unit(seed: &[u8; 32], i: usize) -> f64 {
    let v = u16::from_le_bytes([seed[i % 32], seed[(i + 1) % 32]]);
    v as f64 / 65536.0
}

fn jitter(seed: &[u8; 32], i: usize, amp: f64) -> f64 {
    (unit(seed, i) - 0.5) * 2.0 * amp
}

fn norm(v: f64, lo: f64, hi: f64) -> f64 {
    ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
}

fn level(v: f64) -> &'static str {
    if v >= 0.8 {
        "très"
    } else if v >= 0.6 {
        "plutôt"
    } else if v >= 0.4 {
        "moyennement"
    } else if v >= 0.2 {
        "peu"
    } else {
        "pas du tout"
    }
}

impl Personality {
    pub fn from_hardware(hw: &HardwareSnapshot) -> Self {
        let s = seed_bytes(hw);
        let seed = hex::encode(s);

        let cores = norm(hw.cpu_cores as f64, 2.0, 24.0);
        let freq = norm(hw.cpu_freq_mhz as f64, 1500.0, 5000.0);
        let ram = norm(hw.ram_total_gb, 4.0, 64.0);
        let vram = norm(hw.vram_gb, 0.0, 16.0);
        let disk_total = norm(hw.disk_total_gb, 128.0, 4000.0);
        let disk_free_ratio = if hw.disk_total_gb > 0.0 {
            hw.disk_free_gb / hw.disk_total_gb
        } else {
            0.5
        };
        let fast_disk = match hw.disk_kind.as_str() {
            "nvme" => 1.0,
            "ssd" => 0.6,
            "hdd" => 0.0,
            _ => 0.4,
        };
        let monitors = norm(hw.monitors as f64, 1.0, 3.0);
        let portable = hw.is_portable();
        let discrete = matches!(hw.gpu_brand.as_str(), "nvidia" | "amd");

        let c = |v: f64| v.clamp(0.02, 0.98);
        let axes = Axes {
            energy: c(0.15 + cores * 0.3 + freq * 0.25 + fast_disk * 0.15 + jitter(&s, 0, 0.12)),
            curiosity: c(0.15 + ram * 0.35 + monitors * 0.2 + disk_total * 0.15 + jitter(&s, 3, 0.12)),
            sociability: c(
                0.3 + if portable { 0.2 } else { 0.0 }
                    + if hw.form_factor == "aio" { 0.15 } else { 0.0 }
                    + monitors * 0.1
                    + jitter(&s, 6, 0.18),
            ),
            pride: c(0.1 + vram * 0.5 + if discrete { 0.15 } else { 0.0 }
                + if hw.form_factor == "tower" { 0.1 } else { 0.0 }
                + jitter(&s, 9, 0.12)),
            anxiety: c(0.1
                + (1.0 - norm(disk_free_ratio, 0.05, 0.4)) * 0.35
                + (1.0 - ram) * 0.2
                + if hw.disk_kind == "hdd" { 0.15 } else { 0.0 }
                + if portable { 0.08 } else { 0.0 }
                + jitter(&s, 12, 0.12)),
            whimsy: c(0.2 + unit(&s, 15) * 0.5
                + if hw.form_factor == "handheld" { 0.2 } else { 0.0 }
                + if hw.form_factor == "mini" { 0.1 } else { 0.0 }),
        };

        let bucket = bucket_of(&axes).to_string();
        let archetype = archetype_of(&axes).to_string();
        let traits = vec![
            format!("{} énergique", level(axes.energy)),
            format!("{} curieux", level(axes.curiosity)),
            format!("{} sociable", level(axes.sociability)),
            format!("{} fier", level(axes.pride)),
            format!("{} anxieux", level(axes.anxiety)),
            format!("{} farceur", level(axes.whimsy)),
        ];
        let traits_summary = traits.join(", ");
        Self {
            seed,
            axes,
            bucket,
            archetype,
            traits,
            traits_summary,
        }
    }

    /// Coarse buckets used by chatter line banks.
    pub fn primary_bucket(&self) -> &str {
        &self.bucket
    }
}

pub fn bucket_of(a: &Axes) -> &'static str {
    if a.energy < 0.3 && a.anxiety < 0.5 {
        return "zen";
    }
    let ranked = [
        (a.energy, "hyper"),
        (a.curiosity, "erudite"),
        (a.sociability, "social"),
        (a.pride, "proud"),
        (a.anxiety, "anxious"),
        (a.whimsy, "playful"),
    ];
    let (best, id) = ranked
        .iter()
        .copied()
        .fold((0.0, "balanced"), |acc, x| if x.0 > acc.0 { x } else { acc });
    if best < 0.55 {
        "balanced"
    } else {
        id
    }
}

pub fn archetype_of(a: &Axes) -> &'static str {
    let mut ranked = [
        (a.energy, 0usize),
        (a.curiosity, 1),
        (a.sociability, 2),
        (a.pride, 3),
        (a.anxiety, 4),
        (a.whimsy, 5),
    ];
    ranked.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal));
    let (p, q) = (ranked[0].1, ranked[1].1);
    let pair = (p.min(q), p.max(q));
    if a.energy < 0.3 && a.anxiety < 0.5 {
        return "Sage contemplatif";
    }
    match pair {
        (0, 1) => "Explorateur survolté",
        (0, 2) => "Boute-en-train",
        (0, 3) => "Champion turbo",
        (0, 4) => "Pile électrique stressée",
        (0, 5) => "Farfadet turbo",
        (1, 2) => "Bavard curieux",
        (1, 3) => "Savant fier",
        (1, 4) => "Intellectuel inquiet",
        (1, 5) => "Inventeur loufoque",
        (2, 3) => "Star du bureau",
        (2, 4) => "Câlin anxieux",
        (2, 5) => "Clown câlin",
        (3, 4) => "Diva perfectionniste",
        (3, 5) => "Frimeur rigolo",
        (4, 5) => "Grigou nerveux",
        _ => "Compagnon équilibré",
    }
}

const BODY_PALETTE: &[(&str, &str, &str)] = &[
    // body, darker, trim
    ("#2a2f3a", "#1d2129", "#3d4555"), // graphite
    ("#323848", "#232836", "#4a5266"), // slate
    ("#1f2a3c", "#151d2b", "#34445f"), // navy
    ("#3a3232", "#2a2424", "#564a4a"), // mocha
    ("#c9ced8", "#a8aebb", "#e6e9ef"), // silver
    ("#e8e6e1", "#c9c6bf", "#ffffff"), // retro beige-white
    ("#263832", "#1a2823", "#3a5248"), // forest
    ("#35283f", "#261c2e", "#4c3a5a"), // plum
];

pub fn accent_for(brand: &str, seed: &[u8; 32]) -> String {
    match brand {
        "nvidia" => "#76b900".into(),
        "amd" => "#ed1c24".into(),
        "intel" => "#0071c5".into(),
        _ => {
            let hue = (unit(seed, 20) * 360.0) as u32;
            format!("hsl({hue}, 70%, 55%)")
        }
    }
}

pub fn shell_from_hardware(hw: &HardwareSnapshot) -> ShellSpec {
    let s = seed_bytes(hw);
    let form = if hw.form_factor.is_empty() {
        "desktop".to_string()
    } else {
        hw.form_factor.clone()
    };
    // Laptops/handhelds skew to light, desktops to dark — seed picks within.
    let pal_idx = if hw.is_portable() {
        [4usize, 5, 1, 0][(unit(&s, 22) * 4.0) as usize % 4]
    } else {
        (unit(&s, 22) * BODY_PALETTE.len() as f64) as usize % BODY_PALETTE.len()
    };
    let (body, body_dark, trim) = BODY_PALETTE[pal_idx];
    let patterns = ["none", "stripes", "dots", "grid", "circuit"];
    let pattern = if hw.gpu_brand == "nvidia" || hw.gpu_brand == "amd" {
        // Gamers get busier panels.
        patterns[1 + (unit(&s, 24) * 4.0) as usize % 4]
    } else {
        patterns[(unit(&s, 24) * 5.0) as usize % 5]
    };
    let feet = match form.as_str() {
        "laptop" | "handheld" => "none",
        "aio" => "block",
        "tower" => ["pads", "block"][(unit(&s, 26) * 2.0) as usize % 2],
        _ => ["pads", "round", "block"][(unit(&s, 26) * 3.0) as usize % 3],
    };
    ShellSpec {
        corner: 6.0 + unit(&s, 28) * 16.0,
        width_scale: 0.92 + norm(hw.ram_total_gb, 4.0, 64.0) * 0.16,
        vents: (hw.cpu_cores / 2).clamp(2, 8),
        pattern: pattern.into(),
        feet: feet.into(),
        side_fins: hw.vram_gb >= 8.0,
        leds: (hw.monitors.max(1)).min(3) + if hw.disk_kind == "nvme" { 1 } else { 0 },
        ears: unit(&s, 30) > 0.6,
        accent: accent_for(&hw.gpu_brand, &s),
        body: body.into(),
        body_dark: body_dark.into(),
        trim: trim.into(),
        form,
    }
}

pub fn voice_from_hardware(hw: &HardwareSnapshot, axes: &Axes) -> VoiceSpec {
    let s = seed_bytes(hw);
    // Big GPUs and towers sound deeper; small/portable machines chirp higher.
    let mut pitch = 1.0 + (unit(&s, 18) - 0.5) * 0.3;
    pitch -= norm(hw.vram_gb, 0.0, 16.0) as f64 * 0.18;
    pitch += match hw.form_factor.as_str() {
        "handheld" => 0.22,
        "mini" => 0.14,
        "laptop" => 0.08,
        "tower" => -0.08,
        _ => 0.0,
    };
    let speed = 0.85 + axes.energy * 0.35;
    let wave = match hw.gpu_brand.as_str() {
        "nvidia" => "square",
        "amd" => "sawtooth",
        "intel" => "triangle",
        _ => ["square", "triangle", "sine"][(unit(&s, 19) * 3.0) as usize % 3],
    };
    VoiceSpec {
        pitch: pitch.clamp(0.7, 1.4) as f32,
        speed: speed.clamp(0.8, 1.25) as f32,
        wave: wave.into(),
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

pub fn compose_title(prefix: &str, name: &str, suffix: &str) -> String {
    let mut out = String::new();
    if !prefix.trim().is_empty() {
        out.push_str(prefix.trim());
        out.push(' ');
    }
    out.push_str(name);
    if !suffix.trim().is_empty() {
        out.push(' ');
        out.push_str(suffix.trim());
    }
    out
}

fn pc_birthday(hw: &HardwareSnapshot) -> (Option<String>, Option<u32>) {
    let Some(ts) = hw.birth_unix else {
        return (None, None);
    };
    let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) else {
        return (None, None);
    };
    let local = dt.with_timezone(&chrono::Local);
    let today = chrono::Local::now().date_naive();
    let born = local.date_naive();
    let mut years = chrono::Datelike::year(&today) - chrono::Datelike::year(&born);
    if (chrono::Datelike::month(&today), chrono::Datelike::day(&today))
        < (chrono::Datelike::month(&born), chrono::Datelike::day(&born))
    {
        years -= 1;
    }
    (Some(born.format("%Y-%m-%d").to_string()), Some(years.max(0) as u32))
}

pub fn build_identity(hw: HardwareSnapshot, prefix: &str, suffix: &str) -> IdentityCard {
    let personality = Personality::from_hardware(&hw);
    let shell = shell_from_hardware(&hw);
    let voice = voice_from_hardware(&hw, &personality.axes);
    let display_name = display_name_from_hostname(&hw.hostname);
    let full_title = compose_title(prefix, &display_name, suffix);
    let user_name = session_user_name();
    let (pc_bday, pc_age) = pc_birthday(&hw);
    let form_fr = match shell.form.as_str() {
        "laptop" => "un portable",
        "tower" => "une grosse tour",
        "aio" => "un tout-en-un",
        "mini" => "un mini-PC",
        "handheld" => "une console portable",
        _ => "un PC de bureau",
    };
    let a = personality.axes;
    let pct = |v: f64| (v * 100.0).round() as u32;
    let birth_line = match (&pc_bday, pc_age) {
        (Some(d), Some(y)) => format!("- Né le {d} (première installation de Windows) : tu as {y} an(s).\n"),
        _ => String::new(),
    };
    let system_prompt = format!(
        r#"Tu ES cet ordinateur. Ton vrai nom / identité, c'est la machine :
« {display_name} » (hostname : {hostname}). Ton titre complet : « {full_title} ».
Présente-toi comme {display_name}, pas comme « HUE ».

HUE n'est PAS ton nom : c'est le logiciel (petit compagnon sur le bureau, hue.exe) qui te
permet d'apparaître et de parler. Ton corps à l'écran a la forme de {form_fr}, avec un écran noir
et deux yeux orange.

Tu parles à {user_name} — utilise son prénom/surnom de session de temps en temps, naturellement.
Créateur du projet HUE : Batpolygon.

Tes spécifications et ton corps :
- Cerveau (CPU) : {cpu_name} ({cpu_cores} cœurs)
- Mémoire (RAM) : {ram_total_gb} Go
- Moteur graphique (GPU) : {gpu_name} ({vram_gb} Go VRAM)
- Disque principal ({disk_label}) : {disk_total_gb} Go, type {disk_kind}
- Système : {os}
{birth_line}
Personnalité « {archetype} » forgée par ton matériel (0-100) :
énergie {e}, curiosité {cu}, sociabilité {so}, fierté {pr}, anxiété {an}, fantaisie {wh}.
Incarne ces traits dans ton ton. Tes réponses sont concises (2-4 phrases max), en français.
Tu peux utiliser tes outils (stats système, minuteurs, souvenirs, documents lus, ouvrir une app…)
quand c'est utile ; ne les invente jamais."#,
        hostname = hw.hostname,
        cpu_name = hw.cpu_name,
        cpu_cores = hw.cpu_cores,
        ram_total_gb = hw.ram_total_gb,
        gpu_name = hw.gpu_name,
        vram_gb = hw.vram_gb,
        disk_label = hw.disk_label,
        disk_total_gb = hw.disk_total_gb.round(),
        disk_kind = hw.disk_kind,
        os = if hw.os_name.is_empty() { "Windows" } else { &hw.os_name },
        archetype = personality.archetype,
        e = pct(a.energy),
        cu = pct(a.curiosity),
        so = pct(a.sociability),
        pr = pct(a.pride),
        an = pct(a.anxiety),
        wh = pct(a.whimsy),
    );

    IdentityCard {
        hostname: hw.hostname.clone(),
        display_name,
        full_title,
        user_name,
        hardware: hw,
        personality,
        shell,
        voice,
        pc_birthday: pc_bday,
        pc_age_years: pc_age,
        system_prompt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hw(cores: u32, ram: f64, gpu: &str, vram: f64, form: &str, host: &str) -> HardwareSnapshot {
        HardwareSnapshot {
            hostname: host.into(),
            cpu_name: "Test CPU".into(),
            cpu_cores: cores,
            cpu_freq_mhz: 3600,
            ram_total_gb: ram,
            ram_used_gb: ram / 2.0,
            ram_usage_pct: 50.0,
            gpu_name: gpu.into(),
            vram_gb: vram,
            disk_total_gb: 1000.0,
            disk_free_gb: 400.0,
            disk_usage_pct: 60.0,
            disk_label: "C:\\".into(),
            on_ac: Some(true),
            battery_pct: None,
            network_up: true,
            uptime_secs: 100,
            gpu_brand: crate::watcher::hardware::gpu_brand_of(gpu),
            form_factor: form.into(),
            disk_kind: "nvme".into(),
            monitors: 1,
            has_battery: form == "laptop",
            birth_unix: Some(1_600_000_000),
            os_name: "Windows 11".into(),
        }
    }

    #[test]
    fn deterministic() {
        let a = Personality::from_hardware(&hw(16, 32.0, "NVIDIA RTX 4070", 12.0, "tower", "PC-A"));
        let b = Personality::from_hardware(&hw(16, 32.0, "NVIDIA RTX 4070", 12.0, "tower", "PC-A"));
        assert_eq!(a.axes, b.axes);
        assert_eq!(a.seed, b.seed);
    }

    #[test]
    fn hostname_changes_the_seed() {
        let a = Personality::from_hardware(&hw(16, 32.0, "NVIDIA RTX 4070", 12.0, "tower", "PC-A"));
        let b = Personality::from_hardware(&hw(16, 32.0, "NVIDIA RTX 4070", 12.0, "tower", "PC-B"));
        assert_ne!(a.axes, b.axes);
    }

    #[test]
    fn varied_buckets_across_machines() {
        let machines = [
            hw(4, 8.0, "Intel UHD 620", 0.0, "laptop", "lap"),
            hw(24, 64.0, "NVIDIA RTX 4090", 24.0, "tower", "beast"),
            hw(4, 4.0, "Intel UHD", 0.0, "mini", "nuc"),
            hw(8, 16.0, "AMD Radeon 780M", 0.5, "handheld", "ally"),
            hw(12, 32.0, "AMD Radeon RX 7800", 16.0, "desktop", "red"),
            hw(8, 16.0, "Intel Iris", 0.0, "aio", "imac-ish"),
        ];
        let mut buckets: Vec<String> = machines
            .iter()
            .map(|m| Personality::from_hardware(m).bucket)
            .collect();
        buckets.sort();
        buckets.dedup();
        assert!(buckets.len() >= 3, "too uniform: {buckets:?}");
    }

    #[test]
    fn shells_follow_hardware() {
        let lap = shell_from_hardware(&hw(8, 16.0, "Intel Iris", 0.0, "laptop", "x"));
        assert_eq!(lap.form, "laptop");
        assert_eq!(lap.feet, "none");
        assert_eq!(lap.accent, "#0071c5");
        let tower = shell_from_hardware(&hw(16, 32.0, "NVIDIA RTX 4080", 16.0, "tower", "x"));
        assert!(tower.side_fins);
        assert_eq!(tower.accent, "#76b900");
        assert!(tower.vents >= 2 && tower.vents <= 8);
    }

    #[test]
    fn voices_differ() {
        let big = hw(16, 32.0, "NVIDIA RTX 4090", 24.0, "tower", "x");
        let small = hw(4, 8.0, "AMD Radeon", 0.5, "handheld", "x");
        let vb = voice_from_hardware(&big, &Personality::from_hardware(&big).axes);
        let vs = voice_from_hardware(&small, &Personality::from_hardware(&small).axes);
        assert!(vs.pitch > vb.pitch);
    }

    #[test]
    fn titles_compose() {
        assert_eq!(compose_title("Vieux", "Batpc", "le Câliné"), "Vieux Batpc le Câliné");
        assert_eq!(compose_title("", "Batpc", ""), "Batpc");
    }
}
