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
    "qml/ContextModules.qml",
    "qml/ContextPanel.qml",
    "qml/DocumentView.qml",
    "qml/EditorArea.qml",
    "qml/EditorGroup.qml",
    "qml/EntryDialog.qml",
    "qml/ExtensionsDialog.qml",
    "qml/FileActions.qml",
    "qml/FolderView.qml",
    "qml/Icon.qml",
    "qml/IliasService.qml",
    "qml/IliasView.qml",
    "qml/IconButton.qml",
    "qml/Inspector.qml",
    "qml/MarkdownView.qml",
    "qml/MarkdownPdfExport.qml",
    "qml/PdfViewer.qml",
    "qml/PomodoroTimer.qml",
    "qml/PlacesList.qml",
    "qml/ProcessedView.qml",
    "qml/ProjectTree.qml",
    "qml/PropertiesDialog.qml",
    "qml/GraphDialog.qml",
    "qml/QuickLook.qml",
    "qml/RelateDialog.qml",
    "qml/FolderChooser.qml",
    "qml/PathBar.qml",
    "qml/SettingsDialog.qml",
    "qml/HtmlView.qml",
    "qml/SearchPanel.qml",
    "qml/SendToNoteDialog.qml",
    "qml/SelfTest.qml",
    "qml/SessionMenu.qml",
    "qml/TerminalGroup.qml",
    "qml/TerminalPane.qml",
    "qml/TerminalPanel.qml",
    "qml/TextEditor.qml",
    "qml/TouchpadFling.qml",
    "qml/TrashView.qml",
    "qml/WebRepaint.qml",
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
    let builder = CxxQtBuilder::new_qml_module(module)
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .qt_module("WebEngineCore")
        .qt_module("WebEngineQuick")
        .qrc_resources(web_resources())
        .qrc_resources(style_resources())
        .files(RUST_BRIDGES.iter().copied())
        .cpp_file("cpp/helpers.cpp")
        .cpp_file("cpp/touchpad_hold.cpp");
    // SAFETY: only adds include paths to the C++ compiler.
    let builder = unsafe {
        builder.cc_builder(|cc| {
            // Wayland client headers for the touchpad hold gesture.
            if let Ok(out) = Command::new("pkg-config").args(["--cflags-only-I", "wayland-client"]).output()
                && out.status.success()
            {
                for flag in String::from_utf8_lossy(&out.stdout).split_whitespace() {
                    cc.flag(flag);
                }
            }
        })
    };
    builder.build();
    if let Ok(out) = Command::new("pkg-config").args(["--libs", "wayland-client"]).output()
        && out.status.success()
    {
        for flag in String::from_utf8_lossy(&out.stdout).split_whitespace() {
            if let Some(dir) = flag.strip_prefix("-L") {
                println!("cargo::rustc-link-search=native={dir}");
                // Found at run time too (dev shell; packages use the system's).
                println!("cargo::rustc-link-arg-bins=-Wl,-rpath,{dir}");
            } else if let Some(lib) = flag.strip_prefix("-l") {
                println!("cargo::rustc-link-lib={lib}");
            }
        }
    }

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
