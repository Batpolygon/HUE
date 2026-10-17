use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use sysinfo::{Disks, Networks, System};

static GPU_CACHE: OnceLock<(String, f64)> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareSnapshot {
    pub hostname: String,
    pub cpu_name: String,
    pub cpu_cores: u32,
    pub cpu_freq_mhz: u64,
    pub ram_total_gb: f64,
    pub ram_used_gb: f64,
    pub ram_usage_pct: f64,
    pub gpu_name: String,
    pub vram_gb: f64,
    pub disk_total_gb: f64,
    pub disk_free_gb: f64,
    pub disk_usage_pct: f64,
    pub disk_label: String,
    pub on_ac: Option<bool>,
    pub battery_pct: Option<f64>,
    pub network_up: bool,
    pub uptime_secs: u64,
}

impl HardwareSnapshot {
    pub fn collect() -> Self {
        let mut sys = System::new();
        sys.refresh_memory();
        sys.refresh_cpu_all();

        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "HUE".into());

        let cpu_name = sys
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Unknown CPU".into());
        let cpu_cores = sys.cpus().len() as u32;
        let cpu_freq_mhz = sys
            .cpus()
            .first()
            .map(|c| c.frequency())
            .unwrap_or(0);

        let ram_total = sys.total_memory() as f64;
        let ram_used = sys.used_memory() as f64;
        let ram_total_gb = ram_total / (1024.0 * 1024.0 * 1024.0);
        let ram_used_gb = ram_used / (1024.0 * 1024.0 * 1024.0);
        let ram_usage_pct = if ram_total > 0.0 {
            (ram_used / ram_total) * 100.0
        } else {
            0.0
        };

        let (gpu_name, vram_gb) = detect_gpu();

        let disks = Disks::new_with_refreshed_list();
        let primary = disks.list().iter().max_by_key(|d| d.total_space());
        let (disk_total_gb, disk_free_gb, disk_usage_pct, disk_label) = if let Some(d) = primary {
            let total = d.total_space() as f64 / (1024.0 * 1024.0 * 1024.0);
            let avail = d.available_space() as f64 / (1024.0 * 1024.0 * 1024.0);
            let used_pct = if total > 0.0 {
                ((total - avail) / total) * 100.0
            } else {
                0.0
            };
            let label = d.mount_point().to_string_lossy().to_string();
            (total, avail, used_pct, label)
        } else {
            (0.0, 0.0, 0.0, "/".into())
        };

        let networks = Networks::new_with_refreshed_list();
        let network_up = networks.iter().any(|(_, data)| {
            data.received() > 0 || data.transmitted() > 0 || data.total_received() > 0
        }) || has_default_route();

        let (on_ac, battery_pct) = battery_status_pub();

        Self {
            hostname,
            cpu_name,
            cpu_cores,
            cpu_freq_mhz,
            ram_total_gb: (ram_total_gb * 10.0).round() / 10.0,
            ram_used_gb: (ram_used_gb * 10.0).round() / 10.0,
            ram_usage_pct: (ram_usage_pct * 10.0).round() / 10.0,
            gpu_name,
            vram_gb,
            disk_total_gb: (disk_total_gb * 10.0).round() / 10.0,
            disk_free_gb: (disk_free_gb * 10.0).round() / 10.0,
            disk_usage_pct: (disk_usage_pct * 10.0).round() / 10.0,
            disk_label,
            on_ac,
            battery_pct,
            network_up,
            uptime_secs: System::uptime(),
        }
    }
}

fn detect_gpu() -> (String, f64) {
    GPU_CACHE
        .get_or_init(|| {
            #[cfg(windows)]
            {
                if let Some(info) = windows_gpu_wmi() {
                    return info;
                }
            }
            ("Integrated / Unknown GPU".into(), 0.0)
        })
        .clone()
}

#[cfg(windows)]
fn windows_gpu_wmi() -> Option<(String, f64)> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_VideoController | Sort-Object -Property AdapterRAM -Descending | Select-Object -First 1 | ForEach-Object { \"$($_.Name)|$($_.AdapterRAM)\" })",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?.trim();
    if line.is_empty() {
        return None;
    }
    let mut parts = line.split('|');
    let name = parts.next()?.trim().to_string();
    let ram_bytes: f64 = parts
        .next()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0.0);
    let vram_gb = if ram_bytes > 0.0 {
        (ram_bytes / (1024.0 * 1024.0 * 1024.0) * 10.0).round() / 10.0
    } else {
        0.0
    };
    Some((name, vram_gb))
}

pub fn battery_status_pub() -> (Option<bool>, Option<f64>) {
    #[cfg(windows)]
    {
        use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        unsafe {
            let mut status = SYSTEM_POWER_STATUS::default();
            if GetSystemPowerStatus(&mut status).is_ok() {
                let on_ac = match status.ACLineStatus {
                    0 => Some(false),
                    1 => Some(true),
                    _ => None,
                };
                let pct = if status.BatteryLifePercent <= 100 {
                    Some(status.BatteryLifePercent as f64)
                } else {
                    None
                };
                return (on_ac, pct);
            }
        }
    }
    (None, None)
}

fn has_default_route() -> bool {
    // Best-effort: assume up unless proven otherwise by watcher deltas.
    true
}
