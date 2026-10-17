use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{Disks, System};

#[derive(Debug, Clone, Serialize)]
pub struct LiveStats {
    pub cpu_pct: f64,
    pub ram_pct: f64,
    pub ram_used_gb: f64,
    pub ram_total_gb: f64,
    pub cpu_freq_mhz: u64,
    pub disk_pct: f64,
    pub battery_pct: Option<f64>,
    pub on_ac: Option<bool>,
    /// GPU utilization 0–100 (best-effort).
    pub gpu_pct: f64,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

static SYS: Mutex<Option<System>> = Mutex::new(None);
static GPU_CACHE: Mutex<Option<(Instant, f64)>> = Mutex::new(None);

pub fn collect() -> LiveStats {
    let mut guard = SYS.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        *guard = Some(sys);
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    let sys = guard.as_mut().unwrap();
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let cpu_pct = {
        let cpus = sys.cpus();
        if cpus.is_empty() {
            0.0
        } else {
            cpus.iter().map(|c| c.cpu_usage() as f64).sum::<f64>() / cpus.len() as f64
        }
    };
    let cpu_freq_mhz = sys.cpus().first().map(|c| c.frequency()).unwrap_or(0);

    let ram_total = sys.total_memory() as f64;
    let ram_used = sys.used_memory() as f64;
    let ram_pct = if ram_total > 0.0 {
        (ram_used / ram_total) * 100.0
    } else {
        0.0
    };
    let ram_total_gb = ((ram_total / (1024.0 * 1024.0 * 1024.0)) * 10.0).round() / 10.0;
    let ram_used_gb = ((ram_used / (1024.0 * 1024.0 * 1024.0)) * 10.0).round() / 10.0;

    let disks = Disks::new_with_refreshed_list();
    let disk_pct = disks
        .list()
        .iter()
        .max_by_key(|d| d.total_space())
        .map(|d| {
            let total = d.total_space() as f64;
            let avail = d.available_space() as f64;
            if total > 0.0 {
                ((total - avail) / total) * 100.0
            } else {
                0.0
            }
        })
        .unwrap_or(0.0);

    let gpu_pct = cached_gpu_usage();
    let (on_ac, battery_pct) = crate::watcher::hardware::battery_status_pub();
    let (hour, minute, second) = local_hms();

    LiveStats {
        cpu_pct: (cpu_pct * 10.0).round() / 10.0,
        ram_pct: (ram_pct * 10.0).round() / 10.0,
        ram_used_gb,
        ram_total_gb,
        cpu_freq_mhz,
        disk_pct: (disk_pct * 10.0).round() / 10.0,
        battery_pct,
        on_ac,
        gpu_pct: (gpu_pct * 10.0).round() / 10.0,
        hour,
        minute,
        second,
    }
}

fn cached_gpu_usage() -> f64 {
    let mut g = GPU_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, v)) = *g {
        if at.elapsed() < Duration::from_secs(2) {
            return v;
        }
    }
    let v = sample_gpu_usage().unwrap_or(0.0).clamp(0.0, 100.0);
    *g = Some((Instant::now(), v));
    v
}

fn sample_gpu_usage() -> Option<f64> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        // NVIDIA first (fast, accurate)
        if let Ok(out) = Command::new("nvidia-smi")
            .args([
                "--query-gpu=utilization.gpu",
                "--format=csv,noheader,nounits",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                if let Some(v) = text
                    .lines()
                    .next()
                    .and_then(|l| l.trim().parse::<f64>().ok())
                {
                    return Some(v);
                }
            }
        }

        // Generic Windows GPU engine counters (sum, clamped)
        if let Ok(out) = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-Counter '\\GPU Engine(*)\\Utilization Percentage' -ErrorAction SilentlyContinue).CounterSamples | Measure-Object -Property CookedValue -Maximum | Select-Object -ExpandProperty Maximum",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                if let Some(v) = text.trim().parse::<f64>().ok() {
                    return Some(v.min(100.0));
                }
            }
        }
    }
    None
}

fn local_hms() -> (u32, u32, u32) {
    #[cfg(windows)]
    {
        use windows::Win32::System::SystemInformation::GetLocalTime;
        unsafe {
            let st = GetLocalTime();
            return (st.wHour as u32, st.wMinute as u32, st.wSecond as u32);
        }
    }
    #[allow(unreachable_code)]
    {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let s = (secs % 86400) as u32;
        (s / 3600, (s % 3600) / 60, s % 60)
    }
}
