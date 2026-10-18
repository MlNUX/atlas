// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! An edited text or Markdown file: loading, autosave, recovery copies and
//! external changes. The text itself lives in the QML editor; this object
//! knows which version on disk the edit is based on.
//!
//! All writes (saves and recovery copies) run in order on one background
//! thread, so the UI never waits for `fsync`; [`flush_io`] waits for them
//! on quit.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};

use atlas_core::textfile::{self, RecoveryStore, SaveError};
use atlas_core::watcher::DirWatcher;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, path, READ, NOTIFY = state_changed)]
        /// Where recovery copies go (`.atlas/recovery` or Atlas' state folder).
        #[qproperty(QString, recovery_dir, cxx_name = "recoveryDir", READ, WRITE, NOTIFY)]
        /// Why the file cannot be edited ("" if it can).
        #[qproperty(QString, read_only, cxx_name = "readOnly", READ, NOTIFY = state_changed)]
        /// "" – in sync, "changed" or "deleted" outside of Atlas while edited.
        #[qproperty(QString, conflict, READ, NOTIFY = state_changed)]
        /// Last save error ("" if none).
        #[qproperty(QString, error, READ, NOTIFY = state_changed)]
        /// A recovery copy with unsaved changes exists for this file.
        #[qproperty(bool, has_recovery, cxx_name = "hasRecovery", READ, NOTIFY = state_changed)]
        #[qproperty(bool, saving, READ, NOTIFY = state_changed)]
        type TextDocument = super::TextDocumentRust;

        #[qsignal]
        #[cxx_name = "stateChanged"]
        fn state_changed(self: Pin<&mut TextDocument>);

        /// The text on disk was (re)loaded; the editor replaces its text.
        #[qsignal]
        #[cxx_name = "textLoaded"]
        fn text_loaded(self: Pin<&mut TextDocument>);

        /// A save finished; `generation` is the value passed to `save`.
        #[qsignal]
        fn saved(self: Pin<&mut TextDocument>, generation: i32);

        /// The file changed on disk (QML answers with `checkDisk`).
        #[qsignal]
        #[cxx_name = "diskChanged"]
        fn disk_changed(self: Pin<&mut TextDocument>);

        /// Loads `path`; returns false if it cannot be read.
        #[qinvokable]
        fn open(self: Pin<&mut TextDocument>, path: &QString) -> bool;

        /// Text loaded by `open`/`reload`.
        #[qinvokable]
        fn text(self: &TextDocument) -> QString;

        #[qinvokable]
        fn reload(self: Pin<&mut TextDocument>) -> bool;

        /// Saves in the background unless the file changed on disk or is
        /// read-only. `generation` identifies the edit state being saved.
        #[qinvokable]
        fn save(self: Pin<&mut TextDocument>, text: &QString, generation: i32) -> bool;

        /// Resolves a conflict by writing this version over the external one.
        #[qinvokable]
        #[cxx_name = "saveKeepingMine"]
        fn save_keeping_mine(self: Pin<&mut TextDocument>, text: &QString, generation: i32) -> bool;

        /// Writes the text to a new file (which must not exist) and makes it
        /// this document.
        #[qinvokable]
        #[cxx_name = "saveAs"]
        fn save_as(self: Pin<&mut TextDocument>, path: &QString, text: &QString) -> bool;

        /// Keeps a recovery copy of the unsaved text.
        #[qinvokable]
        #[cxx_name = "writeRecovery"]
        fn write_recovery(self: &TextDocument, text: &QString);

        #[qinvokable]
        /// Text of the recovery copy. If the file changed on disk since the
        /// copy was made, the document enters the conflict state so that
        /// neither version is lost.
        #[cxx_name = "restoreRecovery"]
        fn restore_recovery(self: Pin<&mut TextDocument>) -> QString;

        /// When the recovery copy was written (ms since the epoch, 0 if none).
        #[qinvokable]
        #[cxx_name = "recoveryTime"]
        fn recovery_time(self: &TextDocument) -> f64;

        #[qinvokable]
        #[cxx_name = "discardRecovery"]
        fn discard_recovery(self: Pin<&mut TextDocument>);

        /// Compares the file on disk with the loaded version. Unmodified
        /// documents reload silently; modified ones get a conflict.
        #[qinvokable]
        #[cxx_name = "checkDisk"]
        fn check_disk(self: Pin<&mut TextDocument>, modified: bool);
    }

    impl cxx_qt::Threading for TextDocument {}
}

type Job = Box<dyn FnOnce() + Send>;

enum Io {
    Job(Job),
    Flush(Sender<()>),
}

fn io() -> &'static Mutex<Sender<Io>> {
    static IO: OnceLock<Mutex<Sender<Io>>> = OnceLock::new();
    IO.get_or_init(|| {
        let (tx, rx) = channel::<Io>();
        std::thread::Builder::new()
            .name("atlas-text-io".into())
            .spawn(move || {
                for msg in rx {
                    match msg {
                        Io::Job(job) => job(),
                        Io::Flush(done) => {
                            let _ = done.send(());
                        }
                    }
                }
            })
            .expect("spawn text io thread");
        Mutex::new(tx)
    })
}

fn queue_io(job: impl FnOnce() + Send + 'static) {
    let _ = io().lock().map(|tx| tx.send(Io::Job(Box::new(job))));
}

/// Waits until all queued saves and recovery copies are written.
pub fn flush_io() {
    let (tx, rx) = channel();
    if io().lock().map(|io| io.send(Io::Flush(tx))).is_ok_and(|r| r.is_ok()) {
        let _ = rx.recv();
    }
}

/// State shared with the I/O thread.
#[derive(Default)]
struct Shared {
    /// Hash of the version on disk the edit is based on.
    base_hash: Option<String>,
}

#[derive(Default)]
pub struct TextDocumentRust {
    path: QString,
    recovery_dir: QString,
    read_only: QString,
    conflict: QString,
    error: QString,
    has_recovery: bool,
    saving: bool,

    file: Option<PathBuf>,
    text: String,
    shared: Arc<Mutex<Shared>>,
    /// (mtime_ns, size) of the last version seen on disk.
    stat: Option<(i64, u64)>,
    watcher: Option<DirWatcher>,
    pending_saves: u32,
}

fn qs(s: impl AsRef<str>) -> QString {
    QString::from(s.as_ref())
}

fn stat_of(path: &Path) -> Option<(i64, u64)> {
    std::fs::metadata(path).ok().map(|m| (m.mtime() * 1_000_000_000 + m.mtime_nsec(), m.size()))
}

enum SaveMode {
    Normal,
    KeepMine,
}

impl TextDocumentRust {
    fn store(&self) -> Option<RecoveryStore> {
        (!self.recovery_dir.is_empty()).then(|| RecoveryStore::new(PathBuf::from(self.recovery_dir.to_string())))
    }
}

impl qobject::TextDocument {
    pub fn open(mut self: Pin<&mut Self>, path: &QString) -> bool {
        let file = PathBuf::from(path.to_string());
        {
            let mut this = self.as_mut().rust_mut();
            this.file = Some(file.clone());
            this.path = path.clone();
            this.shared = Arc::new(Mutex::new(Shared::default()));
        }
        // Follow external changes of the file (its folder is watched).
        let thread = self.qt_thread();
        let watcher = DirWatcher::new(move |_| {
            let _ = thread.queue(|doc| doc.disk_event());
        })
        .ok();
        if let (Some(watcher), Some(dir)) = (&watcher, file.parent()) {
            watcher.sync(&[dir.to_path_buf()]);
        }
        self.as_mut().rust_mut().watcher = watcher;
        let ok = self.as_mut().load();
        if ok {
            let has_recovery = self.store().and_then(|s| s.read(&file)).is_some_and(|r| r.text != self.text);
            self.as_mut().rust_mut().has_recovery = has_recovery;
        }
        self.state_changed();
        ok
    }

    fn load(mut self: Pin<&mut Self>) -> bool {
        let Some(file) = self.file.clone() else { return false };
        let result = textfile::load(&file);
        let ok = result.is_ok();
        let mut this = self.as_mut().rust_mut();
        match result {
            Ok(loaded) => {
                this.text = loaded.text;
                this.read_only = qs(loaded.read_only);
                this.shared.lock().unwrap().base_hash = Some(loaded.hash);
                this.error = QString::default();
            }
            Err(err) => {
                this.text.clear();
                this.read_only = qs("Die Datei kann nicht gelesen werden.");
                this.error = qs(format!("{err:#}"));
            }
        }
        this.conflict = QString::default();
        this.stat = stat_of(&file);
        ok
    }

    pub fn text(&self) -> QString {
        qs(&self.text)
    }

    pub fn reload(mut self: Pin<&mut Self>) -> bool {
        let ok = self.as_mut().load();
        self.as_mut().state_changed();
        self.text_loaded();
        ok
    }

    pub fn save(self: Pin<&mut Self>, text: &QString, generation: i32) -> bool {
        self.queue_save(text.to_string(), generation, SaveMode::Normal)
    }

    pub fn save_keeping_mine(self: Pin<&mut Self>, text: &QString, generation: i32) -> bool {
        self.queue_save(text.to_string(), generation, SaveMode::KeepMine)
    }

    fn queue_save(mut self: Pin<&mut Self>, text: String, generation: i32, mode: SaveMode) -> bool {
        let Some(file) = self.file.clone() else { return false };
        if !self.read_only.is_empty() {
            return false;
        }
        if !self.conflict.is_empty() && matches!(mode, SaveMode::Normal) {
            return false; // autosave pauses until the conflict is resolved
        }
        let shared = self.shared.clone();
        let store = self.store();
        let thread = self.qt_thread();
        {
            let mut this = self.as_mut().rust_mut();
            this.pending_saves += 1;
            this.saving = true;
        }
        self.as_mut().state_changed();
        queue_io(move || {
            let expected = shared.lock().unwrap().base_hash.clone();
            let result = match mode {
                SaveMode::Normal => textfile::save(&file, &text, expected.as_deref()),
                SaveMode::KeepMine => textfile::save_force(&file, &text),
            };
            if let Ok(hash) = &result {
                shared.lock().unwrap().base_hash = Some(hash.clone());
                if let Some(store) = &store {
                    store.clear(&file);
                }
            }
            let stat = stat_of(&file);
            let _ = thread.queue(move |mut doc| {
                // A newer file was opened in the meantime.
                if doc.file.as_deref() != Some(file.as_path()) {
                    return;
                }
                {
                    let mut this = doc.as_mut().rust_mut();
                    this.pending_saves = this.pending_saves.saturating_sub(1);
                    this.saving = this.pending_saves > 0;
                    match &result {
                        Ok(_) => {
                            this.stat = stat;
                            this.error = QString::default();
                            this.conflict = QString::default();
                            this.has_recovery = false;
                        }
                        Err(SaveError::Conflict { deleted }) => {
                            this.conflict = qs(if *deleted { "deleted" } else { "changed" });
                        }
                        Err(SaveError::ReadOnly(reason)) => {
                            this.read_only = qs(reason);
                            this.error = qs(reason);
                        }
                        Err(SaveError::Io(err)) => this.error = qs(format!("Speichern fehlgeschlagen: {err:#}")),
                    }
                }
                doc.as_mut().state_changed();
                if result.is_ok() {
                    doc.saved(generation);
                }
            });
        });
        true
    }

    pub fn save_as(mut self: Pin<&mut Self>, path: &QString, text: &QString) -> bool {
        let target = PathBuf::from(path.to_string());
        match textfile::save(&target, &text.to_string(), None) {
            Ok(_) => {
                if let (Some(store), Some(old)) = (self.store(), self.file.clone()) {
                    store.clear(&old);
                }
                self.as_mut().open(path)
            }
            Err(err) => {
                self.as_mut().rust_mut().error = qs(match err {
                    SaveError::Conflict { .. } => "Eine Datei mit diesem Namen existiert bereits.".to_owned(),
                    other => other.to_string(),
                });
                self.state_changed();
                false
            }
        }
    }

    pub fn write_recovery(&self, text: &QString) {
        let (Some(store), Some(file)) = (self.store(), self.file.clone()) else { return };
        let base = self.shared.lock().unwrap().base_hash.clone().unwrap_or_default();
        let text = text.to_string();
        queue_io(move || {
            if let Err(err) = store.write(&file, &base, &text) {
                log::warn!("cannot write recovery copy for {}: {err:#}", file.display());
            }
        });
    }

    pub fn restore_recovery(mut self: Pin<&mut Self>) -> QString {
        let (Some(store), Some(file)) = (self.store(), self.file.clone()) else { return QString::default() };
        let Some(recovery) = store.read(&file) else { return QString::default() };
        let base = self.shared.lock().unwrap().base_hash.clone();
        {
            let mut this = self.as_mut().rust_mut();
            this.has_recovery = false;
            if base.as_deref() != Some(recovery.base_hash.as_str()) {
                this.conflict = qs("changed");
            }
        }
        self.state_changed();
        qs(recovery.text)
    }

    pub fn recovery_time(&self) -> f64 {
        let (Some(store), Some(file)) = (self.store(), self.file.as_ref()) else { return 0.0 };
        store.read(file).map(|r| r.saved_at as f64 * 1000.0).unwrap_or(0.0)
    }

    pub fn discard_recovery(mut self: Pin<&mut Self>) {
        if let (Some(store), Some(file)) = (self.store(), self.file.clone()) {
            queue_io(move || store.clear(&file));
        }
        self.as_mut().rust_mut().has_recovery = false;
        self.state_changed();
    }

    /// Called by the watcher; QML knows whether the editor has unsaved
    /// changes and answers with `checkDisk(modified)`.
    fn disk_event(self: Pin<&mut Self>) {
        if self.file.as_ref().map(|f| stat_of(f)) != Some(self.stat) && self.pending_saves == 0 {
            self.disk_changed();
        }
    }

    pub fn check_disk(mut self: Pin<&mut Self>, modified: bool) {
        let Some(file) = self.file.clone() else { return };
        let stat = stat_of(&file);
        if stat == self.stat || self.pending_saves > 0 {
            return;
        }
        self.as_mut().rust_mut().stat = stat;
        if stat.is_none() {
            self.as_mut().rust_mut().conflict = qs("deleted");
            self.state_changed();
            return;
        }
        // Touched but identical (e.g. `touch`, editors saving unchanged).
        let base = self.shared.lock().unwrap().base_hash.clone();
        if textfile::disk_hash(&file) == base {
            if !self.conflict.is_empty() {
                self.as_mut().rust_mut().conflict = QString::default();
                self.state_changed();
            }
            return;
        }
        if modified {
            self.as_mut().rust_mut().conflict = qs("changed");
            self.state_changed();
        } else {
            self.reload();
        }
    }
}
