// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Google Calendar: sign-in with OAuth 2.0 for installed apps (browser,
//! loopback redirect to `127.0.0.1`, PKCE) and the addresses of Google's
//! APIs. Events are synchronised over Google's CalDAV interface with the
//! same code as Nextcloud ([`super::caldav::Client::google`]); the list of
//! calendars comes from the Calendar API, as Google's CalDAV offers no
//! discovery.
//!
//! The user brings an own OAuth client ("Desktop app") from the Google
//! Cloud Console; Atlas keeps only the refresh token (like the Nextcloud app
//! password, see `secret`).

use std::io::{BufRead, BufReader, Write as _};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

/// Read and write calendars, and the e-mail address of the account.
pub const SCOPE: &str = "openid email https://www.googleapis.com/auth/calendar";

/// Where Google's services are (replaceable for tests).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    pub auth: String,
    pub token: String,
    /// Calendar API v3.
    pub api: String,
    /// CalDAV v2.
    pub caldav: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            auth: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token: "https://oauth2.googleapis.com/token".into(),
            api: "https://www.googleapis.com/calendar/v3".into(),
            caldav: "https://apidata.googleusercontent.com/caldav/v2".into(),
        }
    }
}

/// The user's OAuth client ("Desktop app").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OAuthClient {
    pub id: String,
    /// Not secret for installed apps (Google's own wording), but needed.
    pub secret: String,
}

impl Endpoints {
    /// CalDAV collection of the calendar with `id` (as the Calendar API
    /// names it, e.g. the e-mail address for the main calendar).
    pub fn calendar_url(&self, id: &str) -> String {
        format!("{}/{}/events/", self.caldav, url_encode(id))
    }
}

/// The browser address of the sign-in; `redirect` is the loopback address
/// Atlas listens on, `state` and `verifier` are random per attempt.
pub fn auth_url(endpoints: &Endpoints, client: &OAuthClient, redirect: &str, state: &str, verifier: &str) -> String {
    let params = [
        ("client_id", client.id.as_str()),
        ("redirect_uri", redirect),
        ("response_type", "code"),
        ("scope", SCOPE),
        ("state", state),
        ("code_challenge", verifier),
        ("code_challenge_method", "plain"),
        // A refresh token, also when this client was allowed before.
        ("access_type", "offline"),
        ("prompt", "consent"),
    ];
    format!("{}?{}", endpoints.auth, form(&params))
}

/// A random value for `state` and the PKCE verifier (43+ URL-safe characters).
pub fn random_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

/// Waits (at most `timeout`) for the browser to come back to `listener`
/// with the authorisation code; answers it with a short page.
pub fn wait_for_code(listener: &TcpListener, state: &str, timeout: Duration) -> Result<String> {
    listener.set_nonblocking(true)?;
    let until = Instant::now() + timeout;
    loop {
        let (mut stream, _) = match listener.accept() {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > until {
                    bail!("Keine Antwort aus dem Browser – Anmeldung abgebrochen");
                }
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line)?;
        // "GET /?code=…&state=… HTTP/1.1"; other requests (favicon) wait on.
        let target = line.split_whitespace().nth(1).unwrap_or("");
        let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");
        let params = parse_form(query);
        let get = |k: &str| params.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
        if get("code").is_none() && get("error").is_none() {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            continue;
        }
        let (ok, text) = match (get("code"), get("error")) {
            (Some(_), _) if get("state").as_deref() != Some(state) => (false, "Ungültige Antwort (state passt nicht)."),
            (Some(_), _) => (true, "Atlas ist angemeldet. Du kannst dieses Fenster schließen."),
            (None, _) => (false, "Anmeldung abgelehnt. Du kannst dieses Fenster schließen."),
        };
        let page = format!(
            "<!doctype html><meta charset=utf-8><title>Atlas</title>\
             <body style=\"font-family:sans-serif;background:#0a101c;color:#e6edf7;display:grid;place-items:center;height:90vh\">\
             <p style=\"font-size:18px\">{text}</p></body>"
        );
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
            page.len()
        );
        return match (ok, get("code"), get("error")) {
            (true, Some(code), _) => Ok(code),
            (_, _, Some(error)) => bail!("Google: {error}"),
            _ => bail!("Ungültige Antwort aus dem Browser"),
        };
    }
}

/// Tokens from Google's token endpoint.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tokens {
    pub access: String,
    pub refresh: String,
    pub expires_in: u64,
    /// E-mail address of the account (from the ID token).
    pub email: String,
}

/// `token_request` posts a form to the token endpoint and returns status
/// and body (the caller's HTTP client).
pub fn exchange_code(
    token_request: impl FnOnce(&str) -> Result<(u16, String)>,
    client: &OAuthClient,
    code: &str,
    redirect: &str,
    verifier: &str,
) -> Result<Tokens> {
    let body = form(&[
        ("code", code),
        ("client_id", &client.id),
        ("client_secret", &client.secret),
        ("redirect_uri", redirect),
        ("grant_type", "authorization_code"),
        ("code_verifier", verifier),
    ]);
    let (status, text) = token_request(&body)?;
    let tokens = parse_tokens(status, &text)?;
    if tokens.refresh.is_empty() {
        bail!("Google hat kein Refresh-Token geschickt");
    }
    Ok(tokens)
}

/// The form for a fresh access token from the refresh token.
pub fn refresh_form(client: &OAuthClient, refresh: &str) -> String {
    form(&[
        ("client_id", &client.id),
        ("client_secret", &client.secret),
        ("refresh_token", refresh),
        ("grant_type", "refresh_token"),
    ])
}

/// Reads a token response; errors carry Google's reason.
pub fn parse_tokens(status: u16, text: &str) -> Result<Tokens> {
    let v: serde_json::Value = serde_json::from_str(text).with_context(|| format!("Antwort von Google ({status})"))?;
    if status != 200 {
        let reason = v["error_description"].as_str().or(v["error"].as_str()).unwrap_or("unbekannter Fehler");
        bail!("Google lehnt ab: {reason}");
    }
    let access = v["access_token"].as_str().context("kein access_token")?.to_owned();
    Ok(Tokens {
        access,
        refresh: v["refresh_token"].as_str().unwrap_or_default().to_owned(),
        expires_in: v["expires_in"].as_u64().unwrap_or(3600),
        email: v["id_token"].as_str().and_then(email_from_id_token).unwrap_or_default(),
    })
}

/// The `email` claim of an ID token (a JWT; not verified – it comes
/// straight from Google's token endpoint over TLS).
pub fn email_from_id_token(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let data = base64url_decode(payload)?;
    let v: serde_json::Value = serde_json::from_slice(&data).ok()?;
    v["email"].as_str().map(str::to_owned)
}

fn base64url_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return None,
        } as u32)
    };
    let bytes: Vec<u8> = text.bytes().filter(|&b| b != b'=').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= value(c)? << (18 - 6 * i);
        }
        let take = match chunk.len() {
            4 => 3,
            3 => 2,
            2 => 1,
            _ => return None,
        };
        for i in 0..take {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    Some(out)
}

/// `application/x-www-form-urlencoded`.
pub fn form(params: &[(&str, &str)]) -> String {
    params.iter().map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v))).collect::<Vec<_>>().join("&")
}

fn url_encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn parse_form(query: &str) -> Vec<(String, String)> {
    let decode = |s: &str| {
        let bytes = s.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'+' => out.push(b' '),
                b'%' if i + 2 < bytes.len() => {
                    match u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz"), 16) {
                        Ok(b) => {
                            out.push(b);
                            i += 2;
                        }
                        Err(_) => out.push(b'%'),
                    }
                }
                b => out.push(b),
            }
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    };
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), decode(v))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_forms_and_tokens() {
        let e = Endpoints::default();
        assert_eq!(
            e.calendar_url("ich@gmail.com"),
            "https://apidata.googleusercontent.com/caldav/v2/ich%40gmail.com/events/"
        );
        let client = OAuthClient { id: "123.apps.googleusercontent.com".into(), secret: "s".into() };
        let url = auth_url(&e, &client, "http://127.0.0.1:4711", "st", "ver");
        assert!(
            url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?client_id=123.apps.googleusercontent.com&")
        );
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A4711&"), "{url}");
        assert!(url.contains("scope=openid%20email%20https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcalendar"), "{url}");
        assert!(url.contains("access_type=offline"));
        assert_eq!(
            parse_form("code=4%2F0Ab+c&state=x"),
            [("code".into(), "4/0Ab c".into()), ("state".into(), "x".into())]
        );

        // An ID token with {"email":"ich@gmail.com"} as payload.
        let payload = "eyJlbWFpbCI6ImljaEBnbWFpbC5jb20ifQ";
        assert_eq!(email_from_id_token(&format!("h.{payload}.sig")).as_deref(), Some("ich@gmail.com"));
        let ok =
            format!(r#"{{"access_token":"a1","refresh_token":"r1","expires_in":3599,"id_token":"h.{payload}.s"}}"#);
        let t = exchange_code(
            |body| {
                assert!(body.contains("grant_type=authorization_code") && body.contains("code_verifier=ver"), "{body}");
                Ok((200, ok.clone()))
            },
            &client,
            "c",
            "http://127.0.0.1:4711",
            "ver",
        )
        .unwrap();
        assert_eq!((t.access.as_str(), t.refresh.as_str(), t.email.as_str()), ("a1", "r1", "ich@gmail.com"));
        let err =
            parse_tokens(400, r#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#);
        assert_eq!(err.unwrap_err().to_string(), "Google lehnt ab: Token has been expired or revoked.");
        assert!(random_token().len() >= 43);
    }

    #[test]
    fn loopback_receives_the_code() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let browser = std::thread::spawn(move || {
            let get = |path: &str| {
                let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
                write!(s, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
                let mut answer = String::new();
                let _ = std::io::Read::read_to_string(&mut s, &mut answer);
                answer
            };
            let _ = get("/favicon.ico");
            get("/?state=st&code=4%2Fabc&scope=email")
        });
        let code = wait_for_code(&listener, "st", Duration::from_secs(10)).unwrap();
        assert_eq!(code, "4/abc");
        assert!(browser.join().unwrap().contains("Atlas ist angemeldet"));
    }
}
