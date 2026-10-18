// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Contents of the trash (all trash folders) for the trash view.

use std::pin::Pin;
use std::sync::Arc;

use atlas_core::fileops::{self, TrashEntry};
use atlas_core::listing;
use atlas_core::watcher::DirWatcher;
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
        #[qproperty(i32, count, READ, NOTIFY)]
        #[qproperty(QString, error, READ, NOTIFY)]
        type TrashModel = super::TrashModelRust;

        #[qinvokable]
        fn reload(self: Pin<&mut TrashModel>);

        #[qinvokable]
        #[cxx_name = "idAt"]
        fn id_at(self: &TrashModel, row: i32) -> QString;
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut TrashModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut TrashModel>);

        #[cxx_override]
        fn data(self: &TrashModel, index: &QModelIndex, role: i32) -> QVariant;
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &TrashModel) -> QHash_i32_QByteArray;
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &TrashModel, parent: &QModelIndex) -> i32;
    }

    impl cxx_qt::Threading for TrashModel {}
    impl cxx_qt::Initialize for TrashModel {}
}

const ROLE_ID: i32 = 0x0101;
const ROLE_NAME: i32 = 0x0102;
const ROLE_ORIGINAL_PATH: i32 = 0x0103;
const ROLE_ORIGINAL_DIR: i32 = 0x0104;
const ROLE_DELETED: i32 = 0x0105;
const ROLE_IS_DIR: i32 = 0x0106;
const ROLE_SIZE_TEXT: i32 = 0x0107;

#[derive(Default)]
pub struct TrashModelRust {
    count: i32,
    error: QString,
    entries: Vec<TrashEntry>,
    watcher: Option<Arc<DirWatcher>>,
}

impl cxx_qt::Initialize for qobject::TrashModel {
    fn initialize(mut self: Pin<&mut Self>) {
        // Watch the home trash; other trash folders refresh on reload.
        if let Some(files) = listing::trash_files_dir() {
            let thread = self.qt_thread();
            if let Ok(watcher) = DirWatcher::new(move |_| {
                let _ = thread.queue(|model| model.reload());
            }) {
                watcher.sync(&[files.clone(), files.with_file_name("info")]);
                self.as_mut().rust_mut().watcher = Some(Arc::new(watcher));
            }
        }
        self.reload();
    }
}

impl qobject::TrashModel {
    pub fn reload(self: Pin<&mut Self>) {
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = fileops::trash_list();
            let _ = thread.queue(move |mut model| {
                model.as_mut().begin_reset_model();
                match result {
                    Ok(entries) => {
                        model.as_mut().rust_mut().entries = entries;
                        model.as_mut().rust_mut().error = QString::default();
                    }
                    Err(err) => {
                        model.as_mut().rust_mut().entries.clear();
                        model.as_mut().rust_mut().error = QString::from(format!("{err:#}").as_str());
                    }
                }
                model.as_mut().end_reset_model();
                let count = model.entries.len() as i32;
                model.as_mut().rust_mut().count = count;
                model.as_mut().count_changed();
                model.error_changed();
            });
        });
    }

    pub fn id_at(&self, row: i32) -> QString {
        self.entries.get(row.max(0) as usize).map(|e| QString::from(e.id.as_str())).unwrap_or_default()
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(e) = self.entries.get(index.row().max(0) as usize) else { return QVariant::default() };
        let text = |s: &str| QVariant::from(&QString::from(s));
        match role {
            ROLE_ID => text(&e.id),
            ROLE_NAME => text(&e.name),
            ROLE_ORIGINAL_PATH => text(&e.original_path.to_string_lossy()),
            ROLE_ORIGINAL_DIR => {
                text(&e.original_path.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default())
            }
            ROLE_DELETED => QVariant::from(&(e.deleted_unix as f64 * 1000.0)),
            ROLE_IS_DIR => QVariant::from(&e.is_dir),
            ROLE_SIZE_TEXT => text(&e.size.map(listing::format_size).unwrap_or_default()),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        for (role, name) in [
            (ROLE_ID, "trashId"),
            (ROLE_NAME, "name"),
            (ROLE_ORIGINAL_PATH, "originalPath"),
            (ROLE_ORIGINAL_DIR, "originalDir"),
            (ROLE_DELETED, "deleted"),
            (ROLE_IS_DIR, "isDir"),
            (ROLE_SIZE_TEXT, "sizeText"),
        ] {
            roles.insert(role, QByteArray::from(name));
        }
        roles
    }

    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.entries.len() as i32
    }
}
