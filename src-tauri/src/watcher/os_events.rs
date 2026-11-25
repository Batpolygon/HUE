use super::live_stats::LiveStats;
use chrono::{Local, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use sysinfo::{Disks, Networks};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemEventKind {
    UsbConnect,
    UsbDisconnect,
    DiskFull,
    DiskCleaned,
    AcPlugged,
    AcUnplugged,
    BatteryLow,
    NetworkLost,
    NetworkRestored,
    BandwidthHeavy,
    LongUptime,
    LateNight,
    HotProcess,
    TempHigh,
    /// The whole system stalled (our heartbeat thread missed its beat).
    Lag,
    /// Foreground app is "Not responding".
    AppHung,
    AppRecovered,
    /// Woke up after the PC was suspended.
    Resume,
    TimerDone,
}

/// Raw detection — wording is chosen later by `ai::lines`.
#[derive(Debug, Clone)]
pub struct Detected {
    pub kind: SystemEventKind,
    pub payload: serde_json::Value,
}

pub struct OsWatcher {
    prev: Option<LiveStats>,
    prev_disk_free: f64,
    prev_network_up: bool,
    last_emit: HashMap<SystemEventKind, Instant>,
    late_night_day: Option<u32>,
    networks: Networks,
    heavy_net_streak: u8,
    hot_streak: u8,
    removable: usize,
    disks: Disks,
    disks_at: Instant,
    last_poll: Instant,
}

impl OsWatcher {
    pub fn new() -> Self {
        let disks = Disks::new_with_refreshed_list();
        let removable = disks.list().iter().filter(|d| d.is_removable()).count();
        Self {
            prev: None,
            prev_disk_free: 0.0,
            prev_network_up: has_default_route(),
            last_emit: HashMap::new(),
            late_night_day: None,
            networks: Networks::new_with_refreshed_list(),
            heavy_net_streak: 0,
            hot_streak: 0,
            removable,
            disks,
            disks_at: Instant::now(),
            last_poll: Instant::now(),
        }
    }

    fn allowed(&mut self, kind: SystemEventKind, cooldown_secs: u64) -> bool {
        if cooldown_secs == u64::MAX {
            return false;
        }
        let now = Instant::now();
        let min = Duration::from_secs(cooldown_secs.max(20));
        if let Some(last) = self.last_emit.get(&kind) {
            if now.duration_since(*last) < min {
                return false;
            }
        }
        self.last_emit.insert(kind, now);
        true
    }

    /// Lightweight: reuses the live stats already sampled this tick.
    pub fn poll(&mut self, stats: &LiveStats, cooldown_secs: u64) -> Vec<Detected> {
        let mut out = Vec::new();
        let mut push = |this: &mut Self, kind: SystemEventKind, payload: serde_json::Value| {
            if this.allowed(kind, cooldown_secs) {
                out.push(Detected { kind, payload });
            }
        };

        // Only run the heavier checks every ~3 s.
        if self.last_poll.elapsed() < Duration::from_secs(3) && self.prev.is_some() {
            return out;
        }
        let elapsed = self.last_poll.elapsed().as_secs_f64().max(1.0);
        self.last_poll = Instant::now();

        let Some(prev) = self.prev.clone() else {
            self.prev_disk_free = stats.disk_free_gb;
            self.prev = Some(stats.clone());
            return out;
        };

        if stats.disk_pct > 90.0 && prev.disk_pct <= 90.0 {
            push(self, SystemEventKind::DiskFull, serde_json::json!({ "pct": stats.disk_pct }));
        }
        if stats.disk_free_gb > self.prev_disk_free + 2.0 {
            let freed = stats.disk_free_gb - self.prev_disk_free;
            push(self, SystemEventKind::DiskCleaned, serde_json::json!({ "freed_gb": freed }));
        }
        self.prev_disk_free = stats.disk_free_gb;

        match (prev.on_ac, stats.on_ac) {
            (Some(false), Some(true)) => push(self, SystemEventKind::AcPlugged, serde_json::json!({})),
            (Some(true), Some(false)) => {
                push(self, SystemEventKind::AcUnplugged, serde_json::json!({}))
            }
            _ => {}
        }
        if let Some(pct) = stats.battery_pct {
            if stats.on_ac != Some(true) && pct < 15.0 && prev.battery_pct.unwrap_or(100.0) >= 15.0 {
                push(self, SystemEventKind::BatteryLow, serde_json::json!({ "pct": pct }));
            }
        }

        let up = has_default_route();
        if self.prev_network_up && !up {
            push(self, SystemEventKind::NetworkLost, serde_json::json!({}));
        } else if !self.prev_network_up && up {
            push(self, SystemEventKind::NetworkRestored, serde_json::json!({}));
        }
        self.prev_network_up = up;

        // Persistent Networks → `received()` is the delta since the last refresh.
        self.networks.refresh(true);
        let rx: u64 = self.networks.iter().map(|(_, d)| d.received()).sum();
        let per_sec = rx as f64 / elapsed;
        if per_sec > 3_000_000.0 {
            self.heavy_net_streak = self.heavy_net_streak.saturating_add(1);
        } else {
            self.heavy_net_streak = 0;
        }
        if self.heavy_net_streak >= 3 {
            self.heavy_net_streak = 0;
            push(
                self,
                SystemEventKind::BandwidthHeavy,
                serde_json::json!({ "mb_s": per_sec / 1_000_000.0 }),
            );
        }

        if stats.uptime_secs > 48 * 3600 && prev.uptime_secs <= 48 * 3600 {
            push(self, SystemEventKind::LongUptime, serde_json::json!({ "hours": stats.uptime_secs / 3600 }));
        }

        let now_local = Local::now();
        let hour = now_local.hour();
        if (2..5).contains(&hour) {
            let day_key = now_local.format("%Y%m%d").to_string().parse::<u32>().unwrap_or(0);
            if self.late_night_day != Some(day_key) {
                self.late_night_day = Some(day_key);
                push(self, SystemEventKind::LateNight, serde_json::json!({ "hour": hour }));
            }
        }

        // Needs two consecutive hot samples so a short spike isn't worth a comment.
        match super::sysmon::top_cpu_process() {
            Some(hot) if hot.cpu > 40.0 => {
                self.hot_streak = self.hot_streak.saturating_add(1);
                if self.hot_streak >= 2 {
                    self.hot_streak = 0;
                    push(
                        self,
                        SystemEventKind::HotProcess,
                        serde_json::json!({ "name": hot.name, "cpu": hot.cpu }),
                    );
                }
            }
            _ => self.hot_streak = 0,
        }

        let hottest = stats.cpu_temp.unwrap_or(0.0).max(stats.gpu_temp.unwrap_or(0.0));
        if hottest >= 85.0 {
            push(self, SystemEventKind::TempHigh, serde_json::json!({ "temp": hottest }));
        }

        if self.disks_at.elapsed() > Duration::from_secs(5) {
            self.disks.refresh(true);
            self.disks_at = Instant::now();
            let removable = self.disks.list().iter().filter(|d| d.is_removable()).count();
            if removable > self.removable {
                push(self, SystemEventKind::UsbConnect, serde_json::json!({ "count": removable }));
            } else if removable < self.removable {
                push(self, SystemEventKind::UsbDisconnect, serde_json::json!({ "count": removable }));
            }
            self.removable = removable;
        }

        self.prev = Some(stats.clone());
        out
    }
}

/// A route to the public internet exists (default gateway present).
pub fn has_default_route() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::NetworkManagement::IpHelper::GetBestInterface;
        let mut idx = 0u32;
        // 8.8.8.8 — byte order doesn't matter for this address.
        let dest = u32::from_ne_bytes([8, 8, 8, 8]);
        let rc = unsafe { GetBestInterface(dest, &mut idx) };
        return rc == 0;
    }
    #[allow(unreachable_code)]
    true
}
