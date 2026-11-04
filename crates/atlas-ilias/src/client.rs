// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! HTTP access to ILIAS with the session of the web view.

use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use scraper::Html;
use url::Url;

use super::*;

/// Why reading or downloading stopped.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum IliasError {
    /// The session is gone (or there was none): log in again.
    #[error("Anmeldung bei ILIAS nötig")]
    Login,
    #[error("{0}")]
    Other(String),
}

/// HTTP access to one ILIAS with the session of the web view.
pub struct Client {
    agent: ureq::Agent,
    pub(super) base: Url,
    /// The session's cookies (name, value), sent to the ILIAS host only;
    /// ILIAS may set new ones on the way.
    cookies: Mutex<Vec<(String, String)>>,
}

/// Redirects followed per request at most.
const MAX_HOPS: usize = 12;

impl Client {
    /// `cookies`: the `Cookie` header of the logged-in session for the
    /// ILIAS host; `user_agent`: that of the browser the session belongs
    /// to ("" for Atlas' own).
    pub fn new(base_url: &str, cookies: &str, user_agent: &str) -> Result<Client, IliasError> {
        let base = Url::parse(base_url).map_err(|e| IliasError::Other(format!("ungültige ILIAS-Adresse: {e}")))?;
        let tls = ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
        let agent = ureq::Agent::config_builder()
            .tls_config(tls)
            .timeout_global(Some(Duration::from_secs(120)))
            .user_agent(if user_agent.is_empty() { "Atlas (ILIAS)" } else { user_agent })
            // Redirects are followed by hand ([`Client::get`]): with the
            // cookies on every hop, and to see where they lead.
            .max_redirects(0)
            .build()
            .new_agent();
        trace(&format!(
            "Client: {base} · Cookies: {} · Browserkennung: {}",
            cookie_names(cookies),
            if user_agent.is_empty() { "Atlas" } else { user_agent }
        ));
        let cookies = cookies
            .split(';')
            .filter_map(|c| c.split_once('='))
            .map(|(n, v)| (n.trim().to_owned(), v.trim().to_owned()))
            .filter(|(n, _)| !n.is_empty())
            .collect();
        Ok(Client { agent, base, cookies: Mutex::new(cookies) })
    }

    pub fn base(&self) -> &Url {
        &self.base
    }

    fn cookie_header(&self) -> String {
        let cookies = self.cookies.lock().unwrap_or_else(|e| e.into_inner());
        cookies.iter().map(|(n, v)| format!("{n}={v}")).collect::<Vec<_>>().join("; ")
    }

    /// Takes over the cookies ILIAS sets (removed ones go).
    fn take_cookies(&self, res: &ureq::http::Response<ureq::Body>) {
        let mut cookies = self.cookies.lock().unwrap_or_else(|e| e.into_inner());
        for header in res.headers().get_all("set-cookie").iter().filter_map(|v| v.to_str().ok()) {
            let mut parts = header.split(';');
            let Some((name, value)) = parts.next().and_then(|p| p.split_once('=')) else { continue };
            let (name, value) = (name.trim().to_owned(), value.trim().to_owned());
            let gone =
                value.is_empty() || value == "deleted" || parts.any(|p| p.trim().eq_ignore_ascii_case("max-age=0"));
            cookies.retain(|(n, _)| *n != name);
            trace(&format!("  Set-Cookie {name}{}", if gone { " (gelöscht)" } else { "" }));
            if !gone {
                cookies.push((name, value));
            }
        }
    }

    /// GET `url`, following redirects; the response and its address.
    pub(super) fn get(&self, url: &str) -> Result<(ureq::http::Response<ureq::Body>, Url), IliasError> {
        let mut at = Url::parse(url).map_err(|e| IliasError::Other(format!("{e} ({url})")))?;
        for _ in 0..MAX_HOPS {
            let res = self.get_once(&at)?;
            if at.host_str() == self.base.host_str() {
                self.take_cookies(&res);
            }
            if !res.status().is_redirection() {
                return Ok((res, at));
            }
            let Some(next) = res.headers().get("location").and_then(|v| v.to_str().ok()).and_then(|l| at.join(l).ok())
            else {
                return Ok((res, at));
            };
            trace(&format!("  {} {at} → {next}", res.status().as_u16()));
            at = next;
        }
        Err(IliasError::Other(format!("zu viele Weiterleitungen ({url})")))
    }

    fn get_once(&self, url: &Url) -> Result<ureq::http::Response<ureq::Body>, IliasError> {
        let mut attempts = 0;
        loop {
            attempts += 1;
            let mut req = self.agent.get(url.as_str());
            // The session belongs to ILIAS: not sent to other hosts.
            let cookies = self.cookie_header();
            if url.host_str() == self.base.host_str() && !cookies.is_empty() {
                req = req.header("Cookie", &cookies);
            }
            match req.call() {
                Ok(res) => return Ok(res),
                // Interrupted by a signal (e.g. a finished terminal program),
                // or a kept-alive connection the server had closed: again
                // (GET only reads).
                Err(ureq::Error::Io(e)) if attempts < 3 => {
                    trace(&format!("GET {url} → {e} – noch einmal"));
                    std::thread::sleep(Duration::from_millis(200 * attempts));
                    continue;
                }
                Err(ureq::Error::StatusCode(code)) => {
                    trace(&format!("GET {url} → HTTP {code}"));
                    return Err(IliasError::Other(format!("ILIAS antwortet mit {code} ({url})")));
                }
                Err(e) => {
                    trace(&format!("GET {url} → Fehler: {e}"));
                    return Err(IliasError::Other(format!("{e} ({url})")));
                }
            }
        }
    }

    /// A page of the logged-in ILIAS: its HTML and final address.
    pub fn page(&self, url: &str) -> Result<(String, Url), IliasError> {
        let (mut res, final_url) = self.get(url)?;
        let html = res
            .body_mut()
            .with_config()
            .limit(32 * 1024 * 1024)
            .read_to_string()
            .map_err(|e| IliasError::Other(e.to_string()))?;
        let doc = Html::parse_document(&html);
        let (inside, why) = login_check(&doc, &final_url);
        let moved = if final_url.as_str() == url { String::new() } else { format!(" → {final_url}") };
        trace(&format!(
            "GET {url}{moved} · {} · {} kB · „{}“ · {}: {why}",
            res.status().as_u16(),
            html.len() / 1024,
            page_title(&doc),
            if inside { "angemeldet" } else { "NICHT angemeldet" }
        ));
        // Sent elsewhere (the university's login) or to ILIAS' login.
        if final_url.host_str() != self.base.host_str() {
            trace(&format!(
                "  anderer Rechner als {} – Anmeldung nötig (Kopie: {})",
                self.base,
                dump(&html, &final_url)
            ));
            return Err(IliasError::Login);
        }
        if !inside {
            trace(&format!("  Kopie: {}", dump(&html, &final_url)));
            return Err(IliasError::Login);
        }
        Ok((html, final_url))
    }
}
