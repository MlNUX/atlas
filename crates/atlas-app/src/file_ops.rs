// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! `FileOps`: file operations for QML.
//!
//! Quick operations (create, rename) run directly; copying, moving and
//! trashing run on a worker thread and report back through signals. After a
//! move or rename `moved(from, to)` lets open views follow the file.

use std::path::{Path, PathBuf};
use std::pin::Pin;

use atlas_core::fileops::{self, Conflict, Transfer};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};

use crate::qt_helpers;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, busy, READ, NOTIFY)]
        #[qproperty(QString, status, READ, NOTIFY)]
        type FileOps = super::FileOpsRust;

        /// Creates an empty file; returns its path or "" (see `failed`).
        #[qinvokable]
        #[cxx_name = "newFile"]
        fn new_file(self: Pin<&mut FileOps>, dir: &QString, name: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "newFolder"]
        fn new_folder(self: Pin<&mut FileOps>, dir: &QString, name: &QString) -> QString;

        #[qinvokable]
        fn rename(self: Pin<&mut FileOps>, path: &QString, new_name: &QString) -> QString;

        /// Error text for an invalid name, "" if the name is fine.
        #[qinvokable]
        #[cxx_name = "checkName"]
        fn check_name(self: &FileOps, name: &QString) -> QString;

        /// Entries of `paths` that already exist in `dest`.
        #[qinvokable]
        fn conflicts(self: &FileOps, paths: &QStringList, dest: &QString) -> QStringList;

        /// `kind`: "copy" or "move"; `policy`: "abort", "keep-both", "skip", "replace".
        #[qinvokable]
        fn transfer(self: Pin<&mut FileOps>, kind: &QString, paths: &QStringList, dest: &QString, policy: &QString);

        /// Moves `paths` to the trash.
        #[qinvokable]
        #[cxx_name = "moveToTrash"]
        fn move_to_trash(self: Pin<&mut FileOps>, paths: &QStringList);

        #[qinvokable]
        #[cxx_name = "restoreFromTrash"]
        fn restore_from_trash(self: Pin<&mut FileOps>, ids: &QStringList);

        #[qinvokable]
        #[cxx_name = "deleteFromTrash"]
        fn delete_from_trash(self: Pin<&mut FileOps>, ids: &QStringList);

        /// Puts files on the system clipboard (also for other file managers).
        #[qinvokable]
        #[cxx_name = "setClipboard"]
        fn set_clipboard(self: &FileOps, paths: &QStringList, cut: bool);

        #[qinvokable]
        #[cxx_name = "copyText"]
        fn copy_text(self: &FileOps, text: &QString);

        #[qinvokable]
        #[cxx_name = "clipboardFiles"]
        fn clipboard_files(self: &FileOps) -> QStringList;

        #[qinvokable]
        #[cxx_name = "clipboardIsCut"]
        fn clipboard_is_cut(self: &FileOps) -> bool;

        #[qinvokable]
        #[cxx_name = "parentDir"]
        fn parent_dir(self: &FileOps, path: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "isDir"]
        fn is_dir(self: &FileOps, path: &QString) -> bool;

        /// A file or folder was moved/renamed by Atlas.
        #[qsignal]
        fn moved(self: Pin<&mut FileOps>, from: QString, to: QString);

        /// Items were moved to the trash.
        #[qsignal]
        fn trashed(self: Pin<&mut FileOps>, paths: QStringList);

        /// A finished background operation (for the status bar).
        #[qsignal]
        fn finished(self: Pin<&mut FileOps>, message: QString);

        /// An operation failed; `message` is shown to the user.
        #[qsignal]
        fn failed(self: Pin<&mut FileOps>, message: QString);
    }

    impl cxx_qt::Threading for FileOps {}
}

#[derive(Default)]
pub struct FileOpsRust {
    busy: bool,
    status: QString,
}

fn qs(s: impl AsRef<str>) -> QString {
    QString::from(s.as_ref())
}

fn paths_of(list: &QStringList) -> Vec<PathBuf> {
    list.iter().map(|p| PathBuf::from(p.to_string())).collect()
}

fn list_of(paths: impl IntoIterator<Item = PathBuf>) -> QStringList {
    let mut list = QStringList::default();
    for p in paths {
        list.append(qs(p.to_string_lossy()));
    }
    list
}

fn name_of(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

impl qobject::FileOps {
    fn fail(self: Pin<&mut Self>, err: anyhow::Error) {
        // `{:#}` includes the cause chain ("… : Permission denied").
        self.failed(qs(format!("{err:#}")));
    }

    pub fn new_file(self: Pin<&mut Self>, dir: &QString, name: &QString) -> QString {
        match fileops::create_file(Path::new(&dir.to_string()), &name.to_string()) {
            Ok(path) => qs(path.to_string_lossy()),
            Err(err) => {
                self.fail(err);
                QString::default()
            }
        }
    }

    pub fn new_folder(self: Pin<&mut Self>, dir: &QString, name: &QString) -> QString {
        match fileops::create_dir(Path::new(&dir.to_string()), &name.to_string()) {
            Ok(path) => qs(path.to_string_lossy()),
            Err(err) => {
                self.fail(err);
                QString::default()
            }
        }
    }

    pub fn rename(mut self: Pin<&mut Self>, path: &QString, new_name: &QString) -> QString {
        let from = PathBuf::from(path.to_string());
        match fileops::rename(&from, &new_name.to_string()) {
            Ok(to) => {
                let to = qs(to.to_string_lossy());
                self.as_mut().moved(path.clone(), to.clone());
                to
            }
            Err(err) => {
                self.fail(err);
                QString::default()
            }
        }
    }

    pub fn check_name(&self, name: &QString) -> QString {
        match fileops::validate_name(&name.to_string()) {
            Ok(()) => QString::default(),
            Err(err) => qs(err.to_string()),
        }
    }

    pub fn conflicts(&self, paths: &QStringList, dest: &QString) -> QStringList {
        list_of(fileops::conflicts(&paths_of(paths), Path::new(&dest.to_string())))
    }

    fn set_busy(mut self: Pin<&mut Self>, busy: bool, status: &str) {
        self.as_mut().rust_mut().busy = busy;
        self.as_mut().rust_mut().status = qs(status);
        self.as_mut().busy_changed();
        self.status_changed();
    }

    pub fn transfer(mut self: Pin<&mut Self>, kind: &QString, paths: &QStringList, dest: &QString, policy: &QString) {
        let kind = if kind.to_string() == "move" { Transfer::Move } else { Transfer::Copy };
        let sources = paths_of(paths);
        if sources.is_empty() {
            return;
        }
        let dest = PathBuf::from(dest.to_string());
        let policy = Conflict::from_name(&policy.to_string());
        let verb = if kind == Transfer::Move { "Verschiebe" } else { "Kopiere" };
        self.as_mut().set_busy(true, &format!("{verb} {} Element(e) …", sources.len()));
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let progress_thread = thread.clone();
            let result = fileops::transfer(kind, &sources, &dest, policy, |done, total| {
                let _ = progress_thread.queue(move |mut ops| {
                    ops.as_mut().rust_mut().status = qs(format!("{verb} {done}/{total} …"));
                    ops.status_changed();
                });
            });
            let _ = thread.queue(move |mut ops| {
                ops.as_mut().set_busy(false, "");
                match result {
                    Ok(done) => {
                        if kind == Transfer::Move {
                            for m in &done {
                                ops.as_mut().moved(qs(m.from.to_string_lossy()), qs(m.to.to_string_lossy()));
                            }
                        }
                        let what = if kind == Transfer::Move { "verschoben" } else { "kopiert" };
                        let skipped = sources.len() - done.len();
                        let mut msg = format!("{} Element(e) {what}", done.len());
                        if skipped > 0 {
                            msg += &format!(", {skipped} übersprungen");
                        }
                        ops.finished(qs(msg));
                    }
                    Err(err) => ops.fail(err),
                }
            });
        });
    }

    pub fn move_to_trash(mut self: Pin<&mut Self>, paths: &QStringList) {
        let sources = paths_of(paths);
        if sources.is_empty() {
            return;
        }
        self.as_mut().set_busy(true, "In den Papierkorb …");
        let list = paths.clone();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = fileops::trash(&sources);
            let _ = thread.queue(move |mut ops| {
                ops.as_mut().set_busy(false, "");
                match result {
                    Ok(()) => {
                        let msg = if sources.len() == 1 {
                            format!("„{}“ in den Papierkorb verschoben", name_of(&sources[0]))
                        } else {
                            format!("{} Elemente in den Papierkorb verschoben", sources.len())
                        };
                        ops.as_mut().trashed(list);
                        ops.finished(qs(msg));
                    }
                    Err(err) => ops.fail(err),
                }
            });
        });
    }

    fn run_trash_job(
        mut self: Pin<&mut Self>,
        ids: &QStringList,
        status: &'static str,
        done_msg: &'static str,
        job: fn(&[String]) -> anyhow::Result<()>,
    ) {
        let ids: Vec<String> = ids.iter().map(|s| s.to_string()).collect();
        if ids.is_empty() {
            return;
        }
        self.as_mut().set_busy(true, status);
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = job(&ids);
            let _ = thread.queue(move |mut ops| {
                ops.as_mut().set_busy(false, "");
                match result {
                    Ok(()) => ops.finished(qs(format!("{} {done_msg}", ids.len()))),
                    Err(err) => ops.fail(err),
                }
            });
        });
    }

    pub fn restore_from_trash(self: Pin<&mut Self>, ids: &QStringList) {
        self.run_trash_job(ids, "Wiederherstellen …", "Element(e) wiederhergestellt", fileops::trash_restore);
    }

    pub fn delete_from_trash(self: Pin<&mut Self>, ids: &QStringList) {
        self.run_trash_job(ids, "Endgültig löschen …", "Element(e) endgültig gelöscht", fileops::trash_purge);
    }

    pub fn set_clipboard(&self, paths: &QStringList, cut: bool) {
        qt_helpers::clipboard_set_files(paths, cut);
    }

    pub fn copy_text(&self, text: &QString) {
        qt_helpers::clipboard_set_text(text);
    }

    pub fn clipboard_files(&self) -> QStringList {
        qt_helpers::clipboard_files()
    }

    pub fn clipboard_is_cut(&self) -> bool {
        qt_helpers::clipboard_files_cut()
    }

    pub fn parent_dir(&self, path: &QString) -> QString {
        Path::new(&path.to_string()).parent().map(|p| qs(p.to_string_lossy())).unwrap_or_default()
    }

    pub fn is_dir(&self, path: &QString) -> bool {
        Path::new(&path.to_string()).is_dir()
    }
}
