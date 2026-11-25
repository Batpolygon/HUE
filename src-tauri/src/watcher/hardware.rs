use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use sysinfo::{Disks, Networks, System};


static EXTRA: OnceLock<HardwareExtra> = OnceLock::new();

/// Slow-to-query facts (one PowerShell call per launch, cached).
#[derive(Debug, Clone, Default)]
struct HardwareExtra {
    gpu_name: String,
    vram_gb: f64,
    chassis_types: Vec<u32>,
    disk_kind: String,
    birth_unix: Option<i64>,
    os_name: String,
}

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
    /// "nvidia" | "amd" | "intel" | "other"
    #[serde(default)]
    pub gpu_brand: String,
    /// "laptop" | "desktop" | "tower" | "aio" | "mini" | "handheld"
    #[serde(default)]
    pub form_factor: String,
    /// "nvme" | "ssd" | "hdd" | "unknown"
    #[serde(default)]
    pub disk_kind: String,
    #[serde(default)]
    pub monitors: u32,
    #[serde(default)]
    pub has_battery: bool,
    /// First Windows install (≈ first power-on), Unix seconds.
    #[serde(default)]
    pub birth_unix: Option<i64>,
    #[serde(default)]
    pub os_name: String,
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

        let extra = EXTRA.get_or_init(collect_extra).clone();

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
        });

        let (on_ac, battery_pct) = battery_status_pub();
        let has_battery = has_system_battery();
        let gpu_brand = gpu_brand_of(&extra.gpu_name);
        let form_factor = form_factor_of(&extra.chassis_types, has_battery, &cpu_name, &gpu_brand, cpu_cores);

        Self {
            hostname,
            cpu_name,
            cpu_cores,
            cpu_freq_mhz,
            ram_total_gb: (ram_total_gb * 10.0).round() / 10.0,
            ram_used_gb: (ram_used_gb * 10.0).round() / 10.0,
            ram_usage_pct: (ram_usage_pct * 10.0).round() / 10.0,
            gpu_name: if extra.gpu_name.is_empty() {
                "Integrated / Unknown GPU".into()
            } else {
                extra.gpu_name.clone()
            },
            vram_gb: extra.vram_gb,
            disk_total_gb: (disk_total_gb * 10.0).round() / 10.0,
            disk_free_gb: (disk_free_gb * 10.0).round() / 10.0,
            disk_usage_pct: (disk_usage_pct * 10.0).round() / 10.0,
            disk_label,
            on_ac,
            battery_pct,
            network_up,
            uptime_secs: System::uptime(),
            gpu_brand,
            form_factor,
            disk_kind: extra.disk_kind.clone(),
            monitors: monitor_count(),
            has_battery,
            birth_unix: extra.birth_unix,
            os_name: extra.os_name.clone(),
        }
    }

    pub fn is_portable(&self) -> bool {
        matches!(self.form_factor.as_str(), "laptop" | "handheld")
    }
}

pub fn gpu_brand_of(name: &str) -> String {
    let n = name.to_lowercase();
    if n.contains("nvidia") || n.contains("geforce") || n.contains("rtx") || n.contains("quadro") {
        "nvidia"
    } else if n.contains("amd") || n.contains("radeon") {
        "amd"
    } else if n.contains("intel") || n.contains("arc") || n.contains("iris") || n.contains("uhd") {
        "intel"
    } else {
        "other"
    }
    .into()
}

/// SMBIOS chassis types → coarse shell family.
pub fn form_factor_of(
    chassis: &[u32],
    has_battery: bool,
    cpu_name: &str,
    gpu_brand: &str,
    cores: u32,
) -> String {
    let cpu = cpu_name.to_lowercase();
    if chassis.contains(&11) || cpu.contains("z1 ") || cpu.contains("z1e") || cpu.contains("custom apu") {
        return "handheld".into();
    }
    if chassis.iter().any(|c| matches!(c, 8 | 9 | 10 | 14 | 30 | 31 | 32)) {
        return "laptop".into();
    }
    if chassis.contains(&13) {
        return "aio".into();
    }
    if chassis.iter().any(|c| matches!(c, 35 | 36 | 16)) {
        return "mini".into();
    }
    if chassis.is_empty() && has_battery {
        return "laptop".into();
    }
    let discrete = matches!(gpu_brand, "nvidia" | "amd");
    if !discrete && cores <= 6 {
        return "mini".into();
    }
    if chassis.iter().any(|c| matches!(c, 3 | 4 | 6 | 7 | 17 | 23 | 24)) && discrete {
        return "tower".into();
    }
    "desktop".into()
}

fn monitor_count() -> u32 {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CMONITORS};
        let n = unsafe { GetSystemMetrics(SM_CMONITORS) };
        return n.max(1) as u32;
    }
    #[allow(unreachable_code)]
    1
}

fn has_system_battery() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        unsafe {
            let mut status = SYSTEM_POWER_STATUS::default();
            if GetSystemPowerStatus(&mut status).is_ok() {
                // 128 = "no system battery", 255 = unknown.
                return status.BatteryFlag != 128 && status.BatteryFlag != 255;
            }
        }
    }
    false
}

fn collect_extra() -> HardwareExtra {
    #[cfg(windows)]
    {
        if let Some(e) = windows_extra() {
            return e;
        }
    }
    HardwareExtra::default()
}

#[cfg(windows)]
const EXTRA_SCRIPT: &str = r#"$ErrorActionPreference='SilentlyContinue';
$g = Get-ItemProperty 'HKLM:\SYSTEM\ControlSet001\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0*' | Where-Object { $_.'HardwareInformation.qwMemorySize' } | ForEach-Object { [pscustomobject]@{ n = [string]$_.DriverDesc; m = [uint64]$_.'HardwareInformation.qwMemorySize' } } | Sort-Object m -Descending | Select-Object -First 1;
$v = Get-CimInstance Win32_VideoController | Sort-Object AdapterRAM -Descending | Select-Object -First 1;
$ch = @((Get-CimInstance Win32_SystemEnclosure).ChassisTypes | ForEach-Object { [int]$_ });
$pd = @(Get-PhysicalDisk | ForEach-Object { ([string]$_.MediaType) + '|' + ([string]$_.BusType) });
$dates = @();
$cur = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion').InstallDate; if ($cur) { $dates += [int64]$cur };
Get-ChildItem 'HKLM:\SYSTEM\Setup' | Where-Object { $_.PSChildName -like 'Source OS*' } | ForEach-Object { $d = (Get-ItemProperty $_.PSPath).InstallDate; if ($d) { $dates += [int64]$d } };
$os = [string](Get-CimInstance Win32_OperatingSystem).Caption;
$name = if ($g) { $g.n } else { [string]$v.Name };
$mem = if ($g) { $g.m } else { [uint64]$v.AdapterRAM };
[pscustomobject]@{ gpu_name = $name; gpu_mem = $mem; chassis = $ch; disks = $pd; install_dates = $dates; os = $os } | ConvertTo-Json -Compress"#;

#[cfg(windows)]
fn windows_extra() -> Option<HardwareExtra> {
    let output = crate::util::hidden_command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", EXTRA_SCRIPT])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    Some(parse_extra(&v))
}

fn as_vec(v: &serde_json::Value) -> Vec<serde_json::Value> {
    match v {
        serde_json::Value::Array(a) => a.clone(),
        serde_json::Value::Null => Vec::new(),
        other => vec![other.clone()],
    }
}

fn parse_extra(v: &serde_json::Value) -> HardwareExtra {
    let gpu_name = v["gpu_name"].as_str().unwrap_or("").trim().to_string();
    let mem = v["gpu_mem"].as_f64().unwrap_or(0.0);
    let vram_gb = if mem > 0.0 {
        (mem / (1024.0 * 1024.0 * 1024.0) * 10.0).round() / 10.0
    } else {
        0.0
    };
    let chassis_types = as_vec(&v["chassis"])
        .iter()
        .filter_map(|c| c.as_u64().map(|n| n as u32))
        .collect();
    let disks: Vec<String> = as_vec(&v["disks"])
        .iter()
        .filter_map(|d| d.as_str().map(|s| s.to_lowercase()))
        .collect();
    let disk_kind = if disks.iter().any(|d| d.contains("nvme")) {
        "nvme"
    } else if disks.iter().any(|d| d.starts_with("ssd") || d.starts_with('4')) {
        "ssd"
    } else if disks.iter().any(|d| d.starts_with("hdd") || d.starts_with('3')) {
        "hdd"
    } else {
        "unknown"
    }
    .to_string();
    // Windows 2000-era timestamps or garbage are ignored.
    let birth_unix = as_vec(&v["install_dates"])
        .iter()
        .filter_map(|d| d.as_i64())
        .filter(|d| *d > 946_684_800)
        .min();
    HardwareExtra {
        gpu_name,
        vram_gb,
        chassis_types,
        disk_kind,
        birth_unix,
        os_name: v["os"].as_str().unwrap_or("").trim().to_string(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brands() {
        assert_eq!(gpu_brand_of("NVIDIA GeForce RTX 4070"), "nvidia");
        assert_eq!(gpu_brand_of("AMD Radeon RX 7800 XT"), "amd");
        assert_eq!(gpu_brand_of("Intel(R) UHD Graphics 620"), "intel");
        assert_eq!(gpu_brand_of("Microsoft Basic Display"), "other");
    }

    #[test]
    fn forms() {
        assert_eq!(form_factor_of(&[10], true, "Intel i7", "intel", 8), "laptop");
        assert_eq!(form_factor_of(&[3], false, "AMD Ryzen 7", "nvidia", 16), "tower");
        assert_eq!(form_factor_of(&[13], false, "Intel i5", "intel", 8), "aio");
        assert_eq!(form_factor_of(&[3], false, "Intel N100", "intel", 4), "mini");
        assert_eq!(form_factor_of(&[3], true, "AMD Ryzen Z1 Extreme", "amd", 16), "handheld");
    }

    #[test]
    fn parse_powershell_json() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"gpu_name":"NVIDIA GeForce RTX 3080","gpu_mem":10737418240,"chassis":3,
                "disks":["SSD|NVMe","HDD|SATA"],"install_dates":[1700000000,1600000000],"os":"Windows 11 Pro"}"#,
        )
        .unwrap();
        let e = parse_extra(&v);
        assert_eq!(e.vram_gb, 10.0);
        assert_eq!(e.chassis_types, vec![3]);
        assert_eq!(e.disk_kind, "nvme");
        assert_eq!(e.birth_unix, Some(1600000000));
    }
}
