// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! CalDAV client for Nextcloud (and other RFC 4791 servers).
//!
//! Only what the project calendar needs: discovering the calendars of an
//! account, listing ETags, fetching objects (`calendar-multiget`), writing
//! with `If-Match` / `If-None-Match` and creating a calendar. The app
//! password is passed in for each client and never stored.
//!
//! [`Remote`] abstracts the server so that the synchronisation in
//! [`super::sync`] can be tested without a network.

use std::collections::BTreeMap;
use std::time::Duration;

use thiserror::Error;

/// Errors of a server request.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DavError {
    /// The server cannot be reached (no network, DNS, timeout, TLS …).
    #[error("Server nicht erreichbar: {0}")]
    Offline(String),
    #[error("Anmeldung abgelehnt – Benutzername oder App-Passwort prüfen")]
    Auth,
    #[error("Server antwortet mit {0}: {1}")]
    Http(u16, String),
    #[error("Unerwartete Antwort des Servers: {0}")]
    Invalid(String),
}

impl DavError {
    /// Whether the request may succeed later without user action.
    pub fn is_offline(&self) -> bool {
        matches!(self, DavError::Offline(_))
    }
}

pub type DavResult<T> = Result<T, DavError>;

/// A calendar collection on the server.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RemoteCalendar {
    /// Absolute URL of the collection (ends with `/`).
    pub url: String,
    pub name: String,
    /// `#rrggbb` ("" if the server has none).
    #[serde(default)]
    pub color: String,
    /// The user may create and change entries.
    pub writable: bool,
    /// Supported components (`VEVENT`, `VTODO`; empty = all).
    #[serde(default)]
    pub components: Vec<String>,
}

/// Result of [`Remote::discover`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Discovery {
    /// Calendar home (where new calendars are created), if known.
    pub home: Option<String>,
    pub calendars: Vec<RemoteCalendar>,
}

/// A calendar object resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    /// Absolute URL.
    pub href: String,
    pub etag: String,
    /// iCalendar text.
    pub data: String,
}

/// Outcome of a conditional write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Write {
    /// Written; the new ETag if the server sent one.
    Done(Option<String>),
    /// `412 Precondition Failed`: changed (or created) on the server meanwhile.
    Changed,
    /// The resource no longer exists on the server.
    Gone,
}

/// The server side of the synchronisation.
pub trait Remote {
    fn discover(&self) -> DavResult<Discovery>;
    /// `href → ETag` of all objects; `None` if the calendar is gone.
    fn etags(&self, calendar: &str) -> DavResult<Option<BTreeMap<String, String>>>;
    fn fetch(&self, calendar: &str, hrefs: &[String]) -> DavResult<Vec<Object>>;
    /// Writes `data`; `etag` `None` creates (fails with [`Write::Changed`]
    /// if something exists at `href`), otherwise only replaces that version.
    fn put(&self, href: &str, data: &str, etag: Option<&str>) -> DavResult<Write>;
    fn delete(&self, href: &str, etag: &str) -> DavResult<Write>;
    fn create_calendar(&self, home: &str, name: &str, color: &str) -> DavResult<RemoteCalendar>;
}

// ------------------------------------------------------------------ account

/// Connection settings (without password).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Account {
    /// Nextcloud address, e.g. `https://cloud.example.com`.
    pub server: String,
    pub user: String,
    /// Manually entered CalDAV URL (principal, home or a calendar); "" =
    /// discover from `server`.
    pub dav_url: String,
}

impl Account {
    /// Where discovery starts.
    pub fn start_url(&self) -> String {
        self.start_urls().remove(0)
    }

    /// Addresses discovery tries, in this order: the CalDAV URL entered by
    /// hand only; else the Nextcloud DAV root below the server address (an
    /// address copied from the browser is cut before `/apps/` or
    /// `/index.php`), at the host's root, and the well-known URL.
    pub fn start_urls(&self) -> Vec<String> {
        if !self.dav_url.trim().is_empty() {
            return vec![with_scheme(self.dav_url.trim())];
        }
        let server = with_scheme(self.server.trim());
        let mut base = server.trim_end_matches('/').to_owned();
        for cut in ["/index.php", "/apps/"] {
            if let Some(i) = base.find(cut) {
                base.truncate(i);
            }
        }
        let first = match base.find("/remote.php/") {
            Some(i) => format!("{}/remote.php/dav/", &base[..i]),
            None => format!("{base}/remote.php/dav/"),
        };
        let mut urls = vec![first];
        for url in [format!("{}/remote.php/dav/", origin(&base)), format!("{}/.well-known/caldav", origin(&base))] {
            if !urls.contains(&url) {
                urls.push(url);
            }
        }
        urls
    }
}

fn with_scheme(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") { url.to_owned() } else { format!("https://{url}") }
}

/// `scheme://host[:port]` of an absolute URL.
pub fn origin(url: &str) -> &str {
    let after = url.find("://").map(|i| i + 3).unwrap_or(0);
    match url[after..].find('/') {
        Some(i) => &url[..after + i],
        None => url,
    }
}

/// Resolves `href` from a response against the request URL `base`.
pub fn resolve(base: &str, href: &str) -> String {
    let href = href.trim();
    if href.starts_with("http://") || href.starts_with("https://") {
        href.to_owned()
    } else if href.starts_with('/') {
        format!("{}{href}", origin(base))
    } else {
        format!("{}{href}", &base[..base.rfind('/').map(|i| i + 1).unwrap_or(base.len())])
    }
}

/// The path part of a URL (what `calendar-multiget` expects).
fn path_of(url: &str) -> &str {
    let o = origin(url);
    if url.len() > o.len() { &url[o.len()..] } else { "/" }
}

/// File name for a new object from its UID (only safe characters, so
/// servers report the href exactly as written).
pub fn object_name(uid: &str) -> String {
    let name: String = uid
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' })
        .collect();
    format!("{}.ics", name.trim_matches('.'))
}

/// Unescaped comparison of two hrefs (`%40` vs `@`, trailing slash).
pub fn same_href(a: &str, b: &str) -> bool {
    let norm = |s: &str| crate::markdown::percent_decode(s).trim_end_matches('/').to_owned();
    norm(a) == norm(b)
}

// ---------------------------------------------------------------------- XML

const DAV: &str = "DAV:";
const CALDAV: &str = "urn:ietf:params:xml:ns:caldav";
const APPLE: &str = "http://apple.com/ns/ical/";

/// One `<d:response>` with the properties of its successful propstats.
#[derive(Debug, Default)]
struct Response {
    href: String,
    /// `ns name → element` as text of the property (`calendar-data`,
    /// `getetag`, …) or names of child elements (`resourcetype`).
    text: BTreeMap<String, String>,
    children: BTreeMap<String, Vec<String>>,
    /// `href` values inside properties (`current-user-principal`, …).
    hrefs: BTreeMap<String, String>,
}

fn key(node: roxmltree::Node) -> String {
    format!("{}|{}", node.tag_name().namespace().unwrap_or(""), node.tag_name().name())
}

fn k(ns: &str, name: &str) -> String {
    format!("{ns}|{name}")
}

fn parse_multistatus(xml: &str) -> DavResult<Vec<Response>> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| DavError::Invalid(format!("kein gültiges XML ({e})")))?;
    let mut out = Vec::new();
    for resp in doc.descendants().filter(|n| n.is_element() && key(*n) == k(DAV, "response")) {
        let mut r = Response::default();
        for child in resp.children().filter(|n| n.is_element()) {
            if key(child) == k(DAV, "href") {
                r.href = child.text().unwrap_or_default().trim().to_owned();
            }
        }
        for propstat in resp.children().filter(|n| n.is_element() && key(*n) == k(DAV, "propstat")) {
            let ok = propstat
                .children()
                .find(|n| n.is_element() && key(*n) == k(DAV, "status"))
                .and_then(|s| s.text())
                .is_none_or(|s| s.contains(" 200"));
            if !ok {
                continue;
            }
            for prop in propstat.children().filter(|n| n.is_element() && key(*n) == k(DAV, "prop")) {
                for p in prop.children().filter(|n| n.is_element()) {
                    let name = key(p);
                    let text: String = p.descendants().filter(|n| n.is_text()).filter_map(|n| n.text()).collect();
                    r.text.insert(name.clone(), text.trim().to_owned());
                    let mut names = Vec::new();
                    for c in p.descendants().filter(|n| n.is_element() && *n != p) {
                        if key(c) == k(DAV, "href") {
                            r.hrefs
                                .entry(name.clone())
                                .or_insert_with(|| c.text().unwrap_or_default().trim().to_owned());
                        }
                        // `comp name="VEVENT"` in supported-calendar-component-set.
                        if let Some(n) = c.attribute("name") {
                            names.push(n.to_ascii_uppercase());
                        } else {
                            names.push(key(c));
                        }
                    }
                    r.children.insert(name, names);
                }
            }
        }
        out.push(r);
    }
    Ok(out)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Turns a server color (`#RRGGBBAA`, `#RGB`) into `#rrggbb`.
fn normalize_color(c: &str) -> String {
    let c = c.trim();
    let hex = c.trim_start_matches('#');
    match hex.len() {
        6 | 8 if hex.chars().all(|x| x.is_ascii_hexdigit()) => format!("#{}", hex[..6].to_ascii_lowercase()),
        3 if hex.chars().all(|x| x.is_ascii_hexdigit()) => {
            format!("#{}", hex.chars().flat_map(|x| [x, x]).collect::<String>().to_ascii_lowercase())
        }
        _ => String::new(),
    }
}

fn calendar_from(r: &Response, base: &str) -> Option<RemoteCalendar> {
    let types = r.children.get(&k(DAV, "resourcetype"))?;
    if !types.iter().any(|t| *t == k(CALDAV, "calendar")) {
        return None;
    }
    let url = resolve(base, &r.href);
    let url = if url.ends_with('/') { url } else { format!("{url}/") };
    let privileges = r.children.get(&k(DAV, "current-user-privilege-set"));
    let writable = privileges.is_none_or(|p| {
        p.iter().any(|x| [k(DAV, "write"), k(DAV, "write-content"), k(DAV, "all"), k(DAV, "bind")].contains(x))
    });
    let components = r.children.get(&k(CALDAV, "supported-calendar-component-set")).cloned().unwrap_or_default();
    let components: Vec<String> = components.into_iter().filter(|c| !c.contains('|')).collect();
    let name = r
        .text
        .get(&k(DAV, "displayname"))
        .filter(|n| !n.is_empty())
        .cloned()
        .unwrap_or_else(|| url.trim_end_matches('/').rsplit('/').next().unwrap_or("Kalender").to_owned());
    Some(RemoteCalendar {
        url,
        name,
        color: r.text.get(&k(APPLE, "calendar-color")).map(|c| normalize_color(c)).unwrap_or_default(),
        writable,
        components,
    })
}

/// Calendars Atlas can show (events or tasks).
fn usable(c: &RemoteCalendar) -> bool {
    c.components.is_empty() || c.components.iter().any(|x| x == "VEVENT" || x == "VTODO")
}

// ------------------------------------------------------------------- client

/// HTTP client for one account.
pub struct Client {
    account: Account,
    password: String,
    agent: ureq::Agent,
}

const PROPFIND_HOME: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
  <d:prop><d:current-user-principal/><c:calendar-home-set/><d:resourcetype/></d:prop>
</d:propfind>"#;

const PROPFIND_CALENDARS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:a="http://apple.com/ns/ical/">
  <d:prop>
    <d:resourcetype/><d:displayname/><a:calendar-color/>
    <d:current-user-privilege-set/><c:supported-calendar-component-set/>
  </d:prop>
</d:propfind>"#;

const PROPFIND_ETAGS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:"><d:prop><d:getetag/><d:resourcetype/></d:prop></d:propfind>"#;

impl Client {
    /// `use_proxy`: honour `HTTPS_PROXY`/`HTTP_PROXY` (off for tests on
    /// localhost).
    pub fn new(account: Account, password: &str, use_proxy: bool) -> Self {
        let tls = ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
        let mut config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .allow_non_standard_methods(true)
            .timeout_global(Some(Duration::from_secs(30)))
            .timeout_connect(Some(Duration::from_secs(10)))
            .user_agent(format!("Atlas/{}", env!("CARGO_PKG_VERSION")))
            .tls_config(tls);
        if !use_proxy {
            config = config.proxy(None);
        }
        Self { account, password: password.to_owned(), agent: config.build().into() }
    }

    fn auth(&self) -> String {
        format!("Basic {}", base64(format!("{}:{}", self.account.user, self.password).as_bytes()))
    }

    /// Sends a request; returns status, headers of interest and body.
    fn request(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> DavResult<(u16, Option<String>, String)> {
        let method = ureq::http::Method::from_bytes(method.as_bytes()).map_err(|e| DavError::Invalid(e.to_string()))?;
        let build = || {
            let mut builder =
                ureq::http::Request::builder().method(method.clone()).uri(url).header("Authorization", self.auth());
            for (name, value) in headers {
                builder = builder.header(*name, *value);
            }
            builder
        };
        // A signal (e.g. a terminal's child process exiting: the embedded
        // terminal handles SIGCHLD) interrupts socket reads with a timeout;
        // such a request is simply sent again. All writes are conditional
        // (If-Match / If-None-Match), so a repeat does no harm.
        let mut attempts = 0;
        let result = loop {
            let request = build();
            let result = match body {
                Some(b) => self.agent.run(request.body(b.to_owned()).map_err(|e| DavError::Invalid(e.to_string()))?),
                None => self.agent.run(request.body(()).map_err(|e| DavError::Invalid(e.to_string()))?),
            };
            attempts += 1;
            match result {
                Err(ureq::Error::Io(err)) if err.kind() == std::io::ErrorKind::Interrupted && attempts < 4 => continue,
                other => break other,
            }
        };
        let mut response = result.map_err(|e| match e {
            ureq::Error::StatusCode(code) => DavError::Http(code, String::new()),
            ureq::Error::BadUri(u) => DavError::Invalid(format!("ungültige Adresse {u}")),
            other => DavError::Offline(other.to_string()),
        })?;
        let status = response.status().as_u16();
        let etag = response.headers().get("etag").and_then(|v| v.to_str().ok()).map(str::to_owned);
        let text = response.body_mut().with_config().limit(64 * 1024 * 1024).read_to_string().unwrap_or_default();
        if status == 401 {
            return Err(DavError::Auth);
        }
        Ok((status, etag, text))
    }

    fn propfind(&self, url: &str, depth: &str, body: &str) -> DavResult<Option<Vec<Response>>> {
        let (status, _, text) = self.request(
            "PROPFIND",
            url,
            &[("Depth", depth), ("Content-Type", "application/xml; charset=utf-8")],
            Some(body),
        )?;
        match status {
            207 => Ok(Some(parse_multistatus(&text)?)),
            404 | 410 => Ok(None),
            s => Err(failed("PROPFIND", url, s, &text)),
        }
    }
}

/// An HTTP error with the request (method and address) and the reason.
fn failed(method: &str, url: &str, status: u16, body: &str) -> DavError {
    let reason = short(body);
    DavError::Http(
        status,
        if reason.is_empty() { format!("{method} {url}") } else { format!("{method} {url} – {reason}") },
    )
}

/// The reason in an error body, as one short line of plain text.
fn short(body: &str) -> String {
    let between = |open: &str, close: &str| {
        let start = body.find(open)? + open.len();
        let end = body[start..].find(close)?;
        Some(body[start..start + end].to_owned())
    };
    // Sabre puts the reason into <s:message>, web servers send an HTML page.
    let text = between("<s:message>", "</s:message>")
        .or_else(|| between("<title>", "</title>"))
        .or_else(|| between("<h1>", "</h1>"))
        .unwrap_or_else(|| body.to_owned());
    let mut plain = String::new();
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                plain.push(' ');
            }
            c if !in_tag => plain.push(c),
            _ => {}
        }
    }
    let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    plain.chars().take(160).collect()
}

impl Remote for Client {
    fn discover(&self) -> DavResult<Discovery> {
        let mut home: Option<String> = None;
        let mut principal: Option<String> = None;
        // The first address that answers as CalDAV: the given one, then the
        // usual Nextcloud and well-known (RFC 6764) addresses. Web servers
        // answer PROPFIND on other paths with 405 or 404.
        let mut found = None;
        let mut last_error = None;
        for url in self.account.start_urls() {
            match self.propfind(&url, "0", PROPFIND_HOME) {
                Ok(Some(list)) => {
                    found = Some((url, list));
                    break;
                }
                Ok(None) => last_error = Some(DavError::Http(404, format!("PROPFIND {url}"))),
                Err(DavError::Http(code, reason)) if [400, 403, 404, 405, 501].contains(&code) => {
                    last_error = Some(DavError::Http(code, reason))
                }
                Err(err) => return Err(err),
            }
        }
        let Some((start, first)) = found else {
            let hint =
                "kein CalDAV gefunden – als Server die Adresse der Nextcloud angeben, z. B. https://cloud.example.com";
            return Err(match last_error {
                Some(DavError::Http(code, reason)) => DavError::Http(code, format!("{reason}; {hint}")),
                _ => DavError::Invalid(hint.into()),
            });
        };
        for r in &first {
            if let Some(h) = r.hrefs.get(&k(CALDAV, "calendar-home-set")) {
                home = Some(resolve(&start, h));
            }
            if let Some(p) = r.hrefs.get(&k(DAV, "current-user-principal")) {
                principal = Some(resolve(&start, p));
            }
            // A calendar URL entered by hand.
            if let Some(cal) = calendar_from(r, &start) {
                let calendars = self.propfind(&cal.url, "0", PROPFIND_CALENDARS)?.unwrap_or_default();
                let calendars: Vec<RemoteCalendar> =
                    calendars.iter().filter_map(|r| calendar_from(r, &cal.url)).collect();
                return Ok(Discovery {
                    home: None,
                    calendars: if calendars.is_empty() { vec![cal] } else { calendars },
                });
            }
        }
        if home.is_none()
            && let Some(p) = &principal
            && let Some(list) = self.propfind(p, "0", PROPFIND_HOME)?
        {
            home = list.iter().find_map(|r| r.hrefs.get(&k(CALDAV, "calendar-home-set")).map(|h| resolve(p, h)));
        }
        let Some(home) = home else {
            return Err(DavError::Invalid("kein Kalenderverzeichnis (calendar-home-set) gefunden".into()));
        };
        let list = self.propfind(&home, "1", PROPFIND_CALENDARS)?.unwrap_or_default();
        let mut calendars: Vec<RemoteCalendar> =
            list.iter().filter_map(|r| calendar_from(r, &home)).filter(usable).collect();
        calendars.sort_by_key(|c| c.name.to_lowercase());
        Ok(Discovery { home: Some(home), calendars })
    }

    fn etags(&self, calendar: &str) -> DavResult<Option<BTreeMap<String, String>>> {
        let Some(list) = self.propfind(calendar, "1", PROPFIND_ETAGS)? else { return Ok(None) };
        let mut out = BTreeMap::new();
        for r in list {
            let href = resolve(calendar, &r.href);
            let is_collection =
                r.children.get(&k(DAV, "resourcetype")).is_some_and(|t| t.iter().any(|x| *x == k(DAV, "collection")));
            if is_collection || same_href(&href, calendar) {
                continue;
            }
            if let Some(etag) = r.text.get(&k(DAV, "getetag")) {
                out.insert(href, etag.clone());
            }
        }
        Ok(Some(out))
    }

    fn fetch(&self, calendar: &str, hrefs: &[String]) -> DavResult<Vec<Object>> {
        let mut out = Vec::new();
        for chunk in hrefs.chunks(50) {
            let mut body = String::from(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<c:calendar-multiget xmlns:d=\"DAV:\" xmlns:c=\"urn:ietf:params:xml:ns:caldav\">\n  <d:prop><d:getetag/><c:calendar-data/></d:prop>\n",
            );
            for h in chunk {
                body += &format!("  <d:href>{}</d:href>\n", xml_escape(path_of(h)));
            }
            body += "</c:calendar-multiget>";
            let (status, _, text) = self.request(
                "REPORT",
                calendar,
                &[("Depth", "1"), ("Content-Type", "application/xml; charset=utf-8")],
                Some(&body),
            )?;
            if status != 207 {
                return Err(failed("REPORT", calendar, status, &text));
            }
            for r in parse_multistatus(&text)? {
                let (Some(etag), Some(data)) =
                    (r.text.get(&k(DAV, "getetag")), r.text.get(&k(CALDAV, "calendar-data")))
                else {
                    continue;
                };
                let href = resolve(calendar, &r.href);
                // Report the href as requested (servers may encode differently).
                let href = chunk.iter().find(|h| same_href(h, &href)).cloned().unwrap_or(href);
                out.push(Object { href, etag: etag.clone(), data: data.clone() });
            }
        }
        Ok(out)
    }

    fn put(&self, href: &str, data: &str, etag: Option<&str>) -> DavResult<Write> {
        let mut headers = vec![("Content-Type", "text/calendar; charset=utf-8")];
        match etag {
            Some(e) => headers.push(("If-Match", e)),
            None => headers.push(("If-None-Match", "*")),
        }
        let (status, new_etag, text) = self.request("PUT", href, &headers, Some(data))?;
        match status {
            200..=299 => Ok(Write::Done(new_etag)),
            412 => Ok(Write::Changed),
            404 | 410 if etag.is_some() => Ok(Write::Gone),
            s => Err(failed("PUT", href, s, &text)),
        }
    }

    fn delete(&self, href: &str, etag: &str) -> DavResult<Write> {
        let (status, _, text) = self.request("DELETE", href, &[("If-Match", etag)], None)?;
        match status {
            200..=299 | 404 | 410 => Ok(Write::Done(None)),
            412 => Ok(Write::Changed),
            s => Err(failed("DELETE", href, s, &text)),
        }
    }

    fn create_calendar(&self, home: &str, name: &str, color: &str) -> DavResult<RemoteCalendar> {
        let slug: String = name
            .to_lowercase()
            .chars()
            .map(|c| match c {
                'ä' => 'a',
                'ö' => 'o',
                'ü' => 'u',
                'ß' => 's',
                c if c.is_ascii_alphanumeric() => c,
                _ => '-',
            })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let slug = if slug.is_empty() { "atlas".to_owned() } else { slug };
        let home = if home.ends_with('/') { home.to_owned() } else { format!("{home}/") };
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<c:mkcalendar xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:a="http://apple.com/ns/ical/">
  <d:set><d:prop>
    <d:displayname>{}</d:displayname>
    <a:calendar-color>{}</a:calendar-color>
    <c:supported-calendar-component-set><c:comp name="VEVENT"/><c:comp name="VTODO"/></c:supported-calendar-component-set>
  </d:prop></d:set>
</c:mkcalendar>"#,
            xml_escape(name),
            xml_escape(color)
        );
        // Find a free name (`atlas-projektarbeit`, `…-2`, …).
        for n in 1..20 {
            let url = if n == 1 { format!("{home}{slug}/") } else { format!("{home}{slug}-{n}/") };
            let (status, _, text) =
                self.request("MKCALENDAR", &url, &[("Content-Type", "application/xml; charset=utf-8")], Some(&body))?;
            match status {
                200..=299 => {
                    return Ok(RemoteCalendar {
                        url,
                        name: name.to_owned(),
                        color: normalize_color(color),
                        writable: true,
                        components: vec!["VEVENT".into(), "VTODO".into()],
                    });
                }
                405 | 409 => continue, // exists
                s => return Err(failed("MKCALENDAR", &url, s, &text)),
            }
        }
        Err(DavError::Invalid("kein freier Name für den neuen Kalender".into()))
    }
}

/// Standard base64 (for Basic authentication).
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write as _};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    #[test]
    fn helpers() {
        assert_eq!(base64(b"user:pass"), "dXNlcjpwYXNz");
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(base64(b"a"), "YQ==");
        assert_eq!(origin("https://cloud.example.com/remote.php/dav/"), "https://cloud.example.com");
        assert_eq!(resolve("https://h/a/b/", "/x/y"), "https://h/x/y");
        assert_eq!(resolve("https://h/a/b/", "c.ics"), "https://h/a/b/c.ics");
        assert_eq!(
            Account { server: "cloud.example.com/".into(), ..Default::default() }.start_url(),
            "https://cloud.example.com/remote.php/dav/"
        );
        // An address copied from the browser, Nextcloud in a subfolder.
        let urls = |server: &str| Account { server: server.into(), ..Default::default() }.start_urls();
        assert_eq!(
            urls("https://cloud.example.com/nextcloud/apps/calendar/dayGridMonth/now"),
            [
                "https://cloud.example.com/nextcloud/remote.php/dav/",
                "https://cloud.example.com/remote.php/dav/",
                "https://cloud.example.com/.well-known/caldav"
            ]
        );
        assert_eq!(urls("cloud.example.com/index.php/apps/files/")[0], "https://cloud.example.com/remote.php/dav/");
        assert_eq!(urls("https://c.de/remote.php/dav/calendars/u/")[0], "https://c.de/remote.php/dav/");
        assert_eq!(urls("https://c.de").len(), 2);
        let manual = Account { dav_url: "c.de/dav/".into(), server: "x".into(), ..Default::default() };
        assert_eq!(manual.start_urls(), ["https://c.de/dav/"]);
        // Error pages as one line of plain text.
        assert_eq!(
            short(
                "<html>\r\n<head><title>405 Not Allowed</title></head>\r\n<body>\r\n<center><h1>405 Not Allowed</h1></center>\r\n<hr><center>nginx</center></body></html>"
            ),
            "405 Not Allowed"
        );
        assert_eq!(short("<d:error><s:message>Kalender <b>fehlt</b></s:message></d:error>"), "Kalender fehlt");
        assert_eq!(
            failed("PROPFIND", "https://c.de/x/", 405, "<h1>Nope</h1>").to_string(),
            "Server antwortet mit 405: PROPFIND https://c.de/x/ – Nope"
        );
        assert_eq!(object_name("0a1b@atlas"), "0a1b-atlas.ics");
        assert!(same_href("https://h/c/a%40b.ics", "https://h/c/a@b.ics"));
        assert_eq!(normalize_color("#0082C9FF"), "#0082c9");
    }

    #[test]
    fn parses_nextcloud_calendar_list() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:s="http://sabredav.org/ns" xmlns:cal="urn:ietf:params:xml:ns:caldav" xmlns:x1="http://apple.com/ns/ical/">
 <d:response><d:href>/remote.php/dav/calendars/minux/</d:href>
  <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
 <d:response><d:href>/remote.php/dav/calendars/minux/personal/</d:href>
  <d:propstat><d:prop><d:resourcetype><d:collection/><cal:calendar/></d:resourcetype><d:displayname>Persönlich</d:displayname>
   <x1:calendar-color>#0082C9FF</x1:calendar-color>
   <d:current-user-privilege-set><d:privilege><d:read/></d:privilege><d:privilege><d:write/></d:privilege></d:current-user-privilege-set>
   <cal:supported-calendar-component-set><cal:comp name="VEVENT"/><cal:comp name="VTODO"/></cal:supported-calendar-component-set>
  </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
 <d:response><d:href>/remote.php/dav/calendars/minux/contact_birthdays/</d:href>
  <d:propstat><d:prop><d:resourcetype><d:collection/><cal:calendar/></d:resourcetype><d:displayname>Geburtstage</d:displayname>
   <d:current-user-privilege-set><d:privilege><d:read/></d:privilege></d:current-user-privilege-set>
  </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  <d:propstat><d:prop><x1:calendar-color/></d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat></d:response>
</d:multistatus>"#;
        let list = parse_multistatus(xml).unwrap();
        let cals: Vec<RemoteCalendar> = list
            .iter()
            .filter_map(|r| calendar_from(r, "https://cloud.example.com/remote.php/dav/calendars/minux/"))
            .collect();
        assert_eq!(cals.len(), 2);
        assert_eq!(cals[0].url, "https://cloud.example.com/remote.php/dav/calendars/minux/personal/");
        assert_eq!((cals[0].name.as_str(), cals[0].color.as_str(), cals[0].writable), ("Persönlich", "#0082c9", true));
        assert_eq!(cals[0].components, ["VEVENT", "VTODO"]);
        assert!(!cals[1].writable, "birthdays are read-only");
    }

    // ------------------------------------------------------ test server

    /// A tiny CalDAV server in a thread (one account, calendars in memory),
    /// enough for discovery, ETags, multiget, conditional PUT/DELETE and
    /// MKCALENDAR as Nextcloud answers them.
    /// `href → (ETag, data)`.
    pub type Objects = BTreeMap<String, (String, String)>;

    #[derive(Default)]
    pub struct ServerState {
        /// calendar path → (name, objects).
        pub calendars: BTreeMap<String, (String, Objects)>,
        pub counter: u32,
        pub puts: u32,
        pub requests: Vec<String>,
    }

    pub struct TestServer {
        pub base: String,
        pub state: Arc<Mutex<ServerState>>,
    }

    impl TestServer {
        pub fn start() -> TestServer {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let state = Arc::new(Mutex::new(ServerState::default()));
            state
                .lock()
                .unwrap()
                .calendars
                .insert("/remote.php/dav/calendars/minux/personal/".into(), ("Persönlich".into(), BTreeMap::new()));
            let shared = state.clone();
            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    let state = shared.clone();
                    std::thread::spawn(move || {
                        let _ = handle(stream, &state);
                    });
                }
            });
            TestServer { base, state }
        }
    }

    fn handle(mut stream: std::net::TcpStream, state: &Mutex<ServerState>) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                return Ok(());
            }
            let mut parts = line.split_whitespace();
            let (method, path) = (parts.next().unwrap_or("").to_owned(), parts.next().unwrap_or("").to_owned());
            let mut headers = BTreeMap::new();
            loop {
                let mut h = String::new();
                reader.read_line(&mut h)?;
                let h = h.trim_end();
                if h.is_empty() {
                    break;
                }
                if let Some((n, v)) = h.split_once(':') {
                    headers.insert(n.trim().to_ascii_lowercase(), v.trim().to_owned());
                }
            }
            let len: usize = headers.get("content-length").and_then(|l| l.parse().ok()).unwrap_or(0);
            let mut body = vec![0; len];
            reader.read_exact(&mut body)?;
            let body = String::from_utf8_lossy(&body).into_owned();
            let (status, extra, text) = respond(&method, &path, &headers, &body, state);
            let reason = match status {
                200 => "OK",
                201 => "Created",
                204 => "No Content",
                207 => "Multi-Status",
                401 => "Unauthorized",
                404 => "Not Found",
                412 => "Precondition Failed",
                _ => "X",
            };
            write!(stream, "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\n{extra}\r\n{text}", text.len())?;
            stream.flush()?;
        }
    }

    fn respond(
        method: &str,
        path: &str,
        headers: &BTreeMap<String, String>,
        body: &str,
        state: &Mutex<ServerState>,
    ) -> (u16, String, String) {
        let mut s = state.lock().unwrap();
        s.requests.push(format!("{method} {path}"));
        if headers.get("authorization").map(String::as_str) != Some("Basic bWludXg6Z2VoZWlt") {
            return (401, String::new(), String::new());
        }
        let ms = |inner: String| {
            (
                207,
                "Content-Type: application/xml\r\n".to_owned(),
                format!(
                    "<?xml version=\"1.0\"?><d:multistatus xmlns:d=\"DAV:\" xmlns:c=\"urn:ietf:params:xml:ns:caldav\" xmlns:a=\"http://apple.com/ns/ical/\">{inner}</d:multistatus>"
                ),
            )
        };
        let ok = |prop: String| {
            format!("<d:propstat><d:prop>{prop}</d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>")
        };
        let cal_of = |p: &str| p[..p.rfind('/').map(|i| i + 1).unwrap_or(0)].to_owned();
        match method {
            "PROPFIND" if path == "/remote.php/dav/" => ms(format!(
                "<d:response><d:href>/remote.php/dav/</d:href>{}</d:response>",
                ok("<d:current-user-principal><d:href>/remote.php/dav/principals/users/minux/</d:href></d:current-user-principal>".into())
            )),
            "PROPFIND" if path.starts_with("/remote.php/dav/principals/") => ms(format!(
                "<d:response><d:href>{path}</d:href>{}</d:response>",
                ok("<c:calendar-home-set><d:href>/remote.php/dav/calendars/minux/</d:href></c:calendar-home-set>".into())
            )),
            "PROPFIND" if path == "/remote.php/dav/calendars/minux/" => {
                let mut inner = format!("<d:response><d:href>{path}</d:href>{}</d:response>", ok("<d:resourcetype><d:collection/></d:resourcetype>".into()));
                for (p, (name, _)) in &s.calendars {
                    inner += &format!(
                        "<d:response><d:href>{p}</d:href>{}</d:response>",
                        ok(format!("<d:resourcetype><d:collection/><c:calendar/></d:resourcetype><d:displayname>{name}</d:displayname><a:calendar-color>#0082C9</a:calendar-color><d:current-user-privilege-set><d:privilege><d:write/></d:privilege></d:current-user-privilege-set>"))
                    );
                }
                ms(inner)
            }
            "PROPFIND" => match s.calendars.get(path) {
                Some((_, objects)) => {
                    let mut inner = format!("<d:response><d:href>{path}</d:href>{}</d:response>", ok("<d:resourcetype><d:collection/><c:calendar/></d:resourcetype>".into()));
                    for (href, (etag, _)) in objects {
                        inner += &format!("<d:response><d:href>{href}</d:href>{}</d:response>", ok(format!("<d:getetag>{etag}</d:getetag><d:resourcetype/>")));
                    }
                    ms(inner)
                }
                None => (404, String::new(), String::new()),
            },
            "REPORT" => {
                let Some((_, objects)) = s.calendars.get(path) else { return (404, String::new(), String::new()) };
                let mut inner = String::new();
                for part in body.split("<d:href>").skip(1) {
                    let href = part.split("</d:href>").next().unwrap_or("");
                    if let Some((etag, data)) = objects.get(href) {
                        inner += &format!(
                            "<d:response><d:href>{href}</d:href>{}</d:response>",
                            ok(format!("<d:getetag>{etag}</d:getetag><c:calendar-data>{}</c:calendar-data>", xml_escape(data)))
                        );
                    }
                }
                ms(inner)
            }
            "PUT" => {
                let cal = cal_of(path);
                s.counter += 1;
                let etag = format!("\"e{}\"", s.counter);
                let Some((_, objects)) = s.calendars.get_mut(&cal) else { return (404, String::new(), String::new()) };
                let existing = objects.get(path).map(|o| o.0.clone());
                match (headers.get("if-match"), headers.get("if-none-match"), existing) {
                    (Some(_), _, None) => return (404, String::new(), String::new()),
                    (Some(m), _, Some(e)) if *m != e => return (412, String::new(), String::new()),
                    (_, Some(_), Some(_)) => return (412, String::new(), String::new()),
                    _ => {}
                }
                objects.insert(path.to_owned(), (etag.clone(), body.to_owned()));
                s.puts += 1;
                (201, format!("ETag: {etag}\r\n"), String::new())
            }
            "DELETE" => {
                let cal = cal_of(path);
                let Some((_, objects)) = s.calendars.get_mut(&cal) else { return (404, String::new(), String::new()) };
                match (headers.get("if-match"), objects.get(path)) {
                    (_, None) => (404, String::new(), String::new()),
                    (Some(m), Some(o)) if *m != o.0 => (412, String::new(), String::new()),
                    _ => {
                        objects.remove(path);
                        (204, String::new(), String::new())
                    }
                }
            }
            "MKCALENDAR" => {
                if s.calendars.contains_key(path) {
                    return (405, String::new(), String::new());
                }
                let name = body.split("<d:displayname>").nth(1).and_then(|r| r.split("</d:displayname>").next()).unwrap_or("").to_owned();
                s.calendars.insert(path.to_owned(), (name, BTreeMap::new()));
                (201, String::new(), String::new())
            }
            _ => (405, String::new(), String::new()),
        }
    }

    #[test]
    fn talks_to_a_caldav_server() {
        let server = TestServer::start();
        let account = Account { server: server.base.clone(), user: "minux".into(), dav_url: String::new() };
        let wrong = Client::new(account.clone(), "falsch", false);
        assert_eq!(wrong.discover(), Err(DavError::Auth));

        // An address copied from the browser: the DAV root is found anyway.
        let copied = Account { server: format!("{}/sub/apps/calendar/now", server.base), ..account.clone() };
        let d = Client::new(copied, "geheim", false).discover().unwrap();
        assert_eq!(d.home.as_deref(), Some(format!("{}/remote.php/dav/calendars/minux/", server.base).as_str()));

        let client = Client::new(account, "geheim", false);
        let d = client.discover().unwrap();
        assert_eq!(d.home.as_deref(), Some(format!("{}/remote.php/dav/calendars/minux/", server.base).as_str()));
        assert_eq!(d.calendars.len(), 1);
        let cal = d.calendars[0].url.clone();
        assert_eq!(d.calendars[0].name, "Persönlich");

        let href = format!("{cal}{}", object_name("a@atlas"));
        let data =
            "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:a@atlas\r\nSUMMARY:Test & mehr\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let Write::Done(Some(etag)) = client.put(&href, data, None).unwrap() else { panic!("created") };
        assert_eq!(client.put(&href, data, None).unwrap(), Write::Changed, "If-None-Match prevents duplicates");
        let etags = client.etags(&cal).unwrap().unwrap();
        assert_eq!(etags.get(&href), Some(&etag));
        let objects = client.fetch(&cal, std::slice::from_ref(&href)).unwrap();
        // XML normalises line ends; the iCalendar reader accepts both.
        assert_eq!(objects[0].data, data.replace('\r', "").trim(), "calendar-data unescaped");
        assert_eq!(client.put(&href, data, Some("\"alt\"")).unwrap(), Write::Changed);
        assert!(matches!(client.put(&href, data, Some(&etag)).unwrap(), Write::Done(_)));
        assert_eq!(client.delete(&href, &etag).unwrap(), Write::Changed, "stale ETag");
        assert!(client.etags(&format!("{}/remote.php/dav/calendars/minux/weg/", server.base)).unwrap().is_none());

        let created = client.create_calendar(d.home.as_deref().unwrap(), "Atlas – Projektarbeit", "#2ea043").unwrap();
        assert!(created.url.ends_with("/calendars/minux/atlas-projektarbeit/"), "{}", created.url);
        assert_eq!(client.discover().unwrap().calendars.len(), 2);

        // Unreachable server.
        let offline = Client::new(
            Account { server: "http://127.0.0.1:9".into(), user: "minux".into(), dav_url: String::new() },
            "geheim",
            false,
        );
        assert!(offline.discover().unwrap_err().is_offline());
    }
}
