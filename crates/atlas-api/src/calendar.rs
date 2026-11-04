// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Atlas calendar of a project: events and tasks (iCalendar in
//! `.atlas/`), links to files, reminders, sync with CalDAV or Google.
//! Template: `calendar_model.rs` and `calendar_model/{account,edit,remote}.rs`.
//! The macOS calendars (EventKit, E14) are handled in Swift (P3.13).
//!
//! Times cross the API as ISO 8601 strings: `2026-11-04T09:30` (local
//! time of [`Calendar::time_zone`]) or `2026-11-04` (all day); ends are
//! exclusive.

use crate::{Result, todo_task};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum EntryKind {
    Event,
    Task,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum EntryStatus {
    None,
    Open,
    InProgress,
    Done,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ChecklistItem {
    pub done: bool,
    pub text: String,
}

/// A calendar entry as edited (the whole series for recurring ones).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Entry {
    /// Empty for a new entry ([`Calendar::save`] assigns one).
    pub uid: String,
    pub kind: EntryKind,
    pub title: String,
    pub description: String,
    pub start: Option<String>,
    pub end: Option<String>,
    pub status: EntryStatus,
    pub tags: Vec<String>,
    pub checklist: Vec<ChecklistItem>,
    /// Project-relative files (`/docs/a.md`).
    pub files: Vec<String>,
    /// Linked session name, empty if none.
    pub session: String,
    pub estimate_minutes: Option<u32>,
    /// Minutes before the start.
    pub reminders: Vec<i64>,
    /// RRULE as written, `None` if not recurring.
    pub rrule: Option<String>,
    /// Calendar id: `local` or the URL of a remote calendar.
    pub calendar: String,
}

/// One occurrence in a time range (recurring entries expanded).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Occurrence {
    pub uid: String,
    /// Start of this occurrence (identifies it inside a series).
    pub instance: String,
    pub start: String,
    pub end: String,
    pub all_day: bool,
    pub kind: EntryKind,
    pub title: String,
    pub status: EntryStatus,
    pub calendar: String,
    pub color: String,
    pub recurring: bool,
    pub read_only: bool,
    pub files: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct CalendarInfo {
    /// `local` or the remote URL.
    pub id: String,
    pub name: String,
    pub color: String,
    pub visible: bool,
    /// Shown only in this project.
    pub project_only: bool,
    pub writable: bool,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Reminder {
    pub uid: String,
    pub instance: String,
    pub title: String,
    pub start: String,
    pub minutes_before: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Provider {
    None,
    CalDav,
    Google,
}

/// Account settings (the password is never part of it).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Account {
    pub provider: Provider,
    /// CalDAV server URL.
    pub url: String,
    pub user: String,
    /// Shell command printing the password (instead of a stored one).
    pub password_command: String,
    pub calendars: Vec<CalendarInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SyncState {
    pub connected: bool,
    pub google: bool,
    /// A password is available (typed, remembered or from the command).
    pub unlocked: bool,
    pub fetching_password: bool,
    /// `encrypted` (systemd-creds / keychain), `file` or empty.
    pub remembered: String,
    pub password_error: String,
    pub syncing: bool,
    pub status: String,
    pub failed: bool,
    /// Local changes not yet sent.
    pub pending: u32,
    pub conflicts: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ConflictChoice {
    KeepLocal,
    KeepRemote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Conflict {
    pub id: u32,
    pub uid: String,
    pub calendar: String,
    pub local: Option<Entry>,
    pub remote: Option<Entry>,
}

/// Result of a connection test.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ConnectionTest {
    pub message: String,
    pub calendars: Vec<CalendarInfo>,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Calendar {}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Calendar {
    /// IANA name of the zone times are shown in.
    pub fn time_zone(&self) -> String {
        atlas_core::calendar::local_zone().name().to_owned()
    }

    /// Occurrences overlapping `[from, to)`.
    pub fn occurrences(&self, from: String, to: String) -> Result<Vec<Occurrence>> {
        let _ = (from, to);
        todo_task!("P2.5")
    }

    /// Tasks without a time (backlog).
    pub fn tasks(&self) -> Vec<Entry> {
        todo_task!("P2.5")
    }

    pub fn entry(&self, uid: String) -> Option<Entry> {
        let _ = uid;
        todo_task!("P2.5")
    }

    /// Creates or replaces an entry; returns its uid.
    pub fn save(&self, entry: Entry) -> Result<String> {
        let _ = entry;
        todo_task!("P2.5")
    }

    pub fn remove(&self, uid: String) -> Result<()> {
        let _ = uid;
        todo_task!("P2.5")
    }

    /// Moves one occurrence (drag in the week view); for a series only that
    /// instance changes.
    pub fn move_occurrence(
        &self,
        uid: String,
        instance: String,
        start: String,
        end: String,
        all_day: bool,
    ) -> Result<()> {
        let _ = (uid, instance, start, end, all_day);
        todo_task!("P2.5")
    }

    /// Gives a backlog task a time, or takes it away.
    pub fn schedule(&self, uid: String, start: Option<String>, end: Option<String>) -> Result<()> {
        let _ = (uid, start, end);
        todo_task!("P2.5")
    }

    pub fn set_status(&self, uid: String, status: EntryStatus) -> Result<()> {
        let _ = (uid, status);
        todo_task!("P2.5")
    }

    pub fn toggle_check(&self, uid: String, index: u32) -> Result<()> {
        let _ = (uid, index);
        todo_task!("P2.5")
    }

    pub fn link_file(&self, uid: String, path: String) -> Result<()> {
        let _ = (uid, path);
        todo_task!("P2.5")
    }

    pub fn unlink_file(&self, uid: String, file: String) -> Result<()> {
        let _ = (uid, file);
        todo_task!("P2.5")
    }

    pub fn files_moved(&self, from: String, to: String) {
        let _ = (from, to);
        todo_task!("P2.5")
    }

    /// New event linked to `path` in `[start, end)`; returns its uid.
    pub fn block_for_file(&self, path: String, start: String, end: String) -> Result<String> {
        let _ = (path, start, end);
        todo_task!("P2.5")
    }

    /// The task linked to `path`, created if missing; returns its uid.
    pub fn task_for_file(&self, path: String) -> Result<String> {
        let _ = path;
        todo_task!("P2.5")
    }

    /// Open checklist items of a note as possible tasks.
    pub fn checklist_of(&self, path: String) -> Vec<ChecklistItem> {
        let _ = path;
        todo_task!("P2.5")
    }

    /// Adds tasks for these items, linked to `path`; returns how many.
    pub fn add_tasks(&self, path: String, items: Vec<String>) -> Result<u32> {
        let _ = (path, items);
        todo_task!("P2.5")
    }

    pub fn search(&self, query: String, limit: u32) -> Vec<Occurrence> {
        let _ = (query, limit);
        todo_task!("P2.5")
    }

    /// Reminders due now (the UI shows them once).
    pub fn due_reminders(&self) -> Vec<Reminder> {
        todo_task!("P2.5")
    }

    /// Last view (month/week, date) as opaque JSON, shared with Linux.
    pub fn view_state(&self) -> String {
        todo_task!("P2.5")
    }

    pub fn set_view_state(&self, state: String) {
        let _ = state;
        todo_task!("P2.5")
    }

    pub fn calendars(&self) -> Vec<CalendarInfo> {
        todo_task!("P2.5")
    }

    pub fn set_calendar_visible(&self, id: String, visible: bool) -> Result<()> {
        let _ = (id, visible);
        todo_task!("P2.5")
    }

    pub fn set_calendar_project_only(&self, id: String, only: bool) -> Result<()> {
        let _ = (id, only);
        todo_task!("P2.5")
    }

    /// Calendar new entries go to (`project`: only for this project).
    pub fn set_default_calendar(&self, id: String, project: bool) -> Result<()> {
        let _ = (id, project);
        todo_task!("P2.5")
    }

    // --- account and sync ---

    pub fn account(&self) -> Account {
        todo_task!("P2.5")
    }

    pub async fn test_connection(&self, url: String, user: String, password: String) -> Result<ConnectionTest> {
        let _ = (url, user, password);
        todo_task!("P2.5")
    }

    pub fn save_account(&self, account: Account) -> Result<()> {
        let _ = account;
        todo_task!("P2.5")
    }

    /// Starts the Google OAuth flow: returns the URL to open in the browser
    /// at once; the redirect is caught on localhost. Wait for it with
    /// [`Calendar::google_sign_in_finished`].
    pub fn google_sign_in(&self, client_id: String, client_secret: String) -> Result<String> {
        let _ = (client_id, client_secret);
        todo_task!("P2.5")
    }

    /// Ends when the browser came back (or after the timeout); `Ok` when
    /// signed in. Cancel with [`Calendar::cancel_google_sign_in`].
    pub async fn google_sign_in_finished(&self) -> Result<()> {
        todo_task!("P2.5")
    }

    pub fn cancel_google_sign_in(&self) {
        todo_task!("P2.5")
    }

    /// Unlocks sync for this run; `remember` keeps it in the keychain / systemd-creds.
    pub fn set_password(&self, password: String, remember: bool) -> Result<()> {
        let _ = (password, remember);
        todo_task!("P2.5")
    }

    pub fn forget_password(&self) -> Result<()> {
        todo_task!("P2.5")
    }

    pub fn sync_state(&self) -> SyncState {
        todo_task!("P2.5")
    }

    /// Syncs now in the background (`Event::SyncChanged`, `Event::CalendarChanged`).
    pub fn sync_now(&self) {
        todo_task!("P2.5")
    }

    pub async fn create_remote_calendar(&self, name: String) -> Result<CalendarInfo> {
        let _ = name;
        todo_task!("P2.5")
    }

    pub fn conflicts(&self) -> Vec<Conflict> {
        todo_task!("P2.5")
    }

    pub fn resolve_conflict(&self, id: u32, choice: ConflictChoice) -> Result<()> {
        let _ = (id, choice);
        todo_task!("P2.5")
    }

    /// Forgets downloaded remote entries (they are fetched again).
    pub fn clear_cache(&self) -> Result<()> {
        todo_task!("P2.5")
    }

    pub fn disconnect(&self) -> Result<()> {
        todo_task!("P2.5")
    }
}
