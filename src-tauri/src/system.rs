//! System stats source. One shared sampler thread refreshes about once a second, and only
//! while some widget has asked for data in the last few seconds.

use serde::Serialize;
use serde_json::{Map, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{Disks, Networks, System};

#[derive(Debug, Clone, Default, Serialize)]
pub struct CoreStat {
    pub usage: f32,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Cpu {
    pub usage: f32,
    pub name: String,
    pub count: usize,
    pub cores: Vec<CoreStat>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Mem {
    pub used: u64,
    pub total: u64,
    pub usage: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DiskStat {
    pub name: String,
    pub mount: String,
    pub used: u64,
    pub total: u64,
    pub usage: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Net {
    /// Bytes per second.
    pub rx: f64,
    pub tx: f64,
    pub rx_total: u64,
    pub tx_total: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Os {
    pub name: String,
    pub version: String,
    pub hostname: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Snapshot {
    pub cpu: Cpu,
    pub memory: Mem,
    pub swap: Mem,
    /// The disk holding the OS (or the largest one).
    pub disk: DiskStat,
    pub disks: Vec<DiskStat>,
    pub network: Net,
    pub uptime: u64,
    pub os: Os,
}

fn pct(used: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        (used as f64 / total as f64 * 1000.0).round() / 10.0
    }
}

struct Inner {
    sys: System,
    disks: Disks,
    nets: Networks,
    last_refresh: Instant,
    last_disk_refresh: Instant,
    snapshot: Snapshot,
}

impl Inner {
    fn new() -> Inner {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        let os = Os {
            name: System::name().unwrap_or_default(),
            version: System::os_version().unwrap_or_default(),
            hostname: System::host_name().unwrap_or_default(),
        };
        let mut inner = Inner {
            sys,
            disks: Disks::new_with_refreshed_list(),
            nets: Networks::new_with_refreshed_list(),
            last_refresh: Instant::now(),
            last_disk_refresh: Instant::now(),
            snapshot: Snapshot { os, ..Default::default() },
        };
        inner.refresh_disks();
        inner
    }

    fn refresh_disks(&mut self) {
        self.disks.refresh(true);
        let mut list: Vec<DiskStat> = self
            .disks
            .list()
            .iter()
            .filter(|d| d.total_space() > 0)
            .map(|d| {
                let total = d.total_space();
                let used = total.saturating_sub(d.available_space());
                DiskStat {
                    name: d.name().to_string_lossy().into_owned(),
                    mount: d.mount_point().to_string_lossy().into_owned(),
                    used,
                    total,
                    usage: pct(used, total),
                }
            })
            .collect();
        list.dedup_by(|a, b| a.name == b.name && a.total == b.total);
        let main = list
            .iter()
            .find(|d| d.mount == "/" || d.mount.eq_ignore_ascii_case("C:\\"))
            .or_else(|| list.iter().max_by_key(|d| d.total))
            .cloned()
            .unwrap_or_default();
        self.snapshot.disk = main;
        self.snapshot.disks = list;
        self.last_disk_refresh = Instant::now();
    }

    fn refresh(&mut self) {
        let elapsed = self.last_refresh.elapsed().as_secs_f64().max(0.001);
        self.last_refresh = Instant::now();

        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();
        self.nets.refresh(true);

        let cpus = self.sys.cpus();
        self.snapshot.cpu = Cpu {
            usage: (self.sys.global_cpu_usage() * 10.0).round() / 10.0,
            name: cpus.first().map(|c| c.brand().trim().to_string()).unwrap_or_default(),
            count: cpus.len(),
            cores: cpus.iter().map(|c| CoreStat { usage: (c.cpu_usage() * 10.0).round() / 10.0 }).collect(),
        };
        let (mu, mt) = (self.sys.used_memory(), self.sys.total_memory());
        self.snapshot.memory = Mem { used: mu, total: mt, usage: pct(mu, mt) };
        let (su, st) = (self.sys.used_swap(), self.sys.total_swap());
        self.snapshot.swap = Mem { used: su, total: st, usage: pct(su, st) };

        let (mut rx, mut tx, mut rxt, mut txt) = (0u64, 0u64, 0u64, 0u64);
        for (name, data) in self.nets.iter() {
            if name == "lo" || name.starts_with("Loopback") {
                continue;
            }
            rx += data.received();
            tx += data.transmitted();
            rxt += data.total_received();
            txt += data.total_transmitted();
        }
        self.snapshot.network =
            Net { rx: (rx as f64 / elapsed).round(), tx: (tx as f64 / elapsed).round(), rx_total: rxt, tx_total: txt };
        self.snapshot.uptime = System::uptime();

        if self.last_disk_refresh.elapsed() > Duration::from_secs(30) {
            self.refresh_disks();
        }
    }
}

#[derive(Clone)]
pub struct Sampler {
    inner: Arc<Mutex<Option<Inner>>>,
    last_request: Arc<Mutex<Instant>>,
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}

impl Sampler {
    pub fn new() -> Sampler {
        Sampler { inner: Arc::new(Mutex::new(None)), last_request: Arc::new(Mutex::new(Instant::now())) }
    }

    /// Starts the background thread. Cheap when nobody is asking for data.
    pub fn start(&self) {
        let this = self.clone();
        std::thread::Builder::new()
            .name("forge-system".into())
            .spawn(move || loop {
                let active = this.last_request.lock().unwrap().elapsed() < Duration::from_secs(5);
                if active {
                    let mut guard = this.inner.lock().unwrap();
                    guard.get_or_insert_with(Inner::new).refresh();
                }
                std::thread::sleep(Duration::from_millis(1000));
            })
            .expect("spawn sampler thread");
    }

    pub fn snapshot(&self) -> Snapshot {
        *self.last_request.lock().unwrap() = Instant::now();
        let mut guard = self.inner.lock().unwrap();
        let inner = guard.get_or_insert_with(|| {
            let mut i = Inner::new();
            i.refresh();
            i
        });
        inner.snapshot.clone()
    }
}

/// Removes categories the widget did not ask for. `os` covers uptime and OS names.
pub fn filter(snapshot: &Snapshot, allowed: &[String]) -> Value {
    let full = serde_json::to_value(snapshot).unwrap_or(Value::Null);
    let Value::Object(full) = full else { return Value::Null };
    let has = |c: &str| allowed.iter().any(|a| a == c);
    let mut out = Map::new();
    for (key, val) in full {
        let cat = match key.as_str() {
            "cpu" => "cpu",
            "memory" | "swap" => "memory",
            "disk" | "disks" => "disk",
            "network" => "network",
            "uptime" | "os" => "os",
            _ => continue,
        };
        if has(cat) {
            out.insert(key, val);
        }
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_has_data_and_filter_strips() {
        let s = Sampler::new();
        let snap = s.snapshot();
        assert!(snap.memory.total > 0);
        assert!(snap.cpu.count > 0);
        let v = filter(&snap, &["cpu".to_string()]);
        let obj = v.as_object().unwrap();
        assert!(obj.contains_key("cpu"));
        assert!(!obj.contains_key("memory"));
        assert!(!obj.contains_key("os"));
        let v = filter(&snap, &["memory".into(), "os".into()]);
        let obj = v.as_object().unwrap();
        assert!(obj.contains_key("swap") && obj.contains_key("uptime") && !obj.contains_key("cpu"));
    }
}
