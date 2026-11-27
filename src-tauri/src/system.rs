//! The `system` data source: CPU, memory, disk, network, OS, sensors.

use serde_json::{json, Map, Value};
use std::time::Instant;
use std::collections::HashMap;
use sysinfo::{Components, Disks, Gpus, Networks, ProcessesToUpdate, System};

/// sysinfo's GPU list holds a raw handle to the NVIDIA driver library, so Rust
/// does not mark it thread-safe. It is only used while `AppState.system` is
/// locked, one thread at a time, which is what Send requires.
struct GpuList(Gpus);
unsafe impl Send for GpuList {}

pub struct SystemSource {
    sys: System,
    disks: Disks,
    nets: Networks,
    last_net: Instant,
    rates: (f64, f64),
    last_procs: Option<Instant>,
    procs: Value,
    gpus: Option<GpuList>,
    io_disks: Option<(Disks, Instant)>,
    components: Option<Components>,
}

impl SystemSource {
    pub fn new() -> Result<Self, String> {
        let err = |e: sysinfo::Error| e.to_string();
        let mut sys = System::new().map_err(err)?;
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        Ok(Self {
            sys,
            disks: Disks::new_with_refreshed_list().map_err(err)?,
            nets: Networks::new_with_refreshed_list().map_err(err)?,
            last_net: Instant::now(),
            rates: (0.0, 0.0),
            last_procs: None,
            procs: json!([]),
            gpus: None,
            io_disks: None,
            components: None,
        })
    }

    /// Returns only the categories in `allowed`.
    pub fn snapshot(&mut self, allowed: &[String]) -> Value {
        let has = |k: &str| allowed.iter().any(|a| a == k);
        let mut out = Map::new();

        if has("cpu") {
            self.sys.refresh_cpu_usage();
            let cores: Vec<f32> = self.sys.cpus().iter().map(|c| c.usage()).collect();
            out.insert(
                "cpu".into(),
                json!({ "usage": self.sys.global_cpu_usage(), "count": cores.len(), "cores": cores }),
            );
        }

        if has("memory") {
            self.sys.refresh_memory();
            let total = self.sys.total_memory();
            let used = self.sys.used_memory();
            let usage = if total > 0 { used as f64 * 100.0 / total as f64 } else { 0.0 };
            out.insert(
                "memory".into(),
                json!({
                    "total": total, "used": used, "usage": usage,
                    "swap_total": self.sys.total_swap(), "swap_used": self.sys.used_swap(),
                }),
            );
        }

        if has("disk") {
            self.disks.refresh(true);
            let list: Vec<Value> = self
                .disks
                .iter()
                .filter(|d| d.total_space() > 0)
                .map(|d| {
                    let total = d.total_space();
                    let used = total - d.available_space();
                    json!({
                        "mount": d.mount_point().to_string_lossy(),
                        "total": total, "used": used,
                        "usage": used as f64 * 100.0 / total as f64,
                    })
                })
                .collect();
            // The main disk: "/" on Linux and macOS, C:\ on Windows, else the first.
            let main = list
                .iter()
                .find(|d| matches!(d["mount"].as_str(), Some("/") | Some("C:\\")))
                .or(list.first())
                .cloned()
                .unwrap_or(json!({}));
            let mut disk = main.as_object().cloned().unwrap_or_default();
            disk.insert("list".into(), Value::Array(list));
            out.insert("disk".into(), Value::Object(disk));
        }

        if has("network") {
            // Several widgets may ask at once; only re-measure every half second.
            let dt = self.last_net.elapsed().as_secs_f64();
            if dt >= 0.5 {
                self.nets.refresh(true);
                let (rx, tx) = self
                    .nets
                    .iter()
                    .fold((0u64, 0u64), |(r, t), (_, d)| (r + d.received(), t + d.transmitted()));
                self.rates = (rx as f64 / dt, tx as f64 / dt);
                self.last_net = Instant::now();
            }
            out.insert("network".into(), json!({ "rx": self.rates.0, "tx": self.rates.1 }));
        }

        if has("processes") {
            // CPU per process needs two samples; re-sample at most every 900 ms.
            if self.last_procs.map_or(true, |t| t.elapsed().as_millis() >= 900) {
                self.sys.refresh_processes(ProcessesToUpdate::All, true);
                self.procs = self.top_processes(15);
                self.last_procs = Some(Instant::now());
            }
            out.insert("processes".into(), self.procs.clone());
        }

        if has("gpu") || has("temperature") {
            let temps = self.temperatures();
            if has("temperature") {
                out.insert("temperature".into(), temps.clone());
            }
            if has("gpu") {
                out.insert("gpu".into(), self.gpu(&temps));
            }
        }

        if has("disks") {
            out.insert("disks".into(), self.disk_details());
        }

        if has("battery") {
            out.insert("battery".into(), battery());
        }

        if has("sensors") {
            out.insert("sensors".into(), sensors());
        }

        if has("devices") {
            out.insert("devices".into(), device_batteries());
        }

        if has("interfaces") {
            out.insert("interfaces".into(), self.interfaces());
        }

        if has("os") {
            out.insert(
                "os".into(),
                json!({
                    "name": System::name().ok(), "version": System::os_version().ok(),
                    "hostname": System::host_name().ok(),
                }),
            );
        }

        out.insert("uptime".into(), json!(System::uptime().unwrap_or(0)));
        Value::Object(out)
    }

    /// Processes grouped by name (a browser's many processes become one row),
    /// busiest first. `cpu` is a percent of the whole machine.
    fn top_processes(&self, limit: usize) -> Value {
        let cores = self.sys.cpus().len().max(1) as f32;
        let mut groups: HashMap<String, (f32, u64, u32)> = HashMap::new();
        for p in self.sys.processes().values() {
            // Skip threads listed as their own process on Linux.
            if p.thread_kind().is_some() {
                continue;
            }
            let name = p.name().to_string_lossy().into_owned();
            let g = groups.entry(name).or_insert((0.0, 0, 0));
            g.0 += p.cpu_usage() / cores;
            g.1 += p.memory();
            g.2 += 1;
        }
        let mut rows: Vec<(String, (f32, u64, u32))> = groups.into_iter().collect();
        rows.sort_by(|a, b| {
            b.1 .0.partial_cmp(&a.1 .0).unwrap_or(std::cmp::Ordering::Equal).then(b.1 .1.cmp(&a.1 .1))
        });
        Value::Array(
            rows.into_iter()
                .take(limit)
                .map(|(name, (cpu, memory, count))| json!({ "name": name, "cpu": cpu, "memory": memory, "count": count }))
                .collect(),
        )
    }

    /// Every drive: space, type, and read/write speed since the last call.
    fn disk_details(&mut self) -> Value {
        let (disks, last) = match &mut self.io_disks {
            Some((d, t)) => {
                d.refresh(true);
                (d, t)
            }
            None => match Disks::new_with_refreshed_list() {
                Ok(d) => {
                    let (d, t) = self.io_disks.insert((d, Instant::now()));
                    (d, t)
                }
                Err(_) => return json!({ "list": [] }),
            },
        };
        let dt = last.elapsed().as_secs_f64().max(0.001);
        *last = Instant::now();
        let mut seen = std::collections::HashSet::new();
        let list: Vec<Value> = disks
            .iter()
            .filter(|d| d.total_space() > 0)
            // The same partition can be mounted twice (bind mounts); show it once.
            .filter(|d| seen.insert((d.name().to_os_string(), d.total_space())))
            .map(|d| {
                let total = d.total_space();
                let used = total - d.available_space();
                let u = d.usage();
                json!({
                    "name": d.name().to_string_lossy(),
                    "mount": d.mount_point().to_string_lossy(),
                    "fs": d.file_system().to_string_lossy(),
                    "kind": format!("{:?}", d.kind()),
                    "removable": d.is_removable(),
                    "total": total, "used": used, "free": d.available_space(),
                    "usage": used as f64 * 100.0 / total as f64,
                    "read_rate": u.read_bytes as f64 / dt,
                    "write_rate": u.written_bytes as f64 / dt,
                })
            })
            .collect();
        json!({ "list": list, "count": list.len() })
    }

    /// Temperature sensors: the CPU package, the GPU, and the full list.
    fn temperatures(&mut self) -> Value {
        let comps = match &mut self.components {
            Some(c) => {
                c.refresh(true);
                c
            }
            None => match Components::new_with_refreshed_list() {
                Ok(c) => self.components.insert(c),
                Err(_) => return json!({ "list": [] }),
            },
        };
        let list: Vec<(String, f32)> = comps
            .iter()
            .filter_map(|c| c.temperature().filter(|t| t.is_finite()).map(|t| (c.label().to_string(), t)))
            .collect();
        let find = |words: &[&str]| {
            list.iter()
                .filter(|(l, _)| {
                    let l = l.to_ascii_lowercase();
                    words.iter().any(|w| l.contains(w))
                })
                .map(|(_, t)| *t)
                .fold(None, |a: Option<f32>, t| Some(a.map_or(t, |a| a.max(t))))
        };
        json!({
            "cpu": find(&["package", "tctl", "tdie", "coretemp", "k10temp", "cpu"]),
            "gpu": find(&["gpu", "amdgpu", "nvidia", "nouveau", "radeon", "edge", "junction"]),
            "list": list.iter().map(|(l, t)| json!({ "label": l, "value": t })).collect::<Vec<_>>(),
        })
    }

    /// The first graphics card: name, load and memory where the driver reports them.
    fn gpu(&mut self, temps: &Value) -> Value {
        let gpus = match &mut self.gpus {
            Some(GpuList(g)) => {
                g.refresh(true);
                g
            }
            None => match Gpus::new_with_refreshed_list() {
                Ok(g) => &mut self.gpus.insert(GpuList(g)).0,
                Err(_) => return json!({ "present": false }),
            },
        };
        let Some(g) = gpus.iter().next() else { return json!({ "present": false }) };
        let total = g.total_memory();
        let used = g.used_memory();
        json!({
            "present": true,
            "name": g.model().or(g.vendor()),
            "vendor": g.vendor(),
            "usage": g.usage(),
            "memory_total": total,
            "memory_used": used,
            "memory_usage": match (used, total) { (Some(u), Some(t)) if t > 0 => Some(u as f64 * 100.0 / t as f64), _ => None },
            "temperature": temps["gpu"].clone(),
        })
    }
}

impl SystemSource {
    /// Network adapters with their addresses. VPN tunnels (WireGuard, OpenVPN,
    /// Tailscale, ZeroTier) are recognised by name.
    fn interfaces(&mut self) -> Value {
        self.nets.refresh(true);
        let mut list: Vec<Value> = self
            .nets
            .iter()
            .map(|(name, d)| (name.to_string_lossy().into_owned(), d))
            .filter(|(name, _)| !matches!(name.as_str(), "lo" | "lo0") && !name.starts_with("Loopback"))
            .map(|(name, d)| {
                let addrs: Vec<String> = d.ip_networks().iter().map(|n| n.addr.to_string()).collect();
                let vpn = vpn_kind(&name);
                json!({
                    "name": name, "addresses": addrs, "address": addrs.iter().find(|a| !a.contains(':')).or(addrs.first()),
                    "up": !addrs.is_empty(), "vpn": vpn.is_some(), "vpn_kind": vpn,
                    "total_rx": d.total_received(), "total_tx": d.total_transmitted(),
                })
            })
            .collect();
        list.sort_by(|a, b| b["up"].as_bool().cmp(&a["up"].as_bool()).then(a["name"].as_str().cmp(&b["name"].as_str())));
        let vpns: Vec<Value> = list.iter().filter(|i| i["vpn"] == true && i["up"] == true).cloned().collect();
        json!({ "list": list, "vpn_up": !vpns.is_empty(), "vpns": vpns })
    }
}

pub fn vpn_kind(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    if n.starts_with("wg") || n.contains("wireguard") {
        Some("WireGuard")
    } else if n.starts_with("tailscale") || n == "utun-tailscale" {
        Some("Tailscale")
    } else if n.starts_with("zt") {
        Some("ZeroTier")
    } else if n.starts_with("tun") || n.starts_with("tap") || n.contains("openvpn") {
        Some("VPN tunnel")
    } else if n.starts_with("ppp") || n.starts_with("ipsec") {
        Some("VPN")
    } else if n.starts_with("proton") || n.starts_with("mullvad") || n.starts_with("nordlynx") {
        Some("WireGuard")
    } else {
        None
    }
}

/// Fans, power draw and every temperature the hardware reports.
/// Linux reads /sys/class/hwmon; other systems are not done yet.
#[cfg(target_os = "linux")]
fn sensors() -> Value {
    use std::fs;
    let read = |p: &std::path::Path| fs::read_to_string(p).ok().map(|s| s.trim().to_string());
    let mut temps = Vec::new();
    let mut fans = Vec::new();
    let mut power = Vec::new();
    let mut volts = Vec::new();
    let Ok(dirs) = fs::read_dir("/sys/class/hwmon") else { return json!({ "available": false }) };
    let mut dirs: Vec<_> = dirs.flatten().map(|e| e.path()).collect();
    dirs.sort();
    for dir in dirs {
        let chip = read(&dir.join("name")).unwrap_or_else(|| "sensor".into());
        let Ok(files) = fs::read_dir(&dir) else { continue };
        let mut names: Vec<String> = files.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        for f in names {
            let Some(base) = f.strip_suffix("_input").or_else(|| f.strip_suffix("_average").filter(|b| b.starts_with("power"))) else { continue };
            // power1_average and power1_input can both exist; keep one.
            if f.ends_with("_average") && names_has(&dir, &format!("{base}_input")) {
                continue;
            }
            let Some(raw) = read(&dir.join(&f)).and_then(|v| v.parse::<f64>().ok()) else { continue };
            let label = read(&dir.join(format!("{base}_label"))).unwrap_or_else(|| base.to_string());
            let name = format!("{chip} {label}");
            if base.starts_with("temp") {
                temps.push(json!({ "chip": chip, "label": label, "name": name, "value": raw / 1000.0 }));
            } else if base.starts_with("fan") {
                fans.push(json!({ "chip": chip, "label": label, "name": name, "value": raw }));
            } else if base.starts_with("power") {
                power.push(json!({ "chip": chip, "label": label, "name": name, "value": raw / 1_000_000.0 }));
            } else if base.starts_with("in") {
                volts.push(json!({ "chip": chip, "label": label, "name": name, "value": raw / 1000.0 }));
            }
        }
    }
    let total_power: f64 = power.iter().filter_map(|p| p["value"].as_f64()).sum();
    json!({
        "available": true, "temps": temps, "fans": fans, "power": power, "volts": volts,
        "power_total": if power.is_empty() { Value::Null } else { json!(total_power) },
        "fan_count": fans.len(),
    })
}

#[cfg(target_os = "linux")]
fn names_has(dir: &std::path::Path, name: &str) -> bool {
    dir.join(name).exists()
}

#[cfg(not(target_os = "linux"))]
fn sensors() -> Value {
    json!({ "available": false, "reason": "Fan and power readings work on Linux so far." })
}

/// Batteries of Bluetooth and USB devices (mice, headphones, controllers,
/// phones through KDE Connect), from UPower on Linux.
#[cfg(target_os = "linux")]
fn device_batteries() -> Value {
    let Ok(out) = std::process::Command::new("upower").arg("--dump").output() else {
        return json!({ "available": false, "reason": "UPower is not installed." });
    };
    let text = String::from_utf8_lossy(&out.stdout);
    json!({ "available": true, "list": parse_upower(&text) })
}

#[cfg(not(target_os = "linux"))]
fn device_batteries() -> Value {
    json!({ "available": false, "reason": "Device batteries work on Linux so far." })
}

/// Reads `upower --dump`: one block per device, "key: value" lines.
pub fn parse_upower(text: &str) -> Vec<Value> {
    let mut list = Vec::new();
    for block in text.split("\n\n") {
        let mut path = "";
        let mut f: HashMap<&str, &str> = HashMap::new();
        let mut kind = "";
        for line in block.lines() {
            if let Some(p) = line.strip_prefix("Device: ") {
                path = p.trim();
                continue;
            }
            let t = line.trim();
            if let Some((k, v)) = t.split_once(':') {
                f.insert(k.trim(), v.trim());
            } else if !t.is_empty() && kind.is_empty() && line.starts_with("  ") && !line.starts_with("    ") {
                kind = t; // "battery", "mouse", "headset", ...
            }
        }
        let Some(pct) = f.get("percentage").and_then(|p| p.trim_end_matches('%').trim().parse::<f64>().ok()) else { continue };
        // The laptop's own battery and the AC adapter belong to the Battery widget.
        if path.is_empty() || path.contains("DisplayDevice") || path.contains("line_power") {
            continue;
        }
        if f.get("power supply") == Some(&"yes") && kind == "battery" {
            continue;
        }
        let name = f.get("model").filter(|m| !m.is_empty()).or(f.get("native-path")).copied().unwrap_or(kind);
        list.push(json!({
            "name": name, "kind": kind, "percent": pct,
            "state": f.get("state").copied().unwrap_or(""),
            "low": pct <= 20.0,
        }));
    }
    list
}

/// The first battery, if the computer has one.
fn battery() -> Value {
    let Ok(manager) = starship_battery::Manager::new() else { return json!({ "present": false }) };
    let Ok(mut list) = manager.batteries() else { return json!({ "present": false }) };
    let Some(Ok(b)) = list.next() else { return json!({ "present": false }) };
    let state = format!("{:?}", b.state()).to_lowercase();
    json!({
        "present": true,
        "percent": b.state_of_charge().value * 100.0,
        "state": state,
        "charging": state == "charging",
        "minutes_to_empty": b.time_to_empty().map(|t| (t.value / 60.0).round()),
        "minutes_to_full": b.time_to_full().map(|t| (t.value / 60.0).round()),
        "health": b.state_of_health().value * 100.0,
    })
}
