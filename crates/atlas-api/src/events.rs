// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Everything the core reports on its own: a single callback trait,
//! implemented by the UI. The core calls it from background threads; the
//! UI hops to its main thread itself (Swift: `MainActorEventSink`, FIFO via
//! the main queue, see `docs/porting/spike-uniffi.md`).

use std::sync::Arc;

use crate::search::{SearchMatch, SearchSummary};

/// Receiver of [`Event`]s. One per [`crate::app::Atlas`] (app-wide events)
/// and one per [`crate::project::Project`] (that window's events).
#[cfg_attr(feature = "ffi", uniffi::export(with_foreign))]
pub trait EventSink: Send + Sync {
    fn on_event(&self, event: Event);
}

pub type Sink = Arc<dyn EventSink>;

/// How serious a [`Event::Message`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Level {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Event {
    // --- app-wide (Atlas) ---
    /// Settings changed (by this or another window, or on disk).
    SettingsChanged,
    /// Sidebar places changed.
    PlacesChanged,
    /// Session list of a project changed.
    SessionsChanged { project_root: String },
    /// Installed or enabled extensions changed.
    ExtensionsChanged,
    /// A background job started, ended or wrote output.
    BackgroundJobChanged { key: String },

    // --- per project ---
    /// Files inside these folders were created, removed or renamed.
    FilesChanged { dirs: Vec<String> },
    /// A file's contents changed on disk (open editors reload or warn).
    FileModified { path: String },
    /// Project index progress; `ready` once searchable.
    IndexStatus { ready: bool, text: String },
    /// Notes, links, tags or relations changed; `revision` grows.
    NotesChanged { revision: u32 },
    /// Annotations of a PDF changed (also from another device via sync).
    AnnotationsChanged { path: String },
    /// Broken links after a move, see [`crate::notes::Notes::apply_link_repairs`].
    LinkRepairsFound { path: String, repairs: Vec<crate::notes::LinkRepair> },
    /// Calendar entries changed; `revision` grows.
    CalendarChanged { revision: u32 },
    /// Sync state changed, see [`crate::calendar::Calendar::sync_state`].
    SyncChanged,
    /// Results of a running text search, in order.
    SearchMatches { search_id: u64, matches: Vec<SearchMatch> },
    /// A text search ended (also when cancelled).
    SearchFinished { search_id: u64, summary: SearchSummary },
    /// A file operation (copy, move, trash) made progress or ended.
    FileOperation { operation_id: u64, done: bool, text: String, error: Option<String> },
    /// Something to show the user once (toast).
    Message { level: Level, text: String },
}
