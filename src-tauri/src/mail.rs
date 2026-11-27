//! Email unread count over IMAP (Gmail and iCloud with an app password,
//! Fastmail, most providers and self-hosted mail). Read-only: the mailbox is
//! opened with EXAMINE and messages are fetched with BODY.PEEK, so nothing is
//! marked as read. The password is a secret that never reaches the widget.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

/// Quotes a string for an IMAP command.
pub fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Reads lines until the tagged reply, following {n} literals.
/// Returns the untagged lines (literals joined in) and the final status line.
fn reply<R: BufRead>(r: &mut R, tag: &str) -> Result<(Vec<String>, String), String> {
    let mut lines = Vec::new();
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            return Err("the mail server closed the connection".into());
        }
        // A literal: "... {123}\r\n" then 123 bytes, then the rest of the line.
        while let Some(n) = literal_len(&line) {
            let mut buf = vec![0u8; n];
            r.read_exact(&mut buf).map_err(|e| e.to_string())?;
            line.push_str(&String::from_utf8_lossy(&buf));
            let mut more = String::new();
            r.read_line(&mut more).map_err(|e| e.to_string())?;
            line.push_str(&more);
        }
        if let Some(rest) = line.strip_prefix(&format!("{tag} ")) {
            return Ok((lines, rest.trim().to_string()));
        }
        lines.push(line);
    }
}

fn literal_len(line: &str) -> Option<usize> {
    let t = line.trim_end_matches(['\r', '\n']);
    let open = t.rfind('{')?;
    t.ends_with('}').then(|| t[open + 1..t.len() - 1].parse().ok()).flatten()
}

/// "=?UTF-8?B?...?=" and "=?utf-8?Q?...?=" in headers, for UTF-8 and ASCII.
pub fn decode_words(s: &str) -> String {
    use base64::Engine;
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("=?") {
        let Some(end) = rest[start + 2..].find("?=").map(|e| e + start + 2) else { break };
        let word = &rest[start + 2..end];
        let parts: Vec<&str> = word.splitn(3, '?').collect();
        if parts.len() != 3 {
            break;
        }
        let between = &rest[..start];
        // Spaces between two encoded words are dropped.
        if !(between.trim().is_empty() && !out.is_empty()) {
            out.push_str(between);
        }
        let bytes = match parts[1].to_ascii_uppercase().as_str() {
            "B" => base64::engine::general_purpose::STANDARD.decode(parts[2]).unwrap_or_default(),
            _ => {
                let mut b = Vec::new();
                let q = parts[2].as_bytes();
                let mut i = 0;
                while i < q.len() {
                    match q[i] {
                        b'_' => b.push(b' '),
                        b'=' if i + 2 < q.len() => {
                            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&q[i + 1..i + 3]).unwrap_or(""), 16) {
                                b.push(v);
                            }
                            i += 2;
                        }
                        c => b.push(c),
                    }
                    i += 1;
                }
                b
            }
        };
        out.push_str(&String::from_utf8_lossy(&bytes));
        rest = &rest[end + 2..];
    }
    out.push_str(rest);
    out
}

/// Picks "Subject:" and "From:" out of fetched header text.
pub fn parse_headers(text: &str) -> (String, String) {
    let mut subject = String::new();
    let mut from = String::new();
    let mut current: Option<&mut String> = None;
    for line in text.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(c) = current.as_mut() {
                c.push(' ');
                c.push_str(line.trim());
            }
            continue;
        }
        let low = line.to_ascii_lowercase();
        if low.starts_with("subject:") {
            subject = line[8..].trim().to_string();
            current = Some(&mut subject);
        } else if low.starts_with("from:") {
            from = line[5..].trim().to_string();
            current = Some(&mut from);
        } else {
            current = None;
        }
    }
    let from = decode_words(&from);
    // "Name <a@b.c>" → "Name"
    let from = match from.find('<') {
        Some(i) if i > 0 => from[..i].trim().trim_matches('"').to_string(),
        _ => from.trim_matches(['<', '>']).to_string(),
    };
    (decode_words(&subject), from)
}

pub struct Account {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub folder: String,
    pub preview: usize,
}

pub fn check(a: &Account) -> Result<Value, String> {
    let addr = (a.host.as_str(), a.port)
        .to_socket_addrs()
        .map_err(|e| format!("cannot find {}: {e}", a.host))?
        .next()
        .ok_or("no address")?;
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(8)).map_err(|e| format!("cannot reach {}: {e}", a.host))?;
    tcp.set_read_timeout(Some(Duration::from_secs(15))).ok();
    use rustls_platform_verifier::ConfigVerifierExt;
    let config = rustls::ClientConfig::with_platform_verifier().map_err(|e| e.to_string())?;
    let name = rustls::pki_types::ServerName::try_from(a.host.clone()).map_err(|e| e.to_string())?;
    let conn = rustls::ClientConnection::new(Arc::new(config), name).map_err(|e| e.to_string())?;
    let tls = rustls::StreamOwned::new(conn, tcp);
    let mut r = BufReader::new(tls);

    let mut greeting = String::new();
    r.read_line(&mut greeting).map_err(|e| format!("secure connection failed: {e}"))?;
    let mut n = 0;
    let mut cmd = |r: &mut BufReader<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>, text: &str| -> Result<Vec<String>, String> {
        n += 1;
        let tag = format!("a{n}");
        r.get_mut().write_all(format!("{tag} {text}\r\n").as_bytes()).map_err(|e| e.to_string())?;
        let (lines, status) = reply(r, &tag)?;
        if !status.starts_with("OK") {
            return Err(status);
        }
        Ok(lines)
    };

    cmd(&mut r, &format!("LOGIN {} {}", quote(&a.user), quote(&a.password)))
        .map_err(|e| format!("sign-in refused ({e}). Gmail and iCloud need an app password."))?;
    let folder = quote(&a.folder);
    let status = cmd(&mut r, &format!("STATUS {folder} (UNSEEN MESSAGES)"))?;
    let num = |word: &str| -> Option<u64> {
        status.iter().find_map(|l| {
            let up = l.to_ascii_uppercase();
            let i = up.find(&format!("{word} "))?;
            up[i + word.len() + 1..].split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
        })
    };
    let unseen = num("UNSEEN").unwrap_or(0);
    let total = num("MESSAGES").unwrap_or(0);

    let mut latest = Vec::new();
    if a.preview > 0 && unseen > 0 {
        cmd(&mut r, &format!("EXAMINE {folder}"))?;
        let found = cmd(&mut r, "SEARCH UNSEEN")?;
        let mut ids: Vec<u64> = found
            .iter()
            .filter(|l| l.starts_with("* SEARCH"))
            .flat_map(|l| l.split_whitespace().skip(2).filter_map(|x| x.parse().ok()).collect::<Vec<u64>>())
            .collect();
        ids.sort_unstable();
        let pick: Vec<String> = ids.iter().rev().take(a.preview).map(|i| i.to_string()).collect();
        if !pick.is_empty() {
            let fetched = cmd(&mut r, &format!("FETCH {} (BODY.PEEK[HEADER.FIELDS (FROM SUBJECT)])", pick.join(",")))?;
            for item in fetched.iter().filter(|l| l.starts_with("* ")) {
                let id: u64 = item.split_whitespace().nth(1).and_then(|x| x.parse().ok()).unwrap_or(0);
                let (subject, from) = parse_headers(item);
                latest.push(json!({ "id": id, "subject": subject, "from": from }));
            }
            latest.sort_by(|a, b| b["id"].as_u64().cmp(&a["id"].as_u64()));
        }
    }
    let _ = cmd(&mut r, "LOGOUT");
    Ok(json!({ "configured": true, "ok": true, "unread": unseen, "total": total, "latest": latest, "account": a.user }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_and_tagged_reply() {
        let raw = b"* 3 FETCH (BODY[HEADER.FIELDS (FROM SUBJECT)] {34}\r\nSubject: Hi\r\nFrom: Ann <a@b.c>\r\n\r\n)\r\na4 OK done\r\n";
        let mut r = std::io::BufReader::new(&raw[..]);
        let (lines, status) = reply(&mut r, "a4").unwrap();
        assert_eq!(status, "OK done");
        assert_eq!(lines.len(), 1);
        assert_eq!(parse_headers(&lines[0]), ("Hi".to_string(), "Ann".to_string()));
    }

    #[test]
    fn encoded_headers() {
        assert_eq!(decode_words("=?UTF-8?B?SGVsbG8gd29ybGQ=?="), "Hello world");
        assert_eq!(decode_words("=?utf-8?Q?Caf=C3=A9_time?="), "Café time");
        assert_eq!(decode_words("Re: =?UTF-8?B?SGk=?= =?UTF-8?B?IHRoZXJl?="), "Re: Hi there");
        assert_eq!(quote(r#"a"b\c"#), r#""a\"b\\c""#);
    }
}
