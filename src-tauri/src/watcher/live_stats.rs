use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{Components, Disks, System};

#[derive(Debug, Clone, Serialize)]
pub struct LiveStats {
    pub cpu_pct: f64,
    pub ram_pct: f64,
    pub ram_used_gb: f64,
    pub ram_total_gb: f64,
    pub cpu_freq_mhz: u64,
    pub disk_pct: f64,
    pub disk_free_gb: f64,
    pub battery_pct: Option<f64>,
    pub on_ac: Option<bool>,
    /// GPU utilization 0–100 (best-effort).
    pub gpu_pct: f64,
    /// °C when a sensor is readable without admin rights.
    pub cpu_temp: Option<f64>,
    pub gpu_temp: Option<f64>,
    pub uptime_secs: u64,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

struct Cache {
    sys: System,
    disks: Disks,
    disks_at: Instant,
    components: Option<Components>,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
/// (sampled_at, gpu %, gpu °C)
static GPU: Mutex<Option<(Instant, f64, Option<f64>)>> = Mutex::new(None);
static GPU_SAMPLING: AtomicBool = AtomicBool::new(false);
/// nvidia-smi missing → fall back to the (slow) PowerShell counter, sampled rarely.
static NO_NVIDIA: AtomicBool = AtomicBool::new(false);

pub fn collect() -> LiveStats {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        let comps = Components::new_with_refreshed_list();
        let components = if comps.list().iter().any(|c| c.temperature().is_some()) {
            Some(comps)
        } else {
            None
        };
        *guard = Some(Cache {
            sys,
            disks: Disks::new_with_refreshed_list(),
            disks_at: Instant::now(),
            components,
        });
        std::thread::sleep(Duration::from_millis(60));
    }
    let cache = guard.as_mut().unwrap();
    cache.sys.refresh_cpu_usage();
    cache.sys.refresh_memory();
    if cache.disks_at.elapsed() > Duration::from_secs(10) {
        cache.disks.refresh(true);
        cache.disks_at = Instant::now();
    }

    let sys = &cache.sys;
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
    let gb = 1024.0 * 1024.0 * 1024.0;
    let ram_total_gb = ((ram_total / gb) * 10.0).round() / 10.0;
    let ram_used_gb = ((ram_used / gb) * 10.0).round() / 10.0;

    let (disk_pct, disk_free_gb) = cache
        .disks
        .list()
        .iter()
        .max_by_key(|d| d.total_space())
        .map(|d| {
            let total = d.total_space() as f64;
            let avail = d.available_space() as f64;
            let pct = if total > 0.0 {
                ((total - avail) / total) * 100.0
            } else {
                0.0
            };
            (pct, avail / gb)
        })
        .unwrap_or((0.0, 0.0));

    let cpu_temp = cache.components.as_mut().and_then(|c| {
        c.refresh(false);
        c.list()
            .iter()
            .filter(|x| {
                let l = x.label().to_lowercase();
                l.contains("cpu") || l.contains("package") || l.contains("tctl") || l.contains("acpi")
            })
            .filter_map(|x| x.temperature())
            .map(|t| t as f64)
            .fold(None, |acc: Option<f64>, t| Some(acc.map_or(t, |a| a.max(t))))
    });
    drop(guard);

    let (gpu_pct, gpu_temp) = cached_gpu();
    let (on_ac, battery_pct) = crate::watcher::hardware::battery_status_pub();
    let (hour, minute, second) = local_hms();

    LiveStats {
        cpu_pct: (cpu_pct * 10.0).round() / 10.0,
        ram_pct: (ram_pct * 10.0).round() / 10.0,
        ram_used_gb,
        ram_total_gb,
        cpu_freq_mhz,
        disk_pct: (disk_pct * 10.0).round() / 10.0,
        disk_free_gb: (disk_free_gb * 10.0).round() / 10.0,
        battery_pct,
        on_ac,
        gpu_pct: (gpu_pct * 10.0).round() / 10.0,
        cpu_temp: cpu_temp.filter(|t| *t > 5.0 && *t < 130.0),
        gpu_temp,
        uptime_secs: System::uptime(),
        hour,
        minute,
        second,
    }
}

/// Never blocks the caller: sampling runs on a helper thread, callers read the cache.
fn cached_gpu() -> (f64, Option<f64>) {
    let refresh_every = if NO_NVIDIA.load(Ordering::Relaxed) {
        Duration::from_secs(15)
    } else {
        Duration::from_secs(2)
    };
    let current = *GPU.lock().unwrap_or_else(|e| e.into_inner());
    let stale = current.map(|(at, _, _)| at.elapsed() >= refresh_every).unwrap_or(true);
    if stale && !GPU_SAMPLING.swap(true, Ordering::SeqCst) {
        std::thread::spawn(|| {
            let (pct, temp) = sample_gpu();
            *GPU.lock().unwrap_or_else(|e| e.into_inner()) =
                Some((Instant::now(), pct.clamp(0.0, 100.0), temp));
            GPU_SAMPLING.store(false, Ordering::SeqCst);
        });
    }
    current.map(|(_, p, t)| (p, t)).unwrap_or((0.0, None))
}

fn sample_gpu() -> (f64, Option<f64>) {
    #[cfg(windows)]
    {
        if !NO_NVIDIA.load(Ordering::Relaxed) {
            match crate::util::hidden_command("nvidia-smi")
                .args([
                    "--query-gpu=utilization.gpu,temperature.gpu",
                    "--format=csv,noheader,nounits",
                ])
                .output()
            {
                Ok(out) if out.status.success() => {
                    let text = String::from_utf8_lossy(&out.stdout);
                    if let Some(line) = text.lines().next() {
                        let mut parts = line.split(',').map(|p| p.trim().parse::<f64>().ok());
                        let pct = parts.next().flatten();
                        let temp = parts.next().flatten();
                        if let Some(p) = pct {
                            return (p, temp);
                        }
                    }
                }
                _ => NO_NVIDIA.store(true, Ordering::Relaxed),
            }
        }

        if let Ok(out) = crate::util::hidden_command("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-Counter '\\GPU Engine(*engtype_3D)\\Utilization Percentage' -ErrorAction SilentlyContinue).CounterSamples | Measure-Object -Property CookedValue -Sum | Select-Object -ExpandProperty Sum",
            ])
            .output()
        {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                if let Ok(v) = text.trim().replace(',', ".").parse::<f64>() {
                    return (v.min(100.0), None);
                }
            }
        }
    }
    (0.0, None)
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
