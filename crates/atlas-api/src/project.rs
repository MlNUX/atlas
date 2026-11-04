// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! One open project (one window, E9): identity, index state, sessions
//! (tabs and splits, exchangeable with Linux, E12), per-document view
//! state, and the per-project services.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use atlas_core::project::Project as CoreProject;
use atlas_core::session::ProjectSession;

use crate::inner::Shared;
use crate::util::{path, string};
use crate::{AtlasError, Result, todo_task};

/// Short form of a session for menus („3 Tabs · 2 Splits“).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SessionSummary {
    pub id: String,
    pub name: String,
    pub tabs: u32,
    pub splits: u32,
    /// Prevailing kind of its documents: `pdf`, `notes`, `code`,
    /// `terminal`, `calendar`, `mixed` or `empty`.
    pub kind: String,
    /// Unix seconds of the last save, `None` if never saved.
    pub saved: Option<i64>,
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
    shared: Arc<Shared>,
}

impl Project {
    pub(crate) fn open(root: &Path, sink: crate::events::Sink) -> Result<Self> {
        Ok(Self { shared: Shared::open(root, sink)? })
    }

    fn core(&self) -> Result<&Arc<CoreProject>> {
        self.shared.project.as_ref().ok_or_else(|| AtlasError::Unavailable {
            message: self.shared.storage_error.clone().unwrap_or_else(|| "Projektdaten nicht verfügbar".into()),
        })
    }

    fn active_session(&self) -> Option<ProjectSession> {
        let core = self.shared.project.as_ref()?;
        Some(core.load_named(&core.active_session()))
    }
}

pub(crate) fn summary_of(core: &CoreProject, id: &str, active: bool, name: String) -> SessionSummary {
    let summary = core.load_named(id).summary();
    let saved = std::fs::metadata(core.session_path(id))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64);
    SessionSummary {
        id: id.to_owned(),
        name,
        tabs: summary.tabs as u32,
        splits: summary.splits as u32,
        kind: summary.kind.to_owned(),
        saved,
        active,
    }
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Project {
    pub fn root(&self) -> String {
        string(&self.shared.root)
    }

    /// Display name (the folder name).
    pub fn name(&self) -> String {
        self.shared.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| self.root())
    }

    /// `<root>/.atlas`.
    pub fn atlas_dir(&self) -> String {
        string(&self.shared.root.join(".atlas"))
    }

    /// Why `.atlas/` cannot be written, if so (read-only media …); the
    /// project then works without saving sessions and annotations.
    pub fn storage_error(&self) -> Option<String> {
        self.shared.storage_error.clone()
    }

    pub fn index_state(&self) -> IndexState {
        self.shared.index_state.lock().unwrap().clone()
    }

    /// Path relative to the root (`docs/a.md`), `None` if outside.
    pub fn relative_path(&self, path: String) -> Option<String> {
        self.shared.relative(&crate::util::path(&path)).map(|p| string(&p))
    }

    pub fn absolute_path(&self, relative: String) -> String {
        string(&self.shared.root.join(relative.trim_start_matches('/')))
    }

    // --- sessions ---

    pub fn sessions(&self) -> Vec<SessionSummary> {
        let Some(core) = self.shared.project.as_ref() else { return Vec::new() };
        let active = core.active_session();
        core.list_sessions().into_iter().map(|s| summary_of(core, &s.id, s.id == active, s.name)).collect()
    }

    /// Layout of the active session as Atlas' session JSON (tabs, splits,
    /// sizes; the same file format on Linux and macOS, E12), empty if none.
    pub fn session_layout(&self) -> String {
        match self.active_session() {
            Some(s) if !s.layout.is_null() => s.layout.to_string(),
            _ => String::new(),
        }
    }

    /// Folders expanded in the file tree (absolute).
    pub fn session_expanded(&self) -> Vec<String> {
        let (Some(core), Some(session)) = (self.shared.project.as_ref(), self.active_session()) else {
            return Vec::new();
        };
        session.expanded.iter().filter_map(|d| core.absolute(d)).map(|p| string(&p)).collect()
    }

    /// Saves the active session (written in the background): layout JSON,
    /// expanded folders and the documents the layout refers to.
    pub fn save_session(&self, layout: String, expanded: Vec<String>, documents: Vec<String>) -> Result<()> {
        let core = self.core()?;
        let writer = self
            .shared
            .writer
            .as_ref()
            .ok_or_else(|| AtlasError::Unavailable { message: "kein Schreibzugriff".into() })?;
        let session = self.build_session(&layout, &expanded, &documents)?;
        let documents = documents.iter().filter_map(|d| self.shared.absolute(&path(d))).collect();
        writer.save(core.active_session(), session, documents);
        Ok(())
    }

    /// Writes pending session changes now (quit, window close).
    pub fn flush_session(&self) {
        if let Some(writer) = self.shared.writer.as_ref() {
            writer.flush();
        }
    }

    pub fn switch_session(&self, id: String) -> Result<()> {
        let core = self.core()?;
        self.flush_session();
        core.set_active_session(&id)?;
        Ok(())
    }

    /// New session: empty, or with this layout (copy of the current one);
    /// returns its id.
    pub fn create_session(&self, name: String, layout: Option<String>, expanded: Vec<String>) -> Result<String> {
        let core = self.core()?;
        let from = match layout {
            Some(l) if !l.is_empty() => Some(self.build_session(&l, &expanded, &[])?),
            _ => None,
        };
        Ok(core.create_session(&name, from.as_ref())?)
    }

    pub fn rename_session(&self, id: String, name: String) -> Result<()> {
        self.flush_session();
        Ok(self.core()?.rename_session(&id, &name)?)
    }

    pub fn delete_session(&self, id: String) -> Result<()> {
        Ok(self.core()?.delete_session(&id)?)
    }

    /// Where a document of the session is now: its path, or where it moved
    /// to (found by identity in the index), `None` if gone.
    pub fn resolve_document(&self, path: String) -> Option<String> {
        let p = crate::util::path(&path);
        let Some(core) = self.shared.project.as_ref() else {
            return self.shared.absolute(&p).filter(|p| p.exists()).map(|p| string(&p));
        };
        let relative = self.shared.relative(&p).unwrap_or(p);
        let metadata = self.shared.metadata.lock().unwrap();
        let index = self.shared.index.lock().unwrap();
        core.locate(metadata.as_ref(), index.as_ref(), &relative).map(|p| string(&p))
    }

    /// Per-document view state (scroll position, zoom, cursor) as opaque
    /// JSON, stored in `.atlas/` and shared with Linux.
    pub fn document_state(&self, path: String) -> Option<String> {
        let relative = self.shared.relative(&crate::util::path(&path))?;
        self.shared.metadata.lock().unwrap().as_ref()?.document_state(&relative).ok().flatten()
    }

    pub fn save_document_state(&self, path: String, state: String) {
        if let (Some(writer), Some(relative)) =
            (self.shared.writer.as_ref(), self.shared.relative(&crate::util::path(&path)))
        {
            writer.save_document_state(relative, state);
        }
    }

    // --- services ---

    pub fn files(&self) -> Arc<crate::files::Files> {
        Arc::new(crate::files::Files::new(self.shared.clone()))
    }

    pub fn notes(&self) -> Arc<crate::notes::Notes> {
        todo_task!("P2.3")
    }

    pub fn search(&self) -> Arc<crate::search::Search> {
        Arc::new(crate::search::Search::new(self.shared.clone()))
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
        Arc::new(crate::editor::Editor::new(self.shared.clone()))
    }

    /// Context bar profiles of this project (falls back to the user's).
    pub fn context(&self) -> Arc<crate::settings::Context> {
        Arc::new(crate::settings::Context::new())
    }
}

impl Project {
    fn build_session(&self, layout: &str, expanded: &[String], documents: &[String]) -> Result<ProjectSession> {
        let layout = serde_json::from_str(layout)
            .map_err(|e| AtlasError::Invalid { message: format!("Sitzungs-Layout ist kein JSON: {e}") })?;
        let relative = |p: &String| -> PathBuf {
            let p = crate::util::path(p);
            self.shared.relative(&p).unwrap_or(p)
        };
        Ok(ProjectSession {
            expanded: expanded.iter().map(relative).collect(),
            documents: documents.iter().map(relative).collect(),
            layout,
            ..Default::default()
        })
    }
}
