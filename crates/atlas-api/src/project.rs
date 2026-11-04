// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! One open project (one window, E9): identity, index state, sessions
//! (tabs and splits, exchangeable with Linux, E12), per-document view
//! state, and the per-project services.

use std::path::PathBuf;
use std::sync::Arc;

use crate::events::EventSink;
use crate::{Result, todo_task};

/// Short form of a session for menus.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SessionSummary {
    pub id: String,
    pub name: String,
    pub open_files: u32,
    /// Unix seconds.
    pub last_used: i64,
    pub active: bool,
}

/// State of the project index (files, notes, links).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct IndexState {
    pub ready: bool,
    pub text: String,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Project {
    root: PathBuf,
    #[allow(dead_code)]
    sink: Arc<dyn EventSink>,
}

impl Project {
    #[allow(dead_code)]
    pub(crate) fn new(root: PathBuf, sink: Arc<dyn EventSink>) -> Self {
        Self { root, sink }
    }
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Project {
    pub fn root(&self) -> String {
        crate::util::string(&self.root)
    }

    /// Display name (folder name unless set in `.atlas/project.toml`).
    pub fn name(&self) -> String {
        todo_task!("P2.2")
    }

    /// `<root>/.atlas`.
    pub fn atlas_dir(&self) -> String {
        crate::util::string(&self.root.join(".atlas"))
    }

    /// Why `.atlas/` cannot be written, if so (read-only media …); the
    /// project then works without saving sessions and annotations.
    pub fn storage_error(&self) -> Option<String> {
        todo_task!("P2.2")
    }

    pub fn index_state(&self) -> IndexState {
        todo_task!("P2.2")
    }

    /// Path relative to the root (`docs/a.md`), `None` if outside.
    pub fn relative_path(&self, path: String) -> Option<String> {
        let _ = path;
        todo_task!("P2.2")
    }

    pub fn absolute_path(&self, relative: String) -> String {
        crate::util::string(&self.root.join(relative.trim_start_matches('/')))
    }

    // --- sessions ---

    pub fn sessions(&self) -> Vec<SessionSummary> {
        todo_task!("P2.2")
    }

    /// Layout of the active session as Atlas' session JSON (tabs, splits,
    /// sizes; the same file format on Linux and macOS, E12). The UI parses
    /// it into its own model; the format is described in `docs/data-format.md`.
    pub fn session_layout(&self) -> String {
        todo_task!("P2.2")
    }

    /// Saves the layout of the active session (debounced on disk).
    pub fn save_session_layout(&self, layout: String) {
        let _ = layout;
        todo_task!("P2.2")
    }

    /// Writes pending session changes now (quit, window close).
    pub fn flush_session(&self) {
        todo_task!("P2.2")
    }

    pub fn switch_session(&self, id: String) -> Result<()> {
        let _ = id;
        todo_task!("P2.2")
    }

    /// New session, empty or as a copy of the active one; returns its id.
    pub fn create_session(&self, name: String, copy_current: bool) -> Result<String> {
        let _ = (name, copy_current);
        todo_task!("P2.2")
    }

    pub fn rename_session(&self, id: String, name: String) -> Result<()> {
        let _ = (id, name);
        todo_task!("P2.2")
    }

    pub fn delete_session(&self, id: String) -> Result<()> {
        let _ = id;
        todo_task!("P2.2")
    }

    /// Folders expanded in the file tree (part of the session).
    pub fn expanded_dirs(&self) -> Vec<String> {
        todo_task!("P2.2")
    }

    pub fn set_expanded_dirs(&self, dirs: Vec<String>) {
        let _ = dirs;
        todo_task!("P2.2")
    }

    /// Per-document view state (scroll position, zoom, cursor) as opaque
    /// JSON, stored in `.atlas/` and shared with Linux.
    pub fn document_state(&self, path: String) -> Option<String> {
        let _ = path;
        todo_task!("P2.2")
    }

    pub fn save_document_state(&self, path: String, state: String) {
        let _ = (path, state);
        todo_task!("P2.2")
    }

    // --- services ---

    pub fn files(&self) -> Arc<crate::files::Files> {
        todo_task!("P2.2")
    }

    pub fn notes(&self) -> Arc<crate::notes::Notes> {
        todo_task!("P2.3")
    }

    pub fn search(&self) -> Arc<crate::search::Search> {
        todo_task!("P2.4")
    }

    pub fn calendar(&self) -> Arc<crate::calendar::Calendar> {
        todo_task!("P2.5")
    }

    /// PDF data (annotations, text, citations) of one document.
    pub fn pdf(&self, path: String) -> Result<Arc<crate::pdf::PdfDocument>> {
        let _ = path;
        todo_task!("P2.6")
    }

    pub fn actions(&self) -> Arc<crate::actions::Actions> {
        todo_task!("P2.7")
    }

    pub fn extensions(&self) -> Arc<crate::extensions::Extensions> {
        todo_task!("P2.7")
    }

    pub fn editor(&self) -> Arc<crate::editor::Editor> {
        todo_task!("P2.8")
    }

    /// Context bar profiles of this project (falls back to the user's).
    pub fn context(&self) -> Arc<crate::settings::Context> {
        todo_task!("P2.8")
    }
}
