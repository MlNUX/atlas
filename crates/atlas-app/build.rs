// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

use std::process::Command;

use cxx_qt_build::{CxxQtBuilder, QResource, QResources, QmlFile, QmlModule};

const QML_FILES: &[&str] = &[
    "qml/Main.qml",
    "qml/Banner.qml",
    "qml/CalendarMonth.qml",
    "qml/CalendarSync.qml",
    "qml/CalendarView.qml",
    "qml/CalendarWeek.qml",
    "qml/CommandPalette.qml",
    "qml/ContextPanel.qml",
    "qml/DocumentView.qml",
    "qml/EditorArea.qml",
    "qml/EditorGroup.qml",
    "qml/EntryDialog.qml",
    "qml/ExtensionsDialog.qml",
    "qml/FileActions.qml",
    "qml/FolderView.qml",
    "qml/Icon.qml",
    "qml/IconButton.qml",
    "qml/Inspector.qml",
    "qml/MarkdownView.qml",
    "qml/PdfViewer.qml",
    "qml/PlacesList.qml",
    "qml/ProcessedView.qml",
    "qml/ProjectTree.qml",
    "qml/PropertiesDialog.qml",
    "qml/QuickLook.qml",
    "qml/SearchPanel.qml",
    "qml/SendToNoteDialog.qml",
    "qml/SelfTest.qml",
    "qml/SessionMenu.qml",
    "qml/TerminalGroup.qml",
    "qml/TerminalPane.qml",
    "qml/TerminalPanel.qml",
    "qml/TextEditor.qml",
    "qml/TrashView.qml",
];

const RUST_BRIDGES: &[&str] = &[
    "src/annotations_model.rs",
    "src/app.rs",
    "src/calendar_model.rs",
    "src/file_ops.rs",
    "src/folder_model.rs",
    "src/pdf_items.rs",
    "src/qt_helpers.rs",
    "src/search_model.rs",
    "src/terminal_item.rs",
    "src/text_document.rs",
    "src/trash_model.rs",
    "src/tree_model.rs",
];

fn main() {
    let module = QmlModule::new("Atlas")
        .qml_files(QML_FILES.iter().map(QmlFile::from))
        .qml_file(QmlFile::from("qml/Theme.qml").singleton(true));
    CxxQtBuilder::new_qml_module(module)
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .qt_module("WebEngineCore")
        .qt_module("WebEngineQuick")
        .qrc_resources(web_resources())
        .qrc_resources(style_resources())
        .files(RUST_BRIDGES.iter().copied())
        .cpp_file("cpp/helpers.cpp")
        .build();

    // Qt is linked from the directory qmake reports. Record it as RPATH so
    // the binary runs from the dev shell without LD_LIBRARY_PATH; the Nix
    // package additionally wraps it with wrapQtAppsHook.
    let qmake = std::env::var("QMAKE").unwrap_or_else(|_| "qmake".into());
    if let Ok(out) = Command::new(qmake).args(["-query", "QT_INSTALL_LIBS"]).output()
        && out.status.success()
    {
        let lib = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        println!("cargo::rustc-link-arg-bins=-Wl,-rpath,{lib}");
    }
}

/// Atlas' Qt Quick Controls style, a plain QML module at
/// `qrc:/qt/qml/AtlasStyle` (on the default import path).
fn style_resources() -> QResources {
    let dir = std::path::Path::new("qml/AtlasStyle");
    let mut files: Vec<String> =
        std::fs::read_dir(dir).expect("style dir").flatten().map(|e| e.path().to_string_lossy().into_owned()).collect();
    files.sort();
    println!("cargo::rerun-if-changed=qml/AtlasStyle");
    QResources::new().resource(QResource::new().prefix("/qt").files(files))
}

/// The Markdown preview page and its bundled libraries, served from
/// `qrc:/atlas/web/…`.
fn web_resources() -> QResources {
    fn collect(dir: &std::path::Path, out: &mut Vec<String>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir).expect("web dir").flatten().collect();
        entries.sort_by_key(|e| e.path());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, out);
            } else if !matches!(path.extension().and_then(|e| e.to_str()), Some("md")) {
                out.push(path.to_string_lossy().into_owned());
            }
        }
    }
    let mut files = Vec::new();
    collect(std::path::Path::new("web"), &mut files);
    println!("cargo::rerun-if-changed=web");
    QResources::new().resource(QResource::new().prefix("/atlas").files(files))
}
