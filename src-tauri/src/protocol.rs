//! The `widget://` scheme: serves files from widget folders, and the SDK.
//! URL shape: widget://localhost/<widget id>/<file path>

use crate::AppState;
use std::borrow::Cow;
use tauri::http::{Request, Response};
use tauri::{AppHandle, Manager, Runtime};

const SDK: &str = include_str!("../../src/sdk/forge-sdk.js");
const SDK_TAG: &str = "<script src=\"/__forge/sdk.js\"></script>";

fn reply(status: u16, mime: &str, body: Vec<u8>) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("Access-Control-Allow-Origin", "*")
        .body(Cow::Owned(body))
        .unwrap()
}

fn not_found() -> Response<Cow<'static, [u8]>> {
    reply(404, "text/plain", b"not found".to_vec())
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn mime_for(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html",
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "yaml" | "yml" => "text/yaml",
        _ => "application/octet-stream",
    }
}

fn serve_user_folder<R: Runtime>(app: &AppHandle<R>, rest: &str) -> Response<Cow<'static, [u8]>> {
    let mut parts = rest.splitn(3, '/');
    let (Some(inst_id), Some(key), Some(file)) = (parts.next(), parts.next(), parts.next()) else { return not_found() };
    if file.split(['/', '\\']).any(|p| p == ".." || p.is_empty()) {
        return not_found();
    }
    let state = app.state::<AppState>();
    let Some(inst) = state.instance(inst_id) else { return not_found() };
    let Ok(widget) = state.widget(&inst.widget_id) else { return not_found() };
    let Ok(root) = crate::local::allowed_folder(&widget, &inst, key) else { return not_found() };
    let Ok(full) = root.join(file).canonicalize() else { return not_found() };
    if !full.starts_with(&root) {
        return not_found();
    }
    match std::fs::read(&full) {
        Ok(body) => Response::builder()
            .status(200)
            .header("Content-Type", mime_for(file))
            .header("Cache-Control", "max-age=300")
            .body(Cow::Owned(body))
            .unwrap(),
        Err(_) => not_found(),
    }
}

pub fn handle<R: Runtime>(app: &AppHandle<R>, request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let path = percent_decode(request.uri().path());
    let path = path.trim_start_matches('/');

    if path == "__forge/sdk.js" {
        return reply(200, "text/javascript", SDK.as_bytes().to_vec());
    }

    // Pictures from a folder the user chose: __folder/<instance>/<setting>/<file>.
    // Served without a CORS header, so other pages can show but not read them.
    if let Some(rest) = path.strip_prefix("__folder/") {
        return serve_user_folder(app, rest);
    }

    let Some((widget_id, file)) = path.split_once('/') else { return not_found() };
    // Path safety: no parent or absolute parts.
    if file.is_empty() || file.split(['/', '\\']).any(|p| p == ".." || p.contains(':')) {
        return not_found();
    }

    let state = app.state::<AppState>();
    let (dir, entry) = {
        let widgets = state.widgets.lock().unwrap();
        let Some(w) = widgets.get(widget_id) else { return not_found() };
        let entry = w.manifest.pointer("/ui/entry").and_then(|v| v.as_str()).map(String::from);
        (w.dir.clone(), entry)
    };

    let full = dir.join(file);
    let (Ok(full), Ok(root)) = (full.canonicalize(), dir.canonicalize()) else { return not_found() };
    if !full.starts_with(&root) {
        return not_found();
    }
    let Ok(mut body) = std::fs::read(&full) else { return not_found() };

    // Put the SDK in front of the widget's own scripts.
    if entry.as_deref() == Some(file) {
        let html = String::from_utf8_lossy(&body).into_owned();
        let html = match html.find("<head>") {
            Some(i) => format!("{}{SDK_TAG}{}", &html[..i + 6], &html[i + 6..]),
            None => format!("{SDK_TAG}{html}"),
        };
        body = html.into_bytes();
    }
    reply(200, mime_for(file), body)
}
