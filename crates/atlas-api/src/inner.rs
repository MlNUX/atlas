// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! State of one open project shared by its services (owned by P2.2): the
//! core project, `.atlas/` writer and metadata, the indexes built in the
//! background, the window's event sink.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use atlas_core::finder;
use atlas_core::notes::NoteIndex;
use atlas_core::project::Project as CoreProject;
use atlas_core::project::index::{FileIndex, Previous, Report};
use atlas_core::project::metadata::Metadata;
use atlas_core::project::writer::SessionWriter;

use crate::events::{Event, Sink};
use crate::project::IndexState;

pub(crate) struct Shared {
    pub root: PathBuf,
    /// `None` if `.atlas/` cannot be written ([`Shared::storage_error`]).
    pub project: Option<Arc<CoreProject>>,
    pub storage_error: Option<String>,
    pub metadata: Mutex<Option<Metadata>>,
    pub writer: Option<SessionWriter>,
    pub sink: Sink,
    pub index: Mutex<Option<FileIndex>>,
    pub index_state: Mutex<IndexState>,
    /// Files and folders for quick open.
    pub files: RwLock<Arc<Vec<finder::Entry>>>,
    /// Links and tags of the Markdown files (also without `.atlas/`).
    pub notes: Mutex<Option<NoteIndex>>,
    pub notes_revision: AtomicU32,
    pub watcher: Mutex<Option<atlas_core::watcher::DirWatcher>>,
    /// The running text search of this window (one at a time, as on Linux);
    /// kept here so it outlives the `Search` handle that started it.
    pub search: Mutex<Option<atlas_core::search::SearchHandle>>,
    /// The project's calendar, made on first use (P2.5).
    pub calendar: std::sync::OnceLock<Arc<crate::calendar::Calendar>>,
    next_id: AtomicU64,
}

impl Shared {
    /// Opens `root` and starts building the indexes in the background
    /// (`Event::IndexStatus`, `Event::NotesChanged` when done).
    pub fn open(root: &Path, sink: Sink) -> crate::Result<Arc<Self>> {
        let root = root.canonicalize().map_err(|e| crate::AtlasError::NotFound {
            message: format!("Projektordner {} nicht gefunden: {e}", root.display()),
        })?;
        if !root.is_dir() {
            return Err(crate::AtlasError::Invalid { message: format!("{} ist kein Ordner", root.display()) });
        }
        let (project, storage_error) = match CoreProject::open(&root) {
            Ok(p) => (Some(Arc::new(p)), None),
            Err(err) => {
                log::warn!("project data will not be saved: {err:#}");
                (None, Some(format!("{err:#}")))
            }
        };
        let metadata = project.as_ref().and_then(|p| p.metadata().map_err(|e| log::warn!("metadata: {e:#}")).ok());
        let writer = project.clone().map(SessionWriter::spawn);
        let shared = Arc::new(Self {
            root: root.clone(),
            index_state: Mutex::new(IndexState {
                ready: project.is_none(),
                text: if project.is_some() { "Index wird aufgebaut …".into() } else { String::new() },
            }),
            project,
            storage_error,
            metadata: Mutex::new(metadata),
            writer,
            sink,
            index: Mutex::new(None),
            files: RwLock::new(Arc::default()),
            notes: Mutex::new(None),
            notes_revision: AtomicU32::new(0),
            watcher: Mutex::new(None),
            search: Mutex::new(None),
            calendar: std::sync::OnceLock::new(),
            next_id: AtomicU64::new(1),
        });
        let mut state = atlas_core::session::AppState::load();
        state.remember(&root);
        if let Err(err) = state.save() {
            log::warn!("cannot save app state: {err:#}");
        }
        Shared::build_indexes(&shared);
        Ok(shared)
    }

    fn build_indexes(shared: &Arc<Self>) {
        let weak = Arc::downgrade(shared);
        let root = shared.root.clone();
        let project = shared.project.clone();
        std::thread::Builder::new()
            .name("atlas-api-index".into())
            .spawn(move || {
                let result = project.as_ref().map(|p| FileIndex::ensure(p.root(), &p.index_file()));
                let notes = NoteIndex::build(&root);
                let files = Arc::new(finder::list(&root));
                let Some(shared) = weak.upgrade() else { return };
                *shared.notes.lock().unwrap() = Some(notes);
                *shared.files.write().unwrap() = files;
                shared.bump_notes();
                let text = match result {
                    None => String::new(),
                    Some(Ok((index, report))) => {
                        *shared.index.lock().unwrap() = Some(index);
                        describe(&report)
                    }
                    Some(Err(err)) => format!("Index nicht verfügbar: {err:#}"),
                };
                *shared.index_state.lock().unwrap() = IndexState { ready: true, text: text.clone() };
                shared.emit(Event::IndexStatus { ready: true, text });
            })
            .expect("index thread");
    }

    pub fn emit(&self, event: Event) {
        self.sink.on_event(event);
    }

    pub fn bump_notes(&self) {
        let revision = self.notes_revision.fetch_add(1, Ordering::Relaxed) + 1;
        self.emit(Event::NotesChanged { revision });
    }

    /// Ids for operations reported through events.
    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Path relative to the root (`None` outside the project).
    pub fn relative(&self, path: &Path) -> Option<PathBuf> {
        path.strip_prefix(&self.root).ok().map(Path::to_path_buf)
    }

    /// Absolute path inside the project; `None` for paths that leave it.
    pub fn absolute(&self, path: &Path) -> Option<PathBuf> {
        if path.is_absolute() {
            return Some(path.to_path_buf());
        }
        let safe =
            path.components().all(|c| matches!(c, std::path::Component::Normal(_) | std::path::Component::CurDir));
        safe.then(|| self.root.join(path))
    }
}

fn describe(report: &Report) -> String {
    let previous = match &report.previous {
        Previous::Valid => "aktualisiert".to_owned(),
        Previous::Missing => "neu angelegt".to_owned(),
        Previous::Outdated(v) => format!("neu aufgebaut (Format {v})"),
        Previous::Corrupt(_) => "neu aufgebaut (war beschädigt)".to_owned(),
    };
    format!("Index: {} Einträge, {previous}, {} ms", report.files, report.elapsed.as_millis())
}
