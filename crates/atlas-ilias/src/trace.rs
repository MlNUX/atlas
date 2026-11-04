// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The ILIAS log (`debug.log`) and copies of pages for it.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use url::Url;

use super::*;

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
pub(super) fn dump(html: &str, url: &Url) -> String {
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
