// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Project tree as a flat `QAbstractListModel` (VS Code explorer style).
//!
//! Directory listings run on background threads and are applied on the Qt
//! thread, so even directories with tens of thousands of entries never block
//! the UI. A [`DirWatcher`] keeps the shown directories in sync with changes
//! made outside of Atlas.

use std::collections::HashSet;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use atlas_core::fs_tree::{self, FlatTree, Op, Toggle};
use atlas_core::watcher::DirWatcher;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QStringList, QVariant, QVector};

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
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(QString, root, READ, WRITE = set_root, NOTIFY)]
        #[qproperty(bool, show_hidden, cxx_name = "showHidden", READ, WRITE = set_show_hidden, NOTIFY)]
        #[qproperty(i32, count, READ, NOTIFY)]
        #[qproperty(bool, loading, READ, NOTIFY)]
        #[qproperty(i32, last_load_ms, cxx_name = "lastLoadMs", READ, NOTIFY)]
        #[qproperty(i32, refresh_count, cxx_name = "refreshCount", READ, NOTIFY)]
        type ProjectTreeModel = super::ProjectTreeModelRust;

        #[cxx_name = "setRoot"]
        fn set_root(self: Pin<&mut ProjectTreeModel>, root: QString);
        #[cxx_name = "setShowHidden"]
        fn set_show_hidden(self: Pin<&mut ProjectTreeModel>, show: bool);

        /// Expands or collapses a directory row.
        #[qinvokable]
        fn toggle(self: Pin<&mut ProjectTreeModel>, row: i32);

        /// Expands the given directories once they appear (session restore).
        #[qinvokable]
        #[cxx_name = "restoreExpanded"]
        fn restore_expanded(self: Pin<&mut ProjectTreeModel>, dirs: &QStringList);

        /// Collapses every folder (before applying another session).
        #[qinvokable]
        #[cxx_name = "collapseAll"]
        fn collapse_all(self: Pin<&mut ProjectTreeModel>);

        #[qinvokable]
        #[cxx_name = "expandedDirs"]
        fn expanded_dirs(self: &ProjectTreeModel) -> QStringList;

        #[qinvokable]
        #[cxx_name = "pathAt"]
        fn path_at(self: &ProjectTreeModel, row: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "rowOf"]
        fn row_of(self: &ProjectTreeModel, path: &QString) -> i32;

        /// Re-lists all shown directories.
        #[qinvokable]
        fn refresh(self: Pin<&mut ProjectTreeModel>);
    }

    unsafe extern "RustQt" {
        #[qsignal]
        #[inherit]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut ProjectTreeModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QVector_i32,
        );

        #[inherit]
        #[cxx_name = "beginInsertRows"]
        fn begin_insert_rows(self: Pin<&mut ProjectTreeModel>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        #[cxx_name = "endInsertRows"]
        fn end_insert_rows(self: Pin<&mut ProjectTreeModel>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        fn begin_remove_rows(self: Pin<&mut ProjectTreeModel>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        fn end_remove_rows(self: Pin<&mut ProjectTreeModel>);
        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut ProjectTreeModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut ProjectTreeModel>);
        #[inherit]
        fn index(self: &ProjectTreeModel, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;

        #[cxx_override]
        fn data(self: &ProjectTreeModel, index: &QModelIndex, role: i32) -> QVariant;
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &ProjectTreeModel) -> QHash_i32_QByteArray;
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &ProjectTreeModel, parent: &QModelIndex) -> i32;
    }

    impl cxx_qt::Threading for ProjectTreeModel {}
}

const ROLE_NAME: i32 = 0x0101;
const ROLE_PATH: i32 = 0x0102;
const ROLE_DEPTH: i32 = 0x0103;
const ROLE_IS_DIR: i32 = 0x0104;
const ROLE_EXPANDED: i32 = 0x0105;
const ROLE_LOADING: i32 = 0x0106;
const ROLE_IS_SYMLINK: i32 = 0x0107;
const ROLE_IS_HIDDEN: i32 = 0x0108;

#[derive(Default)]
pub struct ProjectTreeModelRust {
    root: QString,
    show_hidden: bool,
    count: i32,
    loading: bool,
    last_load_ms: i32,
    refresh_count: i32,

    tree: FlatTree,
    watcher: Option<Arc<DirWatcher>>,
    /// Incremented when the root changes; stale listings are dropped.
    generation: u64,
    /// Directories with a listing in flight.
    in_flight: HashSet<PathBuf>,
    /// Directories to expand as soon as they are listed.
    pending_expand: HashSet<PathBuf>,
}

impl qobject::ProjectTreeModel {
    pub fn set_root(mut self: Pin<&mut Self>, root: QString) {
        if self.root == root {
            return;
        }
        self.as_mut().begin_reset_model();
        {
            let mut this = self.as_mut().rust_mut();
            this.root = root.clone();
            this.generation += 1;
            this.in_flight.clear();
            this.tree.reset(PathBuf::from(root.to_string()));
            this.watcher = None;
        }
        self.as_mut().end_reset_model();
        self.as_mut().update_count();
        self.as_mut().root_changed();
        if root.is_empty() {
            return;
        }

        let thread = self.qt_thread();
        let watcher = DirWatcher::new(move |dirs| {
            let _ = thread.queue(move |model| model.on_dirs_changed(dirs));
        });
        match watcher {
            Ok(w) => self.as_mut().rust_mut().watcher = Some(Arc::new(w)),
            Err(err) => log::warn!("file watcher unavailable: {err:#}"),
        }
        let root = PathBuf::from(root.to_string());
        self.list_async(root);
    }

    pub fn set_show_hidden(mut self: Pin<&mut Self>, show: bool) {
        if self.show_hidden == show {
            return;
        }
        self.as_mut().rust_mut().show_hidden = show;
        self.as_mut().show_hidden_changed();
        self.refresh();
    }

    pub fn toggle(mut self: Pin<&mut Self>, row: i32) {
        if row < 0 {
            return;
        }
        match self.tree.plan_toggle(row as usize) {
            Toggle::Load { ops, dir } => {
                self.as_mut().apply_ops(ops);
                self.list_async(dir);
            }
            Toggle::Collapse(ops) => {
                self.as_mut().apply_ops(ops);
                self.sync_watcher();
            }
            Toggle::None => {}
        }
    }

    pub fn collapse_all(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().pending_expand.clear();
        // Collapsing a top-level folder removes its whole subtree; going
        // backwards keeps the remaining indices valid.
        for i in (0..self.tree.len()).rev() {
            let (depth, expanded) = (self.tree.rows()[i].depth, self.tree.rows()[i].expanded);
            if depth == 0 && expanded {
                self.as_mut().toggle(i as i32);
            }
        }
    }

    pub fn restore_expanded(mut self: Pin<&mut Self>, dirs: &QStringList) {
        let dirs: Vec<PathBuf> = dirs.iter().map(|d| PathBuf::from(d.to_string())).collect();
        // Expand directories that are already visible right away.
        let mut visible = Vec::new();
        {
            let mut this = self.as_mut().rust_mut();
            for dir in dirs {
                if this.tree.index_of(&dir).is_some() {
                    visible.push(dir);
                } else {
                    this.pending_expand.insert(dir);
                }
            }
        }
        for dir in visible {
            if let Some(i) = self.tree.index_of(&dir)
                && !self.tree.rows()[i].expanded
            {
                self.as_mut().toggle(i as i32);
            }
        }
    }

    pub fn expanded_dirs(&self) -> QStringList {
        let mut list = QStringList::default();
        for dir in self.tree.expanded_dirs().into_iter().skip(1) {
            list.append(QString::from(dir.to_string_lossy().as_ref()));
        }
        list
    }

    pub fn path_at(&self, row: i32) -> QString {
        self.tree
            .rows()
            .get(row.max(0) as usize)
            .map(|r| QString::from(r.entry.path.to_string_lossy().as_ref()))
            .unwrap_or_default()
    }

    pub fn row_of(&self, path: &QString) -> i32 {
        self.tree.index_of(&PathBuf::from(path.to_string())).map_or(-1, |i| i as i32)
    }

    pub fn refresh(mut self: Pin<&mut Self>) {
        for dir in self.tree.expanded_dirs() {
            self.as_mut().list_async(dir);
        }
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(row) = self.tree.rows().get(index.row().max(0) as usize) else {
            return QVariant::default();
        };
        match role {
            ROLE_NAME => QVariant::from(&QString::from(row.entry.name.as_str())),
            ROLE_PATH => QVariant::from(&QString::from(row.entry.path.to_string_lossy().as_ref())),
            ROLE_DEPTH => QVariant::from(&(row.depth as i32)),
            ROLE_IS_DIR => QVariant::from(&row.entry.is_dir),
            ROLE_EXPANDED => QVariant::from(&row.expanded),
            ROLE_LOADING => QVariant::from(&row.loading),
            ROLE_IS_SYMLINK => QVariant::from(&row.entry.is_symlink),
            ROLE_IS_HIDDEN => QVariant::from(&row.entry.is_hidden()),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        for (role, name) in [
            (ROLE_NAME, "name"),
            (ROLE_PATH, "path"),
            (ROLE_DEPTH, "depth"),
            (ROLE_IS_DIR, "isDir"),
            (ROLE_EXPANDED, "expanded"),
            (ROLE_LOADING, "loading"),
            (ROLE_IS_SYMLINK, "isSymlink"),
            (ROLE_IS_HIDDEN, "isHidden"),
        ] {
            roles.insert(role, QByteArray::from(name));
        }
        roles
    }

    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.tree.len() as i32
    }

    /// Lists `dir` on a background thread and applies the result.
    fn list_async(mut self: Pin<&mut Self>, dir: PathBuf) {
        if !self.as_mut().rust_mut().in_flight.insert(dir.clone()) {
            return;
        }
        if !self.loading {
            self.as_mut().set_loading_internal(true);
        }
        let generation = self.generation;
        let show_hidden = self.show_hidden;
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let started = Instant::now();
            let result = fs_tree::list_dir(&dir, show_hidden);
            let elapsed = started.elapsed().as_millis() as i32;
            let _ = thread.queue(move |model| model.apply_listing(generation, dir, result, elapsed));
        });
    }

    fn apply_listing(
        mut self: Pin<&mut Self>,
        generation: u64,
        dir: PathBuf,
        result: std::io::Result<Vec<fs_tree::Entry>>,
        elapsed_ms: i32,
    ) {
        if generation != self.generation {
            return;
        }
        self.as_mut().rust_mut().in_flight.remove(&dir);
        let started = Instant::now();
        let ops = match result {
            Ok(entries) => self.tree.plan_children(&dir, entries),
            Err(err) => {
                log::debug!("cannot list {}: {err}", dir.display());
                self.tree.plan_failed(&dir)
            }
        };
        self.as_mut().apply_ops(ops);
        let total_ms = elapsed_ms + started.elapsed().as_millis() as i32;
        {
            let mut this = self.as_mut().rust_mut();
            this.last_load_ms = total_ms;
            this.refresh_count += 1;
        }
        self.as_mut().last_load_ms_changed();
        self.as_mut().refresh_count_changed();
        if self.in_flight.is_empty() {
            self.as_mut().set_loading_internal(false);
        }
        self.as_mut().expand_pending();
        self.sync_watcher();
    }

    fn expand_pending(mut self: Pin<&mut Self>) {
        if self.pending_expand.is_empty() {
            return;
        }
        let ready: Vec<PathBuf> =
            self.pending_expand.iter().filter(|d| self.tree.index_of(d).is_some()).cloned().collect();
        for dir in ready {
            self.as_mut().rust_mut().pending_expand.remove(&dir);
            if let Some(i) = self.tree.index_of(&dir)
                && !self.tree.rows()[i].expanded
            {
                self.as_mut().toggle(i as i32);
            }
        }
    }

    fn on_dirs_changed(mut self: Pin<&mut Self>, dirs: Vec<PathBuf>) {
        let shown: HashSet<PathBuf> = self.tree.expanded_dirs().into_iter().collect();
        for dir in dirs {
            if shown.contains(&dir) {
                self.as_mut().list_async(dir);
            }
        }
    }

    fn sync_watcher(&self) {
        if let Some(w) = &self.watcher {
            w.sync(&self.tree.expanded_dirs());
        }
    }

    fn apply_ops(mut self: Pin<&mut Self>, ops: Vec<Op>) {
        let parent = QModelIndex::default();
        let mut count_changed = false;
        for op in ops {
            match &op {
                Op::Insert { at, rows } => {
                    let (first, last) = (*at as i32, (*at + rows.len()) as i32 - 1);
                    self.as_mut().begin_insert_rows(&parent, first, last);
                    self.as_mut().rust_mut().tree.apply(op);
                    self.as_mut().end_insert_rows();
                    count_changed = true;
                }
                Op::Remove { at, count } => {
                    let (first, last) = (*at as i32, (*at + *count) as i32 - 1);
                    self.as_mut().begin_remove_rows(&parent, first, last);
                    self.as_mut().rust_mut().tree.apply(op);
                    self.as_mut().end_remove_rows();
                    count_changed = true;
                }
                Op::Update { at, .. } => {
                    let row = *at as i32;
                    self.as_mut().rust_mut().tree.apply(op);
                    let index = self.index(row, 0, &parent);
                    self.as_mut().data_changed(&index, &index, &QVector::<i32>::default());
                }
            }
        }
        if count_changed {
            self.update_count();
        }
    }

    fn update_count(mut self: Pin<&mut Self>) {
        let count = self.tree.len() as i32;
        if self.count != count {
            self.as_mut().rust_mut().count = count;
            self.count_changed();
        }
    }

    fn set_loading_internal(mut self: Pin<&mut Self>, loading: bool) {
        self.as_mut().rust_mut().loading = loading;
        self.loading_changed();
    }
}
