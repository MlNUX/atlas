// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! ILIAS (e.g. KIT): the courses of the dashboard as folders.
//!
//! Atlas reads the course tree through a logged-in web session (in the
//! app, see `IliasService.qml`) and keeps two JSON files in the state
//! directory: the tree (`index.json`) and what was downloaded where
//! (`downloads.json`). Here: where downloads go and how ILIAS names
//! become safe folder and file names.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

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
}
