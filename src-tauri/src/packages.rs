//! `.forgewidget` packages: a widget folder in a zip file.
//!
//! Export zips a widget's folder. Import shows what a package is and asks for
//! (Hub → Add-ons), then unpacks it into the add-ons folder. Import refuses
//! absolute paths, `..`, links, encrypted entries, files over 20 MB and
//! packages over 100 MB (counted while unpacking, not trusted from the file),
//! and ids starting with `forge.` (those are the built-in widgets). A replaced
//! add-on is moved to add-ons/.replaced/, never deleted.
//!
//! The zip reading and writing is done here (stored or deflate entries, no
//! zip64), using the deflate library the app already includes.

use crate::AppState;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

pub const EXT: &str = "forgewidget";
const MAX_FILE: u64 = 20 * 1024 * 1024;
const MAX_TOTAL: u64 = 100 * 1024 * 1024;
const MAX_ENTRIES: usize = 2000;

// ---------- zip writing ----------

struct Entry {
    name: String,
    crc: u32,
    csize: u32,
    usize: u32,
    offset: u32,
    method: u16,
}

fn dos_time() -> (u16, u16) {
    // 1 Jan 2026 00:00, fixed: the same folder makes the same file.
    let date = ((2026 - 1980) << 9) | (1 << 5) | 1;
    (0, date as u16)
}

/// Zips `files` (path inside the zip, bytes).
pub fn write_zip(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut entries = Vec::new();
    let (time, date) = dos_time();
    for (name, data) in files {
        let crc = crc32fast::hash(data);
        let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(data).map_err(|e| e.to_string())?;
        let deflated = enc.finish().map_err(|e| e.to_string())?;
        // Keep it stored if deflate does not help (pictures).
        let (method, body) = if deflated.len() < data.len() { (8u16, deflated) } else { (0u16, data.clone()) };
        let offset = out.len() as u32;
        out.extend_from_slice(&0x04034b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0x0800u16.to_le_bytes()); // UTF-8 names
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&time.to_le_bytes());
        out.extend_from_slice(&date.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&body);
        entries.push(Entry { name: name.clone(), crc, csize: body.len() as u32, usize: data.len() as u32, offset, method });
    }
    let cd_start = out.len() as u32;
    for e in &entries {
        out.extend_from_slice(&0x02014b50u32.to_le_bytes());
        out.extend_from_slice(&((3u16 << 8) | 20).to_le_bytes()); // made by: Unix
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0x0800u16.to_le_bytes());
        out.extend_from_slice(&e.method.to_le_bytes());
        out.extend_from_slice(&time.to_le_bytes());
        out.extend_from_slice(&date.to_le_bytes());
        out.extend_from_slice(&e.crc.to_le_bytes());
        out.extend_from_slice(&e.csize.to_le_bytes());
        out.extend_from_slice(&e.usize.to_le_bytes());
        out.extend_from_slice(&(e.name.len() as u16).to_le_bytes());
        out.extend_from_slice(&[0u8; 8]); // extra, comment, disk, internal attrs
        out.extend_from_slice(&((0o100644u32) << 16).to_le_bytes()); // regular file
        out.extend_from_slice(&e.offset.to_le_bytes());
        out.extend_from_slice(e.name.as_bytes());
    }
    let cd_size = out.len() as u32 - cd_start;
    out.extend_from_slice(&0x06054b50u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    Ok(out)
}

// ---------- zip reading ----------

fn u16_at(b: &[u8], i: usize) -> Result<u16, String> {
    b.get(i..i + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| "the package is damaged".to_string())
}
fn u32_at(b: &[u8], i: usize) -> Result<u32, String> {
    b.get(i..i + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| "the package is damaged".to_string())
}

/// A safe relative path inside the package, or an error saying why not.
pub fn safe_name(name: &str) -> Result<PathBuf, String> {
    if name.contains('\\') || name.contains('\0') {
        return Err(format!("“{name}” has a character that is not allowed"));
    }
    let p = Path::new(name);
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Normal(s) => out.push(s),
            Component::CurDir => {}
            _ => return Err(format!("“{name}” points outside the package")),
        }
    }
    if out.as_os_str().is_empty() {
        return Err("an empty file name".into());
    }
    Ok(out)
}

/// Unzips into memory with every safety rule applied. Folders are skipped.
pub fn read_zip(bytes: &[u8]) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    if bytes.len() as u64 > MAX_TOTAL {
        return Err("the package is larger than 100 MB".into());
    }
    let eocd = (0..bytes.len().saturating_sub(21))
        .rev()
        .take(65_557)
        .find(|&i| bytes[i..].starts_with(&0x06054b50u32.to_le_bytes()))
        .ok_or("this is not a .forgewidget (zip) file")?;
    let count = u16_at(bytes, eocd + 10)? as usize;
    let mut at = u32_at(bytes, eocd + 16)? as usize;
    if count > MAX_ENTRIES {
        return Err("the package has too many files".into());
    }
    let mut total = 0u64;
    let mut files = Vec::new();
    for _ in 0..count {
        if u32_at(bytes, at)? != 0x02014b50 {
            return Err("the package is damaged".into());
        }
        let flags = u16_at(bytes, at + 8)?;
        let method = u16_at(bytes, at + 10)?;
        let crc = u32_at(bytes, at + 16)?;
        let nlen = u16_at(bytes, at + 28)? as usize;
        let elen = u16_at(bytes, at + 30)? as usize;
        let clen = u16_at(bytes, at + 32)? as usize;
        let made_by = u16_at(bytes, at + 4)? >> 8;
        let attrs = u32_at(bytes, at + 38)?;
        let local = u32_at(bytes, at + 42)? as usize;
        let csize = u32_at(bytes, at + 20)? as usize;
        let name = String::from_utf8_lossy(bytes.get(at + 46..at + 46 + nlen).ok_or("the package is damaged")?).into_owned();
        at += 46 + nlen + elen + clen;
        if name.ends_with('/') {
            safe_name(name.trim_end_matches('/'))?;
            continue;
        }
        let path = safe_name(&name)?;
        if flags & 1 != 0 {
            return Err(format!("“{name}” is encrypted"));
        }
        // Unix mode in the upper half: refuse links and anything not a plain file.
        if made_by == 3 {
            let kind = (attrs >> 16) & 0o170000;
            if kind == 0o120000 {
                return Err(format!("“{name}” is a link; packages may only hold plain files"));
            }
            if kind != 0 && kind != 0o100000 {
                return Err(format!("“{name}” is not a plain file"));
            }
        }
        if u32_at(bytes, local)? != 0x04034b50 {
            return Err("the package is damaged".into());
        }
        let data_at = local + 30 + u16_at(bytes, local + 26)? as usize + u16_at(bytes, local + 28)? as usize;
        let raw = bytes.get(data_at..data_at + csize).ok_or("the package is damaged")?;
        let limit = MAX_FILE.min(MAX_TOTAL - total);
        let data = match method {
            0 => raw.to_vec(),
            8 => {
                let mut out = Vec::new();
                flate2::read::DeflateDecoder::new(raw).take(limit + 1).read_to_end(&mut out).map_err(|e| format!("“{name}”: {e}"))?;
                out
            }
            m => return Err(format!("“{name}” uses a compression this app does not read ({m})")),
        };
        if data.len() as u64 > MAX_FILE {
            return Err(format!("“{name}” is larger than 20 MB"));
        }
        total += data.len() as u64;
        if total > MAX_TOTAL {
            return Err("the package unpacks to more than 100 MB".into());
        }
        if crc32fast::hash(&data) != crc {
            return Err(format!("“{name}” is damaged (checksum)"));
        }
        files.push((path, data));
    }
    // A package may hold the widget folder itself ("my.widget/widget.yaml"):
    // then that one top folder is removed.
    if !files.iter().any(|(p, _)| p == Path::new("widget.yaml")) {
        let tops: std::collections::BTreeSet<_> = files.iter().filter_map(|(p, _)| p.components().next().map(|c| c.as_os_str().to_owned())).collect();
        if tops.len() == 1 {
            let top = tops.into_iter().next().unwrap();
            files = files.into_iter().filter_map(|(p, d)| p.strip_prefix(&top).ok().map(|q| (q.to_path_buf(), d))).collect();
        }
    }
    Ok(files)
}

// ---------- reading a package ----------

#[derive(Debug)]
pub struct Package {
    pub id: String,
    pub manifest: Value,
    pub files: Vec<(PathBuf, Vec<u8>)>,
}

pub fn open_package(bytes: &[u8]) -> Result<Package, String> {
    let files = read_zip(bytes)?;
    let yaml = files.iter().find(|(p, _)| p == Path::new("widget.yaml")).ok_or("the package has no widget.yaml")?;
    let manifest: Value = serde_yaml::from_slice(&yaml.1).map_err(|e| format!("widget.yaml: {e}"))?;
    let id = crate::model::validate(&manifest)?;
    if id.starts_with("forge.") {
        return Err("ids starting with forge. belong to the built-in widgets; this package cannot replace one".into());
    }
    for s in manifest["settings"].as_array().into_iter().flatten() {
        if s["type"] == "command" && s.get("default").is_some_and(|d| !d.is_null() && d != "") {
            return Err("the widget tries to ship a command to run; only you may type one".into());
        }
    }
    Ok(Package { id, manifest, files })
}

/// What a widget may do, in plain words, for the import screen.
pub fn describe_permissions(m: &Value) -> Vec<String> {
    let p = &m["permissions"];
    let mut out = Vec::new();
    let list = |k: &str| p[k].as_array().cloned().unwrap_or_default().into_iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>();
    for c in list("system") {
        out.push(format!("Read this computer's {c}"));
    }
    for h in list("network") {
        out.push(match h.strip_prefix("setting:") {
            Some(k) => format!("Reach the address you type in “{k}”"),
            None => format!("Reach {h}"),
        });
    }
    for k in list("run") {
        out.push(format!("Run the command you type in “{}”", k.trim_start_matches("setting:")));
    }
    for k in list("folders") {
        out.push(format!("Read the folder you choose in “{}”", k.trim_start_matches("setting:")));
    }
    for k in list("files") {
        out.push(format!("Read the file you choose in “{}”", k.trim_start_matches("setting:")));
    }
    for (flag, words) in [
        ("storage", "Save its own notes and lists"),
        ("clipboard", "Read and write the clipboard"),
        ("media", "See and control what is playing"),
        ("audio", "Change volumes"),
        ("docker", "Start and stop Docker containers"),
        ("latency", "Test connections to addresses"),
        ("discord", "Use your Discord sign-in"),
        ("wake", "Send wake-up packets on your network"),
        ("game", "Ask game servers for their status"),
        ("mail", "Check your mailbox"),
        ("activity", "Record which apps you use (kept on this computer)"),
        ("notify", "Show desktop notifications"),
        ("launch", "Open files from the folder you choose"),
    ] {
        if p[flag] == true {
            out.push(words.to_string());
        }
    }
    out
}

// ---------- commands ----------

fn downloads_dir() -> PathBuf {
    dirs::download_dir().or_else(dirs::home_dir).unwrap_or_else(std::env::temp_dir)
}

fn files_of(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    let mut total = 0u64;
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).map_err(|e| e.to_string())?.flatten() {
            let meta = std::fs::symlink_metadata(e.path()).map_err(|e| e.to_string())?;
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                stack.push(e.path());
            } else if meta.is_file() {
                total += meta.len();
                if meta.len() > MAX_FILE || total > MAX_TOTAL {
                    return Err(format!("{name} is too large for a package"));
                }
                let rel = e.path().strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
                out.push((rel, std::fs::read(e.path()).map_err(|e| e.to_string())?));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Saves a widget as <Downloads>/<id>-<version>.forgewidget and returns the path.
#[tauri::command]
pub fn export_widget(window: WebviewWindow, state: State<AppState>, id: String) -> Result<String, String> {
    crate::shell::hub_only(&window)?;
    let w = state.widget(&id)?;
    let files = files_of(&w.dir)?;
    let zip = write_zip(&files)?;
    let version = w.manifest["version"].as_str().unwrap_or("1.0.0").replace(|c: char| !c.is_ascii_alphanumeric() && c != '.', "-");
    let dir = downloads_dir();
    let mut path = dir.join(format!("{id}-{version}.{EXT}"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{id}-{version} ({n}).{EXT}"));
        n += 1;
    }
    std::fs::write(&path, zip).map_err(|e| format!("could not save {}: {e}", path.display()))?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn reveal_file(window: WebviewWindow, path: String) -> Result<(), String> {
    crate::shell::hub_only(&window)?;
    tauri_plugin_opener::reveal_item_in_dir(&path).map_err(|e| e.to_string())
}

fn read_package_file(path: &str) -> Result<Vec<u8>, String> {
    let p = PathBuf::from(path.trim());
    if p.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case(EXT)) != Some(true) {
        return Err("choose a .forgewidget file".into());
    }
    let meta = std::fs::metadata(&p).map_err(|e| format!("cannot open {}: {e}", p.display()))?;
    if meta.len() > MAX_TOTAL {
        return Err("the package is larger than 100 MB".into());
    }
    std::fs::read(&p).map_err(|e| e.to_string())
}

/// What a package is, what it may do, and what it would replace.
#[tauri::command]
pub fn inspect_package(window: WebviewWindow, state: State<AppState>, path: String) -> Result<Value, String> {
    crate::shell::hub_only(&window)?;
    let pkg = open_package(&read_package_file(&path)?)?;
    let installed = state.widgets.lock().unwrap().get(&pkg.id).cloned();
    let perms = describe_permissions(&pkg.manifest);
    let (old_version, new_perms) = match &installed {
        Some(w) => {
            let old = describe_permissions(&w.manifest);
            (w.manifest["version"].clone(), perms.iter().filter(|p| !old.contains(p)).cloned().collect::<Vec<_>>())
        }
        None => (Value::Null, vec![]),
    };
    let html = pkg.manifest.pointer("/ui/entry").is_some();
    Ok(json!({
        "path": path,
        "id": pkg.id,
        "name": pkg.manifest["name"],
        "version": pkg.manifest["version"],
        "author": pkg.manifest["author"],
        "description": pkg.manifest["description"],
        "files": pkg.files.len(),
        "bytes": pkg.files.iter().map(|(_, d)| d.len()).sum::<usize>(),
        "html": html,
        "permissions": perms,
        "installed": installed.is_some(),
        "installed_source": installed.as_ref().map(|w| w.source.clone()),
        "installed_version": old_version,
        "new_permissions": new_perms,
    }))
}

/// Unpacks a package into the add-ons folder. An add-on with the same id is
/// moved to add-ons/.replaced/<id>-<time> first (only with `replace`).
#[tauri::command]
pub fn install_package(window: WebviewWindow, app: AppHandle, state: State<AppState>, path: String, replace: bool) -> Result<Value, String> {
    crate::shell::hub_only(&window)?;
    let pkg = open_package(&read_package_file(&path)?)?;
    let target = state.addons_dir.join(&pkg.id);
    if target.exists() && !replace {
        return Err(format!("{} is already installed", pkg.id));
    }
    let staging = state.addons_dir.join(format!(".installing-{}", pkg.id));
    let _ = std::fs::remove_dir_all(&staging); // our own leftover from a failed install
    for (rel, data) in &pkg.files {
        let full = staging.join(rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&full, data).map_err(|e| e.to_string())?;
    }
    if target.exists() {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let keep = state.addons_dir.join(".replaced");
        std::fs::create_dir_all(&keep).map_err(|e| e.to_string())?;
        std::fs::rename(&target, keep.join(format!("{}-{stamp}", pkg.id))).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&staging, &target).map_err(|e| e.to_string())?;
    let list = crate::rescan_widgets(&state);
    let _ = app.emit("layout-changed", ());
    Ok(json!({ "id": pkg.id, "widgets": list }))
}

/// .forgewidget files given on the command line (double-click, "Open with").
pub fn packages_in_args(args: &[String]) -> Vec<String> {
    args.iter().skip(1).filter(|a| a.to_ascii_lowercase().ends_with(&format!(".{EXT}"))).cloned().collect()
}

/// Shows the Hub's import screen for a package opened from outside.
pub fn offer_import(app: &AppHandle, path: String) {
    crate::show_hub(app);
    let state = app.state::<AppState>();
    state.pending_imports.lock().unwrap().push(path.clone());
    let _ = app.emit("import-package", path);
}

/// Packages opened before the Hub was ready.
#[tauri::command]
pub fn pending_imports(window: WebviewWindow, state: State<AppState>) -> Result<Vec<String>, String> {
    crate::shell::hub_only(&window)?;
    Ok(std::mem::take(&mut *state.pending_imports.lock().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(files: &[(&str, &str)]) -> Vec<u8> {
        write_zip(&files.iter().map(|(n, d)| (n.to_string(), d.as_bytes().to_vec())).collect::<Vec<_>>()).unwrap()
    }
    const YAML: &str = "schema: 1\nid: sam.cpu\nname: CPU\nversion: 1.2.0\npermissions:\n  system: [cpu]\n  storage: true\nui:\n  blocks: []\n";

    #[test]
    fn round_trip() {
        let long = "x".repeat(5000);
        let z = pkg(&[("widget.yaml", YAML), ("assets/a.txt", &long)]);
        let files = read_zip(&z).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[1].0, PathBuf::from("assets/a.txt"));
        assert_eq!(files[1].1.len(), 5000);
        let p = open_package(&z).unwrap();
        assert_eq!(p.id, "sam.cpu");
        assert_eq!(describe_permissions(&p.manifest), vec!["Read this computer's cpu", "Save its own notes and lists"]);
    }

    #[test]
    fn folder_inside_the_zip_is_unwrapped() {
        let z = pkg(&[("sam.cpu/widget.yaml", YAML), ("sam.cpu/icon.svg", "<svg/>")]);
        let p = open_package(&z).unwrap();
        assert!(p.files.iter().any(|(f, _)| f == Path::new("icon.svg")));
    }

    #[test]
    fn unsafe_packages_are_refused() {
        assert!(read_zip(&pkg(&[("../evil.sh", "x")])).is_err());
        assert!(read_zip(&pkg(&[("/etc/passwd", "x")])).is_err());
        assert!(read_zip(&pkg(&[("a\\..\\b", "x")])).is_err());
        assert!(read_zip(b"not a zip at all").is_err());
        let builtin = YAML.replace("sam.cpu", "forge.clock");
        assert!(open_package(&pkg(&[("widget.yaml", &builtin)])).unwrap_err().contains("built-in"));
        let cmd = format!("{YAML}settings:\n  - {{ key: command, type: command, default: \"rm -rf ~\" }}\n");
        assert!(open_package(&pkg(&[("widget.yaml", &cmd)])).is_err());
        assert!(open_package(&pkg(&[("readme.txt", "hi")])).unwrap_err().contains("widget.yaml"));
    }

    #[test]
    fn links_and_damage_are_refused() {
        let mut z = pkg(&[("widget.yaml", YAML)]);
        // Mark the entry as a symlink in the central directory.
        let cd = z.windows(4).position(|w| w == 0x02014b50u32.to_le_bytes()).unwrap();
        z[cd + 38..cd + 42].copy_from_slice(&((0o120777u32) << 16).to_le_bytes());
        assert!(read_zip(&z).unwrap_err().contains("link"));
        let mut z = pkg(&[("widget.yaml", YAML)]);
        let first_data = 30 + "widget.yaml".len();
        z[first_data] ^= 0xFF;
        assert!(read_zip(&z).is_err());
    }

    #[test]
    fn a_zip_bomb_stops_at_the_limit() {
        let big = vec![0u8; (MAX_FILE + 10) as usize];
        let z = write_zip(&[("widget.yaml".into(), YAML.as_bytes().to_vec()), ("big.bin".into(), big)]).unwrap();
        assert!(z.len() < 200_000, "zeros compress well");
        assert!(read_zip(&z).unwrap_err().contains("20 MB"));
    }

    /// Export a real widget folder; with WF_ZIP_OUT set, also save it so other
    /// zip tools can check it. With WF_ZIP_IN set, read a zip another tool made.
    #[test]
    fn real_widget_exports_and_other_zips_read() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../widgets/forge.clock");
        let files = files_of(&dir).unwrap();
        let z = write_zip(&files).unwrap();
        let back = read_zip(&z).unwrap();
        assert_eq!(back.len(), files.len());
        if let Ok(out) = std::env::var("WF_ZIP_OUT") {
            std::fs::write(out, &z).unwrap();
        }
        if let Ok(inp) = std::env::var("WF_ZIP_IN") {
            let p = open_package(&std::fs::read(inp).unwrap()).unwrap();
            assert_eq!(p.id, "sam.zipcheck");
            assert!(p.files.iter().any(|(f, d)| f == Path::new("assets/big.txt") && d.len() == 100_000));
        }
    }

    #[test]
    fn package_arguments() {
        let args = vec!["widgetforge".into(), "--background".into(), "/home/a/My.ForgeWidget".into()];
        assert_eq!(packages_in_args(&args), vec!["/home/a/My.ForgeWidget".to_string()]);
    }
}
