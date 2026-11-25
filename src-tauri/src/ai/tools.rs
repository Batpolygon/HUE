//! Tools the LLM can call. Anything with side effects outside HUE (opening apps,
//! URLs, deleting temp files) asks the user first through a Oui/Non prompt in the HUD.

use crate::ai::achievements::{self, TitleKind};
use crate::state::{AppState, Timer};
use serde_json::{json, Value};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

fn f(name: &str, description: &str, params: Value) -> Value {
    json!({ "type": "function", "function": { "name": name, "description": description, "parameters": params } })
}

fn no_params() -> Value {
    json!({ "type": "object", "properties": {} })
}

pub fn specs() -> Value {
    json!([
        f("get_system_status", "État actuel du PC : CPU, RAM, disque, GPU, températures, batterie, réseau, uptime, processus les plus gourmands.", no_params()),
        f("get_datetime", "Date, heure, jour de la semaine, saison et évènements du jour.", no_params()),
        f("set_timer", "Programme un minuteur / rappel. HUE préviendra l'utilisateur à la fin.", json!({
            "type": "object",
            "properties": {
                "minutes": { "type": "number", "description": "Durée en minutes (0.5 à 1440)" },
                "label": { "type": "string", "description": "Texte du rappel, ex. 'pause café'" }
            },
            "required": ["minutes"]
        })),
        f("list_timers", "Liste les minuteurs en cours.", no_params()),
        f("cancel_timers", "Annule tous les minuteurs en cours.", no_params()),
        f("remember_fact", "Mémorise durablement un fait sur l'utilisateur (préférence, prénom, projet…).", json!({
            "type": "object",
            "properties": { "fact": { "type": "string" } },
            "required": ["fact"]
        })),
        f("forget_fact", "Oublie les souvenirs contenant ce texte.", json!({
            "type": "object",
            "properties": { "query": { "type": "string" } },
            "required": ["query"]
        })),
        f("search_documents", "Cherche dans les fichiers que l'utilisateur t'a fait lire (glissés sur toi).", json!({
            "type": "object",
            "properties": { "query": { "type": "string" } },
            "required": ["query"]
        })),
        f("list_documents", "Liste les fichiers déjà lus avec leur résumé.", no_params()),
        f("open_application", "Ouvre une application Windows courante (demande confirmation).", json!({
            "type": "object",
            "properties": { "name": { "type": "string", "description": "bloc-notes, calculatrice, explorateur, paramètres, gestionnaire des tâches, paint, terminal, corbeille, téléchargements, documents, capture" } },
            "required": ["name"]
        })),
        f("open_url", "Ouvre une adresse web http(s) dans le navigateur (demande confirmation).", json!({
            "type": "object",
            "properties": { "url": { "type": "string" } },
            "required": ["url"]
        })),
        f("clean_temp_files", "Calcule puis supprime (après confirmation) les fichiers temporaires de plus de 24 h.", no_params()),
        f("set_user_birthday", "Enregistre la date d'anniversaire de l'utilisateur pour la lui souhaiter.", json!({
            "type": "object",
            "properties": {
                "day": { "type": "integer" },
                "month": { "type": "integer" },
                "year": { "type": "integer", "description": "optionnel" }
            },
            "required": ["day", "month"]
        })),
        f("change_setting", "Change un réglage de HUE : talk_frequency (muet|discret|normal|bavard|hyperactif) ou movement_mode (taskbar|fullscreen|static).", json!({
            "type": "object",
            "properties": {
                "setting": { "type": "string", "enum": ["talk_frequency", "movement_mode"] },
                "value": { "type": "string" }
            },
            "required": ["setting", "value"]
        })),
        f("hue_action", "Fait faire une action au corps de HUE : sleep (dormir), wake (réveil), tuck (se ranger), dance (danser), show (sortir).", json!({
            "type": "object",
            "properties": { "action": { "type": "string", "enum": ["sleep", "wake", "tuck", "dance", "show"] } },
            "required": ["action"]
        })),
        f("get_achievements", "Succès débloqués / restants et titres disponibles.", no_params()),
        f("set_title", "Porte un titre débloqué en préfixe ou suffixe (chaîne vide pour retirer).", json!({
            "type": "object",
            "properties": {
                "prefix": { "type": "string" },
                "suffix": { "type": "string" }
            }
        })),
    ])
}

pub fn label(name: &str) -> &'static str {
    match name {
        "get_system_status" => "🔧 je regarde mes stats…",
        "get_datetime" => "🔧 je regarde l'heure…",
        "set_timer" | "list_timers" | "cancel_timers" => "⏰ minuteur…",
        "remember_fact" | "forget_fact" => "🧠 je mets à jour mes souvenirs…",
        "search_documents" | "list_documents" => "📄 je fouille mes documents…",
        "open_application" | "open_url" => "🚀 ouverture…",
        "clean_temp_files" => "🧹 je mesure les fichiers temporaires…",
        "set_user_birthday" => "🎂 je note la date…",
        "change_setting" => "⚙ réglage…",
        "hue_action" => "✨ action !",
        "get_achievements" | "set_title" => "🏆 mes succès…",
        _ => "🔧 outil…",
    }
}

/// Ask the user Oui/Non in the HUD. False on timeout.
pub async fn confirm(app: &AppHandle, text: String) -> bool {
    let state = app.state::<AppState>();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let id = {
        let mut next = state.confirm_seq.lock();
        *next += 1;
        *next
    };
    state.pending_confirms.lock().insert(id, tx);
    let _ = app.emit("hue-confirm", json!({ "id": id, "text": text }));
    let ok = matches!(tokio::time::timeout(Duration::from_secs(60), rx).await, Ok(Ok(true)));
    state.pending_confirms.lock().remove(&id);
    let _ = app.emit("hue-confirm-clear", json!({ "id": id }));
    ok
}

fn bump_tools(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut m = state.memory.lock();
    m.stats.tools_used += 1;
    m.mark();
}

pub async fn execute(app: &AppHandle, name: &str, args: &Value) -> String {
    bump_tools(app);
    let out = match name {
        "get_system_status" => system_status(),
        "get_datetime" => datetime(app),
        "set_timer" => set_timer(app, args),
        "list_timers" => list_timers(app),
        "cancel_timers" => {
            app.state::<AppState>().timers.lock().clear();
            json!({ "ok": true })
        }
        "remember_fact" => {
            let fact = args["fact"].as_str().unwrap_or("");
            let state = app.state::<AppState>();
            let added = state.memory.lock().remember(fact);
            state.memory.lock().save_if_needed(true);
            json!({ "ok": true, "new": added })
        }
        "forget_fact" => {
            let q = args["query"].as_str().unwrap_or("");
            let state = app.state::<AppState>();
            let n = state.memory.lock().forget(q);
            state.memory.lock().save_if_needed(true);
            json!({ "removed": n })
        }
        "search_documents" => {
            let q = args["query"].as_str().unwrap_or("");
            let hits = app.state::<AppState>().docs.lock().search(q, 4);
            json!({ "results": hits.iter().map(|h| json!({ "file": h.doc, "section": h.chunk + 1, "text": crate::util::truncate_chars(&h.text, 700) })).collect::<Vec<_>>() })
        }
        "list_documents" => {
            let docs = app.state::<AppState>().docs.lock().list();
            json!({ "documents": docs.iter().map(|d| json!({ "name": d.name, "kind": d.kind, "summary": crate::util::truncate_chars(&d.summary, 300) })).collect::<Vec<_>>() })
        }
        "open_application" => open_application(app, args["name"].as_str().unwrap_or("")).await,
        "open_url" => open_url(app, args["url"].as_str().unwrap_or("")).await,
        "clean_temp_files" => clean_temp(app).await,
        "set_user_birthday" => set_birthday(app, args),
        "change_setting" => change_setting(app, args),
        "hue_action" => hue_action(app, args["action"].as_str().unwrap_or("")),
        "get_achievements" => {
            let state = app.state::<AppState>();
            let mem = state.memory.lock();
            let v = achievements::views(&mem);
            json!({
                "unlocked": v.iter().filter(|a| a.unlocked).map(|a| json!({ "name": a.name, "title": a.title, "kind": a.kind })).collect::<Vec<_>>(),
                "locked": v.iter().filter(|a| !a.unlocked && !a.secret).map(|a| json!({ "name": a.name, "how": a.description })).collect::<Vec<_>>(),
                "secret_left": v.iter().filter(|a| !a.unlocked && a.secret).count(),
            })
        }
        "set_title" => set_title(app, args),
        _ => json!({ "error": format!("outil inconnu : {name}") }),
    };
    out.to_string()
}

fn system_status() -> Value {
    let s = crate::watcher::live_stats::collect();
    let cpu = crate::watcher::sysmon::top_processes(5, false);
    let ram = crate::watcher::sysmon::top_processes(5, true);
    json!({
        "cpu_pct": s.cpu_pct,
        "ram_pct": s.ram_pct,
        "ram_used_gb": s.ram_used_gb,
        "ram_total_gb": s.ram_total_gb,
        "disk_pct": s.disk_pct,
        "disk_free_gb": s.disk_free_gb,
        "gpu_pct": s.gpu_pct,
        "cpu_temp_c": s.cpu_temp,
        "gpu_temp_c": s.gpu_temp,
        "battery_pct": s.battery_pct,
        "on_ac": s.on_ac,
        "uptime_hours": (s.uptime_secs as f64 / 3600.0 * 10.0).round() / 10.0,
        "network_up": crate::watcher::os_events::has_default_route(),
        "top_cpu": cpu.iter().map(|p| json!({ "name": p.name, "cpu_pct": (p.cpu * 10.0).round() / 10.0 })).collect::<Vec<_>>(),
        "top_ram": ram.iter().map(|p| json!({ "name": p.name, "ram_mb": p.ram_mb })).collect::<Vec<_>>(),
    })
}

fn datetime(app: &AppHandle) -> Value {
    let now = chrono::Local::now();
    let state = app.state::<AppState>();
    let events: Vec<String> = state.today_events.lock().iter().map(|e| e.name.clone()).collect();
    json!({
        "date": now.format("%Y-%m-%d").to_string(),
        "time": now.format("%H:%M").to_string(),
        "weekday": now.format("%A").to_string(),
        "season": crate::ai::calendar::season(now.date_naive()),
        "events_today": events,
    })
}

fn set_timer(app: &AppHandle, args: &Value) -> Value {
    let minutes = args["minutes"].as_f64().unwrap_or(0.0);
    if !(0.5..=1440.0).contains(&minutes) {
        return json!({ "error": "durée entre 0.5 et 1440 minutes" });
    }
    let label = args["label"].as_str().unwrap_or("").trim().to_string();
    let due = crate::util::now_unix() + (minutes * 60.0).round() as i64;
    let state = app.state::<AppState>();
    let mut timers = state.timers.lock();
    if timers.len() >= 10 {
        return json!({ "error": "trop de minuteurs (10 max)" });
    }
    timers.push(Timer { due, label: label.clone() });
    let at = chrono::DateTime::from_timestamp(due, 0)
        .map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_default();
    json!({ "ok": true, "rings_at": at, "label": label })
}

fn list_timers(app: &AppHandle) -> Value {
    let now = crate::util::now_unix();
    let timers = app.state::<AppState>().timers.lock().clone();
    json!({ "timers": timers.iter().map(|t| json!({ "label": t.label, "minutes_left": ((t.due - now) as f64 / 60.0 * 10.0).round() / 10.0 })).collect::<Vec<_>>() })
}

fn app_command(name: &str) -> Option<(String, Vec<String>, &'static str)> {
    let n = name.trim().to_lowercase();
    let has = |keys: &[&str]| keys.iter().any(|k| n.contains(k));
    let dir = |d: Option<std::path::PathBuf>| d.map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    Some(if has(&["bloc", "notepad", "note"]) {
        ("notepad.exe".into(), vec![], "le Bloc-notes")
    } else if has(&["calc"]) {
        ("calc.exe".into(), vec![], "la Calculatrice")
    } else if has(&["gestionnaire", "task"]) {
        ("taskmgr.exe".into(), vec![], "le Gestionnaire des tâches")
    } else if has(&["param", "setting", "réglage"]) {
        ("explorer.exe".into(), vec!["ms-settings:".into()], "les Paramètres")
    } else if has(&["paint"]) {
        ("mspaint.exe".into(), vec![], "Paint")
    } else if has(&["terminal", "powershell", "console", "cmd"]) {
        ("wt.exe".into(), vec![], "le Terminal")
    } else if has(&["corbeille", "recycle"]) {
        ("explorer.exe".into(), vec!["shell:RecycleBinFolder".into()], "la Corbeille")
    } else if has(&["téléchargement", "telechargement", "download"]) {
        ("explorer.exe".into(), vec![dir(dirs::download_dir())], "le dossier Téléchargements")
    } else if has(&["document"]) {
        ("explorer.exe".into(), vec![dir(dirs::document_dir())], "le dossier Documents")
    } else if has(&["image", "photo"]) {
        ("explorer.exe".into(), vec![dir(dirs::picture_dir())], "le dossier Images")
    } else if has(&["capture", "snip", "screenshot"]) {
        ("explorer.exe".into(), vec!["ms-screenclip:".into()], "la capture d'écran")
    } else if has(&["explor", "fichier", "dossier"]) {
        ("explorer.exe".into(), vec![], "l'Explorateur")
    } else {
        return None;
    })
}

async fn open_application(app: &AppHandle, name: &str) -> Value {
    let Some((exe, args, pretty)) = app_command(name) else {
        return json!({ "error": "application non autorisée", "allowed": "bloc-notes, calculatrice, explorateur, paramètres, gestionnaire des tâches, paint, terminal, corbeille, téléchargements, documents, images, capture" });
    };
    if !confirm(app, format!("J'ouvre {pretty} ?")).await {
        return json!({ "ok": false, "reason": "l'utilisateur a refusé" });
    }
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(&args);
    match cmd.spawn() {
        Ok(_) => json!({ "ok": true, "opened": pretty }),
        Err(e) if exe == "wt.exe" => match std::process::Command::new("cmd.exe").args(["/c", "start", "powershell"]).spawn() {
            Ok(_) => json!({ "ok": true, "opened": "PowerShell" }),
            Err(_) => json!({ "ok": false, "error": e.to_string() }),
        },
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    }
}

async fn open_url(app: &AppHandle, url: &str) -> Value {
    let u = url.trim();
    if !(u.starts_with("https://") || u.starts_with("http://")) || u.len() > 2000 {
        return json!({ "error": "seules les adresses http(s) sont autorisées" });
    }
    if !confirm(app, format!("J'ouvre {} dans ton navigateur ?", crate::util::truncate_chars(u, 80))).await {
        return json!({ "ok": false, "reason": "l'utilisateur a refusé" });
    }
    match app.opener().open_url(u, None::<&str>) {
        Ok(()) => json!({ "ok": true }),
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    }
}

fn old_temp_files() -> (Vec<std::path::PathBuf>, u64) {
    let root = std::env::temp_dir();
    let cutoff = std::time::SystemTime::now() - Duration::from_secs(24 * 3600);
    let mut files = Vec::new();
    let mut total = 0u64;
    let mut stack = vec![(root, 0u32)];
    let mut visited = 0usize;
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            visited += 1;
            if visited > 50_000 {
                return (files, total);
            }
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                if depth < 4 && !meta.file_type().is_symlink() {
                    stack.push((e.path(), depth + 1));
                }
            } else if meta.modified().map(|m| m < cutoff).unwrap_or(false) {
                total += meta.len();
                files.push(e.path());
            }
        }
    }
    (files, total)
}

async fn clean_temp(app: &AppHandle) -> Value {
    let (files, total) = tokio::task::spawn_blocking(old_temp_files).await.unwrap_or_default();
    if files.is_empty() {
        return json!({ "ok": true, "deleted": 0, "message": "rien à nettoyer" });
    }
    let size = crate::ai::documents::human_size(total);
    if !confirm(
        app,
        format!("Je supprime {} fichier(s) temporaire(s) de plus de 24 h ({size}) ?", files.len()),
    )
    .await
    {
        return json!({ "ok": false, "reason": "l'utilisateur a refusé", "found": files.len(), "size": size });
    }
    let (deleted, freed) = tokio::task::spawn_blocking(move || {
        let mut n = 0usize;
        let mut freed = 0u64;
        for p in files {
            let len = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            // Files in use are simply skipped.
            if std::fs::remove_file(&p).is_ok() {
                n += 1;
                freed += len;
            }
        }
        (n, freed)
    })
    .await
    .unwrap_or((0, 0));
    json!({ "ok": true, "deleted": deleted, "freed": crate::ai::documents::human_size(freed) })
}

fn save_settings(app: &AppHandle) {
    let state = app.state::<AppState>();
    let s = state.settings.lock().clone();
    let _ = s.save();
    let _ = app.emit("hue-settings-changed", ());
}

fn set_birthday(app: &AppHandle, args: &Value) -> Value {
    let d = args["day"].as_u64().unwrap_or(0) as u32;
    let m = args["month"].as_u64().unwrap_or(0) as u32;
    if chrono::NaiveDate::from_ymd_opt(2024, m, d).is_none() {
        return json!({ "error": "date invalide" });
    }
    let value = match args["year"].as_i64() {
        Some(y) if (1900..=2100).contains(&y) => format!("{y:04}-{m:02}-{d:02}"),
        _ => format!("{m:02}-{d:02}"),
    };
    app.state::<AppState>().settings.lock().user_birthday = value.clone();
    save_settings(app);
    crate::brain::refresh_today(app);
    json!({ "ok": true, "saved": value })
}

fn change_setting(app: &AppHandle, args: &Value) -> Value {
    let setting = args["setting"].as_str().unwrap_or("");
    let value = args["value"].as_str().unwrap_or("");
    let state = app.state::<AppState>();
    let ok = match setting {
        "talk_frequency" => crate::config::TalkFrequency::from_label(value)
            .map(|v| state.settings.lock().talk_frequency = v)
            .is_some(),
        "movement_mode" => crate::config::MovementMode::from_label(value)
            .map(|v| state.settings.lock().movement_mode = v)
            .is_some(),
        _ => false,
    };
    if ok {
        save_settings(app);
        json!({ "ok": true })
    } else {
        json!({ "error": "réglage ou valeur inconnus" })
    }
}

fn hue_action(app: &AppHandle, action: &str) -> Value {
    let state = app.state::<AppState>();
    let area = state.cached_area.lock().clone();
    let mut body = state.body.lock();
    match action {
        "sleep" => body.fall_asleep(),
        "wake" => body.wake_up(),
        "dance" => body.start_micro("dance", 2600),
        "tuck" => {
            drop(body);
            crate::tray::peek_hue(app);
            return json!({ "ok": true });
        }
        "show" => {
            body.wake_up();
            if body.peeking {
                body.peeking = false;
                let (x, y) = crate::physics::PhysicsBody::emerge_position(&area, body.width, body.height);
                body.x = x;
                body.y = y;
            }
        }
        _ => return json!({ "error": "action inconnue" }),
    }
    json!({ "ok": true })
}

fn set_title(app: &AppHandle, args: &Value) -> Value {
    let state = app.state::<AppState>();
    let prefix = args["prefix"].as_str();
    let suffix = args["suffix"].as_str();
    {
        let mem = state.memory.lock();
        if let Some(p) = prefix {
            if !achievements::title_allowed(&mem, TitleKind::Prefix, p) {
                return json!({ "error": format!("préfixe « {p} » pas encore débloqué") });
            }
        }
        if let Some(s) = suffix {
            if !achievements::title_allowed(&mem, TitleKind::Suffix, s) {
                return json!({ "error": format!("suffixe « {s} » pas encore débloqué") });
            }
        }
    }
    {
        let mut s = state.settings.lock();
        if let Some(p) = prefix {
            s.title_prefix = p.trim().to_string();
        }
        if let Some(x) = suffix {
            s.title_suffix = x.trim().to_string();
        }
    }
    save_settings(app);
    crate::commands::refresh_title(app);
    json!({ "ok": true })
}
