// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! UI-neutral facade over the Atlas core: services as objects with plain
//! types (records and enums, no JSON), long work `async` or reported through
//! [`events::EventSink`]. No Qt and no Swift types; `atlas-ffi` exports it
//! with UniFFI (feature `ffi`), later the Qt bridge can use it too.
//!
//! Contract: `docs/porting/api.md`. Entry points: [`app::Atlas`] (once per
//! process) and [`project::Project`] (once per window, E9).
//!
//! All objects are `Send + Sync`; every method may be called from any
//! thread. Methods without `async` return within a few milliseconds.

#[cfg(feature = "ffi")]
uniffi::setup_scaffolding!("atlas");

pub mod actions;
pub mod app;
pub mod calendar;
pub mod editor;
pub mod error;
pub mod events;
pub mod extensions;
pub mod files;
pub mod ilias;
pub mod notes;
pub mod ocr;
pub mod pdf;
pub mod project;
pub mod search;
pub mod settings;
pub mod terminal;

mod util;

pub use error::{AtlasError, Result};
