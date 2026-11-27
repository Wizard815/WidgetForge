//! "Sign in with Discord" (OAuth2 authorization code + PKCE, no client secret).
//!
//! The app opens Discord's consent page in the browser, listens once on
//! http://127.0.0.1:47821/discord/callback for the answer, and exchanges the code
//! for tokens. Tokens live in the system keychain and never reach widget code.
//! Widgets ask the core for a few fixed read-only endpoints.
//!
//! Setup (once, by the app's owner): create an application in the Discord
//! Developer Portal, add the redirect URL above under OAuth2 → Redirects, turn on
//! "Public Client", and put its Client ID in Hub → Settings (later: build it in).

use crate::AppState;
use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager, State, WebviewWindow};
use tauri_plugin_opener::OpenerExt;

pub const REDIRECT_PORT: u16 = 47821;
const SCOPES: &str = "identify guilds";
const API: &str = "https://discord.com/api/v10";
const TOKEN_URL: &str = "https://discord.com/api/oauth2/token";
const KEYCHAIN_OWNER: &str = "app";
const KEYCHAIN_KEY: &str = "discord";

fn redirect_uri() -> String {
    format!("http://127.0.0.1:{REDIRECT_PORT}/discord/callback")
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn random_url_safe(bytes: usize) -> Result<String, String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf))
}

fn form(pairs: &[(&str, &str)]) -> String {
    let mut s = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs {
        s.append_pair(k, v);
    }
    s.finish()
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("WidgetForge/0.1")
        .build()
        .map_err(|e| e.to_string())
}

/// The Client ID from Hub → Settings.
pub fn client_id(state: &AppState) -> Option<String> {
    let settings: serde_json::Map<String, Value> = crate::model::read_json(&state.settings_file());
    settings.get("discord_client_id")?.as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn load_tokens(state: &AppState) -> Option<Value> {
    serde_json::from_str(&state.secrets.get(KEYCHAIN_OWNER, KEYCHAIN_KEY)?).ok()
}

fn save_tokens(state: &AppState, resp: &Value) -> Result<(), String> {
    let tokens = json!({
        "access_token": resp["access_token"],
        "refresh_token": resp["refresh_token"],
        "expires_at": now() + resp["expires_in"].as_u64().unwrap_or(0),
        "scope": resp["scope"],
    });
    state.secrets.set(KEYCHAIN_OWNER, KEYCHAIN_KEY, &tokens.to_string())
}

async fn post_token(body: String) -> Result<Value, String> {
    let resp = client()?
        .post(TOKEN_URL)
        .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if !status.is_success() {
        let why = v["error_description"].as_str().or(v["error"].as_str()).unwrap_or("unknown error");
        return Err(format!("Discord refused the sign-in ({status}): {why}"));
    }
    Ok(v)
}

/// A valid access token, refreshed if it is about to expire.
async fn access_token(state: &AppState) -> Result<String, String> {
    let tokens = load_tokens(state).ok_or("not signed in to Discord")?;
    if tokens["expires_at"].as_u64().unwrap_or(0) > now() + 60 {
        return tokens["access_token"].as_str().map(String::from).ok_or("bad saved token".into());
    }
    let id = client_id(state).ok_or("no Discord Client ID set")?;
    let refresh = tokens["refresh_token"].as_str().ok_or("sign in to Discord again")?;
    let resp = post_token(form(&[("client_id", &id), ("grant_type", "refresh_token"), ("refresh_token", refresh)]))
        .await
        .map_err(|e| format!("{e}. Sign in to Discord again."))?;
    save_tokens(state, &resp)?;
    resp["access_token"].as_str().map(String::from).ok_or("Discord sent no access token".into())
}

async fn get(state: &AppState, path: &str) -> Result<Value, String> {
    let token = access_token(state).await?;
    let resp = client()?
        .get(format!("{API}{path}"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(format!("Discord: {}", v["message"].as_str().unwrap_or(status.as_str())));
    }
    Ok(v)
}

/// Waits for Discord to send the browser back with `?code=…&state=…`.
fn wait_for_code(listener: TcpListener, expected_state: &str) -> Result<String, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(300);
    while Instant::now() < deadline {
        let Ok((mut stream, _)) = listener.accept() else {
            std::thread::sleep(Duration::from_millis(200));
            continue;
        };
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]);
        let target = request.split_whitespace().nth(1).unwrap_or("");
        let reply = |stream: &mut std::net::TcpStream, code: &str, text: &str| {
            // PLACEHOLDER wording.
            let body = format!(
                "<!doctype html><meta charset=utf-8><title>WidgetForge</title>\
                 <body style=\"font:16px system-ui;padding:40px;background:#0f131c;color:#e8ecf3\">{text}</body>"
            );
            let _ = write!(
                stream,
                "HTTP/1.1 {code}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        };
        if !target.starts_with("/discord/callback") {
            reply(&mut stream, "404 Not Found", "Not found.");
            continue;
        }
        let Ok(u) = url::Url::parse(&format!("http://127.0.0.1{target}")) else { continue };
        let q: std::collections::HashMap<String, String> = u.query_pairs().into_owned().collect();
        if q.get("state").map(String::as_str) != Some(expected_state) {
            reply(&mut stream, "400 Bad Request", "This sign-in link is not the one WidgetForge started. Try again from the app.");
            return Err("sign-in answer did not match (state)".into());
        }
        if let Some(err) = q.get("error") {
            reply(&mut stream, "200 OK", "Sign-in was cancelled. You can close this tab.");
            return Err(format!("Discord sign-in cancelled ({err})"));
        }
        let code = q.get("code").cloned().ok_or("Discord sent no code")?;
        reply(&mut stream, "200 OK", "Signed in to Discord. You can close this tab and go back to WidgetForge.");
        return Ok(code);
    }
    Err("sign-in timed out after 5 minutes".into())
}

/// Opens Discord's consent page and finishes the sign-in. Hub only.
#[tauri::command]
pub async fn discord_login(window: WebviewWindow, state: State<'_, AppState>) -> Result<Value, String> {
    if window.label() != "hub" {
        return Err("only the Hub may sign in".into());
    }
    let id = client_id(&state).ok_or("Set the Discord Client ID first (see the steps below).")?;
    let listener = TcpListener::bind(("127.0.0.1", REDIRECT_PORT))
        .map_err(|e| format!("port {REDIRECT_PORT} is busy, close the other sign-in first: {e}"))?;
    let verifier = random_url_safe(48)?;
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let csrf = random_url_safe(16)?;
    let mut auth = url::Url::parse("https://discord.com/oauth2/authorize").unwrap();
    auth.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &id)
        .append_pair("scope", SCOPES)
        .append_pair("redirect_uri", &redirect_uri())
        .append_pair("state", &csrf)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("prompt", "consent");
    window.app_handle().opener().open_url(auth.as_str(), None::<&str>).map_err(|e| e.to_string())?;

    let expected = csrf.clone();
    let code = tauri::async_runtime::spawn_blocking(move || wait_for_code(listener, &expected))
        .await
        .map_err(|e| e.to_string())??;
    let resp = post_token(form(&[
        ("client_id", &id),
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", &redirect_uri()),
        ("code_verifier", &verifier),
    ]))
    .await?;
    save_tokens(&state, &resp)?;
    let _ = window.app_handle().emit("discord-changed", ());
    discord_status(window, state).await
}

/// Who is signed in, if anyone. Hub only.
#[tauri::command]
pub async fn discord_status(window: WebviewWindow, state: State<'_, AppState>) -> Result<Value, String> {
    if window.label() != "hub" {
        return Err("only the Hub may ask this".into());
    }
    let configured = client_id(&state).is_some();
    if load_tokens(&state).is_none() {
        return Ok(json!({ "connected": false, "configured": configured, "redirect": redirect_uri() }));
    }
    match get(&state, "/users/@me").await {
        Ok(me) => Ok(json!({
            "connected": true, "configured": configured, "redirect": redirect_uri(),
            "user": { "id": me["id"], "username": me["username"], "global_name": me["global_name"] },
        })),
        Err(e) => Ok(json!({ "connected": false, "configured": configured, "redirect": redirect_uri(), "error": e })),
    }
}

/// Signs out: revokes the tokens at Discord and deletes them here. Hub only.
#[tauri::command]
pub async fn discord_logout(window: WebviewWindow, state: State<'_, AppState>) -> Result<(), String> {
    if window.label() != "hub" {
        return Err("only the Hub may sign out".into());
    }
    if let (Some(tokens), Some(id)) = (load_tokens(&state), client_id(&state)) {
        if let Some(rt) = tokens["refresh_token"].as_str() {
            let body = form(&[("client_id", &id), ("token", rt), ("token_type_hint", "refresh_token")]);
            let _ = client()?
                .post(format!("{TOKEN_URL}/revoke"))
                .header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(body)
                .send()
                .await;
        }
    }
    state.secrets.delete(KEYCHAIN_OWNER, KEYCHAIN_KEY);
    let _ = window.app_handle().emit("discord-changed", ());
    Ok(())
}

/// Sets the Discord application Client ID. Hub only.
#[tauri::command]
pub fn set_discord_client_id(window: WebviewWindow, state: State<'_, AppState>, client_id: String) -> Result<(), String> {
    if window.label() != "hub" {
        return Err("only the Hub may change this".into());
    }
    let id = client_id.trim();
    if !id.is_empty() && !id.chars().all(|c| c.is_ascii_digit()) {
        return Err("a Discord Client ID is a long number".into());
    }
    let file = state.settings_file();
    let mut settings: serde_json::Map<String, Value> = crate::model::read_json(&file);
    settings.insert("discord_client_id".into(), Value::String(id.to_string()));
    crate::model::write_json(&file, &settings)
}

/// The `discord` source for widgets with `permissions.discord: true`.
/// Only these read-only calls exist; widget code never sees the token.
#[tauri::command]
pub async fn discord_api(
    window: WebviewWindow,
    state: State<'_, AppState>,
    instance: Option<String>,
    endpoint: String,
) -> Result<Value, String> {
    let (_, widget) = crate::widget_of(&window, &state, instance)?;
    if !widget.permission("discord") {
        return Err("this widget has no `discord` permission".into());
    }
    if load_tokens(&state).is_none() {
        return Ok(json!({ "signed_in": false }));
    }
    let data = match endpoint.as_str() {
        "me" => get(&state, "/users/@me").await?,
        "guilds" => get(&state, "/users/@me/guilds?with_counts=true").await?,
        other => return Err(format!("unknown Discord endpoint `{other}`")),
    };
    Ok(json!({ "signed_in": true, "data": data }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpStream;

    fn send(port: u16, path: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
        let mut out = String::new();
        let _ = s.read_to_string(&mut out);
        out
    }

    fn listen() -> (TcpListener, u16) {
        let l = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = l.local_addr().unwrap().port();
        (l, port)
    }

    #[test]
    fn callback_returns_code_and_ignores_other_paths() {
        let (l, port) = listen();
        let t = std::thread::spawn(move || wait_for_code(l, "xyz"));
        std::thread::sleep(Duration::from_millis(300));
        assert!(send(port, "/favicon.ico").starts_with("HTTP/1.1 404"));
        assert!(send(port, "/discord/callback?code=abc123&state=xyz").contains("Signed in to Discord"));
        assert_eq!(t.join().unwrap().unwrap(), "abc123");
    }

    #[test]
    fn callback_rejects_wrong_state_and_cancel() {
        let (l, port) = listen();
        let t = std::thread::spawn(move || wait_for_code(l, "xyz"));
        std::thread::sleep(Duration::from_millis(300));
        send(port, "/discord/callback?code=abc&state=EVIL");
        assert!(t.join().unwrap().is_err());

        let (l, port) = listen();
        let t = std::thread::spawn(move || wait_for_code(l, "xyz"));
        std::thread::sleep(Duration::from_millis(300));
        send(port, "/discord/callback?error=access_denied&state=xyz");
        assert!(t.join().unwrap().unwrap_err().contains("cancelled"));
    }

    #[test]
    fn pkce_challenge_is_base64url_sha256() {
        // Expected value computed separately with Python:
        // base64.urlsafe_b64encode(hashlib.sha256(verifier).digest()).rstrip(b"=")
        let verifier = "dBjftJeZ4CVP-mJ92K9fhDGIXbu85UmKYEzBYsgKGlw";
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge, "Plmv-4UAZWnYeg0_xLXu2syChHK7bbiybOw-kkEXO_M");
        assert_eq!(random_url_safe(48).unwrap().len(), 64);
    }
}
