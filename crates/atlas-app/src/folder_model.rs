// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Contents of one folder as a `QAbstractListModel` for the file list.
//!
//! Listing runs on a background thread; a [`DirWatcher`] reloads the folder
//! on external changes, and updates are applied as a minimal diff so that
//! the view keeps its scroll position.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use atlas_core::listing::{self, FileInfo, ListOp, SortKey};
use atlas_core::watcher::DirWatcher;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant, QVector};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!(<QtCore/QAbstractListModel>);
        type QAbstractListModel;

        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qvector.h");
        type QVector_i32 = cxx_qt_lib::QVector<i32>;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(QString, path, READ, WRITE = set_path, NOTIFY)]
        #[qproperty(bool, show_hidden, cxx_name = "showHidden", READ, WRITE = set_show_hidden, NOTIFY)]
        #[qproperty(QString, sort_key, cxx_name = "sortKey", READ, WRITE = set_sort_key, NOTIFY)]
        #[qproperty(bool, sort_ascending, cxx_name = "sortAscending", READ, WRITE = set_sort_ascending, NOTIFY)]
        #[qproperty(i32, count, READ, NOTIFY)]
        #[qproperty(bool, loading, READ, NOTIFY)]
        #[qproperty(QString, error, READ, NOTIFY)]
        type FolderModel = super::FolderModelRust;

        #[cxx_name = "setPath"]
        fn set_path(self: Pin<&mut FolderModel>, path: QString);
        #[cxx_name = "setShowHidden"]
        fn set_show_hidden(self: Pin<&mut FolderModel>, show: bool);
        #[cxx_name = "setSortKey"]
        fn set_sort_key(self: Pin<&mut FolderModel>, key: QString);
        #[cxx_name = "setSortAscending"]
        fn set_sort_ascending(self: Pin<&mut FolderModel>, ascending: bool);

        #[qinvokable]
        fn reload(self: Pin<&mut FolderModel>);

        #[qinvokable]
        #[cxx_name = "pathAt"]
        fn path_at(self: &FolderModel, row: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "isDirAt"]
        fn is_dir_at(self: &FolderModel, row: i32) -> bool;

        #[qinvokable]
        #[cxx_name = "rowOf"]
        fn row_of(self: &FolderModel, path: &QString) -> i32;
    }

    unsafe extern "RustQt" {
        #[qsignal]
        #[inherit]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut FolderModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QVector_i32,
        );
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        fn begin_insert_rows(self: Pin<&mut FolderModel>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        #[cxx_name = "endInsertRows"]
        fn end_insert_rows(self: Pin<&mut FolderModel>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        fn begin_remove_rows(self: Pin<&mut FolderModel>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        fn end_remove_rows(self: Pin<&mut FolderModel>);
        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut FolderModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut FolderModel>);
        #[inherit]
        fn index(self: &FolderModel, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;

        #[cxx_override]
        fn data(self: &FolderModel, index: &QModelIndex, role: i32) -> QVariant;
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &FolderModel) -> QHash_i32_QByteArray;
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &FolderModel, parent: &QModelIndex) -> i32;
    }

    impl cxx_qt::Threading for FolderModel {}
}

const ROLE_NAME: i32 = 0x0101;
const ROLE_PATH: i32 = 0x0102;
const ROLE_IS_DIR: i32 = 0x0103;
const ROLE_IS_SYMLINK: i32 = 0x0104;
const ROLE_IS_HIDDEN: i32 = 0x0105;
const ROLE_SIZE: i32 = 0x0106;
const ROLE_SIZE_TEXT: i32 = 0x0107;
const ROLE_MODIFIED: i32 = 0x0108;
const ROLE_KIND: i32 = 0x0109;

pub struct FolderModelRust {
    path: QString,
    show_hidden: bool,
    sort_key: QString,
    sort_ascending: bool,
    count: i32,
    loading: bool,
    error: QString,

    entries: Vec<FileInfo>,
    watcher: Option<Arc<DirWatcher>>,
    generation: u64,
}

impl Default for FolderModelRust {
    fn default() -> Self {
        Self {
            path: QString::default(),
            show_hidden: false,
            sort_key: QString::from("name"),
            sort_ascending: true,
            count: 0,
            loading: false,
            error: QString::default(),
            entries: Vec::new(),
            watcher: None,
            generation: 0,
        }
    }
}

impl qobject::FolderModel {
    fn key(&self) -> SortKey {
        SortKey::from_name(&self.sort_key.to_string())
    }

    pub fn set_path(mut self: Pin<&mut Self>, path: QString) {
        if self.path == path {
            return;
        }
        self.as_mut().begin_reset_model();
        {
            let mut this = self.as_mut().rust_mut();
            this.path = path.clone();
            this.entries.clear();
            this.generation += 1;
            this.watcher = None;
        }
        self.as_mut().end_reset_model();
        self.as_mut().update_count();
        self.as_mut().path_changed();
        if path.is_empty() {
            return;
        }
        let dir = PathBuf::from(path.to_string());
        let thread = self.qt_thread();
        match DirWatcher::new(move |_| {
            let _ = thread.queue(|model| model.reload());
        }) {
            Ok(watcher) => {
                watcher.sync(std::slice::from_ref(&dir));
                self.as_mut().rust_mut().watcher = Some(Arc::new(watcher));
            }
            Err(err) => log::warn!("folder watcher unavailable: {err:#}"),
        }
        self.reload();
    }

    pub fn set_show_hidden(mut self: Pin<&mut Self>, show: bool) {
        if self.show_hidden != show {
            self.as_mut().rust_mut().show_hidden = show;
            self.as_mut().show_hidden_changed();
            self.reload();
        }
    }

    pub fn set_sort_key(mut self: Pin<&mut Self>, key: QString) {
        if self.sort_key != key {
            self.as_mut().rust_mut().sort_key = key;
            self.as_mut().sort_key_changed();
            self.resort();
        }
    }

    pub fn set_sort_ascending(mut self: Pin<&mut Self>, ascending: bool) {
        if self.sort_ascending != ascending {
            self.as_mut().rust_mut().sort_ascending = ascending;
            self.as_mut().sort_ascending_changed();
            self.resort();
        }
    }

    /// Re-sorting reorders everything, so a reset is the honest signal.
    fn resort(mut self: Pin<&mut Self>) {
        let (key, asc) = (self.key(), self.sort_ascending);
        self.as_mut().begin_reset_model();
        listing::sort(&mut self.as_mut().rust_mut().entries, key, asc);
        self.end_reset_model();
    }

    pub fn reload(mut self: Pin<&mut Self>) {
        let path = self.path.to_string();
        if path.is_empty() {
            return;
        }
        self.as_mut().rust_mut().loading = true;
        self.as_mut().loading_changed();
        let (generation, show_hidden, key, asc) = (self.generation, self.show_hidden, self.key(), self.sort_ascending);
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = listing::list(std::path::Path::new(&path), show_hidden).map(|mut entries| {
                listing::sort(&mut entries, key, asc);
                entries
            });
            let _ = thread.queue(move |model| model.apply_listing(generation, key, asc, result));
        });
    }

    fn apply_listing(
        mut self: Pin<&mut Self>,
        generation: u64,
        key: SortKey,
        asc: bool,
        result: std::io::Result<Vec<FileInfo>>,
    ) {
        // Results for an old folder are dropped; a listing sorted the old way
        // is redone with the current order.
        if generation != self.generation {
            return;
        }
        if key != self.key() || asc != self.sort_ascending {
            self.reload();
            return;
        }
        self.as_mut().rust_mut().loading = false;
        self.as_mut().loading_changed();
        let entries = match result {
            Ok(entries) => {
                self.as_mut().rust_mut().error = QString::default();
                entries
            }
            Err(err) => {
                self.as_mut().rust_mut().error = QString::from(err.to_string().as_str());
                Vec::new()
            }
        };
        self.as_mut().error_changed();
        let parent = QModelIndex::default();
        for op in listing::diff(&self.entries, &entries, key, asc) {
            match &op {
                ListOp::Insert { at, items } => {
                    let (first, last) = (*at as i32, (*at + items.len()) as i32 - 1);
                    self.as_mut().begin_insert_rows(&parent, first, last);
                    listing::apply(&mut self.as_mut().rust_mut().entries, op);
                    self.as_mut().end_insert_rows();
                }
                ListOp::Remove { at, count } => {
                    let (first, last) = (*at as i32, (*at + *count) as i32 - 1);
                    self.as_mut().begin_remove_rows(&parent, first, last);
                    listing::apply(&mut self.as_mut().rust_mut().entries, op);
                    self.as_mut().end_remove_rows();
                }
                ListOp::Update { at, .. } => {
                    let row = *at as i32;
                    listing::apply(&mut self.as_mut().rust_mut().entries, op);
                    let index = self.index(row, 0, &parent);
                    self.as_mut().data_changed(&index, &index, &QVector::<i32>::default());
                }
            }
        }
        self.update_count();
    }

    fn update_count(mut self: Pin<&mut Self>) {
        let count = self.entries.len() as i32;
        if self.count != count {
            self.as_mut().rust_mut().count = count;
            self.count_changed();
        }
    }

    pub fn path_at(&self, row: i32) -> QString {
        self.entries
            .get(row.max(0) as usize)
            .map(|e| QString::from(e.path.to_string_lossy().as_ref()))
            .unwrap_or_default()
    }

    pub fn is_dir_at(&self, row: i32) -> bool {
        self.entries.get(row.max(0) as usize).is_some_and(|e| e.is_dir)
    }

    pub fn row_of(&self, path: &QString) -> i32 {
        let path = PathBuf::from(path.to_string());
        self.entries.iter().position(|e| e.path == path).map_or(-1, |i| i as i32)
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(e) = self.entries.get(index.row().max(0) as usize) else { return QVariant::default() };
        match role {
            ROLE_NAME => QVariant::from(&QString::from(e.name.as_str())),
            ROLE_PATH => QVariant::from(&QString::from(e.path.to_string_lossy().as_ref())),
            ROLE_IS_DIR => QVariant::from(&e.is_dir),
            ROLE_IS_SYMLINK => QVariant::from(&e.is_symlink),
            ROLE_IS_HIDDEN => QVariant::from(&e.is_hidden()),
            ROLE_SIZE => QVariant::from(&(e.size as f64)),
            ROLE_SIZE_TEXT => QVariant::from(&QString::from(
                if e.is_dir { String::new() } else { listing::format_size(e.size) }.as_str(),
            )),
            ROLE_MODIFIED => QVariant::from(&(e.modified_ms as f64)),
            ROLE_KIND => QVariant::from(&QString::from(e.kind.as_str())),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        for (role, name) in [
            (ROLE_NAME, "name"),
            (ROLE_PATH, "path"),
            (ROLE_IS_DIR, "isDir"),
            (ROLE_IS_SYMLINK, "isSymlink"),
            (ROLE_IS_HIDDEN, "isHidden"),
            (ROLE_SIZE, "size"),
            (ROLE_SIZE_TEXT, "sizeText"),
            (ROLE_MODIFIED, "modified"),
            (ROLE_KIND, "kind"),
        ] {
            roles.insert(role, QByteArray::from(name));
        }
        roles
    }

    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.entries.len() as i32
    }
}
