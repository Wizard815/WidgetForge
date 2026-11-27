//! Sources that touch this computer or the local network:
//! picture folders, network latency, the volume mixer, Docker, commands,
//! log files, Wake-on-LAN and game servers.
//! Each needs its own permission in the widget's manifest.

use crate::model::{Instance, WidgetInfo};
use crate::AppState;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri::{State, WebviewWindow};

fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> impl std::future::Future<Output = Result<T, String>> {
    async move { tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string()) }
}

/// Text a user typed into a setting; manifest defaults are NOT used.
fn user_setting(inst: &Instance, key: &str) -> Option<String> {
    inst.settings.get(key)?.as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// A setting value (user's, else the manifest default).
fn setting_text(widget: &WidgetInfo, inst: &Instance, key: &str) -> Option<String> {
    widget.setting(inst, key)?.as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn listed(widget: &WidgetInfo, permission: &str, entry: &str) -> bool {
    widget
        .manifest
        .pointer(&format!("/permissions/{permission}"))
        .and_then(Value::as_array)
        .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(entry)))
}

// ---------- picture folders ----------

const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp", "avif"];
const MAX_FILES: usize = 5000;

/// The folder a widget may read: the user's setting `key`, allowed by
/// `permissions.folders: ["setting:<key>"]`.
pub fn allowed_folder(widget: &WidgetInfo, inst: &Instance, key: &str) -> Result<PathBuf, String> {
    if !listed(widget, "folders", &format!("setting:{key}")) {
        return Err(format!("this widget has no permission for the `{key}` folder"));
    }
    let path = user_setting(inst, key).ok_or("choose a folder in this widget's settings")?;
    let p = PathBuf::from(shellexpand_home(&path));
    if !p.is_absolute() || !p.is_dir() {
        return Err(format!("“{path}” is not a folder"));
    }
    p.canonicalize().map_err(|e| e.to_string())
}

fn shellexpand_home(p: &str) -> String {
    match (p.strip_prefix("~/"), std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))) {
        (Some(rest), Some(home)) => Path::new(&home).join(rest).to_string_lossy().into_owned(),
        _ => p.to_string(),
    }
}

fn walk_kind(dir: &Path, root: &Path, depth: usize, images_only: bool, out: &mut Vec<Value>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        if out.len() >= MAX_FILES {
            return;
        }
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            if images_only && depth > 0 {
                walk_kind(&p, root, depth - 1, images_only, out);
            } else if !images_only {
                out.push(json!({ "name": name, "label": name, "dir": true, "ext": "" }));
            }
            continue;
        }
        let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
        if images_only && !IMAGE_EXT.contains(&ext.as_str()) {
            continue;
        }
        if !images_only {
            let label = app_label(&p).unwrap_or_else(|| {
                p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or(name.clone())
            });
            out.push(json!({ "name": name, "label": label, "dir": false, "ext": ext }));
            continue;
        }
        let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
        let meta = e.metadata().ok();
        let modified = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64);
        out.push(json!({ "name": rel, "size": meta.map(|m| m.len()), "modified": modified }));
    }
}

/// A launcher's name for an app shortcut (Linux .desktop files).
fn app_label(p: &Path) -> Option<String> {
    if p.extension()?.to_str()? != "desktop" {
        return None;
    }
    let text = std::fs::read_to_string(p).ok()?;
    text.lines().find_map(|l| l.strip_prefix("Name=")).map(|s| s.trim().to_string())
}

/// `kind: "all"` lists every file and folder at the top level (a folder
/// portal or app launcher); otherwise pictures, three folders deep.
#[tauri::command]
pub fn folder_list(
    window: WebviewWindow,
    state: State<AppState>,
    instance: Option<String>,
    key: String,
    kind: Option<String>,
) -> Result<Value, String> {
    let (inst, widget) = crate::widget_of(&window, &state, instance)?;
    let root = allowed_folder(&widget, &inst, &key)?;
    let mut files = Vec::new();
    let all = kind.as_deref() == Some("all");
    walk_kind(&root, &root, if all { 0 } else { 3 }, !all, &mut files);
    if all {
        files.sort_by(|a, b| {
            b["dir"].as_bool().cmp(&a["dir"].as_bool()).then(
                a["label"].as_str().unwrap_or("").to_lowercase().cmp(&b["label"].as_str().unwrap_or("").to_lowercase()),
            )
        });
    } else {
        files.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    }
    Ok(json!({ "folder": root, "count": files.len(), "files": files }))
}

/// Opens a file or app shortcut from the folder the user chose, with the
/// system's default program. Only names inside that folder are accepted.
#[tauri::command]
pub fn open_folder_item(
    window: WebviewWindow,
    state: State<AppState>,
    instance: Option<String>,
    key: String,
    name: String,
) -> Result<(), String> {
    let (inst, widget) = crate::widget_of(&window, &state, instance)?;
    if !widget.permission("launch") {
        return Err("this widget has no `launch` permission".into());
    }
    let root = allowed_folder(&widget, &inst, &key)?;
    if name.split(['/', '\\']).any(|p| p == ".." || p.is_empty()) {
        return Err("bad file name".into());
    }
    let full = root.join(&name).canonicalize().map_err(|e| e.to_string())?;
    if !full.starts_with(&root) {
        return Err("that file is outside the chosen folder".into());
    }
    // Linux app shortcuts are started, not opened in a text editor.
    #[cfg(target_os = "linux")]
    if full.extension().and_then(|e| e.to_str()) == Some("desktop") {
        let path = full.to_string_lossy().into_owned();
        if which("gio") {
            return Command::new("gio").args(["launch", &path]).spawn().map(|_| ()).map_err(|e| e.to_string());
        }
        if let Some(id) = full.file_name().and_then(|n| n.to_str()) {
            if which("gtk-launch") {
                return Command::new("gtk-launch").arg(id).spawn().map(|_| ()).map_err(|e| e.to_string());
            }
        }
    }
    tauri_plugin_opener::open_path(&full, None::<&str>).map_err(|e| e.to_string())
}

// ---------- log files ----------

const TAIL_BYTES: u64 = 128 * 1024;

/// The last lines of a file the user chose (`permissions.files: ["setting:<key>"]`).
#[tauri::command]
pub fn tail_file(
    window: WebviewWindow,
    state: State<AppState>,
    instance: Option<String>,
    key: String,
    lines: Option<usize>,
) -> Result<Value, String> {
    let (inst, widget) = crate::widget_of(&window, &state, instance)?;
    if !listed(&widget, "files", &format!("setting:{key}")) {
        return Err(format!("this widget has no permission for the `{key}` file"));
    }
    let Some(path) = user_setting(&inst, &key) else { return Ok(json!({ "configured": false })) };
    let p = PathBuf::from(shellexpand_home(&path));
    if !p.is_absolute() || !p.is_file() {
        return Err(format!("“{path}” is not a file"));
    }
    Ok(tail(&p, lines.unwrap_or(50).clamp(1, 500))?)
}

fn tail(p: &Path, want: usize) -> Result<Value, String> {
    use std::io::{Seek, SeekFrom};
    let mut f = std::fs::File::open(p).map_err(|e| format!("cannot read {}: {e}", p.display()))?;
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = size.saturating_sub(TAIL_BYTES);
    f.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&buf);
    let mut all: Vec<&str> = text.lines().collect();
    if start > 0 && !all.is_empty() {
        all.remove(0); // probably cut in half
    }
    let from = all.len().saturating_sub(want);
    let lines: Vec<Value> = all[from..]
        .iter()
        .map(|l| {
            let low = l.to_ascii_lowercase();
            let level = if ["error", "fatal", "crit", "panic", "fail"].iter().any(|w| low.contains(w)) {
                "bad"
            } else if low.contains("warn") {
                "warn"
            } else {
                ""
            };
            json!({ "text": l, "level": level })
        })
        .collect();
    Ok(json!({ "configured": true, "size": size, "lines": lines, "count": lines.len(), "path": p }))
}

// ---------- Wake-on-LAN ----------

/// Parses "aa:bb:cc:dd:ee:ff" (or with dashes, or none).
pub fn parse_mac(s: &str) -> Option<[u8; 6]> {
    let hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 12 || s.chars().any(|c| !(c.is_ascii_hexdigit() || matches!(c, ':' | '-' | '.' | ' '))) {
        return None;
    }
    let mut mac = [0u8; 6];
    for i in 0..6 {
        mac[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(mac)
}

pub fn magic_packet(mac: [u8; 6]) -> Vec<u8> {
    let mut p = vec![0xFF; 6];
    for _ in 0..16 {
        p.extend_from_slice(&mac);
    }
    p
}

/// Sends a wake-up packet to the machine whose address is in setting `key`
/// ("aa:bb:cc:dd:ee:ff", optionally "aa:..:ff @ 192.168.1.255").
#[tauri::command]
pub fn wake_on_lan(window: WebviewWindow, state: State<AppState>, instance: Option<String>, key: String) -> Result<(), String> {
    let (inst, widget) = crate::widget_of(&window, &state, instance)?;
    if !widget.permission("wake") {
        return Err("this widget has no `wake` permission".into());
    }
    let text = user_setting(&inst, &key).ok_or("type the computer's MAC address in this widget's settings")?;
    let (mac_text, target) = match text.split_once('@') {
        Some((m, t)) => (m.trim(), t.trim().to_string()),
        None => (text.as_str(), "255.255.255.255".to_string()),
    };
    let mac = parse_mac(mac_text).ok_or(format!("“{mac_text}” is not a MAC address"))?;
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
    sock.set_broadcast(true).map_err(|e| e.to_string())?;
    let packet = magic_packet(mac);
    for port in [9, 7] {
        sock.send_to(&packet, (target.as_str(), port)).map_err(|e| format!("could not send: {e}"))?;
    }
    Ok(())
}

// ---------- Minecraft server status ----------

fn varint(mut v: u32, out: &mut Vec<u8>) {
    loop {
        if v & !0x7F == 0 {
            out.push(v as u8);
            return;
        }
        out.push(((v & 0x7F) | 0x80) as u8);
        v >>= 7;
    }
}

fn read_varint(r: &mut impl Read) -> Result<u32, String> {
    let mut v = 0u32;
    for i in 0..5 {
        let mut b = [0u8];
        r.read_exact(&mut b).map_err(|e| e.to_string())?;
        v |= ((b[0] & 0x7F) as u32) << (7 * i);
        if b[0] & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err("bad number from server".into())
}

pub fn minecraft_handshake(host: &str, port: u16) -> Vec<u8> {
    let mut body = vec![0x00];
    varint(767, &mut body); // any recent protocol number works for a status request
    varint(host.len() as u32, &mut body);
    body.extend_from_slice(host.as_bytes());
    body.extend_from_slice(&port.to_be_bytes());
    varint(1, &mut body); // next: status
    let mut packet = Vec::new();
    varint(body.len() as u32, &mut packet);
    packet.extend(body);
    packet.extend([0x01, 0x00]); // status request
    packet
}

/// Minecraft (Java edition) server list ping: players online, version, message.
#[tauri::command]
pub async fn minecraft_status(
    window: WebviewWindow,
    state: State<'_, AppState>,
    instance: Option<String>,
    key: String,
) -> Result<Value, String> {
    let (inst, widget) = crate::widget_of(&window, &state, instance)?;
    if !widget.permission("game") {
        return Err("this widget has no `game` permission".into());
    }
    let Some(addr) = setting_text(&widget, &inst, &key) else { return Ok(json!({ "configured": false })) };
    blocking(move || {
        let (host, port) = match addr.rsplit_once(':') {
            Some((h, p)) if p.parse::<u16>().is_ok() => (h.to_string(), p.parse().unwrap()),
            _ => (addr.clone(), 25565),
        };
        let sa = (host.as_str(), port).to_socket_addrs().map_err(|e| format!("cannot find {host}: {e}"))?.next().ok_or("no address")?;
        let start = Instant::now();
        let mut s = match TcpStream::connect_timeout(&sa, Duration::from_secs(4)) {
            Ok(s) => s,
            Err(e) => return Ok(json!({ "configured": true, "online": false, "error": e.to_string(), "address": addr })),
        };
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        s.set_read_timeout(Some(Duration::from_secs(5))).ok();
        s.write_all(&minecraft_handshake(&host, port)).map_err(|e| e.to_string())?;
        let _len = read_varint(&mut s)?;
        let _id = read_varint(&mut s)?;
        let n = read_varint(&mut s)? as usize;
        if n > 1 << 20 {
            return Err("answer too large".into());
        }
        let mut buf = vec![0u8; n];
        s.read_exact(&mut buf).map_err(|e| e.to_string())?;
        let v: Value = serde_json::from_slice(&buf).map_err(|e| e.to_string())?;
        let motd = match &v["description"] {
            Value::String(t) => t.clone(),
            d => {
                let mut t = d["text"].as_str().unwrap_or("").to_string();
                for x in d["extra"].as_array().into_iter().flatten() {
                    t.push_str(x["text"].as_str().or(x.as_str()).unwrap_or(""));
                }
                t
            }
        };
        let players: Vec<Value> = v["players"]["sample"].as_array().cloned().unwrap_or_default();
        Ok(json!({
            "configured": true, "online": true, "address": addr, "ms": ms,
            "players": v["players"]["online"], "max": v["players"]["max"],
            "names": players.iter().filter_map(|p| p["name"].as_str()).collect::<Vec<_>>(),
            "version": v["version"]["name"], "motd": motd.replace(|c: char| c == '§', ""),
        }))
    })
    .await?
}

// ---------- latency ----------

/// Time to open a TCP connection to host:port. Works without admin rights,
/// unlike ICMP ping, and on every system.
#[tauri::command]
pub async fn net_latency(
    window: WebviewWindow,
    state: State<'_, AppState>,
    instance: Option<String>,
    host: String,
    port: u16,
) -> Result<Value, String> {
    let (_, widget) = crate::widget_of(&window, &state, instance)?;
    if !widget.permission("latency") {
        return Err("this widget has no `latency` permission".into());
    }
    let h = host.clone();
    blocking(move || {
        let addr = (h.as_str(), port)
            .to_socket_addrs()
            .map_err(|e| format!("cannot find {h}: {e}"))?
            .next()
            .ok_or(format!("cannot find {h}"))?;
        let start = Instant::now();
        match TcpStream::connect_timeout(&addr, Duration::from_secs(3)) {
            Ok(_) => Ok(json!({ "ok": true, "ms": start.elapsed().as_secs_f64() * 1000.0, "host": h, "port": port })),
            Err(e) => Ok(json!({ "ok": false, "ms": null, "host": h, "port": port, "error": e.to_string() })),
        }
    })
    .await?
}

// ---------- commands (shared helper) ----------

fn which(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| std::env::split_paths(&paths).any(|d| d.join(name).is_file()))
}

/// Runs a program (no shell), with a time limit, and returns stdout.
fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program).args(args).output().map_err(|e| format!("{program}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{program} failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

// ---------- volume mixer ----------

#[cfg(target_os = "linux")]
mod audio {
    use super::*;

    pub fn status() -> Value {
        if which("pactl") {
            pulse_status().unwrap_or_else(|e| json!({ "available": false, "reason": e }))
        } else if which("pw-dump") && which("wpctl") {
            pipewire_status().unwrap_or_else(|e| json!({ "available": false, "reason": e }))
        } else {
            json!({ "available": false, "reason": "No PipeWire (wpctl) or PulseAudio (pactl) found." })
        }
    }

    pub fn set(change: &Value) -> Result<(), String> {
        let kind = change["kind"].as_str().unwrap_or("");
        let id = change["id"].as_u64().ok_or("missing id")?.to_string();
        if which("pactl") {
            let (vol_cmd, mute_cmd) = match kind {
                "device" => ("set-sink-volume", "set-sink-mute"),
                "app" => ("set-sink-input-volume", "set-sink-input-mute"),
                _ => return Err("unknown audio target".into()),
            };
            if let Some(v) = change["volume"].as_f64() {
                run("pactl", &[vol_cmd, &id, &format!("{}%", (v.clamp(0.0, 1.5) * 100.0).round())])?;
            }
            if let Some(m) = change["muted"].as_bool() {
                run("pactl", &[mute_cmd, &id, if m { "1" } else { "0" }])?;
            }
            if change["default"].as_bool() == Some(true) && kind == "device" {
                // set-default-sink wants the name; find it from the index.
                let sinks: Value = serde_json::from_str(&run("pactl", &["-f", "json", "list", "sinks"])?).map_err(|e| e.to_string())?;
                let name = sinks
                    .as_array()
                    .and_then(|a| a.iter().find(|s| s["index"].as_u64().map(|i| i.to_string()) == Some(id.clone())))
                    .and_then(|s| s["name"].as_str())
                    .ok_or("output not found")?
                    .to_string();
                run("pactl", &["set-default-sink", &name])?;
            }
            return Ok(());
        }
        if let Some(v) = change["volume"].as_f64() {
            run("wpctl", &["set-volume", &id, &format!("{:.2}", v.clamp(0.0, 1.5))])?;
        }
        if let Some(m) = change["muted"].as_bool() {
            run("wpctl", &["set-mute", &id, if m { "1" } else { "0" }])?;
        }
        if change["default"].as_bool() == Some(true) && kind == "device" {
            run("wpctl", &["set-default", &id])?;
        }
        Ok(())
    }

    fn pipewire_status() -> Result<Value, String> {
        let dump: Value = serde_json::from_str(&run("pw-dump", &[])?).map_err(|e| e.to_string())?;
        let default_id = run("wpctl", &["inspect", "@DEFAULT_AUDIO_SINK@"])
            .ok()
            .and_then(|s| s.split_whitespace().nth(1).map(|x| x.trim_end_matches(',').to_string()))
            .and_then(|x| x.parse::<u64>().ok());
        let mut devices = Vec::new();
        let mut apps = Vec::new();
        for o in dump.as_array().into_iter().flatten() {
            if o["type"] != "PipeWire:Interface:Node" {
                continue;
            }
            let props = &o["info"]["props"];
            let class = props["media.class"].as_str().unwrap_or("");
            let Some(id) = o["id"].as_u64() else { continue };
            // wpctl's numbers match what `set-volume` takes.
            let vol = run("wpctl", &["get-volume", &id.to_string()]).unwrap_or_default();
            let volume = vol.split_whitespace().nth(1).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            let muted = vol.contains("[MUTED]");
            match class {
                "Audio/Sink" => {
                    let name = props["node.description"].as_str().or(props["node.nick"].as_str()).or(props["node.name"].as_str());
                    devices.push(json!({ "id": id, "name": name, "volume": volume, "muted": muted, "default": Some(id) == default_id }));
                }
                "Stream/Output/Audio" => {
                    let name = props["application.name"].as_str().or(props["media.name"].as_str()).or(props["node.name"].as_str());
                    apps.push(json!({ "id": id, "name": name, "volume": volume, "muted": muted }));
                }
                _ => {}
            }
        }
        Ok(json!({ "available": true, "system": "PipeWire", "devices": devices, "apps": apps }))
    }

    fn pulse_volume(v: &Value) -> f64 {
        // Average of the channels' "value_percent" ("40%").
        let vals: Vec<f64> = v
            .as_object()
            .map(|m| {
                m.values()
                    .filter_map(|c| c["value_percent"].as_str()?.trim_end_matches('%').parse::<f64>().ok())
                    .collect()
            })
            .unwrap_or_default();
        if vals.is_empty() { 0.0 } else { vals.iter().sum::<f64>() / vals.len() as f64 / 100.0 }
    }

    fn pulse_status() -> Result<Value, String> {
        let sinks: Value = serde_json::from_str(&run("pactl", &["-f", "json", "list", "sinks"])?).map_err(|e| e.to_string())?;
        let inputs: Value = serde_json::from_str(&run("pactl", &["-f", "json", "list", "sink-inputs"])?).map_err(|e| e.to_string())?;
        let default = run("pactl", &["get-default-sink"]).unwrap_or_default().trim().to_string();
        let devices: Vec<Value> = sinks
            .as_array()
            .into_iter()
            .flatten()
            .map(|s| {
                json!({
                    "id": s["index"], "name": s["description"].as_str().or(s["name"].as_str()),
                    "volume": pulse_volume(&s["volume"]), "muted": s["mute"], "default": s["name"].as_str() == Some(default.as_str()),
                })
            })
            .collect();
        let apps: Vec<Value> = inputs
            .as_array()
            .into_iter()
            .flatten()
            .map(|i| {
                json!({
                    "id": i["index"], "name": i["properties"]["application.name"].as_str().or(i["properties"]["media.name"].as_str()),
                    "volume": pulse_volume(&i["volume"]), "muted": i["mute"],
                })
            })
            .collect();
        Ok(json!({ "available": true, "system": "PulseAudio", "devices": devices, "apps": apps }))
    }
}

#[cfg(not(target_os = "linux"))]
mod audio {
    use super::*;
    pub fn status() -> Value {
        json!({ "available": false, "reason": "The volume mixer works on Linux so far; Windows and macOS are planned." })
    }
    pub fn set(_change: &Value) -> Result<(), String> {
        Err("not available on this system yet".into())
    }
}

#[tauri::command]
pub async fn audio_status(window: WebviewWindow, state: State<'_, AppState>, instance: Option<String>) -> Result<Value, String> {
    let (_, widget) = crate::widget_of(&window, &state, instance)?;
    if !widget.permission("audio") {
        return Err("this widget has no `audio` permission".into());
    }
    blocking(audio::status).await
}

#[tauri::command]
pub async fn audio_set(window: WebviewWindow, state: State<'_, AppState>, instance: Option<String>, change: Value) -> Result<(), String> {
    let (_, widget) = crate::widget_of(&window, &state, instance)?;
    if !widget.permission("audio") {
        return Err("this widget has no `audio` permission".into());
    }
    blocking(move || audio::set(&change)).await?
}

// ---------- Docker ----------

/// Docker's API over its local socket (unix:///var/run/docker.sock, Podman's
/// socket) or a remote host (http://server:2375). The address is a setting.
fn docker_address(widget: &WidgetInfo, inst: &Instance) -> String {
    setting_text(widget, inst, "docker").unwrap_or_else(|| "unix:///var/run/docker.sock".into())
}

/// One HTTP request to Docker. Returns (status, body).
fn docker_http(address: &str, method: &str, path: &str) -> Result<(u16, String), String> {
    let request = format!("{method} {path} HTTP/1.0\r\nHost: docker\r\nContent-Length: 0\r\n\r\n");
    let raw = if let Some(sock) = address.strip_prefix("unix://") {
        #[cfg(unix)]
        {
            let mut s = std::os::unix::net::UnixStream::connect(sock)
                .map_err(|e| format!("cannot reach Docker at {sock}: {e} (is it running, and are you in the docker group?)"))?;
            s.set_read_timeout(Some(Duration::from_secs(10))).ok();
            s.write_all(request.as_bytes()).map_err(|e| e.to_string())?;
            let mut buf = Vec::new();
            s.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            buf
        }
        #[cfg(not(unix))]
        {
            let _ = sock;
            return Err("on Windows, enable Docker's TCP address (http://localhost:2375) and use that".into());
        }
    } else {
        let u = url::Url::parse(address).map_err(|_| format!("“{address}” is not a Docker address"))?;
        if u.scheme() != "http" {
            return Err("use unix:///path/to/docker.sock or http://host:port".into());
        }
        let host = u.host_str().ok_or("no host")?;
        let port = u.port().unwrap_or(2375);
        let addr = (host, port).to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("cannot find host")?;
        let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(5)).map_err(|e| format!("cannot reach Docker at {address}: {e}"))?;
        s.set_read_timeout(Some(Duration::from_secs(10))).ok();
        s.write_all(request.as_bytes()).map_err(|e| e.to_string())?;
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        buf
    };
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").ok_or("bad answer from Docker")?;
    let status = head.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    Ok((status, body.to_string()))
}

#[tauri::command]
pub async fn docker_list(window: WebviewWindow, state: State<'_, AppState>, instance: Option<String>) -> Result<Value, String> {
    let (inst, widget) = crate::widget_of(&window, &state, instance)?;
    if !widget.permission("docker") {
        return Err("this widget has no `docker` permission".into());
    }
    let address = docker_address(&widget, &inst);
    blocking(move || {
        let (status, body) = docker_http(&address, "GET", "/containers/json?all=1")?;
        if status != 200 {
            return Err(format!("Docker answered {status}: {}", body.trim()));
        }
        let list: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
        let mut containers: Vec<Value> = list
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| {
                let name = c["Names"][0].as_str().unwrap_or("").trim_start_matches('/').to_string();
                json!({
                    "id": c["Id"].as_str().map(|s| &s[..s.len().min(12)]),
                    "name": name, "image": c["Image"], "state": c["State"], "status": c["Status"],
                    "running": c["State"] == "running",
                })
            })
            .collect();
        containers.sort_by(|a, b| b["running"].as_bool().cmp(&a["running"].as_bool()).then(a["name"].as_str().cmp(&b["name"].as_str())));
        let running = containers.iter().filter(|c| c["running"] == true).count();
        Ok(json!({ "containers": containers, "running": running, "total": containers.len(), "address": address }))
    })
    .await?
}

#[tauri::command]
pub async fn docker_action(
    window: WebviewWindow,
    state: State<'_, AppState>,
    instance: Option<String>,
    id: String,
    action: String,
) -> Result<(), String> {
    let (inst, widget) = crate::widget_of(&window, &state, instance)?;
    if !widget.permission("docker") {
        return Err("this widget has no `docker` permission".into());
    }
    if !["start", "stop", "restart"].contains(&action.as_str()) {
        return Err(format!("unknown Docker action `{action}`"));
    }
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.') {
        return Err("bad container id".into());
    }
    let address = docker_address(&widget, &inst);
    blocking(move || {
        let (status, body) = docker_http(&address, "POST", &format!("/containers/{id}/{action}"))?;
        if status == 204 || status == 304 {
            Ok(())
        } else {
            Err(format!("Docker answered {status}: {}", body.trim()))
        }
    })
    .await?
}

// ---------- desktop notifications ----------

/// A desktop notification (break reminders). Linux uses notify-send,
/// macOS osascript; Windows is not done yet (the widget still flashes).
#[tauri::command]
pub fn notify(window: WebviewWindow, state: State<AppState>, instance: Option<String>, title: String, body: String) -> Result<(), String> {
    let (_, widget) = crate::widget_of(&window, &state, instance)?;
    if !widget.permission("notify") {
        return Err("this widget has no `notify` permission".into());
    }
    let title: String = title.chars().take(80).collect();
    let body: String = body.chars().take(300).collect();
    #[cfg(target_os = "linux")]
    {
        if which("notify-send") {
            return Command::new("notify-send").args(["--app-name=WidgetForge", &title, &body]).spawn().map(|_| ()).map_err(|e| e.to_string());
        }
        return Err("install notify-send (libnotify) for desktop notifications".into());
    }
    #[cfg(target_os = "macos")]
    {
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!("display notification \"{}\" with title \"{}\"", esc(&body), esc(&title));
        return Command::new("osascript").args(["-e", &script]).spawn().map(|_| ()).map_err(|e| e.to_string());
    }
    #[allow(unreachable_code)]
    {
        let _ = (title, body);
        Err("desktop notifications are not done on this system yet".into())
    }
}

// ---------- speed test ----------

/// Measures download and upload speed against the user's own LibreSpeed
/// server (`server` setting). Downloads at most 100 MB or 12 seconds.
#[tauri::command]
pub async fn speed_test(window: WebviewWindow, state: State<'_, AppState>, instance: Option<String>) -> Result<Value, String> {
    let (inst, widget) = crate::widget_of(&window, &state, instance)?;
    if !listed(&widget, "network", "setting:server") {
        return Err("this widget has no permission to reach a speed-test server".into());
    }
    let Some(base) = user_setting(&inst, "server") else { return Ok(json!({ "configured": false })) };
    let base = base.trim_end_matches('/').to_string();
    let client = reqwest::Client::builder().timeout(Duration::from_secs(20)).user_agent("WidgetForge/0.1").build().map_err(|e| e.to_string())?;
    // LibreSpeed's files live in backend/ in its container; older setups have them at the top.
    let mut backend = format!("{base}/backend");
    if client.get(format!("{backend}/empty.php")).send().await.map(|r| !r.status().is_success()).unwrap_or(true) {
        backend = base.clone();
    }
    let mut ping = f64::MAX;
    for _ in 0..4 {
        let t = Instant::now();
        match client.get(format!("{backend}/empty.php?r={}", t.elapsed().as_nanos())).send().await {
            Ok(r) if r.status().is_success() => ping = ping.min(t.elapsed().as_secs_f64() * 1000.0),
            Ok(r) => return Ok(json!({ "configured": true, "ok": false, "error": format!("the server answered {}", r.status()) })),
            Err(e) => return Ok(json!({ "configured": true, "ok": false, "error": e.to_string() })),
        }
    }
    let limit = Duration::from_secs(12);
    let t = Instant::now();
    let mut got = 0u64;
    let mut resp = client.get(format!("{backend}/garbage.php?ckSize=100")).send().await.map_err(|e| e.to_string())?;
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        got += chunk.len() as u64;
        if t.elapsed() > limit {
            break;
        }
    }
    let down = got as f64 * 8.0 / t.elapsed().as_secs_f64().max(0.001) / 1_000_000.0;
    let body = vec![0u8; 16 * 1024 * 1024];
    let t = Instant::now();
    let up = match client.post(format!("{backend}/empty.php")).body(body).send().await {
        Ok(r) if r.status().is_success() => Some(16.0 * 1024.0 * 1024.0 * 8.0 / t.elapsed().as_secs_f64().max(0.001) / 1_000_000.0),
        _ => None,
    };
    Ok(json!({ "configured": true, "ok": true, "down_mbps": down, "up_mbps": up, "ping_ms": ping, "bytes": got }))
}

// ---------- command output ----------

const OUTPUT_LIMIT: usize = 64 * 1024;

/// Runs the command the user typed into the `command` setting (never a
/// manifest default), through the system shell, for at most 15 seconds.
#[tauri::command]
pub async fn run_command(
    window: WebviewWindow,
    state: State<'_, AppState>,
    instance: Option<String>,
    key: Option<String>,
) -> Result<Value, String> {
    let (inst, widget) = crate::widget_of(&window, &state, instance)?;
    // `key` picks which command setting to run (action buttons have several).
    let key = key.unwrap_or_else(|| "command".into());
    if !listed(&widget, "run", &format!("setting:{key}")) {
        return Err("this widget has no permission to run commands".into());
    }
    let Some(cmd) = user_setting(&inst, &key) else {
        return Ok(json!({ "ok": false, "configured": false }));
    };
    blocking(move || run_shell(&cmd, Duration::from_secs(15))).await?
}

pub(crate) fn run_shell(cmd: &str, limit: Duration) -> Result<Value, String> {
    let mut child = if cfg!(windows) {
        Command::new("cmd").args(["/C", cmd]).stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null()).spawn()
    } else {
        Command::new("sh").args(["-c", cmd]).stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null()).spawn()
    }
    .map_err(|e| e.to_string())?;
    let read = |mut r: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = r.by_ref().take(OUTPUT_LIMIT as u64).read_to_end(&mut buf);
            String::from_utf8_lossy(&buf).into_owned()
        })
    };
    let out = read(Box::new(child.stdout.take().unwrap()));
    let err = read(Box::new(child.stderr.take().unwrap()));
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break Some(s);
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = out.join().unwrap_or_default();
    let stderr = err.join().unwrap_or_default();
    let json: Option<Value> = serde_json::from_str(stdout.trim()).ok();
    Ok(json!({
        "configured": true,
        "ok": status.is_some_and(|s| s.success()),
        "code": status.and_then(|s| s.code()),
        "timed_out": status.is_none(),
        "stdout": stdout.trim_end(),
        "stderr": stderr.trim_end(),
        "lines": stdout.trim_end().lines().collect::<Vec<_>>(),
        "json": json,
        "ms": start.elapsed().as_millis() as u64,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_output_exit_code_and_timeout() {
        let r = run_shell("echo hello; echo '{\"a\":1}' >&2; exit 3", Duration::from_secs(5)).unwrap();
        assert_eq!(r["stdout"], "hello");
        assert_eq!(r["code"], 3);
        assert_eq!(r["ok"], false);
        let r = run_shell("echo '{\"temp\": 41}'", Duration::from_secs(5)).unwrap();
        assert_eq!(r["json"]["temp"], 41);
        let r = run_shell("sleep 5", Duration::from_millis(300)).unwrap();
        assert_eq!(r["timed_out"], true);
    }

    /// A tiny fake Docker on a TCP port: answers one request, then reports what it got.
    fn fake_docker(status_line: &'static str, body: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        let t = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut buf = [0u8; 2048];
            let n = s.read(&mut buf).unwrap();
            let req = String::from_utf8_lossy(&buf[..n]).lines().next().unwrap_or("").to_string();
            write!(s, "HTTP/1.0 {status_line}\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
            req
        });
        (addr, t)
    }

    #[test]
    fn docker_requests_and_answers() {
        let (addr, t) = fake_docker("200 OK", "[]");
        assert_eq!(docker_http(&addr, "GET", "/containers/json?all=1").unwrap(), (200, "[]".into()));
        assert_eq!(t.join().unwrap(), "GET /containers/json?all=1 HTTP/1.0");
        let (addr, t) = fake_docker("204 No Content", "");
        assert_eq!(docker_http(&addr, "POST", "/containers/abc123/restart").unwrap().0, 204);
        assert_eq!(t.join().unwrap(), "POST /containers/abc123/restart HTTP/1.0");
        assert!(docker_http("ftp://x", "GET", "/").is_err());
    }

    #[test]
    fn wake_packet_and_mac() {
        assert_eq!(parse_mac("AA:bb:cc:01:02:03"), Some([0xaa, 0xbb, 0xcc, 1, 2, 3]));
        assert_eq!(parse_mac("aa-bb-cc-01-02-03"), Some([0xaa, 0xbb, 0xcc, 1, 2, 3]));
        assert_eq!(parse_mac("aa:bb:cc"), None);
        assert_eq!(parse_mac("zz:bb:cc:01:02:03"), None);
        let p = magic_packet([1, 2, 3, 4, 5, 6]);
        assert_eq!(p.len(), 102);
        assert_eq!(&p[..6], &[0xFF; 6]);
        assert_eq!(&p[96..], &[1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn minecraft_handshake_bytes() {
        let p = minecraft_handshake("mc.local", 25565);
        // length, id 0, protocol 767 (0xFF 0x05), 8-byte name, port, state 1, then the status request
        assert_eq!(&p[..4], &[15, 0x00, 0xFF, 0x05]);
        assert_eq!(p[4], 8);
        assert_eq!(&p[5..13], b"mc.local");
        assert_eq!(&p[13..16], &[0x63, 0xDD, 0x01]);
        assert_eq!(&p[16..], &[0x01, 0x00]);
        let mut r: &[u8] = &[0xFF, 0x05];
        assert_eq!(read_varint(&mut r).unwrap(), 767);
    }

    #[test]
    fn log_tail_marks_errors() {
        let dir = std::env::temp_dir().join(format!("wf-tail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("app.log");
        std::fs::write(&f, "one\ntwo WARN disk\nthree ERROR boom\n").unwrap();
        let r = tail(&f, 2).unwrap();
        assert_eq!(r["count"], 2);
        assert_eq!(r["lines"][0]["level"], "warn");
        assert_eq!(r["lines"][1]["level"], "bad");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn command_comes_only_from_user_settings() {
        let mut inst: Instance = serde_json::from_value(json!({
            "id": "x", "widget_id": "w", "x": 0, "y": 0, "w": 1, "h": 1, "layer": "top"
        }))
        .unwrap();
        assert_eq!(user_setting(&inst, "command"), None);
        inst.settings.insert("command".into(), json!("  uptime "));
        assert_eq!(user_setting(&inst, "command").as_deref(), Some("uptime"));
    }
}
