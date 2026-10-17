use super::hardware::HardwareSnapshot;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemEventKind {
    UsbConnect,
    UsbDisconnect,
    AudioHeadset,
    DiskFull,
    DiskCleaned,
    AcPlugged,
    AcUnplugged,
    BatteryLow,
    NetworkLost,
    NetworkRestored,
    BandwidthHeavy,
    Boot,
    LongUptime,
    LateNight,
    HotProcess,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEvent {
    pub kind: SystemEventKind,
    pub message: String,
    pub payload: serde_json::Value,
}

pub struct OsWatcher {
    prev: HardwareSnapshot,
    prev_disk_free: f64,
    last_emit: HashMap<SystemEventKind, Instant>,
    /// Boot greeting skipped — frontend `hue-identity` welcome covers it.
    boot_emitted: bool,
    /// Calendar day (YYYYMMDD) when LateNight last fired — once per night.
    late_night_day: Option<u32>,
    last_net_rx: u64,
    heavy_net_streak: u8,
}

impl OsWatcher {
    pub fn new(snapshot: HardwareSnapshot) -> Self {
        Self {
            prev_disk_free: snapshot.disk_free_gb,
            prev: snapshot,
            last_emit: HashMap::new(),
            boot_emitted: false,
            late_night_day: None,
            last_net_rx: 0,
            heavy_net_streak: 0,
        }
    }

    pub fn poll(&mut self, cooldown_secs: u64) -> Vec<SystemEvent> {
        let now = Instant::now();
        let snap = HardwareSnapshot::collect();
        let mut events = Vec::new();

        let mut push = |kind: SystemEventKind, message: String, payload: serde_json::Value| {
            if cooldown_secs == u64::MAX {
                return;
            }
            if let Some(last) = self.last_emit.get(&kind) {
                if now.duration_since(*last) < Duration::from_secs(cooldown_secs) {
                    return;
                }
            }
            // Per-kind minimum anti-spam (at least 20s except boot)
            let min = if kind == SystemEventKind::Boot {
                Duration::from_secs(0)
            } else {
                Duration::from_secs(20)
            };
            if let Some(last) = self.last_emit.get(&kind) {
                if now.duration_since(*last) < min {
                    return;
                }
            }
            self.last_emit.insert(kind, now);
            events.push(SystemEvent {
                kind,
                message,
                payload,
            });
        };

        // Identity welcome already greets — skip Boot speech spam.
        if !self.boot_emitted {
            self.boot_emitted = true;
        }

        if snap.disk_usage_pct > 90.0 && self.prev.disk_usage_pct <= 90.0 {
            push(
                SystemEventKind::DiskFull,
                format!(
                    "Je me sens lourd... Mon disque {} va exploser, fais un peu de tri s'il te plaît !",
                    snap.disk_label
                ),
                serde_json::json!({ "usage_pct": snap.disk_usage_pct }),
            );
        }

        if snap.disk_free_gb > self.prev_disk_free + 2.0 {
            push(
                SystemEventKind::DiskCleaned,
                "Aaah, ça fait du bien ! Je me sens tellement plus léger !".into(),
                serde_json::json!({
                    "freed_gb": snap.disk_free_gb - self.prev_disk_free
                }),
            );
        }
        self.prev_disk_free = snap.disk_free_gb;

        match (self.prev.on_ac, snap.on_ac) {
            (Some(false), Some(true)) => push(
                SystemEventKind::AcPlugged,
                "Jus branché ! Puissance maximale !".into(),
                serde_json::json!({}),
            ),
            (Some(true), Some(false)) => push(
                SystemEventKind::AcUnplugged,
                "On est sur mes réserves... Ne sois pas trop gourmand !".into(),
                serde_json::json!({}),
            ),
            _ => {}
        }

        if let Some(pct) = snap.battery_pct {
            if pct < 15.0 && self.prev.battery_pct.unwrap_or(100.0) >= 15.0 {
                push(
                    SystemEventKind::BatteryLow,
                    "Mes forces... s'épuisent... Branche-moi...".into(),
                    serde_json::json!({ "battery_pct": pct }),
                );
            }
        }

        if self.prev.network_up && !snap.network_up {
            push(
                SystemEventKind::NetworkLost,
                "Je ne sens plus le monde extérieur ! Où est passé le Wi-Fi ?!".into(),
                serde_json::json!({}),
            );
        } else if !self.prev.network_up && snap.network_up {
            push(
                SystemEventKind::NetworkRestored,
                "Ouf, le monde extérieur est de retour !".into(),
                serde_json::json!({}),
            );
        }

        // Bandwidth heuristic via sysinfo network counters
        let nets = sysinfo::Networks::new_with_refreshed_list();
        let rx: u64 = nets.iter().map(|(_, d)| d.received()).sum();
        if self.last_net_rx > 0 {
            let delta = rx.saturating_sub(self.last_net_rx);
            if delta > 8_000_000 {
                self.heavy_net_streak = self.heavy_net_streak.saturating_add(1);
            } else {
                self.heavy_net_streak = 0;
            }
            if self.heavy_net_streak >= 2 {
                push(
                    SystemEventKind::BandwidthHeavy,
                    "Ouh là, les paquets affluent à toute allure !".into(),
                    serde_json::json!({ "rx_delta": delta }),
                );
                self.heavy_net_streak = 0;
            }
        }
        self.last_net_rx = rx;

        if snap.uptime_secs > 48 * 3600 && self.prev.uptime_secs <= 48 * 3600 {
            push(
                SystemEventKind::LongUptime,
                "Je suis réveillé depuis 2 jours d'affilée... Un petit redémarrage me ferait un bien fou.".into(),
                serde_json::json!({ "uptime_secs": snap.uptime_secs }),
            );
        }

        let now_local = Local::now();
        let hour = now_local.hour();
        if hour >= 2 && hour < 5 {
            let day_key = now_local.format("%Y%m%d").to_string().parse::<u32>().unwrap_or(0);
            if self.late_night_day != Some(day_key) {
                self.late_night_day = Some(day_key);
                push(
                    SystemEventKind::LateNight,
                    "Tu ne dors jamais ? Mes ventilateurs aimeraient bien faire une sieste...".into(),
                    serde_json::json!({ "hour": hour }),
                );
            }
        }

        if let Some(hot) = super::process::top_cpu_process() {
            if hot.cpu > 60.0 {
                push(
                    SystemEventKind::HotProcess,
                    format!(
                        "Ouah, « {} » chauffe mon CPU à {:.0}% ! Qu'est-ce que tu mijotes ?",
                        hot.name, hot.cpu
                    ),
                    serde_json::json!({ "name": hot.name, "cpu": hot.cpu }),
                );
            }
        }

        self.prev = snap;
        events
    }

    pub fn poll_usb(&mut self, prev_removable: &mut usize, cooldown_secs: u64) -> Vec<SystemEvent> {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        let removable = disks.list().iter().filter(|d| d.is_removable()).count();
        let mut events = Vec::new();
        if cooldown_secs == u64::MAX {
            *prev_removable = removable;
            return events;
        }
        let now = Instant::now();
        if removable > *prev_removable {
            let kind = SystemEventKind::UsbConnect;
            let ok = self
                .last_emit
                .get(&kind)
                .map(|t| now.duration_since(*t) >= Duration::from_secs(cooldown_secs.max(20)))
                .unwrap_or(true);
            if ok {
                self.last_emit.insert(kind, now);
                events.push(SystemEvent {
                    kind,
                    message: "Oh ! Quelque chose vient d'entrer dans mon port USB ! C'est quoi ?"
                        .into(),
                    payload: serde_json::json!({ "removable": removable }),
                });
            }
        } else if removable < *prev_removable {
            let kind = SystemEventKind::UsbDisconnect;
            let ok = self
                .last_emit
                .get(&kind)
                .map(|t| now.duration_since(*t) >= Duration::from_secs(cooldown_secs.max(20)))
                .unwrap_or(true);
            if ok {
                self.last_emit.insert(kind, now);
                events.push(SystemEvent {
                    kind,
                    message:
                        "Aïe ! Tu as débranché ça sans prévenir ! J'espère que personne n'écrivait dessus..."
                            .into(),
                    payload: serde_json::json!({ "removable": removable }),
                });
            }
        }
        *prev_removable = removable;
        events
    }
}

trait HourExt {
    fn hour(&self) -> u32;
}

impl HourExt for chrono::DateTime<Local> {
    fn hour(&self) -> u32 {
        use chrono::Timelike;
        Timelike::hour(self)
    }
}
