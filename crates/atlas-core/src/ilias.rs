// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! ILIAS (e.g. KIT): the courses of the dashboard as folders.
//!
//! The login happens in the app's web view (the university's single
//! sign-on needs a browser); its session cookies come here as a `Cookie`
//! header. With them [`Client`] fetches ILIAS pages, [`objects`] finds the
//! courses, groups, sessions, folders and files on a page, [`scan`] reads
//! the whole tree from the dashboard (several pages at a time) and
//! [`download`] saves a file. Two JSON files in the state directory keep
//! the tree (`index.json`) and what was downloaded where with which ILIAS
//! version (`downloads.json`).

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use anyhow::{Result, bail};
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

/// State files of the ILIAS view.
pub fn state_dir() -> PathBuf {
    crate::paths::state_root().join("ilias")
}

/// The JSON file `name` ("index" or "downloads"); other names are refused.
pub fn state_file(name: &str) -> Option<PathBuf> {
    matches!(name, "index" | "downloads").then(|| state_dir().join(format!("{name}.json")))
}

/// Reads a state file ("" if there is none).
pub fn load(name: &str) -> String {
    state_file(name).and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default()
}

/// Writes a state file; `json` must be valid JSON.
pub fn save(name: &str, json: &str) -> Result<()> {
    let Some(path) = state_file(name) else { bail!("unbekannte ILIAS-Datei {name}") };
    serde_json::from_str::<serde_json::Value>(json)?;
    crate::paths::write_atomic(&path, json.as_bytes())
}

// --------------------------------------------------------------- the log

/// The ILIAS log (`debug.log`): every page fetched, where it led, whether
/// it counted as logged in and why, what was found on it. Cookie values
/// are never written, only their names.
pub fn debug_log() -> PathBuf {
    state_dir().join("debug.log")
}

/// Copies of pages kept for the log (dashboards, pages without login or
/// without objects), from the last reading.
pub fn debug_pages() -> PathBuf {
    state_dir().join("debug")
}

static TRACE: Mutex<()> = Mutex::new(());

/// Adds a line to the ILIAS log (and to the program's log).
pub fn trace(line: &str) {
    log::info!("ilias: {line}");
    if cfg!(test) {
        return;
    }
    let _guard = TRACE.lock().unwrap_or_else(|e| e.into_inner());
    let path = debug_log();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Long logs start over.
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 4 * 1024 * 1024) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{} {line}", chrono::Local::now().format("%H:%M:%S%.3f"));
    }
}

/// Starts the log of a new reading (call before [`scan`]): a header line,
/// no old page copies.
pub fn trace_start(what: &str) {
    let _ = std::fs::remove_dir_all(debug_pages());
    trace(&format!("==== {what} ({})", chrono::Local::now().format("%Y-%m-%d")));
    trace(&format!("Seitenkopien in {}", debug_pages().display()));
}

/// Keeps a copy of a page for the log; returns its file name.
fn dump(html: &str, url: &Url) -> String {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    if cfg!(test) {
        return String::new();
    }
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tail: String = url.as_str().rsplit('/').next().unwrap_or_default().chars().take(60).collect();
    let name = format!("{n:03}-{}.html", safe_segment(&tail).replace([' ', '?', '&', '='], "_"));
    let dir = debug_pages();
    if std::fs::create_dir_all(&dir).is_ok() {
        let _ = std::fs::write(dir.join(&name), html);
    }
    name
}

/// Only the names of the cookies in a `Cookie` header.
pub fn cookie_names(header: &str) -> String {
    let names: Vec<&str> =
        header.split(';').filter_map(|c| c.split('=').next()).map(str::trim).filter(|n| !n.is_empty()).collect();
    if names.is_empty() { "keine".to_owned() } else { names.join(", ") }
}

/// A name from ILIAS as one path segment: no slashes or control
/// characters, not "." or "..", no leading dots (hidden files), at most
/// 120 characters.
pub fn safe_segment(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            c if c.is_whitespace() => ' ',
            '/' | '\\' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let cleaned = cleaned.trim_start_matches('.').trim();
    let mut out: String = cleaned.chars().take(120).collect();
    out = out.trim_end().to_owned();
    if out.is_empty() { "_".to_owned() } else { out }
}

/// Folder for a file below `root`: one segment per course, group, session
/// or folder on the way (`segments`), created if needed. Stays below
/// `root` whatever the names are.
pub fn target_dir(root: &Path, segments: &[String]) -> Result<PathBuf> {
    if root.as_os_str().is_empty() {
        bail!("Kein Download-Ordner für ILIAS gewählt");
    }
    let mut dir = root.to_path_buf();
    for s in segments {
        dir.push(safe_segment(s));
    }
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// File name for a download: the name the server suggests, else the ILIAS
/// title; made safe as a single segment.
pub fn file_name(suggested: &str, title: &str) -> String {
    let base = suggested.rsplit(['/', '\\']).next().unwrap_or_default();
    safe_segment(if base.trim().is_empty() { title } else { base })
}

// ---------------------------------------------------------------- the tree

/// An ILIAS object: course (`crs`), group (`grp`), session (`sess`),
/// folder (`fold`) or file (`file`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub url: String,
    /// What ILIAS shows about it (type, size, version, date): a changed
    /// text means a new version.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub info: String,
    /// Files: the address that sends the file itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download: Option<String>,
    /// Containers: what is inside (None for files).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<Node>>,
    /// Containers that could not be read.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub error: String,
}

impl Node {
    fn key(&self) -> String {
        format!("{}{}", self.kind, self.id)
    }
    fn is_container(&self) -> bool {
        self.kind != "file"
    }
}

/// Logged in on page `at`: it links to the logout (ILIAS 7: `logout.php`,
/// 8 and later: `cmd=doLogout`, in any case); else it must not be a login
/// page (address `login.php` or Shibboleth, a password field, or a link to
/// log in – ILIAS shows one to visitors).
pub fn logged_in(html: &str, at: &Url) -> bool {
    login_check(&Html::parse_document(html), at).0
}

/// [`logged_in`] with the reason, for the log.
fn login_check(doc: &Html, at: &Url) -> (bool, String) {
    let targets = Selector::parse("a[href], form[action]").unwrap();
    let addresses: Vec<String> = doc
        .select(&targets)
        .filter_map(|e| e.value().attr("href").or(e.value().attr("action")))
        .map(str::to_lowercase)
        .collect();
    if let Some(a) = addresses.iter().find(|a| a.contains("logout")) {
        return (true, format!("Abmelde-Link {a}"));
    }
    let login = |a: &str| a.contains("login.php") || a.contains("shib_login") || a.contains("cmd=showlogin");
    if login(&at.as_str().to_lowercase()) {
        return (false, "Adresse ist eine Anmeldeseite".into());
    }
    let password = Selector::parse(r#"input[type="password" i]"#).unwrap();
    if doc.select(&password).next().is_some() {
        return (false, "Seite hat ein Passwortfeld".into());
    }
    if let Some(a) = addresses.iter().find(|a| login(a)) {
        return (false, format!("Seite verlinkt die Anmeldung ({a})"));
    }
    (true, "keine Anmeldeseite (kein Abmelde-Link gefunden)".into())
}

fn page_title(doc: &Html) -> String {
    let sel = Selector::parse("title").unwrap();
    doc.select(&sel).next().map(|t| clean(&t.text().collect::<String>())).unwrap_or_default()
}

/// Kind and ID of an ILIAS object link: from permalinks (`target=fold_12`,
/// `goto_…_fold_12.html`, `/go/fold/12`) or, for other links with a
/// `ref_id`, from the type icon of the list item.
fn classify(href: &str, icon: Option<&str>) -> Option<(String, String)> {
    static PATTERNS: std::sync::OnceLock<[Regex; 3]> = std::sync::OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        [
            Regex::new(r"(?i)[?&]target=(crs|grp|sess|fold|file)_(\d+)").unwrap(),
            Regex::new(r"(?i)goto_[^/?#]*?_(crs|grp|sess|fold|file)_(\d+)").unwrap(),
            Regex::new(r"(?i)/go/(crs|grp|sess|fold|file)/(\d+)").unwrap(),
        ]
    });
    let decoded = percent_decode(href);
    for re in patterns {
        if let Some(c) = re.captures(&decoded) {
            return Some((c[1].to_lowercase(), c[2].to_owned()));
        }
    }
    let ref_id = Regex::new(r"(?i)[?&]ref_id=(\d+)").unwrap().captures(&decoded)?;
    let kind = Regex::new(r"(?i)icon_(crs|grp|sess|fold|file)\b").unwrap().captures(icon?)?;
    Some((kind[1].to_lowercase(), ref_id[1].to_owned()))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit()
        {
            let hex = |b: u8| (b as char).to_digit(16).unwrap_or(0) as u8;
            out.push(hex(bytes[i + 1]) * 16 + hex(bytes[i + 2]));
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Address that sends the file itself.
fn download_url(href: &str, id: &str, page: &Url) -> String {
    let lower = href.to_lowercase();
    if lower.contains("_download")
        || lower.contains("/download")
        || lower.contains("cmd=sendfile")
        || lower.contains("cmd=download")
    {
        return href.to_owned();
    }
    if let Some(m) = Regex::new(r"(?i)(target=file_\d+)").unwrap().find(href) {
        return format!("{}_download{}", &href[..m.end()], &href[m.end()..]);
    }
    page.join(&format!("goto.php?target=file_{id}_download")).map(|u| u.to_string()).unwrap_or_default()
}

fn clean(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Classes and IDs of navigation, tabs and breadcrumbs: their links lead
/// back up or sideways, not into the container.
fn in_navigation(a: ElementRef) -> bool {
    a.ancestors().filter_map(ElementRef::wrap).any(|e| {
        let el = e.value();
        matches!(el.name(), "nav" | "header")
            || el.id() == Some("mainspacekeeper")
            || el
                .classes()
                .any(|c| matches!(c, "breadcrumb" | "il-breadcrumbs" | "ilTabs" | "il-maincontrols" | "ilLocator"))
    })
}

/// The list item a link belongs to (title, icon and properties are there).
fn list_item(a: ElementRef) -> Option<ElementRef> {
    a.ancestors().filter_map(ElementRef::wrap).find(|e| {
        let el = e.value();
        matches!(el.name(), "li" | "tr")
            || el.classes().any(|c| matches!(c, "il_ContainerListItem" | "il-std-item" | "il-item" | "ilObjListRow"))
    })
}

/// The ILIAS objects linked in the main content of a page, in order and
/// each once, with name and info.
pub fn objects(html: &str, page: &Url) -> Vec<Node> {
    let doc = Html::parse_document(html);
    let main = Selector::parse("#il_center_col, main, [role=main]").unwrap();
    let links = Selector::parse("a[href]").unwrap();
    let icon_sel = Selector::parse(r#"img[src*="icon_"]"#).unwrap();
    let title_sel = Selector::parse(".il_ContainerItemTitle, .il-item-title").unwrap();
    let props_sel = Selector::parse(".il_ItemProperties, .il-item-properties").unwrap();
    let root = doc.select(&main).next().unwrap_or_else(|| doc.root_element());
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for a in root.select(&links) {
        if in_navigation(a) {
            continue;
        }
        let Some(href) = a.value().attr("href").and_then(|h| page.join(h).ok()) else { continue };
        if href.host_str() != page.host_str() {
            continue;
        }
        let item = list_item(a);
        let icon = item.and_then(|i| i.select(&icon_sel).next()).and_then(|i| i.value().attr("src"));
        let Some((kind, id)) = classify(href.as_str(), icon) else { continue };
        let title = item.and_then(|i| i.select(&title_sel).next()).map(|t| t.text().collect::<String>());
        let name = clean(&title.unwrap_or_else(|| a.text().collect()));
        let name = if name.is_empty() { clean(a.value().attr("title").unwrap_or_default()) } else { name };
        if name.is_empty() || !seen.insert(format!("{kind}{id}")) {
            continue;
        }
        let info = item
            .map(|i| {
                i.select(&props_sel)
                    .map(|p| clean(&p.text().collect::<Vec<_>>().join(" ")))
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
            .unwrap_or_default();
        let href = href.to_string();
        out.push(Node {
            download: (kind == "file").then(|| download_url(&href, &id, page)),
            children: None,
            kind,
            id,
            name,
            url: href,
            info,
            error: String::new(),
        });
    }
    out
}

// --------------------------------------------------------------- the web

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
    base: Url,
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
    fn get(&self, url: &str) -> Result<(ureq::http::Response<ureq::Body>, Url), IliasError> {
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
                // A signal (e.g. a finished terminal program) interrupted the read.
                Err(ureq::Error::Io(e)) if e.kind() == std::io::ErrorKind::Interrupted && attempts < 4 => continue,
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

/// Progress of reading: pages read so far and what is read now.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub pages: usize,
    pub text: String,
}

/// Dashboard views that list the courses and groups one is a member of
/// (ILIAS 7 to 9 name them differently).
const DASHBOARDS: &[&str] = &[
    "ilias.php?baseClass=ilDashboardGUI&cmd=jumpToMemberships",
    "ilias.php?baseClass=ilmembershipoverviewgui",
    "ilias.php?baseClass=ilDashboardGUI&cmd=jumpToSelectedItems",
    "ilias.php?baseClass=ilDashboardGUI&cmd=show",
];
const MAX_DEPTH: usize = 10;
const PARALLEL: usize = 4;

/// Reads the courses and groups of the dashboard with everything in them
/// (groups, sessions, folders, files), several pages at a time. Links to
/// other courses inside a course are not followed.
pub fn scan(client: &Client, progress: &(dyn Fn(Progress) + Sync)) -> Result<Vec<Node>, IliasError> {
    let mut courses = Vec::new();
    let mut seen = HashSet::new();
    for d in DASHBOARDS {
        let url = client.base.join(d).map_err(|e| IliasError::Other(e.to_string()))?;
        let (html, at) = match client.page(url.as_str()) {
            Ok(p) => p,
            Err(IliasError::Login) => {
                trace("Abbruch: nicht angemeldet");
                return Err(IliasError::Login);
            }
            Err(e) => {
                trace(&format!("  Dashboard übersprungen: {e}"));
                continue;
            }
        };
        let found = objects(&html, &at);
        trace(&format!("  Dashboard: {} (Kopie: {})", summary(&found), dump(&html, &at)));
        for o in found {
            if (o.kind == "crs" || o.kind == "grp") && seen.insert(o.key()) {
                courses.push(o);
            }
        }
    }
    trace(&format!(
        "Kurse und Gruppen: {}",
        if courses.is_empty() {
            "keine".to_owned()
        } else {
            courses.iter().map(|c| format!("{} ({})", c.name, c.key())).collect::<Vec<_>>().join(" · ")
        }
    ));
    progress(Progress { pages: 0, text: format!("{} Kurse", courses.len()) });

    struct Work {
        queue: VecDeque<(String, String, usize, String)>,
        children: HashMap<String, Vec<Node>>,
        errors: HashMap<String, String>,
        visited: HashSet<String>,
        active: usize,
        pages: usize,
        login: bool,
    }
    let work = Mutex::new(Work {
        queue: courses.iter().map(|c| (c.key(), c.url.clone(), 0, c.name.clone())).collect(),
        children: HashMap::new(),
        errors: HashMap::new(),
        visited: courses.iter().map(Node::key).collect(),
        active: 0,
        pages: 0,
        login: false,
    });
    let wake = Condvar::new();
    std::thread::scope(|s| {
        for _ in 0..PARALLEL {
            s.spawn(|| {
                loop {
                    let task = {
                        let mut w = work.lock().unwrap();
                        loop {
                            if w.login {
                                return;
                            }
                            if let Some(t) = w.queue.pop_front() {
                                w.active += 1;
                                break t;
                            }
                            if w.active == 0 {
                                return;
                            }
                            w = wake.wait(w).unwrap();
                        }
                    };
                    let (key, url, depth, name) = task;
                    let result = client.page(&url);
                    let mut w = work.lock().unwrap();
                    w.active -= 1;
                    match result {
                        Ok((html, at)) => {
                            w.pages += 1;
                            let pages = w.pages;
                            let found: Vec<Node> =
                                objects(&html, &at).into_iter().filter(|o| o.kind != "crs" && o.key() != key).collect();
                            let mut kept = Vec::new();
                            for o in found {
                                if o.is_container() {
                                    // Each container once (it may be linked twice).
                                    if !w.visited.insert(o.key()) {
                                        continue;
                                    }
                                    if depth < MAX_DEPTH {
                                        w.queue.push_back((o.key(), o.url.clone(), depth + 1, o.name.clone()));
                                    }
                                }
                                kept.push(o);
                            }
                            w.children.insert(key, kept);
                            if pages.is_multiple_of(5) {
                                progress(Progress { pages, text: name });
                            }
                        }
                        Err(IliasError::Login) => w.login = true,
                        Err(IliasError::Other(e)) => {
                            w.errors.insert(key, e);
                        }
                    }
                    wake.notify_all();
                }
            });
        }
    });
    let mut w = work.into_inner().unwrap();
    if w.login {
        trace("Abbruch: Sitzung unterwegs verloren");
        return Err(IliasError::Login);
    }
    trace(&format!("Fertig: {} Seiten, {} Fehler", w.pages, w.errors.len()));
    fn build(node: &mut Node, w: &mut Work) {
        if !node.is_container() {
            return;
        }
        let key = node.key();
        if let Some(e) = w.errors.remove(&key) {
            node.error = e;
        }
        let mut children = w.children.remove(&key).unwrap_or_default();
        for c in &mut children {
            build(c, w);
        }
        node.children = Some(children);
    }
    for c in &mut courses {
        build(c, &mut w);
    }
    Ok(courses)
}

/// What was found on a page, by kind (for the log).
fn summary(found: &[Node]) -> String {
    if found.is_empty() {
        return "nichts gefunden".to_owned();
    }
    let mut counts: Vec<(String, usize)> = Vec::new();
    for o in found {
        match counts.iter_mut().find(|(k, _)| *k == o.kind) {
            Some((_, n)) => *n += 1,
            None => counts.push((o.kind.clone(), 1)),
        }
    }
    counts.iter().map(|(k, n)| format!("{n}× {k}")).collect::<Vec<_>>().join(", ")
}

/// File name from a `Content-Disposition` header (RFC 6266/5987).
pub fn disposition_name(header: &str) -> Option<String> {
    let star = Regex::new(r#"(?i)filename\*\s*=\s*(?:UTF-8|ISO-8859-1)?'[^']*'([^;]+)"#).unwrap();
    if let Some(c) = star.captures(header) {
        return Some(percent_decode(c[1].trim()));
    }
    let plain = Regex::new(r#"(?i)filename\s*=\s*(?:"([^"]*)"|([^;]+))"#).unwrap();
    let c = plain.captures(header)?;
    Some(c.get(1).or(c.get(2))?.as_str().trim().to_owned())
}

/// Downloads `url` into `dir` and returns the file: named as the server
/// says, else `title`; an older file there is replaced only when the new
/// one is complete. HTML instead of a file means the session is gone, or
/// ILIAS shows an info page.
pub fn download(
    client: &Client,
    url: &str,
    dir: &Path,
    title: &str,
    progress: &dyn Fn(u64, Option<u64>),
) -> Result<PathBuf, IliasError> {
    let io = |e: std::io::Error| IliasError::Other(e.to_string());
    trace(&format!("Download {url}"));
    let (mut res, at) = client.get(url)?;
    let content_type = res.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_lowercase();
    let disposition = res.headers().get("content-disposition").and_then(|v| v.to_str().ok()).unwrap_or("").to_owned();
    trace(&format!("  {} · {content_type} · {disposition}", res.status().as_u16()));
    if content_type.starts_with("text/html") && !disposition.to_lowercase().contains("attachment") {
        let html = res.body_mut().read_to_string().unwrap_or_default();
        let (inside, why) = login_check(&Html::parse_document(&html), &at);
        trace(&format!(
            "  HTML statt Datei von {at}: {}: {why} (Kopie: {})",
            if inside { "angemeldet" } else { "NICHT angemeldet" },
            dump(&html, &at)
        ));
        if at.host_str() != client.base.host_str() || !inside {
            return Err(IliasError::Login);
        }
        return Err(IliasError::Other(format!("ILIAS hat für „{title}“ keine Datei geliefert")));
    }
    let suggested = disposition_name(&disposition).unwrap_or_default();
    let name = file_name(&suggested, title);
    let total = res.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
    std::fs::create_dir_all(dir).map_err(io)?;
    let target = dir.join(&name);
    let part = dir.join(format!(".{name}.part"));
    let mut file = std::fs::File::create(&part).map_err(io)?;
    let mut body = res.body_mut().with_config().limit(u64::MAX).reader();
    let mut buf = vec![0u8; 64 * 1024];
    let mut received = 0u64;
    loop {
        let n = match body.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                let _ = std::fs::remove_file(&part);
                return Err(io(e));
            }
        };
        file.write_all(&buf[..n]).map_err(io)?;
        received += n as u64;
        progress(received, total);
    }
    file.sync_all().map_err(io)?;
    drop(file);
    std::fs::rename(&part, &target).map_err(io)?;
    trace(&format!("  gespeichert: {} ({received} Bytes)", target.display()));
    Ok(target)
}

/// Remembers a download in `downloads.json`: url → {path, info, time}.
pub fn record_download(url: &str, path: &Path, info: &str) -> Result<()> {
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().unwrap();
    let mut all: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&load("downloads")).unwrap_or_default();
    let time =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    all.insert(url.to_owned(), serde_json::json!({ "path": path, "info": info, "time": time }));
    save("downloads", &serde_json::Value::Object(all).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_become_safe_segments() {
        assert_eq!(safe_segment("Höhere Mathematik I / WS 26"), "Höhere Mathematik I _ WS 26");
        assert_eq!(safe_segment(".."), "_");
        assert_eq!(safe_segment("../../etc"), "_.._etc");
        assert_eq!(safe_segment("  Folien\t\n1  "), "Folien 1");
        assert_eq!(safe_segment(".versteckt"), "versteckt");
        assert_eq!(safe_segment(&"x".repeat(300)).len(), 120);
        assert_eq!(file_name("vl01.pdf", "Vorlesung 1"), "vl01.pdf");
        assert_eq!(file_name("", "Vorlesung 1"), "Vorlesung 1");
        assert_eq!(file_name("../../evil.sh", "x"), "evil.sh");
    }

    #[test]
    fn target_dir_stays_below_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ILIAS");
        let t = target_dir(&root, &["Kurs A".into(), "..".into(), "Ordner/1".into()]).unwrap();
        assert_eq!(t, root.join("Kurs A").join("_").join("Ordner_1"));
        assert!(t.is_dir() && t.starts_with(&root));
        assert!(target_dir(Path::new(""), &[]).is_err());
    }

    // A small ILIAS: without the cookie `sid=1` every page sends to the
    // login; a course with a folder, a session (ref_id link, recognisable
    // by its icon) and files; a group; navigation and breadcrumbs that
    // link back up.
    struct Mock {
        base: String,
        version: std::sync::Arc<std::sync::atomic::AtomicU32>,
    }

    fn item(kind: &str, id: &str, name: &str, version: u32) -> String {
        let (href, icon, props) = match kind {
            "file" => (
                format!("/goto.php?target=file_{id}_download&amp;client_id=test"),
                "file",
                format!(
                    r#"<div class="il_ItemProperties"><span class="il_ItemProperty">pdf</span> <span class="il_ItemProperty">Version: {version}</span></div>"#
                ),
            ),
            "sess" => (format!("/ilias.php?ref_id={id}&amp;cmdClass=ilobjsessiongui"), "sess", String::new()),
            k => (format!("/goto.php?target={k}_{id}&amp;client_id=test"), k, String::new()),
        };
        format!(
            r#"<div class="il_ContainerListItem"><img src="/images/icon_{icon}.svg"><div class="il_ContainerItemTitle"><a href="{href}">{name}</a></div>{props}</div>"#
        )
    }

    fn page_html(title: &str, items: &str) -> String {
        format!(
            r#"<html><body><nav class="il-maincontrols"><a href="/logout.php">Abmelden</a> <a href="/goto.php?target=crs_100">Mathe</a></nav><ol class="breadcrumb"><li><a href="/goto.php?target=crs_100">Mathe</a></li></ol><div id="il_center_col"><h1>{title}</h1>{items}</div></body></html>"#
        )
    }

    fn respond(path: &str, cookie: &str, version: u32) -> (u16, Vec<(String, String)>, Vec<u8>) {
        let html = |s: String| (200, vec![("Content-Type".into(), "text/html; charset=utf-8".into())], s.into_bytes());
        if path.starts_with("/login.php") {
            return html("<html><body>Anmeldung</body></html>".into());
        }
        if !cookie.contains("sid=1") {
            return (302, vec![("Location".into(), "/login.php".into())], Vec::new());
        }
        let files = [
            ("102", "Organisatorisches", "orga.pdf"),
            ("104", "Folien", "folien 1.pdf"),
            ("106", "Blatt 2", "blatt02.pdf"),
            ("108", "Skript", ""),
        ];
        for (id, title, file) in files {
            if path.starts_with(&format!("/goto.php?target=file_{id}_download")) {
                let v = if id == "102" { version } else { 1 };
                let mut headers = vec![("Content-Type".to_owned(), "application/pdf".to_owned())];
                if !file.is_empty() {
                    headers.push((
                        "Content-Disposition".into(),
                        format!("attachment; filename*=UTF-8''{}", file.replace(' ', "%20")),
                    ));
                }
                return (200, headers, format!("{title} Version {v}").into_bytes());
            }
        }
        // A redirect that renews the session cookie: followed with it.
        if path.starts_with("/ilias.php?baseClass=ilDashboardGUI&cmd=jumpToMemberships") {
            let headers = vec![
                ("Location".to_owned(), "/ilias.php?baseClass=ilDashboardGUI&cmd=show".to_owned()),
                ("Set-Cookie".to_owned(), "sid=1; Path=/; HttpOnly".to_owned()),
            ];
            return (302, headers, Vec::new());
        }
        if path.starts_with("/ilias.php?baseClass=ilDashboardGUI")
            || path.starts_with("/ilias.php?baseClass=ilmembershipoverviewgui")
        {
            return html(page_html(
                "Dashboard",
                &(item("crs", "100", "Höhere Mathematik I", 1) + &item("grp", "200", "Tutorium 3", 1)),
            ));
        }
        let pages = [
            (
                "/goto.php?target=crs_100",
                vec![
                    item("fold", "101", "Vorlesung", 1),
                    item("file", "102", "Organisatorisches", version),
                    item("sess", "103", "Sitzung 1", 1),
                ],
            ),
            (
                "/goto.php?target=fold_101",
                vec![
                    item("file", "104", "Folien", 1),
                    item("fold", "105", "Übungen", 1),
                    item("fold", "101", "Vorlesung", 1),
                ],
            ),
            ("/goto.php?target=fold_105", vec![item("file", "106", "Blatt 2", 1)]),
            ("/ilias.php?ref_id=103", vec![item("file", "108", "Skript", 1)]),
            ("/goto.php?target=grp_200", vec![item("crs", "300", "Anderer Kurs", 1)]),
        ];
        for (p, items) in pages {
            if path.starts_with(p) {
                return html(page_html("Seite", &items.concat()));
            }
        }
        (404, Vec::new(), b"nicht gefunden".to_vec())
    }

    impl Mock {
        fn start() -> Mock {
            use std::io::BufRead;
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://{}/", listener.local_addr().unwrap());
            let version = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(1));
            let shared = version.clone();
            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    let version = shared.clone();
                    std::thread::spawn(move || -> std::io::Result<()> {
                        let mut stream = stream;
                        let mut reader = std::io::BufReader::new(stream.try_clone()?);
                        let mut line = String::new();
                        reader.read_line(&mut line)?;
                        let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
                        let mut cookie = String::new();
                        loop {
                            let mut h = String::new();
                            reader.read_line(&mut h)?;
                            if h.trim().is_empty() {
                                break;
                            }
                            if let Some((n, v)) = h.split_once(':')
                                && n.eq_ignore_ascii_case("cookie")
                            {
                                cookie = v.trim().to_owned();
                            }
                        }
                        let (status, headers, body) =
                            respond(&path, &cookie, version.load(std::sync::atomic::Ordering::SeqCst));
                        let mut out =
                            format!("HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
                        for (n, v) in headers {
                            out.push_str(&format!("{n}: {v}\r\n"));
                        }
                        out.push_str("\r\n");
                        stream.write_all(out.as_bytes())?;
                        stream.write_all(&body)
                    });
                }
            });
            Mock { base, version }
        }
    }

    #[test]
    fn links_are_classified() {
        assert_eq!(classify("https://x/goto.php?target=fold_12&client_id=k", None), Some(("fold".into(), "12".into())));
        assert_eq!(classify("https://x/goto_produktiv_crs_7.html", None), Some(("crs".into(), "7".into())));
        assert_eq!(classify("https://x/go/grp/9", None), Some(("grp".into(), "9".into())));
        assert_eq!(classify("https://x/goto.php%3Ftarget%3Dfile_5_download", None), Some(("file".into(), "5".into())));
        assert_eq!(
            classify("https://x/ilias.php?ref_id=3&cmd=x", Some("/img/icon_sess.svg")),
            Some(("sess".into(), "3".into()))
        );
        assert_eq!(classify("https://x/ilias.php?ref_id=3", Some("/img/icon_tst.svg")), None);
        assert_eq!(classify("https://x/ilias.php?ref_id=3", None), None);
        assert_eq!(percent_decode("a%20b%zz%4"), "a b%zz%4");
    }

    #[test]
    fn objects_of_a_page() {
        let page = Url::parse("https://ilias.example/ilias.php?ref_id=100").unwrap();
        let html = page_html(
            "Kurs",
            &(item("fold", "101", "  Vorlesung\n ", 1)
                + &item("file", "102", "Orga", 3)
                + &item("sess", "103", "Sitzung", 1)
                + r#"<a href="https://anders.example/goto.php?target=fold_9">fremd</a><a href="/goto.php?target=fold_101">Vorlesung</a>"#),
        );
        let found = objects(&html, &page);
        let keys: Vec<_> = found.iter().map(|o| (o.key(), o.name.as_str())).collect();
        assert_eq!(keys, [("fold101".into(), "Vorlesung"), ("file102".into(), "Orga"), ("sess103".into(), "Sitzung")]);
        assert_eq!(found[1].info, "pdf Version: 3");
        assert_eq!(
            found[1].download.as_deref(),
            Some("https://ilias.example/goto.php?target=file_102_download&client_id=test")
        );
        assert_eq!(found[2].url, "https://ilias.example/ilias.php?ref_id=103&cmdClass=ilobjsessiongui");
        assert!(logged_in(&html, &page));
        let ilias9 = r#"<a href="ilias.php?baseClass=ilstartupgui&cmd=doLogout">Abmelden</a>"#;
        assert!(logged_in(ilias9, &page));
        assert!(logged_in("<html><body>Dashboard</body></html>", &page), "user menu loaded later");
        let login = Url::parse("https://ilias.example/login.php?target=&cmd=force_login").unwrap();
        assert!(!logged_in("<html><body>Anmeldung</body></html>", &login));
        assert!(!logged_in(r#"<a href="/shib_login.php?target=">Anmelden</a>"#, &page));
        assert!(!logged_in(r#"<form><input type="password" name="pw"></form>"#, &page));
        assert_eq!(
            download_url("https://i.ex/goto.php?target=file_7&client_id=k", "7", &page),
            "https://i.ex/goto.php?target=file_7_download&client_id=k"
        );
        assert_eq!(download_url("https://i.ex/x", "7", &page), "https://ilias.example/goto.php?target=file_7_download");
    }

    #[test]
    fn disposition_names() {
        assert_eq!(disposition_name(r#"attachment; filename="vl 01.pdf""#).as_deref(), Some("vl 01.pdf"));
        assert_eq!(disposition_name("attachment; filename=a.pdf; size=3").as_deref(), Some("a.pdf"));
        assert_eq!(
            disposition_name(r#"attachment; filename="Ubung.pdf"; filename*=UTF-8''%C3%9Cbung.pdf"#).as_deref(),
            Some("Übung.pdf")
        );
        assert_eq!(disposition_name("inline"), None);
    }

    #[test]
    fn scan_needs_a_session() {
        let mock = Mock::start();
        let client = Client::new(&mock.base, "", "").unwrap();
        assert_eq!(scan(&client, &|_| {}), Err(IliasError::Login));
        let dir = tempfile::tempdir().unwrap();
        let url = format!("{}goto.php?target=file_102_download", mock.base);
        assert_eq!(download(&client, &url, dir.path(), "Orga", &|_, _| {}), Err(IliasError::Login));
    }

    #[test]
    fn scan_reads_the_tree_and_downloads() {
        let mock = Mock::start();
        let client = Client::new(&mock.base, "lang=de; sid=1", "Mozilla/5.0 Test").unwrap();
        let messages = Mutex::new(Vec::new());
        let tree = scan(&client, &|p| messages.lock().unwrap().push(p.text)).unwrap();
        assert_eq!(messages.lock().unwrap()[0], "2 Kurse");

        fn outline(nodes: &[Node], depth: usize, out: &mut Vec<String>) {
            for n in nodes {
                out.push(format!("{}{} {}", " ".repeat(depth), n.key(), n.name));
                outline(n.children.as_deref().unwrap_or_default(), depth + 1, out);
            }
        }
        let mut lines = Vec::new();
        outline(&tree, 0, &mut lines);
        assert_eq!(
            lines,
            [
                "crs100 Höhere Mathematik I",
                " fold101 Vorlesung",
                "  file104 Folien",
                "  fold105 Übungen",
                "   file106 Blatt 2",
                " file102 Organisatorisches",
                " sess103 Sitzung 1",
                "  file108 Skript",
                "grp200 Tutorium 3",
            ]
        );
        assert_eq!(tree[1].children, Some(Vec::new()), "courses in a group are not followed");
        assert!(tree[0].children.as_ref().unwrap()[1].children.is_none(), "files have no children");
        let json = serde_json::to_string(&tree).unwrap();
        assert_eq!(serde_json::from_str::<Vec<Node>>(&json).unwrap(), tree);

        let dir = tempfile::tempdir().unwrap();
        let orga = &tree[0].children.as_ref().unwrap()[1];
        let seen = Mutex::new(0u64);
        let path = download(&client, orga.download.as_deref().unwrap(), dir.path(), &orga.name, &|n, total| {
            assert_eq!(total, Some(27));
            *seen.lock().unwrap() = n;
        })
        .unwrap();
        assert_eq!(path, dir.path().join("orga.pdf"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "Organisatorisches Version 1");
        assert_eq!(*seen.lock().unwrap(), 27);

        // A new version replaces the file; no partial file stays behind.
        mock.version.store(2, std::sync::atomic::Ordering::SeqCst);
        download(&client, orga.download.as_deref().unwrap(), dir.path(), &orga.name, &|_, _| {}).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "Organisatorisches Version 2");
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, ["orga.pdf"]);

        // Names from the header (percent-encoded) or else the title.
        let folien = format!("{}goto.php?target=file_104_download", mock.base);
        assert_eq!(
            download(&client, &folien, dir.path(), "Folien", &|_, _| {}).unwrap(),
            dir.path().join("folien 1.pdf")
        );
        let skript = format!("{}goto.php?target=file_108_download", mock.base);
        assert_eq!(download(&client, &skript, dir.path(), "Skript", &|_, _| {}).unwrap(), dir.path().join("Skript"));
        // An ILIAS page instead of a file.
        let page = format!("{}goto.php?target=crs_100", mock.base);
        assert!(matches!(download(&client, &page, dir.path(), "Kurs", &|_, _| {}), Err(IliasError::Other(_))));
    }
}
