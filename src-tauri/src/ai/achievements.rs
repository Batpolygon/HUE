//! Achievements. Each one unlocks a title the user can wear as prefix or suffix
//! (e.g. "Vétéran Batpc le Câliné").

use crate::ai::memory::{PersistentMemory, Stats};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TitleKind {
    Prefix,
    Suffix,
}

pub struct Achievement {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub title: &'static str,
    pub kind: TitleKind,
    /// Hidden until unlocked.
    pub secret: bool,
    check: fn(&Stats, &Ctx) -> bool,
}

/// Context that isn't a plain counter.
pub struct Ctx {
    pub uptime_hours: u64,
    pub pc_age_years: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AchievementView {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub title: &'static str,
    pub kind: TitleKind,
    pub unlocked: bool,
    pub unlocked_at: Option<i64>,
    pub secret: bool,
}

macro_rules! ach {
    ($id:expr, $name:expr, $desc:expr, $title:expr, $kind:ident, $secret:expr, $check:expr) => {
        Achievement {
            id: $id,
            name: $name,
            description: $desc,
            title: $title,
            kind: TitleKind::$kind,
            secret: $secret,
            check: $check,
        }
    };
}

pub fn all() -> &'static [Achievement] {
    static LIST: std::sync::OnceLock<Vec<Achievement>> = std::sync::OnceLock::new();
    LIST.get_or_init(|| {
        vec![
            ach!("first_pet", "Première caresse", "Caresser HUE avec la souris.", "Doux", Prefix, false, |s, _| s.pets >= 1),
            ach!("pets_25", "Câlinothérapie", "25 caresses.", "le Câliné", Suffix, false, |s, _| s.pets >= 25),
            ach!("pets_100", "Animal de compagnie", "100 caresses.", "Adoré", Prefix, false, |s, _| s.pets >= 100),
            ach!("first_chat", "Premier mot", "Discuter une première fois.", "le Causeur", Suffix, false, |s, _| s.chats >= 1),
            ach!("chats_50", "Grand bavard", "50 messages échangés.", "Philosophe", Prefix, false, |s, _| s.chats >= 50),
            ach!("chats_250", "Confident", "250 messages échangés.", "le Confident", Suffix, false, |s, _| s.chats >= 250),
            ach!("first_file", "Rat de bibliothèque", "Lui faire lire un fichier.", "le Lecteur", Suffix, false, |s, _| s.files_read >= 1),
            ach!("files_10", "Érudition", "10 fichiers lus.", "Érudit", Prefix, false, |s, _| s.files_read >= 10),
            ach!("first_dizzy", "Tête qui tourne", "Le secouer jusqu'au tournis.", "l'Étourdi", Suffix, false, |s, _| s.dizzies >= 1),
            ach!("dizzy_10", "Toupie humaine", "10 tournis.", "Toupie", Prefix, false, |s, _| s.dizzies >= 10),
            ach!("tosses_10", "Voltige", "Le lancer 10 fois.", "le Voltigeur", Suffix, false, |s, _| s.tosses >= 10),
            ach!("first_lag", "Survivant", "Survivre à un freeze du PC.", "le Survivant", Suffix, false, |s, _| s.lags >= 1),
            ach!("lags_10", "Increvable", "Survivre à 10 freezes.", "Indestructible", Prefix, false, |s, _| s.lags >= 10),
            ach!("first_nap", "Sieste", "S'endormir quand tu t'absentes.", "le Dormeur", Suffix, false, |s, _| s.naps >= 1),
            ach!("peeks_20", "L'art de se cacher", "Être rangé 20 fois.", "le Discret", Suffix, false, |s, _| s.peeks >= 20),
            ach!("night_owl", "Oiseau de nuit", "Discuter entre 2 h et 5 h du matin.", "le Noctambule", Suffix, false, |s, _| s.night_chats >= 1),
            ach!("early_bird", "Lève-tôt", "Être allumé avant 7 h.", "Matinal", Prefix, false, |s, _| s.early_greets >= 1),
            ach!("streak_7", "Fidélité", "7 jours d'affilée ensemble.", "le Fidèle", Suffix, false, |s, _| s.best_streak >= 7),
            ach!("days_30", "Vieux copains", "30 jours passés ensemble.", "Vétéran", Prefix, false, |s, _| s.days_seen >= 30),
            ach!("days_365", "Une année ensemble", "365 jours passés ensemble.", "Légendaire", Prefix, false, |s, _| s.days_seen >= 365),
            ach!("uptime_48", "Marathonien", "48 h d'uptime continu.", "l'Infatigable", Suffix, false, |_, c| c.uptime_hours >= 48),
            ach!("timers_5", "Ponctualité", "5 minuteurs terminés.", "le Ponctuel", Suffix, false, |s, _| s.timers_done >= 5),
            ach!("tools_10", "Débrouillard", "Utiliser 10 fois ses outils.", "le Débrouillard", Suffix, false, |s, _| s.tools_used >= 10),
            ach!("voice_1", "À l'écoute", "Lui parler avec la voix.", "l'Attentif", Suffix, false, |s, _| s.voice_uses >= 1),
            ach!("birthday", "Fête !", "Fêter un anniversaire ensemble.", "le Festif", Suffix, false, |s, _| s.birthdays >= 1),
            ach!("old_pc", "Ancêtre", "Un PC de 5 ans ou plus.", "l'Ancien", Suffix, true, |_, c| c.pc_age_years.unwrap_or(0) >= 5),
            ach!("christmas", "Esprit de Noël", "Passer Noël ensemble.", "Givré", Prefix, true, |s, _| s.seasonal.iter().any(|x| x == "christmas")),
            ach!("halloween", "Bouh !", "Passer Halloween ensemble.", "le Spectral", Suffix, true, |s, _| s.seasonal.iter().any(|x| x == "halloween")),
            ach!("new_year", "Bonne année", "Fêter le Nouvel an ensemble.", "le Pétillant", Suffix, true, |s, _| s.seasonal.iter().any(|x| x == "new_year")),
            ach!("april_fools", "Farceur", "Survivre au 1er avril.", "Farceur", Prefix, true, |s, _| s.seasonal.iter().any(|x| x == "april_fools")),
            ach!("hours_100", "Centenaire", "100 heures de vie de HUE.", "Centenaire", Prefix, false, |s, _| s.minutes_alive >= 6000),
        ]
    })
}

/// Unlock what is newly earned. Returns the fresh unlocks.
pub fn check(mem: &mut PersistentMemory, ctx: &Ctx) -> Vec<&'static Achievement> {
    let now = crate::util::now_unix();
    let mut fresh = Vec::new();
    for a in all() {
        if mem.achievements.contains_key(a.id) {
            continue;
        }
        if (a.check)(&mem.stats, ctx) {
            mem.achievements.insert(a.id.to_string(), now);
            fresh.push(a);
        }
    }
    if !fresh.is_empty() {
        mem.mark();
    }
    fresh
}

pub fn views(mem: &PersistentMemory) -> Vec<AchievementView> {
    all()
        .iter()
        .map(|a| {
            let at = mem.achievements.get(a.id).copied();
            AchievementView {
                id: a.id,
                name: a.name,
                description: a.description,
                title: a.title,
                kind: a.kind,
                unlocked: at.is_some(),
                unlocked_at: at,
                secret: a.secret,
            }
        })
        .collect()
}

/// Is `title` unlocked for `kind`? (Empty title is always allowed.)
pub fn title_allowed(mem: &PersistentMemory, kind: TitleKind, title: &str) -> bool {
    title.trim().is_empty()
        || all().iter().any(|a| {
            a.kind == kind && a.title == title.trim() && mem.achievements.contains_key(a.id)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlocks_once() {
        let mut m = PersistentMemory::default();
        let ctx = Ctx { uptime_hours: 1, pc_age_years: Some(1) };
        assert!(check(&mut m, &ctx).is_empty());
        m.stats.pets = 1;
        let got = check(&mut m, &ctx);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].title, "Doux");
        assert!(check(&mut m, &ctx).is_empty());
        assert!(title_allowed(&m, TitleKind::Prefix, "Doux"));
        assert!(!title_allowed(&m, TitleKind::Suffix, "Doux"));
        assert!(!title_allowed(&m, TitleKind::Prefix, "Vétéran"));
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = all().iter().map(|a| a.id).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }
}
