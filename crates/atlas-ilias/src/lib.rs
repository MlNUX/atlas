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
//!
//! - `trace`: the ILIAS log and copies of pages for it
//! - `session`: the web view's cookies for ILIAS
//! - `client`: HTTP with that session (redirects, login detection)
//! - `parse`: what a page says (logged in? which objects?)
//! - `scan`: reading the favourites, several pages at a time
//! - `download`: target folders, file names, the download itself

use std::path::PathBuf;

use anyhow::{Result, bail};
pub use url::Url;

mod client;
mod download;
mod parse;
mod scan;
mod session;
mod trace;

pub use client::*;
pub use download::*;
pub use parse::*;
pub use scan::*;
pub use session::*;
pub use trace::*;

/// State files of the ILIAS view.
pub fn state_dir() -> PathBuf {
    atlas_core::paths::state_root().join("ilias")
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
    atlas_core::paths::write_atomic(&path, json.as_bytes())
}

#[cfg(test)]
mod tests;
