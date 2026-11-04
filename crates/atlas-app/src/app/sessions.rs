// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The project's sessions: layout, open documents, switching and renaming.

use super::*;

impl qobject::AppController {
    fn session(&self) -> Option<ProjectSession> {
        self.project.as_ref().map(|p| p.load_named(&self.active_session.to_string()))
    }

    /// Re-reads the session list from `.atlas/` and notifies QML.
    pub(super) fn refresh_sessions(mut self: Pin<&mut Self>) {
        let (mut ids, mut names) = (QStringList::default(), QStringList::default());
        let (mut active, mut active_name) = (String::new(), String::new());
        if let Some(project) = self.project.clone() {
            active = project.active_session();
            active_name = project.session_name();
            for s in project.list_sessions() {
                ids.append(qs(&s.id));
                names.append(qs(&s.name));
            }
        }
        {
            let mut this = self.as_mut().rust_mut();
            this.session_ids = ids;
            this.session_names = names;
            this.active_session = qs(active);
            this.active_session_name = qs(active_name);
        }
        self.sessions_changed();
    }

    fn session_failed(self: Pin<&mut Self>, err: anyhow::Error) {
        self.session_error(qs(format!("{err:#}")));
    }

    pub fn load_layout(&self) -> QString {
        match self.session() {
            Some(s) if !s.layout.is_null() => qs(s.layout.to_string()),
            _ => QString::default(),
        }
    }

    pub fn load_expanded(&self) -> QStringList {
        let mut list = QStringList::default();
        if let (Some(project), Some(session)) = (self.project.as_ref(), self.session()) {
            for dir in session.expanded.iter().filter_map(|d| project.absolute(d)) {
                list.append(qs(dir.to_string_lossy()));
            }
        }
        list
    }

    fn build_session(&self, layout_json: &QString, expanded: &QStringList, documents: &[PathBuf]) -> ProjectSession {
        ProjectSession {
            expanded: expanded.iter().map(|d| self.relative(&path_of(d))).collect(),
            documents: documents.iter().map(|d| self.relative(d)).collect(),
            layout: serde_json::from_str(&layout_json.to_string()).unwrap_or(serde_json::Value::Null),
            ..Default::default()
        }
    }

    pub fn save_session(
        self: Pin<&mut Self>,
        layout_json: &QString,
        expanded: &QStringList,
        documents: &QStringList,
    ) -> bool {
        let Some(writer) = self.writer.as_ref() else { return false };
        let documents: Vec<PathBuf> = documents.iter().filter_map(|d| self.absolute(&path_of(d))).collect();
        let session = self.build_session(layout_json, expanded, &documents);
        writer.save(self.active_session.to_string(), session, documents);
        true
    }

    pub fn flush_session(&self) {
        crate::text_document::flush_io();
        crate::annotations_model::flush_io();
        if let Some(writer) = self.writer.as_ref() {
            writer.flush();
        }
    }

    pub fn switch_session(mut self: Pin<&mut Self>, id: &QString) -> bool {
        let Some(project) = self.project.clone() else { return false };
        // Pending writes for the old session must land before switching.
        self.flush_session();
        match project.set_active_session(&id.to_string()) {
            Ok(()) => {
                self.refresh_sessions();
                true
            }
            Err(err) => {
                self.as_mut().session_failed(err);
                false
            }
        }
    }

    pub fn create_session(
        mut self: Pin<&mut Self>,
        name: &QString,
        layout_json: &QString,
        expanded: &QStringList,
    ) -> QString {
        let Some(project) = self.project.clone() else { return QString::default() };
        let from = (!layout_json.is_empty()).then(|| self.build_session(layout_json, expanded, &[]));
        match project.create_session(&name.to_string(), from.as_ref()) {
            Ok(id) => {
                self.refresh_sessions();
                qs(id)
            }
            Err(err) => {
                self.as_mut().session_failed(err);
                QString::default()
            }
        }
    }

    pub fn session_summaries(&self) -> QString {
        let list: Vec<_> = self.project.as_deref().map(session_entry).into_iter().collect();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn all_sessions(&self) -> QString {
        let mut list: Vec<_> = self.project.as_deref().map(session_entry).into_iter().collect();
        if self.persist_session {
            for root in AppState::load().projects {
                if Some(&root) == self.root.as_ref() || !root.is_dir() {
                    continue;
                }
                if let Some(project) = Project::open_existing(&root) {
                    list.push(session_entry(&project));
                }
            }
        }
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn rename_project_session(mut self: Pin<&mut Self>, root: &QString, name: &QString) -> bool {
        let root = path_of(root);
        let result = if Some(&root) == self.root.as_ref() {
            let Some(project) = self.project.clone() else { return false };
            self.flush_session();
            project.rename(&name.to_string())
        } else {
            match Project::open_existing(&root) {
                Some(project) => project.rename(&name.to_string()),
                None => Err(anyhow::anyhow!("{} hat keine Session.", root.display())),
            }
        };
        match result {
            Ok(()) => {
                self.refresh_sessions();
                true
            }
            Err(err) => {
                self.as_mut().session_failed(err);
                false
            }
        }
    }

    pub fn forget_project(mut self: Pin<&mut Self>, root: &QString) -> bool {
        let root = path_of(root);
        if Some(&root) == self.root.as_ref() {
            return false;
        }
        if let Some(project) = Project::open_existing(&root)
            && let Err(err) = project.clear_sessions()
        {
            self.as_mut().session_failed(err);
            return false;
        }
        let mut state = AppState::load();
        state.forget(&root);
        if let Err(err) = state.save() {
            self.as_mut().session_failed(err);
            return false;
        }
        self.sessions_changed();
        true
    }

    pub fn rename_session(mut self: Pin<&mut Self>, id: &QString, name: &QString) -> bool {
        let Some(project) = self.project.clone() else { return false };
        self.flush_session();
        match project.rename_session(&id.to_string(), &name.to_string()) {
            Ok(()) => {
                self.refresh_sessions();
                true
            }
            Err(err) => {
                self.as_mut().session_failed(err);
                false
            }
        }
    }

    pub fn delete_session(mut self: Pin<&mut Self>, id: &QString) -> bool {
        let Some(project) = self.project.clone() else { return false };
        match project.delete_session(&id.to_string()) {
            Ok(()) => {
                self.refresh_sessions();
                true
            }
            Err(err) => {
                self.as_mut().session_failed(err);
                false
            }
        }
    }

    pub fn resolve_document(&self, path: &QString) -> QString {
        let path = path_of(path);
        let Some(project) = self.project.as_ref() else {
            return self.absolute(&path).filter(|p| p.exists()).map(|p| qs(p.to_string_lossy())).unwrap_or_default();
        };
        let relative = self.relative(&path);
        project
            .locate(self.metadata.as_ref(), self.index.as_ref(), &relative)
            .map(|p| qs(p.to_string_lossy()))
            .unwrap_or_default()
    }

    pub fn document_state(&self, path: &QString) -> QString {
        let relative = self.relative(&path_of(path));
        self.metadata.as_ref().and_then(|m| m.document_state(&relative).ok().flatten()).map(qs).unwrap_or_default()
    }

    pub fn save_document_state(&self, path: &QString, state_json: &QString) {
        // Only files inside the project are remembered.
        let path = path_of(path);
        if let (Some(writer), Some(root)) = (self.writer.as_ref(), self.root.as_ref())
            && let Ok(relative) = path.strip_prefix(root)
        {
            writer.save_document_state(relative.to_path_buf(), state_json.to_string());
        }
    }

    pub fn request_save_all(self: Pin<&mut Self>) {
        self.save_all_requested();
    }
}
