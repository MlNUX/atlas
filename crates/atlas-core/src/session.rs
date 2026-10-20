// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Session data: device wide [`AppState`] (XDG state directory) and the
//! per project [`ProjectSession`] (stored by [`crate::project::Project`] in
//! `.atlas/sessions/<device-id>/active.json`).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths;

pub const FORMAT_VERSION: u32 = 1;

/// Projects kept in [`AppState::projects`].
const MAX_PROJECTS: usize = 30;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppState {
    #[serde(default)]
    pub version: u32,
    pub last_project: Option<PathBuf>,
    /// Opened projects, most recent first (for the sessions of all
    /// projects).
    #[serde(default)]
    pub projects: Vec<PathBuf>,
}

impl AppState {
    pub fn load() -> Self {
        read_json(&paths::app_state_file()).unwrap_or_default()
    }

    /// Makes `root` the last and most recently used project.
    pub fn remember(&mut self, root: &Path) {
        self.last_project = Some(root.to_owned());
        self.projects.retain(|p| p != root);
        self.projects.insert(0, root.to_owned());
        self.projects.truncate(MAX_PROJECTS);
    }

    /// Takes `root` off the list of projects.
    pub fn forget(&mut self, root: &Path) {
        self.projects.retain(|p| p != root);
        if self.last_project.as_deref() == Some(root) {
            self.last_project = self.projects.first().cloned();
        }
    }

    pub fn save(&mut self) -> Result<()> {
        self.version = FORMAT_VERSION;
        let data = serde_json::to_vec_pretty(self).context("serialize app state")?;
        paths::write_atomic(&paths::app_state_file(), &data)
    }
}

/// The active session of one device in one project. All paths are relative
/// to the project root.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectSession {
    #[serde(default)]
    pub version: u32,
    /// Display name of the session ("Standard", "Literaturrecherche", …).
    #[serde(default)]
    pub name: String,
    /// Directories expanded in the project tree.
    #[serde(default)]
    pub expanded: Vec<PathBuf>,
    /// Documents referenced by `layout`; their identities live in
    /// `metadata.sqlite`.
    #[serde(default)]
    pub documents: Vec<PathBuf>,
    /// Opaque layout description owned by the UI (panes, splits, zoom …).
    #[serde(default)]
    pub layout: serde_json::Value,
}

/// What a session shows, for the session list ("3 Tabs · 2 Splits").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionSummary {
    pub tabs: usize,
    pub splits: usize,
    /// Prevailing kind of its documents: "pdf", "notes", "code",
    /// "terminal", "calendar", "mixed" or "empty".
    pub kind: &'static str,
}

impl ProjectSession {
    pub fn summary(&self) -> SessionSummary {
        let editor = &self.layout["editor"];
        // Phase 0 layouts had one document per split ("panes").
        let groups: Vec<Vec<&str>> = match (editor["groups"].as_array(), editor["panes"].as_array()) {
            (Some(groups), _) => groups
                .iter()
                .map(|g| {
                    g["tabs"]
                        .as_array()
                        .map(|t| t.iter().filter_map(|t| t["path"].as_str()).filter(|p| !p.is_empty()).collect())
                        .unwrap_or_default()
                })
                .collect(),
            (None, Some(panes)) => panes.iter().filter_map(|p| p["path"].as_str()).map(|p| vec![p]).collect(),
            _ => Vec::new(),
        };
        let paths: Vec<&str> = groups.iter().flatten().copied().collect();
        let mut counts = std::collections::BTreeMap::<&'static str, usize>::new();
        for p in &paths {
            let ext = p.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
            let kind = if p.starts_with("terminal:") {
                "terminal"
            } else if p.starts_with("calendar:") {
                "calendar"
            } else if ext == "pdf" {
                "pdf"
            } else if matches!(ext.as_str(), "md" | "markdown" | "txt") {
                "notes"
            } else {
                "code"
            };
            *counts.entry(kind).or_default() += 1;
        }
        let kind = match counts.iter().max_by_key(|(_, n)| **n) {
            None => "empty",
            Some((kind, n)) if *n * 2 > paths.len() => kind,
            Some(_) => "mixed",
        };
        SessionSummary { tabs: paths.len(), splits: groups.iter().filter(|g| !g.is_empty()).count(), kind }
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let data = std::fs::read(path).ok()?;
    match serde_json::from_slice(&data) {
        Ok(v) => Some(v),
        Err(err) => {
            log::warn!("ignoring unreadable state file {}: {err}", path.display());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_projects_most_recent_first() {
        let mut state: AppState = serde_json::from_str(r#"{"version": 1, "last_project": "/a"}"#).unwrap();
        assert!(state.projects.is_empty());
        for p in ["/a", "/b", "/a", "/c"] {
            state.remember(Path::new(p));
        }
        assert_eq!(state.projects, ["/c", "/a", "/b"].map(PathBuf::from));
        assert_eq!(state.last_project.as_deref(), Some(Path::new("/c")));
        for i in 0..40 {
            state.remember(&PathBuf::from(format!("/p{i}")));
        }
        assert_eq!(state.projects.len(), MAX_PROJECTS);
        assert_eq!(state.projects[0], PathBuf::from("/p39"));
        state.forget(Path::new("/p39"));
        assert_eq!(state.projects[0], PathBuf::from("/p38"));
        assert_eq!(state.last_project.as_deref(), Some(Path::new("/p38")));
    }

    #[test]
    fn summary_counts_tabs_splits_and_kind() {
        let session = |layout: serde_json::Value| ProjectSession { layout, ..Default::default() };
        let s = session(serde_json::json!({ "editor": { "groups": [
            { "tabs": [{ "path": "docs/a.pdf" }, { "path": "docs/b.pdf" }] },
            { "tabs": [{ "path": "notes/n.md" }] },
        ] } }));
        assert_eq!(s.summary(), SessionSummary { tabs: 3, splits: 2, kind: "pdf" });
        let s = session(
            serde_json::json!({ "editor": { "groups": [{ "tabs": [{ "path": "src/main.rs" }, { "path": "notes/n.md" }] }] } }),
        );
        assert_eq!(s.summary().kind, "mixed");
        assert_eq!(
            session(serde_json::json!({ "editor": { "groups": [{ "tabs": [] }] } })).summary(),
            SessionSummary { tabs: 0, splits: 0, kind: "empty" }
        );
        // Phase 0: one document per pane.
        let s = session(serde_json::json!({ "editor": { "panes": [{ "path": "a.rs" }, { "path": "b.rs" }] } }));
        assert_eq!(s.summary(), SessionSummary { tabs: 2, splits: 2, kind: "code" });
    }
}
