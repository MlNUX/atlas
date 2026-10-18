// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

use std::process::Command;

use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

const QML_FILES: &[&str] = &[
    "qml/Main.qml",
    "qml/DocumentPane.qml",
    "qml/EditorArea.qml",
    "qml/IconButton.qml",
    "qml/PdfViewer.qml",
    "qml/ProjectTree.qml",
    "qml/SearchPanel.qml",
    "qml/TerminalGroup.qml",
    "qml/TerminalPane.qml",
    "qml/TerminalPanel.qml",
    "qml/TextViewer.qml",
];

const RUST_BRIDGES: &[&str] = &[
    "src/app.rs",
    "src/pdf_items.rs",
    "src/qt_helpers.rs",
    "src/search_model.rs",
    "src/terminal_item.rs",
    "src/tree_model.rs",
];

fn main() {
    let module = QmlModule::new("Atlas")
        .qml_files(QML_FILES.iter().map(QmlFile::from))
        .qml_file(QmlFile::from("qml/Theme.qml").singleton(true));
    CxxQtBuilder::new_qml_module(module)
        .qt_module("Quick")
        .qt_module("QuickControls2")
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
