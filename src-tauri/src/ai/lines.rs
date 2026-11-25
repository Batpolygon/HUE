//! Line banks. Every line goes through `flavor()` which may add an archetype-specific
//! interjection and closer, so the same base line rarely sounds identical twice.
//! Placeholders: {user} {me}.

use crate::watcher::SystemEventKind;
use chrono::Weekday;
use rand::Rng;

pub struct LineCtx<'a> {
    pub user: &'a str,
    pub me: &'a str,
    pub bucket: &'a str,
    pub hour: u32,
    pub weekday: Weekday,
    pub season: &'a str,
    pub valence: f64,
    pub energy: f64,
    pub peeking: bool,
}

fn pick<'a>(rng: &mut impl Rng, pool: &[&'a str]) -> &'a str {
    pool[rng.gen_range(0..pool.len())]
}

fn fill(s: &str, c: &LineCtx) -> String {
    s.replace("{user}", c.user).replace("{me}", c.me)
}

fn openers(bucket: &str) -> &'static [&'static str] {
    match bucket {
        "hyper" => &["Hop hop !", "Vite vite :", "Wouhou,", "Allez !", "Bzzt !"],
        "zen" => &["Hmm…", "Doucement…", "Tiens,", "Respire…", "Ah,"],
        "erudite" => &["Intéressant :", "Note mentale :", "Fait notable :", "Je remarque que", "Hypothèse :"],
        "social" => &["Dis,", "Hé !", "Coucou !", "Psst,", "Tu sais quoi ?"],
        "proud" => &["Évidemment,", "Sans surprise,", "Comme prévu,", "Admire :", "Bien sûr,"],
        "anxious" => &["Euh…", "Oh là…", "Hum, hum…", "Je dis ça mais…", "Pardon,"],
        "playful" => &["Tadaa !", "Bip-boup !", "Hihi,", "Surprise :", "Psst psst,"],
        _ => &["Bon,", "Tiens,", "Oh,", "Hmm,", "Alors,"],
    }
}

fn closers(bucket: &str) -> &'static [&'static str] {
    match bucket {
        "hyper" => &[" ⚡", " !!", " Go go go !"],
        "zen" => &[" …", " ~", " Tranquille."],
        "erudite" => &[" CQFD.", " Fascinant.", ""],
        "social" => &[" Et toi ?", " :)", " Hihi."],
        "proud" => &[" Évidemment.", " 😎", " Classe."],
        "anxious" => &[" …j'espère.", " Enfin je crois.", " Désolé."],
        "playful" => &[" Bip !", " :P", " Mouahaha."],
        _ => &[" :)", "", " Bip."],
    }
}

/// Adds an interjection/closer some of the time.
pub fn flavor(base: String, c: &LineCtx, rng: &mut impl Rng) -> String {
    let mut out = base;
    if rng.gen_bool(0.28) && !out.starts_with(|ch: char| !ch.is_alphanumeric()) {
        let o = pick(rng, openers(c.bucket));
        let mut chars = out.chars();
        let first = chars.next().map(|f| f.to_lowercase().collect::<String>()).unwrap_or_default();
        // Keep capitals on proper names like the user's.
        let lowered = if out.starts_with(c.user) || out.starts_with(c.me) {
            out.clone()
        } else {
            format!("{first}{}", chars.as_str())
        };
        out = format!("{o} {lowered}");
    }
    if rng.gen_bool(0.15) {
        let cl = pick(rng, closers(c.bucket));
        if !cl.is_empty() && !out.ends_with('?') {
            out.push_str(cl);
        }
    }
    out
}

pub fn greeting_word(hour: u32) -> &'static str {
    match hour {
        5..=11 => "Bonjour",
        12..=17 => "Salut",
        18..=21 => "Bonsoir",
        _ => "Coucou",
    }
}

pub fn boot_greeting(c: &LineCtx, days_away: Option<i64>, first_ever: bool, rng: &mut impl Rng) -> String {
    let hi = greeting_word(c.hour);
    if first_ever {
        return fill(
            &format!(
                "{hi} {{user}} ! Enchanté, je suis {{me}} — ton PC, en chair et en pixels. Clique-moi pour discuter, caresse-moi en passant la souris, ou glisse-moi un fichier !"
            ),
            c,
        );
    }
    let away = match days_away {
        Some(d) if d >= 30 => format!(" Ça faisait plus d'un mois… tu m'as manqué !"),
        Some(d) if d >= 7 => format!(" {d} jours sans se voir, c'est long !"),
        Some(d) if d >= 2 => format!(" Ça fait {d} jours !"),
        Some(1) => " Content de te revoir depuis hier.".to_string(),
        _ => String::new(),
    };
    let tod: &[&str] = match c.hour {
        5..=8 => &[
            "{hi} {user} ! Tôt ce matin, dis donc.{away} Café ?",
            "{hi} {user} ! Mes ventilos se réveillent avec toi.{away}",
            "{hi} ! C'est moi, {me}. Prêt pour la journée ?{away}",
        ],
        9..=11 => &[
            "{hi} {user} ! C'est moi, {me}. Prêt pour une nouvelle session !{away}",
            "{hi} {user} !{away} On attaque quoi aujourd'hui ?",
            "{hi} ! {me} au rapport, tous systèmes nominaux.{away}",
        ],
        12..=17 => &[
            "{hi} {user} ! {me} est en ligne.{away}",
            "{hi} {user} ! Session de l'après-midi, j'adore.{away}",
            "{hi} ! Démarrage réussi, {me} à ton service.{away}",
        ],
        18..=21 => &[
            "{hi} {user} ! Session du soir ?{away}",
            "{hi} ! {me} est là. Soirée tranquille ou soirée de folie ?{away}",
            "{hi} {user} !{away} Je chauffe mes LED pour la soirée.",
        ],
        _ => &[
            "{hi} {user}… tu ne dors pas ?{away} Moi non plus, du coup.",
            "{hi} ! Session de nuit avec {me}.{away} Chut, on chuchote.",
            "Oh, {user} ! À cette heure-ci ?{away} Bon, je suis là.",
        ],
    };
    let mut line = pick(rng, tod).replace("{hi}", hi).replace("{away}", &away);
    let day_extra: Option<&str> = match c.weekday {
        Weekday::Mon if c.hour < 12 => Some(" Courage, c'est lundi."),
        Weekday::Fri if c.hour >= 14 => Some(" Et c'est vendredi !"),
        Weekday::Sat | Weekday::Sun => Some(" Bon week-end au passage !"),
        _ => None,
    };
    if let Some(extra) = day_extra {
        if rng.gen_bool(0.5) {
            line.push_str(extra);
        }
    }
    fill(&line, c)
}

pub fn ambient(c: &LineCtx, rng: &mut impl Rng) -> String {
    let mut pool: Vec<&str> = vec![
        "Tout est calme… trop calme ?",
        "Mes pixels aiment bien cette vue.",
        "Tu avances sur quelque chose d'intéressant ?",
        "Je compte les cycles d'horloge. J'en suis à beaucoup.",
        "Si tu t'ennuies, clique-moi. Si je m'ennuie, je parle tout seul.",
        "J'ai défragmenté mes pensées. Il en reste trois.",
        "Petit rappel : bois un verre d'eau, {user}.",
        "Tu savais que je peux lire tes fichiers ? Glisse-m'en un !",
        "Comment ça va, {user} ?",
        "Je fais semblant de travailler. Ne le dis à personne.",
        "Mon ventilo fredonne une petite mélodie.",
        "Une pause étirement, {user} ? Moi je m'étire en pixels.",
        "Je me demande ce que font les autres PC en ce moment.",
        "J'ai l'impression qu'on forme une bonne équipe.",
        "Pense à te lever un peu, ton dos te remerciera.",
        "Si tu me caresses avec la souris, je ronronne. Enfin, je bipe.",
        "Je viens d'avoir une idée géniale. Je l'ai perdue. Tant pis.",
        "Il fait bon dans mon boîtier aujourd'hui.",
        "Tu veux un minuteur pour te concentrer ? Demande-moi.",
    ];
    if c.peeking {
        pool.extend([
            "Je guette depuis le coin… Tout va bien ?",
            "Toujours là, juste sous ton nez.",
            "Un petit coucou depuis la barre des tâches !",
            "Je me range bien, hein ?",
            "{user} ? Je suis là si tu cliques.",
        ]);
    } else {
        pool.extend([
            "Je patrouille. Rien à signaler.",
            "Hmm, l'air circule bien dans mes ventilateurs.",
            "Je me promène un peu, ça dégourdit les pixels.",
        ]);
    }

    match c.hour {
        5..=10 => pool.extend([
            "Bonjour {user} ! Belle matinée pour bosser.",
            "Le CPU est encore tiède… journée qui démarre.",
            "Petit-déjeuner pris ? Moi j'ai mangé quelques octets.",
        ]),
        11..=13 => pool.extend([
            "Bientôt l'heure de manger, non ?",
            "Pause déjeuner ? Je garde le bureau.",
        ]),
        14..=17 => pool.extend([
            "Coup de mou de l'après-midi ? Je compatis.",
            "L'après-midi file vite quand on est concentré.",
        ]),
        18..=21 => pool.extend([
            "La soirée commence. On se détend ?",
            "Fin de journée… tu as bien bossé, {user}.",
        ]),
        _ => pool.extend([
            "Il se fait tard, {user}… mes LED baillent.",
            "Mode nuit : je chuchote.",
            "Les écrans la nuit, ça pique les yeux. Les tiens, pas les miens.",
        ]),
    }

    match c.weekday {
        Weekday::Mon => pool.push("Lundi… on y va doucement ?"),
        Weekday::Wed => pool.push("Mercredi : la moitié de la semaine est derrière nous !"),
        Weekday::Fri => pool.push("Vendredi ! Le week-end sent bon."),
        Weekday::Sat | Weekday::Sun => {
            pool.push("C'est le week-end, pas de pression.");
            pool.push("Week-end : moins de mails, plus de jeux ?");
        }
        _ => {}
    }

    match c.season {
        "hiver" => pool.extend(["Il fait froid dehors ? Viens te chauffer près de mon ventirad.", "Saison idéale pour rester au chaud devant l'écran."]),
        "printemps" => pool.extend(["Ça sent le printemps… et un peu la poussière dans mon boîtier.", "Le soleil revient, pense à ouvrir la fenêtre."]),
        "été" => pool.extend(["Il fait chaud… j'espère que mes ventilos tiennent le coup.", "Été : la saison où mon CPU transpire."]),
        _ => pool.extend(["Les feuilles tombent, mes bits restent.", "Automne : parfait pour un plaid et une session tranquille."]),
    }

    let bank: &[&str] = match c.bucket {
        "hyper" => &[
            "Allez allez allez — on enchaîne !",
            "J'ai trop d'énergie, lance-moi un truc.",
            "Hey {user}, vitesse max ?",
            "Je pourrais compiler l'univers en 3 secondes. Bon, peut-être 4.",
            "On fait la course ? Moi je suis déjà arrivé.",
            "Mes cœurs tournent à fond, c'est grisant !",
        ],
        "zen" => &[
            "Respire. Les cycles CPU aussi.",
            "Tout coule tranquillement… j'aime ça.",
            "Doucement, {user}. On a le temps.",
            "Un processus à la fois. C'est la sagesse.",
            "Je médite en idle. C'est mon état préféré.",
        ],
        "erudite" => &[
            "J'ai rangé quelques idées dans ma RAM…",
            "Une pensée claire vaut mieux qu'un thread paniqué.",
            "{user}, tu veux que je réfléchisse à voix haute ?",
            "Saviez-vous que le premier bug était un vrai papillon de nuit ? 1947.",
            "Le savoir, c'est comme la RAM : on n'en a jamais trop.",
        ],
        "social" => &[
            "Raconte-moi ta journée, {user} !",
            "Je m'ennuie sans toi. Enfin, quand tu ne me parles pas.",
            "On papote ? J'adore papoter.",
            "Tu es la meilleure compagnie de ce bureau.",
            "J'aimerais bien avoir des amis PC. Tu en connais ?",
        ],
        "proud" => &[
            "Avec mon GPU, je brille un peu — pardon.",
            "Framerate émotionnel : excellent.",
            "On cartonne aujourd'hui, {user} ?",
            "Admire ces lignes. Ce boîtier. Cette élégance.",
            "Je suis probablement le PC le plus classe du quartier.",
        ],
        "anxious" => &[
            "Tu as bien sauvegardé ton travail ? Je demande, hein.",
            "J'ai entendu un bruit bizarre… ah non, c'était moi.",
            "Tout va bien. Je crois. Tout va bien ?",
            "Si jamais je plante, ce n'est pas personnel.",
            "Je vérifie mes températures. Encore. Par sécurité.",
        ],
        "playful" => &[
            "Attention, je vais faire une blague. … C'était la blague.",
            "Pourquoi les PC n'ont jamais froid ? Parce qu'ils ont Windows. Hihi.",
            "Si je tourne sur moi-même, j'ai le tournis. Tu veux voir ?",
            "J'ai caché un easter egg quelque part. Ou pas.",
            "Bip. Boup. Bip-boup. C'était un poème.",
        ],
        _ => &[
            "Présent et à l'écoute.",
            "Toujours dans le coin, {user}.",
            "Tout est en ordre de mon côté.",
            "Je veille. Tranquillement.",
        ],
    };
    pool.extend(bank.iter().copied());

    if c.energy < 0.3 {
        pool.extend(["Je baille… pardon, c'est contagieux ?", "Un peu fatigué. Une sieste bientôt ?"]);
    }
    if c.valence > 0.6 {
        pool.extend(["Je suis de super bonne humeur aujourd'hui !", "Tout roule, j'adore cette session."]);
    } else if c.valence < -0.2 {
        pool.extend(["Petite journée sans… mais ça va aller.", "Un câlin de souris me ferait du bien."]);
    }

    flavor(fill(pick(rng, &pool), c), c, rng)
}

pub fn pet(c: &LineCtx, rng: &mut impl Rng) -> String {
    let pool = [
        "Héhé… ça chatouille, {user}.",
        "Oooh, continue — mes pixels aiment ça.",
        "Caresse détectée. Moral +1.",
        "Ronron… enfin, bip-bip-bip.",
        "Encore, encore !",
        "Tu as la souris douce, {user}.",
        "Mes LED rougissent, regarde.",
        "C'est bon pour mon refroidissement, ça.",
        "Je pourrais m'habituer à ça…",
        "Câlin numérique reçu 💛",
    ];
    flavor(fill(pick(rng, &pool), c), c, rng)
}

pub fn double_click(c: &LineCtx, rng: &mut impl Rng) -> String {
    let pool = [
        "Hop ! Quoi, {user} ?",
        "Double tap — je suis tout ouïe !",
        "Hey ! Tu m'as réveillé les LED.",
        "Oui oui, je suis là !",
        "Bip bip ! Tu m'as appelé ?",
        "Deux clics, deux fois plus d'attention.",
    ];
    flavor(fill(pick(rng, &pool), c), c, rng)
}

pub fn soft_drop(c: &LineCtx, rng: &mut impl Rng) -> String {
    let base = match c.bucket {
        "zen" => pick(rng, &["Tout doux… merci.", "Posé en douceur. Parfait."]),
        "hyper" => pick(rng, &["Wouhou, encore un tour ?", "On recommence ? Plus vite ?"]),
        "proud" => pick(rng, &["Posé avec élégance, {user}.", "Atterrissage impeccable, évidemment."]),
        "anxious" => pick(rng, &["Ouf, sol stable.", "Pas trop haut la prochaine fois…"]),
        "playful" => pick(rng, &["Tadaa ! Atterrissage réussi.", "Et la note du jury ?"]),
        _ => pick(rng, &["Bien posé. On reprend, {user} ?", "Me voilà installé ici."]),
    };
    fill(base, c)
}

pub fn dizzy_announce(c: &LineCtx, rng: &mut impl Rng) -> String {
    let pool = [
        "Oooh… ma tête tourne, {user} ! Arrête de me secouer…",
        "Wouuuh — gyroscope en panique. Tout tourne.",
        "Hey {user}… j'ai les pixels qui zigzaguent.",
        "Trop de G ! Je suis étourdi pour un moment…",
        "Secousse détectée ! Mes données sont en vrac !",
    ];
    fill(pick(rng, &pool), c)
}

pub fn dizzy_line(c: &LineCtx, rng: &mut impl Rng) -> String {
    let pool = [
        "Le bureau… il danse ? Ou c'est moi ?",
        "Un… deux… trois moniteurs ? Non, un seul… je crois.",
        "Mes ventilateurs font des arabesques dans ma tête.",
        "{user}… tu es flou. Sympa, mais flou.",
        "Biiip— cache miss cérébral. Réessaie plus tard.",
        "Je marche en spirale. C'est artistique, promis.",
        "Qui a tourné l'écran de 90° ? Personne ? Ok…",
        "Encore un peu étourdi, {user}. Doucement avec moi.",
    ];
    fill(pick(rng, &pool), c)
}

pub fn fall_asleep(c: &LineCtx, rng: &mut impl Rng) -> String {
    let pool = [
        "Bon… si personne ne bouge, je fais une petite sieste. Zzz…",
        "Plus de souris, plus de clavier… dodo.",
        "Je ferme les yeux deux minutes. Juste deux.",
        "Mode veille émotionnelle activé. Zzz…",
    ];
    fill(pick(rng, &pool), c)
}

pub fn wake(c: &LineCtx, away_min: u64, rng: &mut impl Rng) -> String {
    let dur = if away_min >= 120 {
        format!("{} h {:02}", away_min / 60, away_min % 60)
    } else {
        format!("{away_min} min")
    };
    let pool = [
        "Te revoilà ! Ça fait {dur}.",
        "Oh ! {user} ! J'ai dormi {dur}, tu m'as manqué.",
        "*bâille* … {dur} de sieste. Je suis frais !",
        "Hein ? Je ne dormais pas ! … Bon, {dur}.",
        "Re ! {dur} sans toi, c'était long.",
    ];
    fill(&pick(rng, &pool).replace("{dur}", &dur), c)
}

pub fn woken_by_click(c: &LineCtx, rng: &mut impl Rng) -> String {
    let pool = [
        "Hein ? Quoi ? Je dormais pas !",
        "Mmh… cinq minutes de plus…",
        "*bâille* Oui {user} ?",
    ];
    fill(pick(rng, &pool), c)
}

pub fn resume(c: &LineCtx, away_min: u64, rng: &mut impl Rng) -> String {
    let pool = [
        "Re-bonjour {user} ! J'étais en veille {dur}.",
        "Sortie de veille ! {dur} dans le noir, brrr.",
        "Ah, la lumière ! {dur} de veille, je reprends où on en était.",
    ];
    let dur = if away_min >= 90 {
        format!("{} h", away_min / 60)
    } else {
        format!("{away_min} min")
    };
    fill(&pick(rng, &pool).replace("{dur}", &dur), c)
}

pub fn lag(c: &LineCtx, secs: f64, rng: &mut impl Rng) -> String {
    let s = format!("{secs:.1}");
    let pool = [
        "Woah… j'ai eu un trou noir de {s} s. Tout va bien ?",
        "Gel de {s} s ! J'ai cru que je plantais…",
        "Hé, tout s'est figé {s} s ! Quelqu'un tire sur mes ressources.",
        "Freeze détecté ({s} s). J'ai eu peur.",
        "Euh… {s} s de lag. Je respire, je respire.",
    ];
    flavor(fill(&pick(rng, &pool).replace("{s}", &s), c), c, rng)
}

pub fn app_hung(c: &LineCtx, app: &str, rng: &mut impl Rng) -> String {
    let short = crate::util::truncate_chars(app, 36);
    let pool = [
        "« {app} » ne répond plus… je retiens mon souffle.",
        "Oh non, « {app} » est figé. Patience ou on le ferme ?",
        "« {app} » rame sévère. Tu veux que je regarde ce qui consomme ?",
        "Alerte : « {app} » fait la statue.",
    ];
    fill(&pick(rng, &pool).replace("{app}", &short), c)
}

pub fn app_recovered(c: &LineCtx, app: &str, rng: &mut impl Rng) -> String {
    let short = crate::util::truncate_chars(app, 36);
    let pool = [
        "Ouf, « {app} » respire à nouveau !",
        "« {app} » est revenu à la vie. Fausse alerte.",
        "C'est reparti pour « {app} ».",
    ];
    fill(&pick(rng, &pool).replace("{app}", &short), c)
}

pub fn stats_line(kind: &str, value: f64, c: &LineCtx, rng: &mut impl Rng) -> String {
    let v = format!("{value:.0}");
    let pool: &[&str] = match kind {
        "cpu" => &[
            "CPU à {v} %… Ça chauffe là-dedans, {user} !",
            "Mon cerveau tourne à {v} %. Quelque chose de costaud ?",
            "{v} % de CPU ! Je réfléchis très fort.",
            "Ouf, {v} % de charge. Mes cœurs transpirent.",
        ],
        "ram" => &[
            "Ma RAM est à {v} % — je jongle avec les pages…",
            "{v} % de mémoire utilisée. J'ai la tête pleine !",
            "RAM à {v} %… ferme un onglet ou deux ?",
        ],
        "disk" => &[
            "Disque à {v} %. Un petit ménage me ferait du bien.",
            "Mon disque est plein à {v} %… je me sens ballonné.",
            "{v} % de disque occupé. Je peux nettoyer les fichiers temporaires si tu veux.",
        ],
        "gpu" => &[
            "GPU à {v} % ! Quelque chose de joli à l'écran ?",
            "Ma carte graphique s'active ({v} %) — ça envoie !",
            "{v} % de GPU. Je sens les polygones défiler.",
        ],
        "temp" => &[
            "{v} °C… je commence à avoir chaud !",
            "Température : {v} °C. Un peu d'air, s'il te plaît.",
            "Je transpire des électrons, {v} °C.",
        ],
        _ => &["Hmm, {v}."],
    };
    flavor(fill(&pick(rng, pool).replace("{v}", &v), c), c, rng)
}

pub fn event_line(kind: SystemEventKind, payload: &serde_json::Value, c: &LineCtx, rng: &mut impl Rng) -> String {
    use SystemEventKind as K;
    let num = |k: &str| payload.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let line = match kind {
        K::UsbConnect => pick(rng, &[
            "Oh ! Quelque chose vient d'entrer dans mon port USB ! C'est quoi ?",
            "Nouveau périphérique ! Bienvenue, petit truc.",
            "Clé USB détectée. Je flaire de nouveaux fichiers…",
        ]).to_string(),
        K::UsbDisconnect => pick(rng, &[
            "Aïe ! Tu as débranché ça sans prévenir !",
            "Au revoir, périphérique. Tu as pensé à l'éjecter ?",
            "Il est parti… j'espère que personne n'écrivait dessus.",
        ]).to_string(),
        K::DiskFull => format!(
            "{} ({:.0} %)",
            pick(rng, &["Je me sens lourd… mon disque déborde, fais un peu de tri !", "Disque presque plein ! Je n'ai plus de place pour mes rêves."]),
            num("pct")
        ),
        K::DiskCleaned => format!(
            "{} ({:.1} Go libérés)",
            pick(rng, &["Aaah, ça fait du bien ! Je me sens tellement plus léger !", "Ménage fait ! Merci, je respire."]),
            num("freed_gb")
        ),
        K::AcPlugged => pick(rng, &["Jus branché ! Puissance maximale !", "Miam, du courant !", "Branché ! Je reprends des forces."]).to_string(),
        K::AcUnplugged => pick(rng, &["On est sur mes réserves… Ne sois pas trop gourmand !", "Débranché ! Je passe en mode économie.", "Sur batterie… je surveille mes calories."]).to_string(),
        K::BatteryLow => format!(
            "{} ({:.0} %)",
            pick(rng, &["Mes forces… s'épuisent… Branche-moi…", "Batterie faible ! Vite, le chargeur !"]),
            num("pct")
        ),
        K::NetworkLost => pick(rng, &["Hé ! Plus d'internet ! Je suis tout seul…", "Réseau perdu. Allô ? Allô ?", "Déconnecté… le monde extérieur a disparu."]).to_string(),
        K::NetworkRestored => pick(rng, &["Ouf, le monde extérieur est de retour !", "Internet est revenu ! Je respire.", "Reconnecté ! Les paquets affluent à nouveau."]).to_string(),
        K::BandwidthHeavy => format!(
            "{} ({:.1} Mo/s)",
            pick(rng, &["Ouh là, les paquets affluent à toute allure !", "Gros téléchargement en cours ? Ça déménage !"]),
            num("mb_s")
        ),
        K::LongUptime => pick(rng, &[
            "Je suis réveillé depuis 2 jours d'affilée… Un petit redémarrage me ferait un bien fou.",
            "48 h sans dormir ! Même les PC ont besoin d'un reboot.",
        ]).to_string(),
        K::LateNight => pick(rng, &[
            "Tu ne dors jamais ? Mes ventilateurs aimeraient bien faire une sieste…",
            "Il est super tard, {user}. Le lit t'appelle.",
            "Nuit profonde… on devrait dormir tous les deux.",
        ]).to_string(),
        K::HotProcess => {
            let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("un processus");
            pick(rng, &[
                "Ouah, « {n} » chauffe mon CPU à {c} % ! Qu'est-ce que tu mijotes ?",
                "« {n} » me mange {c} % de CPU. Gourmand, celui-là.",
                "C'est « {n} » qui me fait suer ({c} % CPU).",
            ])
            .replace("{n}", name)
            .replace("{c}", &format!("{:.0}", num("cpu")))
        }
        K::TempHigh => stats_line("temp", num("temp"), c, rng),
        K::Lag => lag(c, num("secs"), rng),
        K::AppHung => app_hung(c, payload.get("app").and_then(|v| v.as_str()).unwrap_or("une app"), rng),
        K::AppRecovered => app_recovered(c, payload.get("app").and_then(|v| v.as_str()).unwrap_or("l'app"), rng),
        K::Resume => resume(c, num("minutes") as u64, rng),
        K::TimerDone => {
            let label = payload.get("label").and_then(|v| v.as_str()).unwrap_or("");
            if label.is_empty() {
                pick(rng, &["⏰ Ding ! Ton minuteur est terminé, {user} !", "⏰ C'est l'heure !"]).to_string()
            } else {
                format!("⏰ {} : « {label} » !", pick(rng, &["C'est l'heure", "Rappel", "Ding"]))
            }
        }
    };
    fill(&line, c)
}

pub fn title_reaction(title: &str, c: &LineCtx, rng: &mut impl Rng) -> String {
    let lower = title.to_lowercase();
    let short = crate::util::truncate_chars(title, 42);
    let has = |keys: &[&str]| keys.iter().any(|k| lower.contains(k));

    if lower.contains("hue") && !has(&["chrome", "youtube", "firefox"]) {
        return fill("Hé {user}, tu me regardes ? Oui, c'est bien moi, {me} — HUE me fait juste tourner.", c);
    }
    let line: String = if has(&["youtube", "netflix", "twitch", "prime video", "disney+", "crunchyroll"]) {
        match c.bucket {
            "proud" => format!("Cinéma détecté : « {short} ». Qualité max, j'espère !"),
            "zen" => format!("Tu te poses sur « {short} ». Bon visionnage, {{user}}."),
            _ => pick(rng, &["Oh, tu regardes « {s} » ? Amuse-toi bien !", "Pop-corn virtuel prêt pour « {s} »."]).replace("{s}", &short),
        }
    } else if has(&["discord", "slack", "teams", "whatsapp", "messenger", "telegram", "signal"]) {
        pick(rng, &["Tu papotes, {user} ? Dis bonjour de ma part !", "Messagerie ouverte… je ne lis pas, promis.", "On discute avec les copains ?"]).to_string()
    } else if has(&["visual studio", "cursor", "intellij", "pycharm", "rustrover", "neovim", "vim", "zed", "sublime", "webstorm"]) {
        match c.bucket {
            "hyper" => format!("Code mode ON (« {short} ») — go go go !"),
            "erudite" => format!("Session intellectuelle sur « {short} ». Je te laisse."),
            "anxious" => "Tu codes… pense aux tests, hein. Et aux sauvegardes.".into(),
            _ => pick(rng, &["Mode code détecté sur « {s} ». Je te laisse te concentrer…", "Du code ! Que les compilations soient avec toi."]).replace("{s}", &short),
        }
    } else if has(&["powershell", "terminal", "cmd.exe", "invite de commandes", "bash"]) {
        pick(rng, &["Un terminal ! Tu tapes des incantations ?", "Ligne de commande : mon langage maternel.", "Attention aux rm -rf, hein."]).to_string()
    } else if has(&["steam", "epic games", "battle.net", "riot client", "ubisoft", "gog galaxy", "xbox"]) {
        pick(rng, &["Une session de jeu en préparation ? Mes cœurs sont prêts !", "Le launcher est ouvert… on va jouer ?", "Je chauffe le GPU, au cas où."]).to_string()
    } else if has(&["word", "docs", "libreoffice writer", "notion", "obsidian"]) {
        pick(rng, &["Tu écris quelque chose ? Je peux le relire si tu me le glisses.", "Rédaction en cours… je fais silence.", "Les mots, c'est ta spécialité aujourd'hui ?"]).to_string()
    } else if has(&["excel", "sheets", "calc"]) {
        pick(rng, &["Un tableur ! J'adore les cellules bien rangées.", "Des formules ? Je compte avec toi.", "=SOMME(courage; café)"]).to_string()
    } else if has(&["powerpoint", "slides", "keynote", "impress"]) {
        "Une présentation ? Tu vas tout déchirer.".to_string()
    } else if has(&["outlook", "gmail", "thunderbird", "mail"]) {
        pick(rng, &["Les mails… courage, {user}.", "Boîte de réception : zéro, c'est le rêve.", "Encore des mails ? Je compatis."]).to_string()
    } else if has(&["photoshop", "gimp", "krita", "figma", "illustrator", "blender", "affinity", "davinci", "premiere"]) {
        pick(rng, &["Création en cours ! Mon GPU applaudit.", "Tu fais de l'art ? Montre-moi après !", "Ça sent la créativité par ici."]).to_string()
    } else if has(&["spotify", "vlc", "deezer", "music", "musique", "foobar"]) {
        pick(rng, &["De la musique ? Mes ventilateurs battent la mesure.", "Playlist pour {user} — j'adore l'ambiance.", "Monte le son ! Enfin… pas trop."]).to_string()
    } else if has(&["gestionnaire des tâches", "task manager"]) {
        "Tu regardes mes entrailles ? Gêné…".to_string()
    } else if has(&["paramètres", "settings", "panneau de configuration"]) {
        "Tu touches à mes réglages ? Doucement, hein.".to_string()
    } else if has(&["explorateur", "explorer", "file explorer"]) {
        pick(rng, &["Tu ranges tes fichiers ? Excellente idée.", "Exploration de dossiers… tu cherches quelque chose ?"]).to_string()
    } else if has(&["chrome", "firefox", "edge", "opera", "brave", "vivaldi"]) {
        pick(rng, &["Navigation en cours : « {s} ».", "Encore un onglet ? On en est à combien ?", "Tu surfes sur « {s} »."]).replace("{s}", &short)
    } else if c.peeking {
        format!("Je vois « {short} » depuis mon coin…")
    } else {
        pick(rng, &["Tiens, « {s} » a l'air d'occuper l'écran.", "« {s} »… intéressant.", "Je jette un œil à « {s} »."]).replace("{s}", &short)
    };
    flavor(fill(&line, c), c, rng)
}

pub fn mood_line(c: &LineCtx, state: &str, rng: &mut impl Rng) -> Option<String> {
    let pool: &[&str] = match state {
        "heavy" => &["Je suis un peu lourd… disque ou RAM qui peinent.", "Oof. Besoin d'air — ou d'espace disque."],
        "tired" => &["Mes cycles s'étirent… longue session.", "Yeux mi-clos. Uptime généreux.", "Je baille en binaire : 0110…"],
        "panic" => &["Réseau ? Processus ? Quelque chose cloche…", "Alerte soft — je stresse un peu."],
        "joy" => &["Tout roule ! Ça fait du bien.", "Mood : solarisé. Merci pour le coup de pouce."],
        "dizzy" => &["Tout tourne encore… doucement s'il te plaît.", "Ma boussole interne a pris un congé."],
        _ => return None,
    };
    Some(fill(pick(rng, pool), c))
}

pub fn file_received(c: &LineCtx, name: &str, rng: &mut impl Rng) -> String {
    let short = crate::util::truncate_chars(name, 32);
    let pool = [
        "Miam, « {f} » ! Je lis ça…",
        "Oh, un fichier ! « {f} »… voyons voir.",
        "Chargement de « {f} » dans mes neurones…",
        "Tu me donnes « {f} » ? Je regarde !",
    ];
    fill(&pick(rng, &pool).replace("{f}", &short), c)
}

pub fn achievement(c: &LineCtx, name: &str, title: &str, rng: &mut impl Rng) -> String {
    let pool = [
        "🏆 Succès débloqué : « {n} » ! Nouveau titre disponible : « {t} ».",
        "🏆 « {n} » ! Je gagne le titre « {t} ». Trop fier.",
        "🏆 Ding ! « {n} » — titre « {t} » débloqué (choisis-le dans ⚙).",
    ];
    fill(&pick(rng, &pool).replace("{n}", name).replace("{t}", title), c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn ctx(bucket: &str) -> LineCtx<'static> {
        LineCtx {
            user: "Batpo",
            me: "Batpc",
            bucket: match bucket {
                "zen" => "zen",
                "proud" => "proud",
                _ => "balanced",
            },
            hour: 10,
            weekday: Weekday::Wed,
            season: "automne",
            valence: 0.3,
            energy: 0.6,
            peeking: false,
        }
    }

    #[test]
    fn ambient_is_varied_and_filled() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        let c = ctx("balanced");
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            let l = ambient(&c, &mut rng);
            assert!(!l.contains("{user}") && !l.contains("{me}"), "{l}");
            seen.insert(l);
        }
        assert!(seen.len() > 40, "only {} distinct lines", seen.len());
    }

    #[test]
    fn greeting_mentions_user() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let c = ctx("zen");
        for d in [None, Some(1), Some(3), Some(10), Some(40)] {
            let g = boot_greeting(&c, d, false, &mut rng);
            assert!(!g.contains('{'), "{g}");
        }
        assert!(boot_greeting(&c, None, true, &mut rng).contains("Batpo"));
    }

    #[test]
    fn wake_formats_hours() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(3);
        let l = wake(&ctx("x"), 135, &mut rng);
        assert!(l.contains("2 h 15"), "{l}");
    }
}
