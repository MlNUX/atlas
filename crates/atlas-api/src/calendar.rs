// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Atlas calendar of a project: events, work blocks, deadlines and
//! tasks (iCalendar in `.atlas/calendar/`), links to files, reminders, sync
//! with CalDAV or Google.
//! Template: `calendar_model.rs` and `calendar_model/{account,edit,remote}.rs`.
//! The macOS calendars (EventKit, E14) are handled in Swift (P3.13).
//!
//! Times cross the API as ISO 8601 strings: `2026-11-04T09:30` (local
//! time of [`Calendar::time_zone`]) or `2026-11-04` (all day); ends are
//! exclusive.
//!
//! One instance per open project ([`crate::project::Project::calendar`]):
//! it keeps the entries, the running sync and the Google sign-in.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use atlas_core::calendar::sync::SyncStore;
use atlas_core::calendar::{self as core, Kind, LOCAL, Status, Store, ical::Time, recur};
use atlas_core::config::{CalendarConfig, Config};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use chrono_tz::Tz;
use serde_json::{Value, json};

use crate::events::{Event, Level, Sink};
use crate::{AtlasError, Result};

mod account;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum EntryKind {
    /// Termin
    Event,
    /// Arbeitsblock (a scheduled task)
    Block,
    /// Frist (a point in time, no end)
    Deadline,
    /// Aufgabe ohne Zeit (backlog)
    Task,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum EntryStatus {
    Planned,
    Started,
    Done,
    Postponed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ChecklistItem {
    pub done: bool,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

/// A repeat rule as the entry dialog edits it. Rules with parts Atlas cannot
/// edit (`supported == false`) stay untouched as long as the four editable
/// fields come back unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Recurrence {
    pub frequency: Frequency,
    pub interval: u32,
    pub count: Option<u32>,
    /// Last day (`2026-12-31`), inclusive.
    pub until: Option<String>,
    /// Read only: description for the UI („Jede Woche“ …).
    pub text: String,
    /// Read only: false if the rule has parts Atlas does not evaluate.
    pub supported: bool,
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
    /// `None` for tasks; kept as it is when saving another kind without one.
    pub start: Option<String>,
    /// `None` for tasks and timed deadlines.
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
    pub recurrence: Option<Recurrence>,
    /// Calendar id: `local` or the URL of a remote calendar; empty when
    /// saving a new entry: the default calendar.
    pub calendar: String,
    /// Read only: the calendar cannot be written.
    pub read_only: bool,
    /// Read only: changes wait for the server.
    pub pending: bool,
}

/// One occurrence in a time range (recurring entries expanded).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Occurrence {
    pub uid: String,
    /// Original start of this occurrence (identifies it inside a series;
    /// equal to `start` for single entries).
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
    pub session: String,
    pub checklist_done: u32,
    pub checklist_total: u32,
}

/// A task list item of a Markdown note (`- [ ] …`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct NoteTask {
    /// 1-based line.
    pub line: u32,
    pub done: bool,
    pub text: String,
    /// A task with this text linked to the note exists.
    pub planned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SearchHit {
    pub uid: String,
    pub title: String,
    pub kind: EntryKind,
    pub start: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct CalendarInfo {
    /// `local` or the remote URL.
    pub id: String,
    pub name: String,
    pub color: String,
    pub visible: bool,
    /// Shown only in some projects (the project calendar: always).
    pub project_only: bool,
    pub writable: bool,
    pub remote: bool,
    /// Gone on the server.
    pub missing: bool,
    /// New entries go here.
    pub is_default: bool,
    /// Default chosen for this project.
    pub project_default: bool,
    /// Default chosen for all projects.
    pub global_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Reminder {
    pub uid: String,
    pub title: String,
    /// Start of the occurrence.
    pub start: String,
    pub minutes_before: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Provider {
    None,
    /// Nextcloud or another CalDAV server.
    CalDav,
    Google,
}

/// Account settings (the password is never part of it).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Account {
    pub provider: Provider,
    /// CalDAV: server address (Nextcloud) …
    pub server: String,
    /// … or the CalDAV URL itself.
    pub dav_url: String,
    /// CalDAV user, Google e-mail.
    pub user: String,
    /// Shell command printing the password (instead of a stored one).
    pub password_command: String,
    /// Keep the password (Google: the sign-in) on this machine.
    pub remember_password: bool,
    /// The user's own OAuth client for Google.
    pub google_client_id: String,
    pub google_client_secret: String,
    /// Read only.
    pub last_sync: Option<String>,
    /// Read only: calendar home on the server.
    pub home: Option<String>,
    /// The remote calendars with their choice (`id`, `name`, `visible` are
    /// read by [`Calendar::save_account`]).
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
    /// Last problem reading or writing the calendar files, empty if none.
    pub problem: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ConflictChoice {
    KeepLocal,
    KeepRemote,
    /// The local version becomes a new entry.
    KeepBoth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Conflict {
    pub id: u32,
    pub uid: String,
    pub title: String,
    pub calendar: String,
    pub calendar_name: String,
    pub message: String,
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
pub struct Calendar {
    inner: Arc<Inner>,
}

impl Calendar {
    /// The calendar of the project at `root`; `atlas_dir` `None` (no
    /// writable `.atlas/`) uses Atlas' state folder like Linux does.
    pub(crate) fn new(root: &Path, atlas_dir: Option<&Path>, sink: Sink) -> Self {
        let dir = match atlas_dir {
            Some(a) => Store::for_atlas_dir(a).dir().to_path_buf(),
            None => atlas_core::paths::state_root().join(core::DIR),
        };
        let inner = Arc::new(Inner {
            root: root.to_path_buf(),
            dir,
            sink,
            tz: core::local_zone(),
            revision: AtomicU32::new(0),
            state: Mutex::new(State {
                entries: Vec::new(),
                config: CalendarConfig::default(),
                last_reminder_check: Utc::now(),
                syncing: false,
                sync_again: false,
                meta: BTreeMap::new(),
                waiting: BTreeSet::new(),
                problem: String::new(),
            }),
            sign_in: Mutex::new(None),
        });
        inner.reload();
        Self { inner }
    }
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Calendar {
    /// IANA name of the zone times are shown in.
    pub fn time_zone(&self) -> String {
        self.inner.tz.name().to_owned()
    }

    /// Grows with every change (also `Event::CalendarChanged`).
    pub fn revision(&self) -> u32 {
        self.inner.revision.load(Ordering::Relaxed)
    }

    /// Reads the calendar files and the account settings again.
    pub fn reload(&self) {
        self.inner.reload();
    }

    /// Occurrences overlapping `[from, to)`.
    pub fn occurrences(&self, from: String, to: String) -> Result<Vec<Occurrence>> {
        let (from, to) = (parse(&from)?, parse(&to)?);
        let inner = &self.inner;
        let s = inner.lock();
        let mut infos = BTreeMap::new();
        Ok(core::occurrences(&s.entries, inner.tz, from, to)
            .into_iter()
            .map(|o| {
                let e = master(&s, &o.uid);
                let calendar = e.map(|e| e.calendar.clone()).unwrap_or_else(|| LOCAL.into());
                let (_, color, writable) =
                    infos.entry(calendar.clone()).or_insert_with(|| inner.calendar_info(&s, &calendar)).clone();
                let time = |t: NaiveDateTime| if o.all_day { day(t) } else { iso(t) };
                Occurrence {
                    uid: o.uid.clone(),
                    instance: time(o.instance.unwrap_or(o.start)),
                    start: time(o.start),
                    end: time(o.end),
                    all_day: o.all_day,
                    kind: o.kind.into(),
                    title: o.title.clone(),
                    status: o.status.into(),
                    calendar,
                    color,
                    recurring: o.recurring,
                    read_only: !writable,
                    files: e.map(|e| e.files.clone()).unwrap_or_default(),
                    tags: e.map(|e| e.tags.clone()).unwrap_or_default(),
                    session: e.map(|e| e.session.clone()).unwrap_or_default(),
                    checklist_done: e.map(|e| e.checklist.iter().filter(|c| c.0).count() as u32).unwrap_or(0),
                    checklist_total: e.map(|e| e.checklist.len() as u32).unwrap_or(0),
                }
            })
            .collect())
    }

    /// Tasks without a time (backlog).
    pub fn tasks(&self) -> Vec<Entry> {
        let s = self.inner.lock();
        s.entries
            .iter()
            .filter(|e| e.kind == Kind::Task && e.start.is_none() && e.recurrence_id.is_none())
            .map(|e| self.inner.entry_out(&s, e))
            .collect()
    }

    pub fn entry(&self, uid: String) -> Option<Entry> {
        let s = self.inner.lock();
        master(&s, &uid).map(|e| self.inner.entry_out(&s, e))
    }

    /// Creates or replaces an entry; returns its uid.
    pub fn save(&self, entry: Entry) -> Result<String> {
        self.inner.save(entry)
    }

    pub fn remove(&self, uid: String) -> Result<()> {
        let inner = &self.inner;
        let calendar = {
            let mut s = inner.lock();
            let e = master(&s, &uid).ok_or_else(|| not_found(&uid))?;
            let calendar = e.calendar.clone();
            if !inner.writable(&s, &calendar) {
                return Err(read_only());
            }
            s.entries.retain(|e| e.uid != uid);
            calendar
        };
        let _ = inner.store().forget_links(&uid);
        inner.persist(&calendar, &uid)
    }

    /// Moves an occurrence so it starts at `start` and ends at `end`;
    /// `all_day` switches between timed and all-day. A series moves as a
    /// whole, by the distance its occurrence `instance` moved.
    pub fn move_occurrence(
        &self,
        uid: String,
        instance: String,
        start: String,
        end: String,
        all_day: bool,
    ) -> Result<()> {
        let (start, end) = (parse(&start)?, parse(&end)?);
        let instance = core::parse_local(&instance);
        let tz = self.inner.tz;
        self.inner.modify(&uid, |e| {
            let Some(old_start) = e.start else {
                return Err(AtlasError::Invalid {
                    message: "Eine Aufgabe ohne Zeit wird eingeplant, nicht verschoben.".into(),
                });
            };
            let reference = instance.unwrap_or_else(|| old_start.local(tz));
            let delta = start - reference;
            let length = (end - start).max(Duration::minutes(if all_day { 24 * 60 } else { 15 }));
            let new_start = old_start.local(tz) + delta;
            let zoned = |local: NaiveDateTime| Time::Zoned(local, tz);
            if all_day {
                e.start = Some(Time::Date(new_start.date()));
                e.end = Some(Time::Date(new_start.date() + Duration::days(length.num_days().max(1))));
            } else {
                e.start =
                    Some(if old_start.is_date() { zoned(new_start) } else { old_start.with_local(new_start, tz) });
                e.end = match (e.kind, e.end) {
                    (Kind::Deadline, None) if !old_start.is_date() => None,
                    (_, Some(old)) if !old.is_date() => Some(old.with_local(new_start + length, tz)),
                    _ => Some(zoned(new_start + length)),
                };
            }
            Ok(true)
        })
    }

    /// Plans a task as a work block from `start` to `end` (default: its
    /// estimate or an hour); `start` `None` turns a work block back into a
    /// task without time.
    pub fn schedule(&self, uid: String, start: Option<String>, end: Option<String>) -> Result<()> {
        let tz = self.inner.tz;
        let Some(start) = start else {
            return self.inner.modify(&uid, |e| {
                if e.start.is_none() || e.rrule.is_some() {
                    return Ok(false);
                }
                if e.estimate_minutes.is_none()
                    && let (Some(s), Some(end)) = (e.start, e.end)
                    && !s.is_date()
                {
                    e.estimate_minutes = Some((end.local(tz) - s.local(tz)).num_minutes().max(0) as u32);
                }
                e.kind = Kind::Task;
                e.start = None;
                e.end = None;
                Ok(true)
            });
        };
        let start = parse(&start)?;
        let end = end.as_deref().and_then(core::parse_local);
        self.inner.modify(&uid, |e| {
            let length = e.estimate_minutes.map(|m| Duration::minutes(m as i64)).unwrap_or(Duration::hours(1));
            let end = end.filter(|e| *e > start).unwrap_or(start + length);
            e.kind = Kind::Block;
            e.start = Some(Time::Zoned(start, tz));
            e.end = Some(Time::Zoned(end, tz));
            Ok(true)
        })
    }

    pub fn set_status(&self, uid: String, status: EntryStatus) -> Result<()> {
        self.inner.modify(&uid, |e| {
            e.status = status.into();
            Ok(true)
        })
    }

    pub fn toggle_check(&self, uid: String, index: u32) -> Result<()> {
        self.inner.modify(&uid, |e| match e.checklist.get_mut(index as usize) {
            Some(item) => {
                item.0 = !item.0;
                Ok(true)
            }
            None => Err(AtlasError::Invalid { message: format!("Kein Punkt {index} in der Checkliste") }),
        })
    }

    /// Links a project file (absolute path).
    pub fn link_file(&self, uid: String, path: String) -> Result<()> {
        let file = self.inner.relative(&path);
        self.inner.modify(&uid, |e| {
            if e.files.contains(&file) {
                return Ok(false);
            }
            e.files.push(file);
            Ok(true)
        })
    }

    /// Removes a link (`file` as in [`Entry::files`]).
    pub fn unlink_file(&self, uid: String, file: String) -> Result<()> {
        self.inner.modify(&uid, |e| {
            let before = e.files.len();
            e.files.retain(|f| f != &file);
            Ok(e.files.len() != before)
        })
    }

    /// Links follow a file or folder moved or renamed in Atlas (absolute paths).
    pub fn files_moved(&self, from: String, to: String) {
        self.inner.files_moved(&from, &to);
    }

    /// New work block linked to `path` (absolute) in `[start, end)`; returns its uid.
    pub fn block_for_file(&self, path: String, start: String, end: String) -> Result<String> {
        let start = parse(&start)?;
        let end = core::parse_local(&end).filter(|e| *e > start).unwrap_or(start + Duration::hours(1));
        let mut e = core::Entry::new(Kind::Block, &title_for(&path));
        e.start = Some(Time::Zoned(start, self.inner.tz));
        e.end = Some(Time::Zoned(end, self.inner.tz));
        e.files = vec![self.inner.relative(&path)];
        self.inner.add(e)
    }

    /// A new task without time linked to `path` (absolute); returns its uid.
    pub fn task_for_file(&self, path: String) -> Result<String> {
        let mut e = core::Entry::new(Kind::Task, &title_for(&path));
        e.files = vec![self.inner.relative(&path)];
        self.inner.add(e)
    }

    /// Task list items of the Markdown file `path` (absolute).
    pub fn checklist_of(&self, path: String) -> Vec<NoteTask> {
        let file = self.inner.relative(&path);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let s = self.inner.lock();
        core::checklist_items(&text)
            .into_iter()
            .map(|(line, done, text)| NoteTask {
                line: line as u32,
                done,
                planned: s.entries.iter().any(|e| e.title == text && e.files.iter().any(|f| f == &file)),
                text,
            })
            .collect()
    }

    /// Adds tasks for these items of the note `path` (absolute), linked to
    /// it; returns how many.
    pub fn add_tasks(&self, path: String, items: Vec<NoteTask>) -> Result<u32> {
        let file = self.inner.relative(&path);
        let mut count = 0;
        for item in items {
            let mut e = core::Entry::new(Kind::Task, &item.text);
            e.files = vec![file.clone()];
            if item.line > 0 {
                e.description = format!("Aus {file}, Zeile {}", item.line);
            }
            self.inner.add(e)?;
            count += 1;
        }
        Ok(count)
    }

    pub fn search(&self, query: String, limit: u32) -> Vec<SearchHit> {
        let s = self.inner.lock();
        let tz = self.inner.tz;
        let mut found: Vec<(i32, SearchHit)> = s
            .entries
            .iter()
            .filter(|e| e.recurrence_id.is_none())
            .filter_map(|e| {
                let hay = format!("{} {} {}", e.title, e.tags.join(" "), e.description);
                let score = atlas_core::finder::score(query.trim(), &hay)?;
                Some((
                    score,
                    SearchHit {
                        uid: e.uid.clone(),
                        title: e.title.clone(),
                        kind: e.kind.into(),
                        start: e.start.map(|t| time_out(&t, tz)),
                    },
                ))
            })
            .collect();
        found.sort_by_key(|f| std::cmp::Reverse(f.0));
        found.truncate(limit.max(1) as usize);
        found.into_iter().map(|f| f.1).collect()
    }

    /// Reminders due since the last call (the UI shows each once).
    pub fn due_reminders(&self) -> Vec<Reminder> {
        let now = Utc::now();
        let mut s = self.inner.lock();
        let from = std::mem::replace(&mut s.last_reminder_check, now);
        reminders_between(&s.entries, self.inner.tz, from, now)
    }

    /// Last view (month/week, date) as opaque JSON, shared with Linux.
    pub fn view_state(&self) -> String {
        self.inner.store().state().to_string()
    }

    pub fn set_view_state(&self, state: String) {
        if let Ok(v) = serde_json::from_str::<Value>(&state)
            && let Err(err) = self.inner.store().save_state(&v)
        {
            log::warn!("calendar state: {err:#}");
        }
    }

    pub fn calendars(&self) -> Vec<CalendarInfo> {
        let s = self.inner.lock();
        self.inner.calendars(&s)
    }

    /// Calendar new entries go to.
    pub fn default_calendar(&self) -> String {
        let s = self.inner.lock();
        self.inner.default_calendar(&s)
    }

    pub fn set_calendar_visible(&self, id: String, visible: bool) -> Result<()> {
        self.inner.change_choice(&id, |c, root| {
            c.hidden = !visible;
            // Shown in this project again: also if it was limited to others.
            if visible && !c.projects.is_empty() && !c.projects.iter().any(|p| p == root) {
                c.projects.push(root.to_owned());
            }
        })
    }

    /// Limits a calendar to this project (or shows it everywhere again).
    pub fn set_calendar_project_only(&self, id: String, only: bool) -> Result<()> {
        self.inner.change_choice(&id, |c, root| {
            c.projects = if only { vec![root.to_owned()] } else { Vec::new() };
        })
    }

    /// Calendar new entries go to (`project`: only for this project).
    pub fn set_default_calendar(&self, id: String, project: bool) -> Result<()> {
        let inner = &self.inner;
        let result = if project {
            let mut settings = inner.project_settings();
            settings["default_calendar"] = json!(id);
            serde_json::to_string_pretty(&settings).map_err(anyhow::Error::from).and_then(|t| {
                std::fs::create_dir_all(&inner.dir)?;
                atlas_core::paths::write_atomic(&inner.dir.join(SETTINGS), t.as_bytes())
            })
        } else {
            let mut config = Config::load().calendar;
            config.default = if id == LOCAL { String::new() } else { id };
            Config::save_calendar(&config)
        };
        inner.reload();
        result.map_err(|e| failed("Standardkalender kann nicht gespeichert werden", e))
    }

    // --- account and sync (calendar/account.rs) ---

    pub fn account(&self) -> Account {
        self.inner.account()
    }

    /// Checks a CalDAV account and lists its calendars. Empty `password`:
    /// the one known in this run, else the account's password command.
    pub async fn test_connection(&self, account: Account, password: String) -> Result<ConnectionTest> {
        self.inner.test_connection(account, password).await
    }

    /// Saves the account and the chosen calendars, then syncs. A change of
    /// provider or user forgets the old sign-in, password and cache;
    /// [`Provider::None`] disconnects.
    pub fn save_account(&self, account: Account) -> Result<()> {
        self.inner.save_account(account)
    }

    /// Starts the Google OAuth flow: returns the URL to open in the browser
    /// at once; the redirect is caught on localhost. Wait for it with
    /// [`Calendar::google_sign_in_finished`].
    pub fn google_sign_in(&self, client_id: String, client_secret: String) -> Result<String> {
        self.inner.google_sign_in(client_id, client_secret)
    }

    /// Ends when the browser came back (or after the timeout); returns the
    /// message (`Angemeldet als … – n Kalender gefunden`). The found
    /// calendars are in [`Calendar::account`] then. Cancel with
    /// [`Calendar::cancel_google_sign_in`].
    pub async fn google_sign_in_finished(&self) -> Result<String> {
        self.inner.clone().google_sign_in_finished().await
    }

    pub fn cancel_google_sign_in(&self) {
        self.inner.cancel_google_sign_in();
    }

    /// Unlocks sync for this run; `remember` keeps it on this machine
    /// (keychain / systemd-creds) and is stored as the account's choice.
    pub fn set_password(&self, password: String, remember: bool) -> Result<()> {
        self.inner.set_password(password, remember)
    }

    /// Forgets the remembered password (the one of this run stays).
    pub fn forget_password(&self) -> Result<()> {
        self.inner.remember_password(false)
    }

    /// Runs the password command again (after it failed).
    pub fn retry_password_command(&self) {
        account::reset_command();
        self.inner.fetch_password();
    }

    pub fn sync_state(&self) -> SyncState {
        self.inner.sync_state()
    }

    /// Syncs now in the background (`Event::SyncChanged`,
    /// `Event::CalendarChanged`, the result as `Event::Message`).
    pub fn sync_now(&self) {
        self.inner.sync_now();
    }

    /// Creates a calendar on the server (after the user confirmed).
    pub async fn create_remote_calendar(&self, name: String) -> Result<CalendarInfo> {
        self.inner.clone().create_remote_calendar(name).await
    }

    pub fn conflicts(&self) -> Vec<Conflict> {
        self.inner.conflicts()
    }

    pub fn resolve_conflict(&self, id: u32, choice: ConflictChoice) -> Result<()> {
        self.inner.resolve_conflict(id, choice)
    }

    /// Forgets downloaded remote entries (pending changes included; they are
    /// fetched again).
    pub fn clear_cache(&self) -> Result<()> {
        let result = self.inner.sync_store().clear();
        self.inner.reload();
        self.inner.sync_now();
        result.map_err(|e| failed("Cache kann nicht gelöscht werden", e))
    }

    /// Forgets account, password and cache; nothing is deleted on the server.
    pub fn disconnect(&self) -> Result<()> {
        self.inner.disconnect()
    }
}

// ------------------------------------------------------------------ inside

const ISO: &str = "%Y-%m-%dT%H:%M";
const DATE: &str = "%Y-%m-%d";
/// Per-project calendar settings (`default_calendar`), shared with Linux.
const SETTINGS: &str = "settings.json";
const LOCAL_NAME: &str = "Projekt (nur lokal)";
const LOCAL_COLOR: &str = "#4fa3ff";
const REMOTE_COLOR: &str = "#0082c9";

pub(crate) struct Inner {
    root: PathBuf,
    /// `.atlas/calendar/` (or the state folder's).
    dir: PathBuf,
    sink: Sink,
    tz: Tz,
    revision: AtomicU32,
    state: Mutex<State>,
    sign_in: Mutex<Option<account::SignIn>>,
}

struct State {
    /// Project entries and those of the visible remote calendars.
    entries: Vec<core::Entry>,
    config: CalendarConfig,
    last_reminder_check: DateTime<Utc>,
    syncing: bool,
    /// Another sync was requested while one ran.
    sync_again: bool,
    /// Remote calendars as last read from the cache: URL → (name, colour,
    /// writable and present).
    meta: BTreeMap<String, (String, String, bool)>,
    /// (calendar, uid) of entries with changes waiting for the server.
    waiting: BTreeSet<(String, String)>,
    problem: String,
}

fn iso(t: NaiveDateTime) -> String {
    t.format(ISO).to_string()
}

fn day(t: NaiveDateTime) -> String {
    t.format(DATE).to_string()
}

fn time_out(t: &Time, tz: Tz) -> String {
    if t.is_date() { day(t.local(tz)) } else { iso(t.local(tz)) }
}

fn parse(text: &str) -> Result<NaiveDateTime> {
    core::parse_local(text).ok_or_else(|| AtlasError::Invalid { message: format!("Ungültige Zeit: {text}") })
}

fn not_found(uid: &str) -> AtlasError {
    AtlasError::NotFound { message: format!("Kein Kalendereintrag {uid}") }
}

fn read_only() -> AtlasError {
    AtlasError::Invalid { message: "Der Kalender ist schreibgeschützt.".into() }
}

fn failed(what: &str, err: anyhow::Error) -> AtlasError {
    AtlasError::Failed { message: format!("{what}: {err:#}") }
}

/// The master entry (not a changed occurrence of a series).
fn master<'a>(s: &'a State, uid: &str) -> Option<&'a core::Entry> {
    s.entries.iter().find(|e| e.uid == uid && e.recurrence_id.is_none())
}

fn index_of(s: &State, uid: &str) -> Option<usize> {
    s.entries.iter().position(|e| e.uid == uid && e.recurrence_id.is_none())
}

fn title_for(path: &str) -> String {
    let p = Path::new(path);
    p.file_stem().or(p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_owned())
}

fn reminders_between(entries: &[core::Entry], tz: Tz, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<Reminder> {
    core::due_reminders(entries, tz, from, to)
        .into_iter()
        .map(|(uid, title, start, minutes)| Reminder { uid, title, start: iso(start), minutes_before: minutes })
        .collect()
}

fn recurrence_out(rrule: Option<&str>) -> Option<Recurrence> {
    let rule = recur::Rule::parse(rrule?)?;
    Some(Recurrence {
        frequency: match rule.freq {
            recur::Freq::Daily => Frequency::Daily,
            recur::Freq::Weekly => Frequency::Weekly,
            recur::Freq::Monthly => Frequency::Monthly,
            recur::Freq::Yearly => Frequency::Yearly,
        },
        interval: rule.interval,
        count: rule.count,
        until: rule.until.map(day),
        text: rule.describe(),
        supported: rule.supported,
    })
}

/// The editable part of a rule (what decides whether it was changed).
fn editable(r: &Option<Recurrence>) -> Option<(Frequency, u32, Option<u32>, Option<String>)> {
    r.as_ref().map(|r| (r.frequency, r.interval.max(1), r.count.filter(|c| *c > 0), r.until.clone()))
}

fn rule_of(r: &Recurrence) -> String {
    let mut rule = recur::Rule::new(match r.frequency {
        Frequency::Daily => recur::Freq::Daily,
        Frequency::Weekly => recur::Freq::Weekly,
        Frequency::Monthly => recur::Freq::Monthly,
        Frequency::Yearly => recur::Freq::Yearly,
    });
    rule.interval = r.interval.max(1);
    rule.count = r.count.filter(|c| *c > 0);
    rule.until =
        r.until.as_deref().and_then(core::parse_local).map(|d| d + Duration::hours(23) + Duration::minutes(59));
    rule.to_value()
}

impl From<Kind> for EntryKind {
    fn from(k: Kind) -> Self {
        match k {
            Kind::Event => EntryKind::Event,
            Kind::Block => EntryKind::Block,
            Kind::Deadline => EntryKind::Deadline,
            Kind::Task => EntryKind::Task,
        }
    }
}

impl From<EntryKind> for Kind {
    fn from(k: EntryKind) -> Self {
        match k {
            EntryKind::Event => Kind::Event,
            EntryKind::Block => Kind::Block,
            EntryKind::Deadline => Kind::Deadline,
            EntryKind::Task => Kind::Task,
        }
    }
}

impl From<Status> for EntryStatus {
    fn from(s: Status) -> Self {
        match s {
            Status::Planned => EntryStatus::Planned,
            Status::Started => EntryStatus::Started,
            Status::Done => EntryStatus::Done,
            Status::Postponed => EntryStatus::Postponed,
        }
    }
}

impl From<EntryStatus> for Status {
    fn from(s: EntryStatus) -> Self {
        match s {
            EntryStatus::Planned => Status::Planned,
            EntryStatus::Started => Status::Started,
            EntryStatus::Done => Status::Done,
            EntryStatus::Postponed => Status::Postponed,
        }
    }
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn emit(&self, event: Event) {
        self.sink.on_event(event);
    }

    fn message(&self, level: Level, text: String) {
        self.emit(Event::Message { level, text });
    }

    fn bump(&self) {
        let revision = self.revision.fetch_add(1, Ordering::Relaxed) + 1;
        self.emit(Event::CalendarChanged { revision });
    }

    fn store(&self) -> Store {
        Store::new(&self.dir)
    }

    fn sync_store(&self) -> SyncStore {
        SyncStore::new(&self.dir)
    }

    fn root_text(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }

    /// `/rel/path` for files inside the project, else the path as given.
    fn relative(&self, path: &str) -> String {
        match Path::new(path).strip_prefix(&self.root) {
            Ok(rel) => format!("/{}", rel.to_string_lossy()),
            _ => path.to_owned(),
        }
    }

    fn set_problem(&self, problem: String) {
        let changed = {
            let mut s = self.lock();
            let changed = s.problem != problem;
            s.problem = problem;
            changed
        };
        if changed {
            self.emit(Event::SyncChanged);
        }
    }

    /// Remote calendars shown (and synced) in this project.
    fn visible_remote(&self, s: &State) -> Vec<String> {
        if !s.config.connected() {
            return Vec::new();
        }
        let root = self.root_text();
        s.config.calendars.iter().filter(|c| c.visible_in(&root)).map(|c| c.url.clone()).collect()
    }

    fn writable(&self, s: &State, calendar: &str) -> bool {
        calendar == LOCAL || s.meta.get(calendar).is_none_or(|m| m.2)
    }

    /// Name, colour and writability of a calendar.
    fn calendar_info(&self, s: &State, id: &str) -> (String, String, bool) {
        if id == LOCAL {
            return (LOCAL_NAME.into(), LOCAL_COLOR.into(), true);
        }
        s.meta.get(id).cloned().unwrap_or_else(|| {
            let name = s
                .config
                .calendars
                .iter()
                .find(|c| c.url == id)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| id.to_owned());
            (name, REMOTE_COLOR.into(), true)
        })
    }

    fn project_settings(&self) -> Value {
        std::fs::read_to_string(self.dir.join(SETTINGS))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(json!({}))
    }

    fn default_calendar(&self, s: &State) -> String {
        let visible = self.visible_remote(s);
        let usable = |id: &str| id == LOCAL || (visible.iter().any(|v| v == id) && self.writable(s, id));
        let project = self.project_settings()["default_calendar"].as_str().unwrap_or_default().to_owned();
        if !project.is_empty() && usable(&project) {
            return project;
        }
        if !s.config.default.is_empty() && usable(&s.config.default) {
            return s.config.default.clone();
        }
        LOCAL.into()
    }

    fn calendars(&self, s: &State) -> Vec<CalendarInfo> {
        let default = self.default_calendar(s);
        let project_default = self.project_settings()["default_calendar"].as_str().unwrap_or_default().to_owned();
        let mut list = vec![CalendarInfo {
            id: LOCAL.into(),
            name: LOCAL_NAME.into(),
            color: LOCAL_COLOR.into(),
            visible: true,
            project_only: true,
            writable: true,
            remote: false,
            missing: false,
            is_default: default == LOCAL,
            project_default: project_default == LOCAL,
            global_default: s.config.default.is_empty(),
        }];
        if s.config.connected() {
            let state = self.sync_store().load();
            let root = self.root_text();
            for c in &s.config.calendars {
                let (name, color, writable) = self.calendar_info(s, &c.url);
                list.push(CalendarInfo {
                    id: c.url.clone(),
                    name: if name == c.url && !c.name.is_empty() { c.name.clone() } else { name },
                    color,
                    visible: c.visible_in(&root),
                    project_only: !c.projects.is_empty(),
                    writable,
                    remote: true,
                    missing: state.calendars.get(&c.url).is_some_and(|x| x.missing),
                    is_default: default == c.url,
                    project_default: project_default == c.url,
                    global_default: s.config.default == c.url,
                });
            }
        }
        list
    }

    fn entry_out(&self, s: &State, e: &core::Entry) -> Entry {
        let tz = self.tz;
        Entry {
            uid: e.uid.clone(),
            kind: e.kind.into(),
            title: e.title.clone(),
            description: e.description.clone(),
            start: e.start.map(|t| time_out(&t, tz)),
            end: e.end.map(|t| time_out(&t, tz)),
            status: e.status.into(),
            tags: e.tags.clone(),
            checklist: e
                .checklist
                .iter()
                .map(|(done, text)| ChecklistItem { done: *done, text: text.clone() })
                .collect(),
            files: e.files.clone(),
            session: e.session.clone(),
            estimate_minutes: e.estimate_minutes,
            reminders: e.reminders.clone(),
            recurrence: recurrence_out(e.rrule.as_deref()),
            calendar: e.calendar.clone(),
            read_only: !self.writable(s, &e.calendar),
            pending: s.waiting.contains(&(e.calendar.clone(), e.uid.clone())),
        }
    }

    /// Reads entries and account settings again.
    fn reload(self: &Arc<Self>) {
        let config = Config::load().calendar;
        let store = self.store();
        let (mut entries, problem) = match store.load() {
            Ok(e) => (e, String::new()),
            Err(err) => {
                log::warn!("calendar: {err:#}");
                (Vec::new(), format!("{err:#}"))
            }
        };
        {
            let mut s = self.lock();
            s.config = config;
            let remote = self.visible_remote(&s);
            if !remote.is_empty() {
                let links = store.links();
                for mut e in self.sync_store().load().entries(&remote) {
                    if let Some(l) = links.get(&e.uid) {
                        if e.files.is_empty() {
                            e.files = l.files.clone();
                        }
                        if e.session.is_empty() {
                            e.session = l.session.clone();
                        }
                    }
                    entries.push(e);
                }
            }
            s.entries = entries;
        }
        self.set_problem(problem);
        self.update_sync_state();
        self.fetch_password();
        self.bump();
    }

    /// Rewrites `events.ics` if `calendar` is the project calendar (remote
    /// entries go through the queue) and records the links of all entries.
    fn commit(&self, s: &State, calendar: &str) -> anyhow::Result<()> {
        let store = self.store();
        if calendar == LOCAL {
            let local: Vec<core::Entry> = s.entries.iter().filter(|e| e.calendar == LOCAL).cloned().collect();
            store.save(&local)?;
        }
        store.save_links(s.entries.iter())
    }

    /// Queues the object `uid` of the remote `calendar` for the server.
    fn queue(&self, s: &State, calendar: &str, uid: &str) -> anyhow::Result<()> {
        let parts: Vec<&core::Entry> = s.entries.iter().filter(|e| e.uid == uid && e.calendar == calendar).collect();
        if parts.is_empty() {
            self.sync_store().update(|q| q.local_delete(calendar, uid))
        } else {
            let data = core::sync::object_data(&parts);
            self.sync_store().update(|q| q.local_put(calendar, uid, data))
        }
    }

    /// Stores the changed entry `uid` of `calendar` and sends it if remote.
    fn persist(self: &Arc<Self>, calendar: &str, uid: &str) -> Result<()> {
        let result = {
            let s = self.lock();
            self.commit(&s, calendar).map_err(|e| failed("Kalender kann nicht gespeichert werden", e)).and_then(|()| {
                if calendar == LOCAL || calendar.is_empty() {
                    return Ok(());
                }
                self.queue(&s, calendar, uid).map_err(|e| failed("Kalender-Cache kann nicht geschrieben werden", e))
            })
        };
        self.set_problem(result.as_ref().err().map(|e| e.to_string()).unwrap_or_default());
        if calendar != LOCAL && !calendar.is_empty() {
            self.update_sync_state();
            self.sync_now();
        }
        self.bump();
        result
    }

    /// Changes the entry `uid` with `f` (`Ok(false)`: nothing changed) and stores it.
    fn modify(self: &Arc<Self>, uid: &str, f: impl FnOnce(&mut core::Entry) -> Result<bool>) -> Result<()> {
        let calendar = {
            let mut s = self.lock();
            let i = index_of(&s, uid).ok_or_else(|| not_found(uid))?;
            let calendar = s.entries[i].calendar.clone();
            if !self.writable(&s, &calendar) {
                return Err(read_only());
            }
            let mut e = s.entries[i].clone();
            if !f(&mut e)? {
                return Ok(());
            }
            e.sequence += 1;
            s.entries[i] = e;
            calendar
        };
        self.persist(&calendar, uid)
    }

    /// Adds a new entry in the default calendar; returns its uid.
    fn add(self: &Arc<Self>, mut e: core::Entry) -> Result<String> {
        let (uid, calendar) = {
            let mut s = self.lock();
            e.calendar = self.default_calendar(&s);
            // Google's calendars hold events only: tasks stay in the project.
            if e.kind == Kind::Task && s.config.is_google() {
                e.calendar = LOCAL.into();
            }
            let ids = (e.uid.clone(), e.calendar.clone());
            s.entries.push(e);
            ids
        };
        self.persist(&calendar, &uid)?;
        Ok(uid)
    }

    fn save(self: &Arc<Self>, input: Entry) -> Result<String> {
        let tz = self.tz;
        let kind: Kind = input.kind.into();
        let (uid, calendar, old_calendar, existed) = {
            let mut s = self.lock();
            let existing = match input.uid.as_str() {
                "" => None,
                uid => Some(index_of(&s, uid).ok_or_else(|| not_found(uid))?),
            };
            let mut entry = match existing {
                Some(i) => s.entries[i].clone(),
                None => {
                    let mut e = core::Entry::new(kind, "");
                    e.calendar = self.default_calendar(&s);
                    e
                }
            };
            let old_calendar = entry.calendar.clone();
            if !input.calendar.is_empty() {
                entry.calendar = input.calendar.clone();
            }
            if !self.writable(&s, &old_calendar) || !self.writable(&s, &entry.calendar) {
                return Err(read_only());
            }
            entry.kind = kind;
            if kind == Kind::Task && s.config.is_google() && entry.calendar != LOCAL {
                entry.calendar = LOCAL.into();
            }
            entry.title = input.title.trim().to_owned();
            if entry.title.is_empty() {
                entry.title = "Ohne Titel".into();
            }
            entry.description = input.description.clone();
            entry.status = input.status.into();
            entry.tags = input
                .tags
                .iter()
                .map(|t| t.trim().trim_start_matches('#').to_owned())
                .filter(|t| !t.is_empty())
                .collect();
            entry.files = input.files.clone();
            entry.session = input.session.clone();
            entry.checklist = input
                .checklist
                .iter()
                .map(|c| (c.done, c.text.trim().to_owned()))
                .filter(|c| !c.1.is_empty())
                .collect();
            entry.estimate_minutes = input.estimate_minutes;
            entry.reminders = input.reminders.clone();

            // Times: tasks have none; all-day entries use dates.
            if kind == Kind::Task {
                entry.start = None;
                entry.end = None;
            } else if let Some(text) = input.start.as_deref() {
                let start = parse(text)?;
                let all_day = !text.contains('T');
                let end = input.end.as_deref().and_then(core::parse_local);
                let end = if all_day {
                    end.filter(|e| *e > start).unwrap_or(start + Duration::days(1))
                } else {
                    end.filter(|e| *e >= start).unwrap_or(start + Duration::hours(1))
                };
                let new_time = |t: NaiveDateTime| if all_day { Time::Date(t.date()) } else { Time::Zoned(t, tz) };
                // Keep the zone kind of existing entries (UTC from a server stays UTC).
                entry.start = Some(match entry.start {
                    Some(old) if !all_day && !old.is_date() => old.with_local(start, tz),
                    _ => new_time(start),
                });
                entry.end = if kind == Kind::Deadline && !all_day {
                    None
                } else {
                    Some(match entry.end {
                        Some(old) if !all_day && !old.is_date() => old.with_local(end, tz),
                        _ => new_time(end),
                    })
                };
            } else if entry.start.is_none() {
                return Err(AtlasError::Invalid { message: "Der Eintrag braucht eine Startzeit.".into() });
            }

            // Recurrence: only replaced when it was changed, so rules Atlas
            // cannot fully edit stay as they are.
            let old_rule = recurrence_out(entry.rrule.as_deref());
            if editable(&old_rule) != editable(&input.recurrence) {
                entry.rrule = match &input.recurrence {
                    Some(r) if entry.start.is_some() => Some(rule_of(r)),
                    _ => None,
                };
                entry.exdates.clear();
                // Changed occurrences of the old rule no longer fit.
                let uid = entry.uid.clone();
                s.entries.retain(|e| !(e.uid == uid && e.recurrence_id.is_some()));
            }
            entry.sequence += 1;
            let uid = entry.uid.clone();
            let calendar = entry.calendar.clone();
            // Changed occurrences of a series move with it to another calendar.
            for e in s.entries.iter_mut().filter(|e| e.uid == uid && e.recurrence_id.is_some()) {
                e.calendar = calendar.clone();
            }
            match index_of(&s, &uid) {
                Some(i) => s.entries[i] = entry,
                None => s.entries.push(entry),
            }
            (uid, calendar, old_calendar, existing.is_some())
        };
        if existed && old_calendar != calendar {
            if old_calendar == LOCAL {
                let s = self.lock();
                self.commit(&s, LOCAL).map_err(|e| failed("Kalender kann nicht gespeichert werden", e))?;
            } else {
                // Removed from the old remote calendar.
                let _ = self.sync_store().update(|q| q.local_delete(&old_calendar, &uid));
            }
        }
        self.persist(&calendar, &uid)?;
        Ok(uid)
    }

    fn files_moved(self: &Arc<Self>, from: &str, to: &str) {
        let (from, to) = (self.relative(from), self.relative(to));
        if !from.starts_with('/') || !to.starts_with('/') {
            return;
        }
        let moved = |f: &str| -> Option<String> {
            if f == from {
                Some(to.clone())
            } else {
                f.strip_prefix(&format!("{from}/")).map(|rest| format!("{to}/{rest}"))
            }
        };
        let touched: Vec<(String, String)> = {
            let mut s = self.lock();
            let touched: Vec<(String, String)> = s
                .entries
                .iter()
                .filter(|e| e.files.iter().any(|f| moved(f).is_some()))
                .map(|e| (e.uid.clone(), e.calendar.clone()))
                .collect();
            for e in s.entries.iter_mut() {
                for f in e.files.iter_mut() {
                    if let Some(n) = moved(f) {
                        *f = n;
                    }
                }
            }
            touched
        };
        if touched.is_empty() {
            return;
        }
        // Local entries are rewritten; links are recorded for all.
        let local = touched.iter().any(|t| t.1 == LOCAL);
        let result = self.persist(if local { LOCAL } else { "" }, "");
        if let Err(err) = result {
            log::warn!("calendar links: {err}");
        }
        for (uid, calendar) in touched.iter().filter(|t| t.1 != LOCAL) {
            let _ = self.persist(calendar, uid);
        }
    }

    fn change_choice(
        self: &Arc<Self>,
        id: &str,
        f: impl FnOnce(&mut atlas_core::config::CalendarChoice, &str),
    ) -> Result<()> {
        let mut config = Config::load().calendar;
        let root = self.root_text();
        let choice = config
            .calendars
            .iter_mut()
            .find(|c| c.url == id)
            .ok_or_else(|| AtlasError::NotFound { message: format!("Kein Kalender {id}") })?;
        f(choice, &root);
        let result = Config::save_calendar(&config);
        self.reload();
        self.sync_now();
        result.map_err(|e| failed("Einstellungen können nicht gespeichert werden", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_rules_compare_equal() {
        let unsupported = recurrence_out(Some("FREQ=WEEKLY;BYSETPOS=2;BYDAY=MO,TU")).unwrap();
        let mut edited = unsupported.clone();
        edited.text = String::new();
        edited.supported = true;
        assert_eq!(editable(&Some(unsupported)), editable(&Some(edited.clone())));
        edited.interval = 2;
        assert_ne!(editable(&recurrence_out(Some("FREQ=WEEKLY"))), editable(&Some(edited)));
    }

    #[test]
    fn rule_round_trip() {
        let r = Recurrence {
            frequency: Frequency::Monthly,
            interval: 2,
            count: None,
            until: Some("2026-12-31".into()),
            text: String::new(),
            supported: true,
        };
        let back = recurrence_out(Some(&rule_of(&r))).unwrap();
        assert_eq!(editable(&Some(back)), editable(&Some(r)));
    }
}
