// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Background writer for durable project data.
//!
//! Saving a session means an atomic file replace plus a SQLite transaction,
//! both with `fsync` – too slow for the UI thread. [`SessionWriter`] owns a
//! worker thread per project that performs the writes in order. If several
//! saves queue up, only the newest session is written, so an older state can
//! never overwrite a newer one.

use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender, bounded, unbounded};

use super::Project;
use crate::identity::FileId;
use crate::session::ProjectSession;

enum Job {
    Save { id: String, session: ProjectSession, documents: Vec<PathBuf> },
    DocState { relative: PathBuf, state: String },
    Flush(Sender<()>),
}

pub struct SessionWriter {
    jobs: Option<Sender<Job>>,
    worker: Option<JoinHandle<()>>,
}

impl SessionWriter {
    pub fn spawn(project: Arc<Project>) -> Self {
        let (tx, rx) = unbounded();
        let worker = std::thread::Builder::new()
            .name("atlas-session-writer".into())
            .spawn(move || run(project, rx))
            .expect("spawn session writer");
        Self { jobs: Some(tx), worker: Some(worker) }
    }

    /// Queues `session` (relative paths) and the identities of `documents`
    /// (absolute paths inside the project) for writing.
    pub fn save(&self, id: String, session: ProjectSession, documents: Vec<PathBuf>) {
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(Job::Save { id, session, documents });
        }
    }

    /// Queues the viewer state (JSON) of a document, by relative path.
    pub fn save_document_state(&self, relative: PathBuf, state: String) {
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(Job::DocState { relative, state });
        }
    }

    /// Blocks until everything queued so far is on disk.
    pub fn flush(&self) {
        let Some(jobs) = &self.jobs else { return };
        let (done_tx, done_rx) = bounded(1);
        if jobs.send(Job::Flush(done_tx)).is_ok() {
            let _ = done_rx.recv();
        }
    }
}

impl Drop for SessionWriter {
    fn drop(&mut self) {
        // Closing the channel lets the worker write what is left and exit.
        self.jobs.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run(project: Arc<Project>, jobs: Receiver<Job>) {
    let mut metadata = match project.metadata() {
        Ok(m) => Some(m),
        Err(err) => {
            log::warn!("cannot open metadata: {err:#}");
            None
        }
    };
    while let Ok(job) = jobs.recv() {
        // Coalesce: of all saves waiting right now, only the last one counts.
        let mut latest = None;
        let mut states = std::collections::HashMap::new();
        let mut flushes = Vec::new();
        for job in std::iter::once(job).chain(jobs.try_iter()) {
            match job {
                Job::Save { id, session, documents } => latest = Some((id, session, documents)),
                Job::DocState { relative, state } => {
                    states.insert(relative, state);
                }
                Job::Flush(done) => flushes.push(done),
            }
        }
        if let Some((id, session, documents)) = latest {
            if let Err(err) = project.save_named(&id, &session) {
                log::warn!("cannot save session: {err:#}");
            }
            let ids: Vec<FileId> = documents.iter().filter_map(|d| FileId::of(d).ok()).collect();
            if let Some(metadata) = metadata.as_mut()
                && let Err(err) = metadata.remember(project.root(), &ids)
            {
                log::warn!("cannot store document metadata: {err:#}");
            }
        }
        if let Some(metadata) = metadata.as_mut() {
            for (relative, state) in states {
                if let Err(err) = metadata.set_document_state(&relative, &state) {
                    log::warn!("cannot store document state: {err:#}");
                }
            }
        }
        for done in flushes {
            let _ = done.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn writes_in_background_and_keeps_the_newest_session() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a.md"), "# a").unwrap();
        let project = Arc::new(Project::open_as(&root, "test-device").unwrap());
        let writer = SessionWriter::spawn(project.clone());
        for i in 0..50 {
            let session = ProjectSession { layout: serde_json::json!({ "n": i }), ..Default::default() };
            writer.save("standard".into(), session, vec![root.join("a.md")]);
        }
        writer.flush();
        assert_eq!(project.load_named("standard").layout, serde_json::json!({ "n": 49 }));
        let meta = project.metadata().unwrap();
        assert!(meta.document(&root, Path::new("a.md")).unwrap().is_some());
        drop(writer);
    }
}
