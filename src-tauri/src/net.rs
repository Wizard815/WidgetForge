//! The `net` source: HTTP requests to the hosts a widget lists in
//! `permissions.network`. Responses are capped at 2 MB.

use serde_json::{json, Value};
use std::time::Duration;

pub const MAX_RESPONSE: usize = 2 * 1024 * 1024;

/// `host` or `host:port`, compared with the widget's list.
fn host_key(u: &url::Url) -> Option<(String, String)> {
    let host = u.host_str()?.to_ascii_lowercase();
    let with_port = match u.port() {
        Some(p) => format!("{host}:{p}"),
        None => host.clone(),
    };
    Some((host, with_port))
}

pub fn allowed(u: &url::Url, hosts: &[String]) -> bool {
    if !matches!(u.scheme(), "https" | "http") {
        return false;
    }
    let Some((host, with_port)) = host_key(u) else { return false };
    hosts.iter().any(|h| {
        let h = h.to_ascii_lowercase();
        h == with_port || (u.port().is_none() && h == host)
    })
}

pub struct Request {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

pub async fn fetch(req: Request, hosts: Vec<String>) -> Result<Value, String> {
    let u = url::Url::parse(&req.url).map_err(|e| format!("bad URL: {e}"))?;
    if !allowed(&u, &hosts) {
        return Err(format!(
            "`{}` is not in this widget's network permission",
            u.host_str().unwrap_or("?")
        ));
    }
    // Redirects may only go to hosts the widget is allowed to reach.
    let redirect_hosts = hosts.clone();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("WidgetForge/0.1")
        .redirect(reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() > 5 || !allowed(attempt.url(), &redirect_hosts) {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|e| e.to_string())?;

    let method = match req.method.to_ascii_uppercase().as_str() {
        "GET" | "" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "DELETE" => reqwest::Method::DELETE,
        other => return Err(format!("method `{other}` is not allowed")),
    };
    let mut builder = client.request(method, u);
    for (k, v) in req.headers {
        builder = builder.header(k, v);
    }
    if let Some(body) = req.body {
        builder = builder.body(body);
    }

    let mut resp = builder.send().await.map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    // Session cookies ("SID=..."), for services that sign in with a form (qBittorrent).
    let cookies: Vec<String> = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok()?.split(';').next().map(|c| c.trim().to_string()))
        .collect();
    let mut bytes = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > MAX_RESPONSE {
            return Err("response is larger than 2 MB".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let parsed: Option<Value> = serde_json::from_str(&text).ok();
    Ok(json!({
        "status": status,
        "ok": (200..300).contains(&status),
        "content_type": content_type,
        "cookie": cookies.join("; "),
        "text": if parsed.is_some() { Value::Null } else { Value::String(text) },
        "json": parsed,
    }))
}

#[cfg(test)]
mod tests {
    use super::allowed;

    fn ok(u: &str, hosts: &[&str]) -> bool {
        let hosts: Vec<String> = hosts.iter().map(|s| s.to_string()).collect();
        allowed(&url::Url::parse(u).unwrap(), &hosts)
    }

    #[test]
    fn host_rules() {
        assert!(ok("https://api.open-meteo.com/v1/forecast", &["api.open-meteo.com"]));
        assert!(!ok("https://evil.com/?api.open-meteo.com", &["api.open-meteo.com"]));
        assert!(!ok("https://sub.api.open-meteo.com/", &["api.open-meteo.com"]));
        assert!(!ok("file:///etc/passwd", &["api.open-meteo.com"]));
        assert!(ok("http://192.168.1.5:8123/api", &["192.168.1.5:8123"]));
        assert!(!ok("http://192.168.1.5:9000/api", &["192.168.1.5:8123"]));
        assert!(ok("http://homeassistant.local/api", &["homeassistant.local"]));
        assert!(!ok("https://api.open-meteo.com@evil.com/", &["api.open-meteo.com"]));
    }
}
