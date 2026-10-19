// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Project search results (ripgrep) as a `QAbstractListModel`.

use std::path::PathBuf;
use std::pin::Pin;

use atlas_core::search::{self, Match, Query, SearchHandle};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

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
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(bool, running, READ, NOTIFY)]
        #[qproperty(i32, count, READ, NOTIFY)]
        #[qproperty(i32, elapsed_ms, cxx_name = "elapsedMs", READ, NOTIFY)]
        #[qproperty(bool, truncated, READ, NOTIFY)]
        #[qproperty(QString, error, READ, NOTIFY)]
        type SearchModel = super::SearchModelRust;

        /// Starts a search below `root`; a running search is cancelled.
        /// `globs` is a comma separated list such as `*.rs, !docs/**`; build
        /// and dependency folders are skipped unless `include_build_dirs`.
        #[qinvokable]
        fn search(
            self: Pin<&mut SearchModel>,
            root: &QString,
            pattern: &QString,
            regex: bool,
            case_sensitive: bool,
            globs: &QString,
            include_build_dirs: bool,
        );

        #[qinvokable]
        fn cancel(self: Pin<&mut SearchModel>);

        #[qinvokable]
        fn clear(self: Pin<&mut SearchModel>);
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        fn begin_insert_rows(self: Pin<&mut SearchModel>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        #[cxx_name = "endInsertRows"]
        fn end_insert_rows(self: Pin<&mut SearchModel>);
        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut SearchModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut SearchModel>);

        #[cxx_override]
        fn data(self: &SearchModel, index: &QModelIndex, role: i32) -> QVariant;
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &SearchModel) -> QHash_i32_QByteArray;
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &SearchModel, parent: &QModelIndex) -> i32;
    }

    impl cxx_qt::Threading for SearchModel {}
}

const ROLE_PATH: i32 = 0x0101;
const ROLE_RELATIVE: i32 = 0x0102;
const ROLE_LINE: i32 = 0x0103;
const ROLE_TEXT: i32 = 0x0104;
const ROLE_MATCH_START: i32 = 0x0105;
const ROLE_MATCH_END: i32 = 0x0106;
const ROLE_FIRST_IN_FILE: i32 = 0x0107;

#[derive(Default)]
pub struct SearchModelRust {
    running: bool,
    count: i32,
    elapsed_ms: i32,
    truncated: bool,
    error: QString,

    matches: Vec<Match>,
    handle: Option<SearchHandle>,
    generation: u64,
}

impl qobject::SearchModel {
    pub fn search(
        mut self: Pin<&mut Self>,
        root: &QString,
        pattern: &QString,
        regex: bool,
        case_sensitive: bool,
        globs: &QString,
        include_build_dirs: bool,
    ) {
        self.as_mut().clear();
        let pattern = pattern.to_string();
        if pattern.is_empty() || root.is_empty() {
            return;
        }
        let query = Query {
            pattern,
            regex,
            case_sensitive: case_sensitive.then_some(true),
            globs: (!include_build_dirs)
                .then(atlas_core::finder::build_dir_globs)
                .unwrap_or_default()
                .into_iter()
                .chain(globs.to_string().split(',').map(str::trim).filter(|g| !g.is_empty()).map(str::to_owned))
                .collect(),
        };
        let generation = self.generation;
        let batch_thread = self.qt_thread();
        let done_thread = self.qt_thread();
        let root = PathBuf::from(root.to_string());
        let handle = search::start(
            &root,
            query,
            move |batch| {
                let _ = batch_thread.queue(move |model| model.append(generation, batch));
            },
            move |summary| {
                let _ = done_thread.queue(move |mut model| {
                    if model.generation != generation {
                        return;
                    }
                    {
                        let mut this = model.as_mut().rust_mut();
                        this.running = false;
                        this.elapsed_ms = summary.elapsed.as_millis() as i32;
                        this.truncated = summary.truncated;
                        this.error = QString::from(summary.error.unwrap_or_default().as_str());
                        this.handle = None;
                    }
                    model.as_mut().running_changed();
                    model.as_mut().elapsed_ms_changed();
                    model.as_mut().truncated_changed();
                    model.as_mut().error_changed();
                });
            },
        );
        match handle {
            Ok(handle) => {
                self.as_mut().rust_mut().handle = Some(handle);
                self.as_mut().rust_mut().running = true;
                self.as_mut().running_changed();
            }
            Err(err) => {
                self.as_mut().rust_mut().error = QString::from(format!("{err:#}").as_str());
                self.as_mut().error_changed();
            }
        }
    }

    pub fn cancel(mut self: Pin<&mut Self>) {
        if let Some(handle) = self.as_mut().rust_mut().handle.take() {
            handle.cancel();
        }
        if self.running {
            self.as_mut().rust_mut().running = false;
            self.as_mut().running_changed();
        }
    }

    pub fn clear(mut self: Pin<&mut Self>) {
        self.as_mut().cancel();
        self.as_mut().begin_reset_model();
        {
            let mut this = self.as_mut().rust_mut();
            this.generation += 1;
            this.matches.clear();
            this.count = 0;
            this.elapsed_ms = 0;
            this.truncated = false;
            this.error = QString::default();
        }
        self.as_mut().end_reset_model();
        self.as_mut().count_changed();
        self.as_mut().elapsed_ms_changed();
        self.as_mut().truncated_changed();
        self.as_mut().error_changed();
    }

    fn append(mut self: Pin<&mut Self>, generation: u64, batch: Vec<Match>) {
        if generation != self.generation || batch.is_empty() {
            return;
        }
        let first = self.matches.len() as i32;
        let last = first + batch.len() as i32 - 1;
        self.as_mut().begin_insert_rows(&QModelIndex::default(), first, last);
        {
            let mut this = self.as_mut().rust_mut();
            this.matches.extend(batch);
            this.count = this.matches.len() as i32;
        }
        self.as_mut().end_insert_rows();
        self.as_mut().count_changed();
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let row = index.row().max(0) as usize;
        let Some(m) = self.matches.get(row) else { return QVariant::default() };
        // Byte offsets from ripgrep → UTF-16 offsets for QML string handling.
        let utf16 = |byte: usize| m.text.get(..byte.min(m.text.len())).map_or(0, |s| s.encode_utf16().count()) as i32;
        match role {
            ROLE_PATH => QVariant::from(&QString::from(m.path.to_string_lossy().as_ref())),
            ROLE_RELATIVE => QVariant::from(&QString::from(m.relative.as_str())),
            ROLE_LINE => QVariant::from(&(m.line as i32)),
            ROLE_TEXT => QVariant::from(&QString::from(m.text.as_str())),
            ROLE_MATCH_START => QVariant::from(&utf16(m.column.0)),
            ROLE_MATCH_END => QVariant::from(&utf16(m.column.1)),
            ROLE_FIRST_IN_FILE => {
                let first = row == 0 || self.matches[row - 1].path != m.path;
                QVariant::from(&first)
            }
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        for (role, name) in [
            (ROLE_PATH, "path"),
            (ROLE_RELATIVE, "relativePath"),
            (ROLE_LINE, "line"),
            (ROLE_TEXT, "text"),
            (ROLE_MATCH_START, "matchStart"),
            (ROLE_MATCH_END, "matchEnd"),
            (ROLE_FIRST_IN_FILE, "firstInFile"),
        ] {
            roles.insert(role, QByteArray::from(name));
        }
        roles
    }

    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.matches.len() as i32
    }
}
