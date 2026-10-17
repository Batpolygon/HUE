use sysinfo::{ProcessesToUpdate, System};

#[derive(Debug, Clone)]
pub struct HotProcess {
    pub name: String,
    pub cpu: f32,
}

pub fn top_cpu_process() -> Option<HotProcess> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.processes()
        .values()
        .filter_map(|p| {
            let cpu = p.cpu_usage();
            if cpu < 1.0 {
                return None;
            }
            Some(HotProcess {
                name: p.name().to_string_lossy().to_string(),
                cpu,
            })
        })
        .max_by(|a, b| a.cpu.partial_cmp(&b.cpu).unwrap_or(std::cmp::Ordering::Equal))
}
