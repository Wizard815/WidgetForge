//! The widget catalog (Hub → Browse): an `index.json` listing widgets and
//! where to download them, built by the registry repository's checks (see
//! registry-template/). Downloads are checked against the size and SHA-256 in
//! the index, then go through the normal package import screen.
//!
//! Only https addresses are used, except http on this computer (localhost),
//! for testing a catalog or running your own.

use crate::AppState;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tauri::{State, WebviewWindow};

/// The catalog the app uses until the user picks another.
/// TODO(owner): set once the public widgets repository exists, e.g.
/// "https://<owner>.github.io/widgets/index.json". Empty = none yet.
pub const DEFAULT_INDEX: &str = "https://wizard815.github.io/widgetforge-widgets/index.json";

const MAX_INDEX: usize = 5 * 1024 * 1024;
const MAX_PACKAGE: u64 = 100 * 1024 * 1024;

/// An address the catalog may use: https anywhere, http only on this computer.
pub fn allowed_url(u: &url::Url) -> bool {
    match u.scheme() {
        "https" => u.host_str().is_some(),
        "http" => matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
        _ => false,
    }
}

pub fn index_url(state: &AppState) -> String {
    crate::shell::read_settings(state)
        .get("registry_url")
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| DEFAULT_INDEX.to_string())
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent("WidgetForge/0.1")
        .redirect(reqwest::redirect::Policy::custom(|a| {
            if a.previous().len() > 5 || !allowed_url(a.url()) { a.stop() } else { a.follow() }
        }))
        .build()
        .map_err(|e| e.to_string())
}

/// Reads an index: keeps well-formed entries, makes their addresses absolute,
/// and drops entries whose addresses are not allowed.
pub fn parse_index(base: &url::Url, text: &str) -> Result<Value, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("the catalog is not valid: {e}"))?;
    if v["format"] != "widgetforge-index" {
        return Err("this address is not a WidgetForge catalog".into());
    }
    if v["version"].as_u64() != Some(1) {
        return Err("this catalog is for a newer WidgetForge; update the app".into());
    }
    let mut out = Vec::new();
    for w in v["widgets"].as_array().into_iter().flatten() {
        let (Some(id), Some(name), Some(version), Some(download), Some(sha)) = (
            w["id"].as_str(), w["name"].as_str(), w["version"].as_str(), w["download"].as_str(), w["sha256"].as_str(),
        ) else {
            continue;
        };
        if id.starts_with("forge.") || sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let Ok(dl) = base.join(download) else { continue };
        if !allowed_url(&dl) {
            continue;
        }
        let icon = w["icon"].as_str().and_then(|i| base.join(i).ok()).filter(allowed_url).map(|u| u.to_string());
        let homepage = w["homepage"].as_str().filter(|h| h.starts_with("https://")).map(String::from);
        out.push(json!({
            "id": id, "name": name, "version": version,
            "author": w["author"].as_str().unwrap_or(""),
            "description": w["description"].as_str().unwrap_or(""),
            "category": w["category"].as_str().unwrap_or("other"),
            "permissions": w["permissions"].clone(),
            "html": w["html"].as_bool().unwrap_or(false),
            "download": dl.to_string(), "sha256": sha.to_ascii_lowercase(),
            "size": w["size"].as_u64(), "icon": icon, "homepage": homepage,
        }));
    }
    Ok(json!({ "generated": v["generated"], "widgets": out }))
}

/// The catalog address and the widgets in it.
#[tauri::command]
pub async fn registry_index(window: WebviewWindow, state: State<'_, AppState>) -> Result<Value, String> {
    crate::shell::hub_only(&window)?;
    let address = index_url(&state);
    if address.trim().is_empty() {
        return Ok(json!({ "url": "", "configured": false, "widgets": [] }));
    }
    let base = url::Url::parse(address.trim()).map_err(|e| format!("bad catalog address: {e}"))?;
    if !allowed_url(&base) {
        return Err("the catalog address must start with https:// (http only for localhost)".into());
    }
    let mut resp = client()?.get(base.clone()).send().await.map_err(|e| format!("the catalog could not be reached: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("the catalog answered {}", resp.status()));
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        body.extend_from_slice(&chunk);
        if body.len() > MAX_INDEX {
            return Err("the catalog is larger than 5 MB".into());
        }
    }
    let mut out = parse_index(&base, &String::from_utf8_lossy(&body))?;
    out["url"] = json!(address);
    out["configured"] = json!(true);
    Ok(out)
}

#[tauri::command]
pub fn set_registry_url(window: WebviewWindow, state: State<AppState>, url: String) -> Result<String, String> {
    crate::shell::hub_only(&window)?;
    let url = url.trim().to_string();
    if !url.is_empty() {
        let u = url::Url::parse(&url).map_err(|e| format!("bad address: {e}"))?;
        if !allowed_url(&u) {
            return Err("use an https:// address (http only for localhost)".into());
        }
    }
    let mut s = crate::shell::read_settings(&state);
    if url.is_empty() || url == DEFAULT_INDEX {
        s.remove("registry_url");
    } else {
        s.insert("registry_url".into(), json!(url));
    }
    crate::model::write_json(&state.settings_file(), &s)?;
    Ok(index_url(&state))
}

/// Checks downloaded bytes against the index.
pub fn verify(data: &[u8], sha256: &str, size: Option<u64>) -> Result<(), String> {
    if let Some(n) = size {
        if data.len() as u64 != n {
            return Err(format!("the download is {} bytes, the catalog says {n}", data.len()));
        }
    }
    let got = format!("{:x}", Sha256::digest(data));
    if !got.eq_ignore_ascii_case(sha256) {
        return Err("the download does not match the catalog (checksum); it was not installed".into());
    }
    Ok(())
}

pub async fn download(url: &str, sha256: &str, size: Option<u64>) -> Result<Vec<u8>, String> {
    let u = url::Url::parse(url).map_err(|e| e.to_string())?;
    if !allowed_url(&u) {
        return Err("that download address is not allowed".into());
    }
    if size.is_some_and(|n| n > MAX_PACKAGE) {
        return Err("the package is larger than 100 MB".into());
    }
    let mut resp = client()?.get(u).send().await.map_err(|e| format!("download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("download failed: {}", resp.status()));
    }
    let limit = size.unwrap_or(MAX_PACKAGE).min(MAX_PACKAGE);
    let mut data = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        data.extend_from_slice(&chunk);
        if data.len() as u64 > limit {
            return Err("the download is larger than the catalog says".into());
        }
    }
    verify(&data, sha256, size)?;
    Ok(data)
}

/// Downloads a catalog widget and keeps it in the app's download folder;
/// returns its path for the import screen (inspect_package, install_package).
#[tauri::command]
pub async fn registry_download(
    window: WebviewWindow,
    state: State<'_, AppState>,
    url: String,
    sha256: String,
    size: Option<u64>,
) -> Result<String, String> {
    crate::shell::hub_only(&window)?;
    let data = download(&url, &sha256, size).await?;
    let dir = state.storage_dir.parent().unwrap_or(&state.storage_dir).join("downloads");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // Named by checksum: the same file is only kept once.
    let path = dir.join(format!("{}.{}", &sha256[..16.min(sha256.len())], crate::packages::EXT));
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn address_rules() {
        let ok = |s: &str| allowed_url(&url::Url::parse(s).unwrap());
        assert!(ok("https://sam.github.io/widgets/index.json"));
        assert!(ok("http://localhost:8000/index.json"));
        assert!(ok("http://127.0.0.1:8000/index.json"));
        assert!(!ok("http://example.com/index.json"));
        assert!(!ok("file:///etc/passwd"));
        assert!(!ok("ftp://x/y"));
    }

    #[test]
    fn index_entries_are_checked_and_made_absolute() {
        let base = url::Url::parse("https://sam.github.io/widgets/index.json").unwrap();
        let sha = "a".repeat(64);
        let text = json!({
            "format": "widgetforge-index", "version": 1,
            "widgets": [
                { "id": "sam.clock", "name": "Clock", "version": "1.0.0", "download": "packages/sam.clock-1.0.0.forgewidget", "sha256": sha, "icon": "icons/sam.clock.svg" },
                { "id": "forge.clock", "name": "Fake", "version": "9.0.0", "download": "x.forgewidget", "sha256": sha },
                { "id": "sam.http", "name": "Plain http", "version": "1.0.0", "download": "http://evil.example/x.forgewidget", "sha256": sha },
                { "id": "sam.nosha", "name": "No checksum", "version": "1.0.0", "download": "x.forgewidget", "sha256": "abc" },
                { "name": "No id" }
            ]
        })
        .to_string();
        let out = parse_index(&base, &text).unwrap();
        let list = out["widgets"].as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["download"], "https://sam.github.io/widgets/packages/sam.clock-1.0.0.forgewidget");
        assert_eq!(list[0]["icon"], "https://sam.github.io/widgets/icons/sam.clock.svg");
        assert!(parse_index(&base, "{\"format\":\"other\"}").is_err());
        assert!(parse_index(&base, "{\"format\":\"widgetforge-index\",\"version\":2}").unwrap_err().contains("newer"));
    }

    #[test]
    fn downloads_must_match_the_index() {
        let data = b"package bytes";
        let sha = format!("{:x}", Sha256::digest(data));
        assert!(verify(data, &sha, Some(data.len() as u64)).is_ok());
        assert!(verify(data, &sha, Some(5)).is_err());
        assert!(verify(b"something else", &sha, None).unwrap_err().contains("checksum"));
    }

    /// A one-request web server on this computer.
    fn serve(body: Vec<u8>) -> String {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://127.0.0.1:{}/pkg.forgewidget", l.local_addr().unwrap().port());
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut buf = [0u8; 1024];
            let _ = s.read(&mut buf);
            write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
            s.write_all(&body).unwrap();
        });
        addr
    }

    /// With WF_INDEX_URL set (a catalog built by registry-template/tools/build_index.py
    /// and served on this computer): read it, download every widget, open each package.
    #[test]
    fn a_built_catalog_installs() {
        let Ok(address) = std::env::var("WF_INDEX_URL") else { return };
        let base = url::Url::parse(&address).unwrap();
        tauri::async_runtime::block_on(async {
            let text = client().unwrap().get(base.clone()).send().await.unwrap().text().await.unwrap();
            let index = parse_index(&base, &text).unwrap();
            let list = index["widgets"].as_array().unwrap();
            assert!(!list.is_empty());
            for w in list {
                let data = download(w["download"].as_str().unwrap(), w["sha256"].as_str().unwrap(), w["size"].as_u64()).await.unwrap();
                let pkg = crate::packages::open_package(&data).unwrap();
                assert_eq!(pkg.id, w["id"].as_str().unwrap());
                println!("ok {} {} ({} files)", pkg.id, w["version"], pkg.files.len());
            }
        });
    }

    #[test]
    fn real_download_is_verified() {
        let data = b"PK fake package".to_vec();
        let sha = format!("{:x}", Sha256::digest(&data));
        let url = serve(data.clone());
        let got = tauri::async_runtime::block_on(download(&url, &sha, Some(data.len() as u64))).unwrap();
        assert_eq!(got, data);
        let url = serve(b"tampered".to_vec());
        assert!(tauri::async_runtime::block_on(download(&url, &sha, None)).is_err());
    }
}
