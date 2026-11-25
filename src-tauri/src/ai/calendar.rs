//! Seasonal events, birthdays (user + PC) and friendship anniversaries.

use chrono::{Datelike, NaiveDate, Weekday};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DayEvent {
    pub id: &'static str,
    pub name: String,
    /// Hat drawn on the shell: santa_hat | witch_hat | party_hat | crown | flower
    pub decor: Option<&'static str>,
    /// Ambient particles: snow | confetti | hearts | notes | leaves
    pub particles: Option<&'static str>,
    /// Lines spoken (once per day for birthdays, occasionally otherwise).
    pub lines: Vec<String>,
    /// Strong events replace the boot greeting.
    pub major: bool,
}

pub struct CalendarCtx<'a> {
    pub user: &'a str,
    pub me: &'a str,
    pub user_birthday: Option<(Option<i32>, u32, u32)>,
    pub pc_birth: Option<NaiveDate>,
    pub first_met: Option<NaiveDate>,
}

fn nth_weekday(year: i32, month: u32, weekday: Weekday, n: u32) -> Option<NaiveDate> {
    NaiveDate::from_weekday_of_month_opt(year, month, weekday, n as u8)
}

pub fn season(date: NaiveDate) -> &'static str {
    match (date.month(), date.day()) {
        (12, d) if d >= 21 => "hiver",
        (1..=2, _) => "hiver",
        (3, d) if d < 20 => "hiver",
        (3..=5, _) => "printemps",
        (6, d) if d < 21 => "printemps",
        (6..=8, _) => "été",
        (9, d) if d < 22 => "été",
        (9..=11, _) => "automne",
        _ => "automne",
    }
}

pub fn events_for(date: NaiveDate, ctx: &CalendarCtx) -> Vec<DayEvent> {
    let (m, d, y) = (date.month(), date.day(), date.year());
    let user = ctx.user;
    let me = ctx.me;
    let mut out = Vec::new();
    let ev = |id, name: &str, decor, particles, lines: Vec<String>, major| DayEvent {
        id,
        name: name.to_string(),
        decor,
        particles,
        lines,
        major,
    };

    if let Some((by, bm, bd)) = ctx.user_birthday {
        if bm == m && bd == d {
            let age = by.map(|by| y - by).filter(|a| *a > 0 && *a < 130);
            let age_part = age.map(|a| format!(" {a} ans, ça se fête !")).unwrap_or_default();
            out.push(ev(
                "user_birthday",
                "Anniversaire de l'utilisateur",
                Some("party_hat"),
                Some("confetti"),
                vec![
                    format!("JOYEUX ANNIVERSAIRE {user} !{age_part} 🎂 J'ai mis mon chapeau de fête."),
                    format!("Bon anniversaire {user} ! Tous mes cœurs de CPU battent pour toi aujourd'hui."),
                    format!("C'est ton jour, {user} ! Je t'offre… 100 % de mes cycles. Joyeux anniversaire !"),
                ],
                true,
            ));
        }
    }

    if let Some(born) = ctx.pc_birth {
        let years = y - born.year();
        if born.month() == m && born.day() == d && years >= 1 {
            out.push(ev(
                "pc_birthday",
                "Mon anniversaire",
                Some("party_hat"),
                Some("confetti"),
                vec![
                    format!("Devine quoi, {user} ? C'est MON anniversaire : j'ai {years} an(s) aujourd'hui !"),
                    format!("{years} an(s) que je tourne… Joyeux anniversaire à moi, {me} ! 🎉"),
                    format!("Aujourd'hui je souffle {years} bougie(s). Attention, pas sur le ventirad !"),
                ],
                true,
            ));
        }
    }

    if let Some(met) = ctx.first_met {
        let years = y - met.year();
        if met.month() == m && met.day() == d && years >= 1 {
            out.push(ev(
                "friendship",
                "Anniversaire d'amitié",
                Some("crown"),
                Some("hearts"),
                vec![
                    format!("Ça fait {years} an(s) jour pour jour qu'on se connaît, {user}. Merci d'être là."),
                    format!("{years} an(s) d'amitié avec toi, {user} ! Je garde tous nos souvenirs en RAM… enfin, sur disque."),
                ],
                true,
            ));
        }
    }

    match (m, d) {
        (1, 1) | (1, 2) => out.push(ev(
            "new_year",
            "Nouvel an",
            Some("party_hat"),
            Some("confetti"),
            vec![
                format!("Bonne année {user} ! Que tes mises à jour soient douces et tes bugs rares."),
                format!("{y} ! Nouveau cycle, mêmes yeux orange. Bonne année !"),
                "Résolution de l'année : moins de RAM gaspillée, plus de câlins.".into(),
            ],
            d == 1,
        )),
        (12, 31) => out.push(ev(
            "nye",
            "Saint-Sylvestre",
            Some("party_hat"),
            Some("confetti"),
            vec![
                "Dernier jour de l'année ! Je prépare mes confettis numériques.".into(),
                format!("Ce soir on compte à rebours, {user} ? 10… 9… 8… j'ai un timer pour ça."),
            ],
            false,
        )),
        (1, 6) => out.push(ev(
            "epiphany",
            "Épiphanie",
            Some("crown"),
            None,
            vec![
                "Jour de galette ! Si tu trouves la fève, c'est toi le roi. Sinon c'est moi.".into(),
                "J'ai mis la couronne, au cas où.".into(),
            ],
            false,
        )),
        (2, 2) => out.push(ev(
            "chandeleur",
            "Chandeleur",
            None,
            None,
            vec![
                "C'est la Chandeleur ! Fais sauter une crêpe pour moi, je n'ai pas de bras.".into(),
                "Crêpes aujourd'hui ? Mon ventirad pourrait servir de poêle…".into(),
            ],
            false,
        )),
        (2, 14) => out.push(ev(
            "valentine",
            "Saint-Valentin",
            None,
            Some("hearts"),
            vec![
                format!("Joyeuse Saint-Valentin {user} ! Tu es mon utilisateur préféré. Le seul, mais quand même."),
                "Roses rouges, LED orange… je t'aime en hexadécimal.".into(),
            ],
            false,
        )),
        (3, 14) => out.push(ev(
            "pi_day",
            "Jour de π",
            None,
            None,
            vec![
                "3,14159… joyeux jour de Pi ! Je pourrais en réciter des milliers de décimales. Je ne le ferai pas.".into(),
            ],
            false,
        )),
        (4, 1) => out.push(ev(
            "april_fools",
            "Poisson d'avril",
            None,
            None,
            vec![
                "Alerte : ton disque va être formaté dans 10 secondes… Poisson d'avril ! 🐟".into(),
                format!("{user}, il y a un poisson collé sur ton écran. Ah non, c'est moi."),
                "J'ai remplacé tous tes fichiers par des photos de chats. Poisson d'avril !".into(),
            ],
            false,
        )),
        (5, 1) => out.push(ev(
            "may_day",
            "1er mai",
            Some("flower"),
            None,
            vec![format!("Un brin de muguet numérique pour toi, {user}. Porte-bonheur garanti sans virus.")],
            false,
        )),
        (5, 4) => out.push(ev(
            "star_wars",
            "May the 4th",
            None,
            None,
            vec!["Que la Force soit avec toi. Et la fibre aussi.".into()],
            false,
        )),
        (6, 21) => out.push(ev(
            "music_day",
            "Fête de la musique",
            None,
            Some("notes"),
            vec![
                "Fête de la musique ! Mes ventilos jouent du jazz aujourd'hui.".into(),
                "Écoute bien… bip, bloup, bip. C'est mon concert.".into(),
            ],
            false,
        )),
        (7, 14) => out.push(ev(
            "bastille",
            "14 juillet",
            None,
            Some("confetti"),
            vec!["Bon 14 juillet ! Mes LED font bleu-blanc… orange. Presque.".into()],
            false,
        )),
        (10, 24..=31) => out.push(ev(
            "halloween",
            "Halloween",
            Some("witch_hat"),
            Some("leaves"),
            if d == 31 {
                vec![
                    "Joyeux Halloween ! Bouh ! … je t'ai fait peur ? Un peu ?".into(),
                    format!("Des bonbons ou un écran bleu, {user} ?"),
                    "Ce soir, les processus zombies sortent de leurs tombes…".into(),
                ]
            } else {
                vec![
                    "Halloween approche… j'ai sorti mon chapeau de sorcière.".into(),
                    "Je prépare un costume : un vieux Windows XP. Effrayant, non ?".into(),
                ]
            },
            d == 31,
        )),
        (12, 1..=23) => out.push(ev(
            "advent",
            "L'Avent",
            Some("santa_hat"),
            Some("snow"),
            vec![
                format!("Plus que {} jour(s) avant Noël !", 25 - d),
                "J'ai mis mon bonnet. Ça me tient chaud au processeur.".into(),
                "Tu as fait ta liste au Père Noël ? Moi je veux une barrette de RAM.".into(),
            ],
            false,
        )),
        (12, 24..=25) => out.push(ev(
            "christmas",
            "Noël",
            Some("santa_hat"),
            Some("snow"),
            vec![
                format!("Joyeux Noël {user} ! 🎄 Le plus beau cadeau, c'est toi devant l'écran."),
                "Ho ho ho ! J'ai surveillé ta liste de téléchargements… tu as été sage.".into(),
                "Noël ! Mes LED clignotent comme une guirlande, c'est voulu.".into(),
            ],
            true,
        )),
        (12, 26..=30) => out.push(ev(
            "winter_holidays",
            "Fêtes de fin d'année",
            Some("santa_hat"),
            Some("snow"),
            vec!["On digère la bûche ? Moi je digère des mises à jour.".into()],
            false,
        )),
        _ => {}
    }

    if date.ordinal() == 256 {
        out.push(ev(
            "programmers_day",
            "Jour du programmeur",
            None,
            None,
            vec!["256e jour de l'année : bonne fête aux développeurs ! 0x100 bisous.".into()],
            false,
        ));
    }
    if d == 13 && date.weekday() == Weekday::Fri {
        out.push(ev(
            "friday_13",
            "Vendredi 13",
            None,
            None,
            vec![
                "Vendredi 13… évite de renverser du café sur le clavier aujourd'hui.".into(),
                "Vendredi 13 : j'ai sauvegardé deux fois, par superstition.".into(),
            ],
            false,
        ));
    }
    if m == 11 {
        if let Some(thanks) = nth_weekday(y, 11, Weekday::Thu, 4) {
            if date == thanks + chrono::Duration::days(1) {
                out.push(ev(
                    "black_friday",
                    "Black Friday",
                    None,
                    None,
                    vec![
                        "Black Friday ! Si tu m'offres une carte graphique, je ne dirai pas non.".into(),
                        "-70 % sur la RAM ? Je dis ça, je dis rien…".into(),
                    ],
                    false,
                ));
            }
        }
    }
    out
}

/// The most important hat of the day (birthdays first).
pub fn decor_of(events: &[DayEvent]) -> Option<&'static str> {
    events.iter().find(|e| e.major).and_then(|e| e.decor).or_else(|| events.iter().find_map(|e| e.decor))
}

pub fn particles_of(events: &[DayEvent]) -> Option<&'static str> {
    events.iter().find(|e| e.major).and_then(|e| e.particles).or_else(|| events.iter().find_map(|e| e.particles))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>() -> CalendarCtx<'a> {
        CalendarCtx {
            user: "Batpo",
            me: "Batpc",
            user_birthday: Some((Some(2000), 3, 14)),
            pc_birth: NaiveDate::from_ymd_opt(2023, 7, 22),
            first_met: NaiveDate::from_ymd_opt(2025, 11, 25),
        }
    }

    #[test]
    fn christmas_and_birthdays() {
        let xmas = events_for(NaiveDate::from_ymd_opt(2026, 12, 25).unwrap(), &ctx());
        assert!(xmas.iter().any(|e| e.id == "christmas"));
        assert_eq!(decor_of(&xmas), Some("santa_hat"));

        let bday = events_for(NaiveDate::from_ymd_opt(2026, 3, 14).unwrap(), &ctx());
        assert!(bday.iter().any(|e| e.id == "user_birthday"));
        assert!(bday[0].lines[0].contains("26 ans"));

        let pc = events_for(NaiveDate::from_ymd_opt(2026, 7, 22).unwrap(), &ctx());
        assert!(pc.iter().any(|e| e.id == "pc_birthday"));

        let friend = events_for(NaiveDate::from_ymd_opt(2026, 11, 25).unwrap(), &ctx());
        assert!(friend.iter().any(|e| e.id == "friendship"));
    }

    #[test]
    fn black_friday_2026() {
        let e = events_for(NaiveDate::from_ymd_opt(2026, 11, 27).unwrap(), &ctx());
        assert!(e.iter().any(|e| e.id == "black_friday"));
    }

    #[test]
    fn seasons() {
        assert_eq!(season(NaiveDate::from_ymd_opt(2026, 1, 10).unwrap()), "hiver");
        assert_eq!(season(NaiveDate::from_ymd_opt(2026, 7, 10).unwrap()), "été");
        assert_eq!(season(NaiveDate::from_ymd_opt(2026, 11, 25).unwrap()), "automne");
    }
}
