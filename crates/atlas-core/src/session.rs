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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppState {
    #[serde(default)]
    pub version: u32,
    pub last_project: Option<PathBuf>,
}

impl AppState {
    pub fn load() -> Self {
        read_json(&paths::app_state_file()).unwrap_or_default()
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
