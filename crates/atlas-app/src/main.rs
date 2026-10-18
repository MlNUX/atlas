// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atlas Workspace – Qt 6/QML shell.

mod app;
mod cli;
mod pdf_items;
mod qt_helpers;
mod search_model;
mod terminal_item;
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
        if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
            std::env::set_var("QT_QUICK_CONTROLS_STYLE", "Fusion");
        }
    }

    let mut app = QGuiApplication::new();
    // Wayland app_id, used by compositors such as Hyprland for window rules.
    QGuiApplication::set_desktop_file_name(&QString::from("atlas"));
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
