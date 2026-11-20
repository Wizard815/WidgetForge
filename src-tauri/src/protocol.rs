//! The `widget://` scheme serves files from widget folders, e.g.
//! `widget://localhost/forge.clock/icon.svg` (or `http://widget.localhost/...` on Windows).
//! HTML pages get the WidgetForge SDK injected and a CSP that blocks direct network
//! access: widgets must go through `forge.fetch`, which enforces their allowed hosts.

use std::borrow::Cow;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext};

use crate::manifest::is_safe_relative;
use crate::state::AppState;

const SDK: &str = include_str!("../../src/sdk/forge-sdk.js");
const CSP: &str = "default-src widget: http://widget.localhost 'unsafe-inline' data: blob:; \
                   connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'";

fn percent_decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s).decode_utf8_lossy().into_owned()
}

fn respond(status: StatusCode, mime: &str, body: Vec<u8>) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_SECURITY_POLICY, CSP)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Cow::Owned(body))
        .unwrap()
}

/// Inserts the SDK as the first script of the document.
pub fn inject_sdk(html: &str) -> String {
    let tag = format!("<script>{SDK}</script>");
    let lower = html.to_ascii_lowercase();
    if let Some(i) = lower.find("<head>") {
        let at = i + "<head>".len();
        format!("{}{tag}{}", &html[..at], &html[at..])
    } else {
        format!("{tag}{html}")
    }
}

pub fn handle<R: Runtime>(ctx: UriSchemeContext<'_, R>, request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let path = percent_decode(request.uri().path());
    let path = path.trim_start_matches('/');
    let (widget_id, rel) = path.split_once('/').unwrap_or((path, ""));
    if !is_safe_relative(rel) {
        return respond(StatusCode::BAD_REQUEST, "text/plain", b"bad path".to_vec());
    }
    let state = ctx.app_handle().state::<AppState>();
    let Ok(dir) = state.widget_dir(widget_id) else {
        return respond(StatusCode::NOT_FOUND, "text/plain", b"unknown widget".to_vec());
    };
    let file = dir.join(rel);
    let Ok(bytes) = std::fs::read(&file) else {
        return respond(StatusCode::NOT_FOUND, "text/plain", b"not found".to_vec());
    };
    let mime = mime_guess::from_path(&file).first_or_octet_stream().essence_str().to_string();
    if mime == "text/html" {
        let html = inject_sdk(&String::from_utf8_lossy(&bytes));
        return respond(StatusCode::OK, "text/html; charset=utf-8", html.into_bytes());
    }
    respond(StatusCode::OK, &mime, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_after_head() {
        let out = inject_sdk("<html><HEAD><title>x</title></HEAD></html>");
        assert!(out.starts_with("<html><HEAD><script>"));
        let out = inject_sdk("<p>hi</p>");
        assert!(out.starts_with("<script>") && out.ends_with("<p>hi</p>"));
    }

    #[test]
    fn decodes_paths() {
        assert_eq!(percent_decode("/forge.x/my%20file.png"), "/forge.x/my file.png");
        assert_eq!(percent_decode("/a+b"), "/a+b");
    }
}
