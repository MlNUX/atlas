// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atlas Workspace – Qt 6/QML shell.

mod annotations_model;
mod app;
mod calendar_model;
mod cli;
mod file_ops;
mod folder_model;
mod pdf_items;
mod qt_helpers;
mod search_model;
mod terminal_item;
mod text_document;
mod trash_model;
mod tree_model;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    cli::Cli::init();

    // Native Wayland only: never silently fall back to XWayland.
    // SAFETY: no other threads exist yet.
    unsafe {
        if std::env::var_os("QT_QPA_PLATFORM").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_some() {
            std::env::set_var("QT_QPA_PLATFORM", "wayland");
        }
        // Atlas' own style (qml/AtlasStyle, falls back to Fusion).
        if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
            std::env::set_var("QT_QUICK_CONTROLS_STYLE", "AtlasStyle");
        }
        // The incremental QML garbage collector (Qt 6.8+) lost values of
        // `var` properties set while a collection was running (seen with
        // the graph layout's allocations): collect in one go instead.
        if std::env::var_os("QV4_GC_TIMELIMIT").is_none() {
            std::env::set_var("QV4_GC_TIMELIMIT", "0");
        }
    }

    rename_legacy_extensions();
    atlas_core::listing::set_natural_sort(atlas_core::config::Config::load().files.natural_sort);

    // The Markdown preview uses QtWebEngine, which must be set up first.
    qt_helpers::init_web_engine();
    let mut app = QGuiApplication::new();
    // Wayland app_id, used by compositors such as Hyprland for window rules.
    QGuiApplication::set_desktop_file_name(&QString::from("atlas"));
    qt_helpers::install_web_scheme_handler();
    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from("Atlas"));
        app.as_mut().set_organization_name(&QString::from("atlas"));
        app.set_application_version(&QString::from(env!("CARGO_PKG_VERSION")));
    }

    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/Atlas/qml/Main.qml"));
    }
    if let Some(app) = app.as_mut() {
        std::process::exit(app.exec());
    }
}

/// Extensions from the template of Atlas 1.0 were all called
/// „Beispiel-Erweiterung“: rename them to „Core“, „Core 2“ … and keep
/// switched-off ones switched off.
fn rename_legacy_extensions() {
    use atlas_core::config::Config;
    let root = atlas_core::extensions::user_root(&atlas_core::config::config_dir());
    let renamed = atlas_core::extensions::migrate_legacy(&root);
    let mut config = Config::load().extensions;
    let mut changed = false;
    for (old, new) in &renamed {
        log::info!("extension {old} renamed to {new}");
        for name in config.disabled.iter_mut().filter(|n| *n == old) {
            *name = new.clone();
            changed = true;
        }
    }
    if changed && let Err(err) = Config::save_extensions(&config) {
        log::warn!("cannot save [extensions]: {err:#}");
    }
}
