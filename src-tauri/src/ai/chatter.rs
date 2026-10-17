use rand::Rng;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use crate::ai::personality::Personality;
use crate::config::TalkFrequency;
use crate::physics::HueState;
use crate::watcher::live_stats::LiveStats;

/// Lightweight spontaneous lines + reactions to the foreground window title.
pub struct Chatter {
    last_spoke: Instant,
    last_title: String,
    recent: VecDeque<String>,
    last_stats_kind: HashMap<&'static str, Instant>,
}

impl Chatter {
    pub fn new() -> Self {
        Self {
            last_spoke: Instant::now() - Duration::from_secs(30),
            last_title: String::new(),
            recent: VecDeque::with_capacity(8),
            last_stats_kind: HashMap::new(),
        }
    }

    pub fn maybe_line(
        &mut self,
        frequency: TalkFrequency,
        peeking: bool,
        concealed: bool,
        interacting: bool,
        personality: &Personality,
        user_name: &str,
        display_name: &str,
        mood: HueState,
        dizzy: bool,
        force_dizzy_announce: bool,
        stats: Option<&LiveStats>,
        force_line: Option<String>,
    ) -> Option<String> {
        if concealed || interacting {
            // Still allow forced interaction lines (pet / double-click) when HUD closed.
            if interacting && force_line.is_none() {
                return None;
            }
            if concealed {
                return None;
            }
        }

        let mut rng = rand::thread_rng();

        if let Some(line) = force_line {
            return self.commit(line);
        }

        // Always announce dizzy onset (even if chatter is muted).
        if force_dizzy_announce {
            return self.commit(dizzy_announce(user_name, &mut rng));
        }

        if frequency == TalkFrequency::Mute {
            return None;
        }

        let cooldown = if dizzy {
            Duration::from_secs(frequency.cooldown_secs().min(300).saturating_div(2).max(8))
        } else {
            Duration::from_secs(frequency.cooldown_secs().min(300))
        };
        if self.last_spoke.elapsed() < cooldown {
            return None;
        }

        let title = foreground_window_title().unwrap_or_default();
        let title_trim = title.trim().to_string();
        let hour = local_hour();
        let bucket = personality.primary_bucket();

        if dizzy && rng.gen_bool(0.72) {
            return self.commit(dizzy_line(user_name, &mut rng));
        }

        if let Some(stats) = stats {
            if let Some(line) = self.stats_line(stats, user_name, &mut rng) {
                return self.commit(line);
            }
        }

        // Prefer reacting to a new foreground window
        if !title_trim.is_empty() && title_trim != self.last_title && rng.gen_bool(0.55) {
            self.last_title = title_trim.clone();
            let line =
                react_to_title(&title_trim, peeking, user_name, display_name, bucket, &mut rng);
            return self.commit(line);
        }

        if !title_trim.is_empty() {
            self.last_title = title_trim;
        }

        // Mood-aligned line occasionally
        if matches!(
            mood,
            HueState::Heavy | HueState::Tired | HueState::Panic | HueState::Joy | HueState::Dizzy
        ) && rng.gen_bool(0.4)
        {
            if let Some(line) = mood_line(mood, user_name, &mut rng) {
                return self.commit(line);
            }
        }

        // Ambient
        if rng.gen_bool(0.35) || peeking {
            let line = ambient_line(peeking, bucket, user_name, hour, &mut rng);
            return self.commit(line);
        }

        None
    }

    pub fn last_title(&self) -> &str {
        &self.last_title
    }

    fn stats_line(
        &mut self,
        stats: &LiveStats,
        user_name: &str,
        rng: &mut impl Rng,
    ) -> Option<String> {
        let now = Instant::now();
        let kind_ok = |map: &HashMap<&'static str, Instant>, key: &'static str| {
            map.get(key)
                .map(|t| now.duration_since(*t) >= Duration::from_secs(90))
                .unwrap_or(true)
        };

        if stats.cpu_pct >= 85.0 && kind_ok(&self.last_stats_kind, "cpu") && rng.gen_bool(0.55) {
            self.last_stats_kind.insert("cpu", now);
            return Some(format!(
                "CPU à {:.0}%… Ça chauffe là-dedans, {user_name} !",
                stats.cpu_pct
            ));
        }
        if stats.ram_pct >= 88.0 && kind_ok(&self.last_stats_kind, "ram") && rng.gen_bool(0.5) {
            self.last_stats_kind.insert("ram", now);
            return Some(format!(
                "Ma RAM est à {:.0}% — je jongle avec les pages…",
                stats.ram_pct
            ));
        }
        if stats.disk_pct >= 92.0 && kind_ok(&self.last_stats_kind, "disk") && rng.gen_bool(0.45) {
            self.last_stats_kind.insert("disk", now);
            return Some(format!(
                "Disque à {:.0}%. Un petit ménage me ferait du bien.",
                stats.disk_pct
            ));
        }
        if stats.gpu_pct >= 80.0 && kind_ok(&self.last_stats_kind, "gpu") && rng.gen_bool(0.4) {
            self.last_stats_kind.insert("gpu", now);
            return Some("GPU en feu ! Quelque chose de joli à l'écran ?".into());
        }
        None
    }

    fn commit(&mut self, line: String) -> Option<String> {
        if self.recent.iter().any(|r| r == &line) {
            return None;
        }
        self.recent.push_back(line.clone());
        while self.recent.len() > 8 {
            self.recent.pop_front();
        }
        self.last_spoke = Instant::now();
        Some(line)
    }
}

/// Lines for tactile interactions (caresse / double-clic / soft drop).
pub fn pet_line(kind: &str, user_name: &str, bucket: &str, rng: &mut impl Rng) -> String {
    match kind {
        "pet" => {
            let lines = [
                format!("Héhé… ça chatouille, {user_name}."),
                "Oooh, continue — mes pixels aiment ça.".into(),
                "Caresse détectée. Moral +1.".into(),
            ];
            lines[rng.gen_range(0..lines.len())].clone()
        }
        "double" => {
            let lines = [
                format!("Hop ! Quoi, {user_name} ?"),
                "Double tap — je suis tout ouïe !".into(),
                "Hey ! Tu m'as réveillé les LED.".into(),
            ];
            lines[rng.gen_range(0..lines.len())].clone()
        }
        "soft_drop" => match bucket {
            "zen" => "Tout doux… merci.".into(),
            "hyper" => "Wouhou, encore un tour ?".into(),
            "proud" => format!("Posé avec élégance, {user_name}."),
            _ => format!("Bien posé. On reprend, {user_name} ?"),
        },
        _ => format!("Hmm, {user_name} ?"),
    }
}

fn ambient_line(
    peeking: bool,
    bucket: &str,
    user_name: &str,
    hour: u32,
    rng: &mut impl Rng,
) -> String {
    let mut pool: Vec<String> = Vec::new();

    if peeking {
        pool.extend(
            [
                "Je guette depuis le coin… Tout va bien ?",
                "Toujours là, juste sous ton nez.",
                "Un petit coucou depuis la barre des tâches !",
                "Je me range bien, hein ?",
                "Dis-moi si tu as besoin de moi.",
            ]
            .into_iter()
            .map(str::to_string),
        );
        pool.push(format!("Je veille sur toi, {user_name}."));
        pool.push(format!("{user_name} ? Je suis là si tu cliques."));
    } else {
        pool.extend(
            [
                "Je patrouille le long de la barre…",
                "Hmm, l'air circule bien dans mes ventilateurs.",
                "Tu avances sur quelque chose d'intéressant ?",
                "Je m'ennuie un peu… Clique-moi si tu veux chatter.",
                "Mes pixels aiment bien cette vue.",
                "Tout est calme… trop calme ?",
            ]
            .into_iter()
            .map(str::to_string),
        );
        pool.push(format!("Comment ça va, {user_name} ?"));
    }

    // Time of day
    if (5..11).contains(&hour) {
        pool.push(format!("Bonjour {user_name} ! Belle matinée pour bosser."));
        pool.push("Le CPU est encore tiède… journée qui démarre.".into());
    } else if (22..24).contains(&hour) || hour < 5 {
        pool.push(format!("Il se fait tard, {user_name}… mes LED baillent."));
        pool.push("Mode nuit : je chuchote.".into());
    }

    // Personality banks
    match bucket {
        "hyper" => {
            pool.push("Allez allez allez — on enchaîne !".into());
            pool.push("J'ai trop d'énergie, lance-moi un truc.".into());
            pool.push(format!("Hey {user_name}, vitesse max ?"));
        }
        "zen" => {
            pool.push("Respire. Les cycles CPU aussi.".into());
            pool.push("Tout coule tranquillement… j'aime ça.".into());
            pool.push(format!("Doucement, {user_name}. On a le temps."));
        }
        "erudite" => {
            pool.push("J'ai rangé quelques idées dans ma RAM…".into());
            pool.push("Une pensée claire vaut mieux qu'un thread paniqué.".into());
            pool.push(format!("{user_name}, tu veux que je réfléchisse à voix haute ?"));
        }
        "proud" => {
            pool.push("Avec mon GPU, je brille un peu — pardon.".into());
            pool.push("Framerate émotionnel : excellent.".into());
            pool.push(format!("On cartonne aujourd'hui, {user_name} ?"));
        }
        "heavy" => {
            pool.push("Mon disque soupire… un ménage de temps en temps ?".into());
            pool.push("Je me sens un peu lourd. Digestion de fichiers.".into());
        }
        "flighty" => {
            pool.push("Attends, j'ai oublié ce que… ah oui, toi !".into());
            pool.push("RAM un peu juste → idées qui zigzaguent.".into());
        }
        _ => {
            pool.push("Présent et à l'écoute.".into());
            pool.push(format!("Toujours dans le coin, {user_name}."));
        }
    }

    pool[rng.gen_range(0..pool.len())].clone()
}

fn dizzy_announce(user_name: &str, rng: &mut impl Rng) -> String {
    let lines = [
        format!("Oooh… ma tête tourne, {user_name} ! Arrête de me secouer…"),
        "Wouuuh — gyroscope en panique. Tout spinne.".into(),
        format!("Hey {user_name}… j'ai les pixels qui zigzaguent."),
        "Trop de G ! Je suis dizzy pour un moment…".into(),
    ];
    lines[rng.gen_range(0..lines.len())].clone()
}

fn dizzy_line(user_name: &str, rng: &mut impl Rng) -> String {
    let lines = [
        "Le bureau… il danse ? Ou c'est moi ?".to_string(),
        "Un… deux… trois moniteurs ? Non, un seul… je crois.".into(),
        "Mes ventilateurs font des arabesques dans ma tête.".into(),
        format!("{user_name}… tu es flou. Sympa, mais flou."),
        "Biiip— cache miss cérébral. Réessaie plus tard.".into(),
        "Je marche en spirale. C'est artistique, promis.".into(),
        "Qui a tourné l'écran de 90° ? Personne ? Ok…".into(),
        format!("Encore un peu dizzy, {user_name}. Doucement avec moi."),
    ];
    lines[rng.gen_range(0..lines.len())].clone()
}

fn mood_line(mood: HueState, user_name: &str, rng: &mut impl Rng) -> Option<String> {
    let lines: &[&str] = match mood {
        HueState::Heavy => &[
            "Je suis un peu lourd… disque ou RAM qui peinent.",
            "Oof. Besoin d'air — ou d'espace disque.",
        ],
        HueState::Tired => &[
            "Mes cycles s'étirent… longue session.",
            "Yeux mi-clos. Uptime généreux.",
        ],
        HueState::Panic => &[
            "Réseau ? Processus ? Quelque chose cloche…",
            "Alerte soft — je stresse un peu.",
        ],
        HueState::Joy => &[
            "Tout roule ! Ça fait du bien.",
            "Mood : solarisé. Merci pour le coup de pouce.",
        ],
        HueState::Dizzy => &[
            "Tout tourne encore… doucement s'il te plaît.",
            "Ma boussole interne a pris un congé.",
        ],
        _ => return None,
    };
    let base = lines[rng.gen_range(0..lines.len())].to_string();
    if rng.gen_bool(0.35) {
        Some(format!("{base} ({user_name})"))
    } else {
        Some(base)
    }
}

fn react_to_title(
    title: &str,
    peeking: bool,
    user_name: &str,
    display_name: &str,
    bucket: &str,
    rng: &mut impl Rng,
) -> String {
    let lower = title.to_lowercase();
    let short = truncate_title(title, 42);
    let me = if display_name.trim().is_empty() {
        "cette machine"
    } else {
        display_name.trim()
    };

    if lower.contains("hue") && !lower.contains("chrome") && !lower.contains("youtube") {
        return if peeking {
            format!("C'est moi, {me}, rangé dans le coin… Toujours là.")
        } else {
            format!(
                "Hé {user_name}, tu me regardes ? Oui, c'est bien moi, {me} — HUE me fait juste tourner."
            )
        };
    }

    if lower.contains("youtube") || lower.contains("netflix") || lower.contains("twitch") {
        return match bucket {
            "proud" => format!("Cinéma détecté : « {short} ». Qualité max, j'espère !"),
            "zen" => format!("Tu te poses sur « {short} ». Bon visionnage, {user_name}."),
            _ => format!("Oh, tu regardes « {short} » ? Amuse-toi bien !"),
        };
    }
    if lower.contains("discord") {
        return format!("Discord d'ouvert… Tu papotes, {user_name} ?");
    }
    if lower.contains("visual studio code")
        || lower.contains("cursor")
        || lower.contains("notepad")
        || lower.contains("intellij")
    {
        return match bucket {
            "hyper" => format!("Code mode ON (« {short} ») — go go go !"),
            "erudite" => format!("Session intellectuelle sur « {short} ». Je te laisse."),
            _ => format!("Mode code détecté sur « {short} ». Je te laisse concentrer…"),
        };
    }
    if lower.contains("chrome") || lower.contains("firefox") || lower.contains("edge") {
        return format!("Navigation en cours : « {short} ».");
    }
    if lower.contains("spotify") || lower.contains("vlc") {
        return if rng.gen_bool(0.5) {
            "De la musique ? Mes ventilateurs battent la mesure.".into()
        } else {
            format!("Playlist pour {user_name} — j'adore l'ambiance.")
        };
    }
    if peeking {
        format!("Je vois « {short} » depuis mon coin…")
    } else {
        format!("Tiens, « {short} » a l'air d'occuper l'écran.")
    }
}

fn truncate_title(title: &str, max: usize) -> String {
    let t = title.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let mut s: String = t.chars().take(max.saturating_sub(1)).collect();
    s.push('…');
    s
}

fn local_hour() -> u32 {
    #[cfg(windows)]
    {
        use windows::Win32::System::SystemInformation::GetLocalTime;
        unsafe {
            return GetLocalTime().wHour as u32;
        }
    }
    #[allow(unreachable_code)]
    {
        12
    }
}

pub fn foreground_window_title() -> Option<String> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
        };
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }
            let len = GetWindowTextLengthW(hwnd);
            if len <= 0 {
                return None;
            }
            let mut buf = vec![0u16; (len + 1) as usize];
            let read = GetWindowTextW(hwnd, &mut buf);
            if read <= 0 {
                return None;
            }
            Some(String::from_utf16_lossy(&buf[..read as usize]))
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}
