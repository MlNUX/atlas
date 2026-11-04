// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Downloads: target folders and names, the file itself, what was saved where.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use anyhow::{Result, bail};
use regex::Regex;
use scraper::Html;

use super::*;

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

static DISPOSITION_STAR: LazyLock<Regex> =
    LazyLock::new(|| regex(r#"(?i)filename\*\s*=\s*(?:UTF-8|ISO-8859-1)?'[^']*'([^;]+)"#));
static DISPOSITION_PLAIN: LazyLock<Regex> = LazyLock::new(|| regex(r#"(?i)filename\s*=\s*(?:"([^"]*)"|([^;]+))"#));

/// File name from a `Content-Disposition` header (RFC 6266/5987).
pub fn disposition_name(header: &str) -> Option<String> {
    if let Some(c) = DISPOSITION_STAR.captures(header) {
        return Some(atlas_core::text::percent_decode(c[1].trim()));
    }
    let c = DISPOSITION_PLAIN.captures(header)?;
    Some(c.get(1).or(c.get(2))?.as_str().trim().to_owned())
}

/// Downloads `url` into `dir` and returns the file: named as the server
/// says, else `title` (numbered if the name is taken); `previous` (its
/// earlier download) is replaced, only once the new one is complete. HTML instead of a file means the session is gone, or
/// ILIAS shows an info page.
pub fn download(
    client: &Client,
    url: &str,
    dir: &Path,
    title: &str,
    previous: Option<&Path>,
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
    let total = res.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
    std::fs::create_dir_all(dir).map_err(io)?;
    // A new version replaces the file's earlier download; any other file
    // of that name (another course's, the user's own) stays.
    let target = match previous.filter(|p| p.parent() == Some(dir)) {
        Some(p) => p.to_path_buf(),
        None => free_name(dir, &file_name(&suggested, title)),
    };
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
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

/// `dir/name`, or with a number before the extension (`Blatt (2).pdf`) if
/// that exists.
pub(super) fn free_name(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, format!(".{ext}")),
        _ => (name, String::new()),
    };
    (2..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|p| !p.exists()).unwrap_or(first)
}

/// Where file `url` was downloaded to before (`downloads.json`).
pub fn previous_download(url: &str) -> Option<PathBuf> {
    let all: serde_json::Value = serde_json::from_str(&load("downloads")).ok()?;
    all.get(url)?.get("path")?.as_str().map(PathBuf::from)
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
