// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! System and application core of Atlas Workspace.
//!
//! Everything in this crate is independent of Qt so that it can be tested
//! without a display. The Qt/QML shell (`atlas-app`) only adapts these types
//! to QObjects and item models.

pub mod config;
pub mod fileops;
pub mod fs_tree;
pub mod identity;
pub mod listing;
pub mod markdown;
pub mod notes;
pub mod paths;
pub mod pdf;
pub mod project;
pub mod search;
pub mod session;
pub mod symbols;
pub mod terminal;
pub mod textfile;
pub mod watcher;
