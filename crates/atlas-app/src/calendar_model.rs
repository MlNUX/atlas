// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! `CalendarModel`: the project calendar for QML (see
//! `atlas_core::calendar`). Times cross the bridge as local ISO strings
//! (`2026-10-19T14:30`) in the user's time zone; lists and entries as JSON.
//!
//! Entries come from the project (`events.ics`) and from the visible
//! Nextcloud calendars (cache in `.atlas/calendar/cache/`). Changes to
//! Nextcloud entries go into the offline queue and are sent right away when
//! the app password is known; it is asked once per Atlas start (or printed
//! by the configured password command) and only kept in memory.

use std::path::Path;
use std::pin::Pin;
use std::sync::Mutex;

use atlas_core::calendar::caldav::{Account, Client, Remote};
use atlas_core::calendar::google;
use atlas_core::calendar::sync::{self, Choice, SyncStore};
use atlas_core::calendar::{self, Entry, Kind, LOCAL, Status, Store, ical::Time, recur};
use atlas_core::config::{CalendarChoice, CalendarConfig, Config};
use atlas_core::secret::SecretStore;
use chrono::{Duration, NaiveDateTime, Utc};
use chrono_tz::Tz;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use crate::util::qs;

mod account;
mod edit;
mod remote;
use serde_json::{Value, json};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        /// `.atlas` of the project ("" = Atlas' state folder).
        #[qproperty(QString, atlas_dir, cxx_name = "atlasDir", READ, WRITE, NOTIFY)]
        #[qproperty(QString, project_root, cxx_name = "projectRoot", READ, WRITE, NOTIFY)]
        /// Incremented after every change (views reload).
        #[qproperty(i32, revision, READ, NOTIFY)]
        /// Last problem reading or writing the calendar ("" if none).
        #[qproperty(QString, error, READ, NOTIFY)]
        /// IANA name of the time zone used for display.
        #[qproperty(QString, zone, READ, CONSTANT)]
        /// A Nextcloud account is set up.
        #[qproperty(bool, connected, READ, NOTIFY = sync_changed)]
        /// The account is a Google account (else Nextcloud/CalDAV).
        #[qproperty(bool, google, READ, NOTIFY = sync_changed)]
        /// The app password for this Atlas start is known.
        #[qproperty(bool, unlocked, READ, NOTIFY = sync_changed)]
        /// The password command is running.
        #[qproperty(bool, fetching_password, cxx_name = "fetchingPassword", READ, NOTIFY = sync_changed)]
        /// How the password is remembered on this machine: "encrypted"
        /// (systemd-creds), "file" (mode 600) or "" (not).
        #[qproperty(QString, remembered, READ, NOTIFY = sync_changed)]
        /// Why the password command failed ("" if it did not).
        #[qproperty(QString, password_error, cxx_name = "passwordError", READ, NOTIFY = sync_changed)]
        #[qproperty(bool, syncing, READ, NOTIFY = sync_changed)]
        /// Last synchronisation or its error, for the UI.
        #[qproperty(QString, sync_status, cxx_name = "syncStatus", READ, NOTIFY = sync_changed)]
        #[qproperty(bool, sync_failed, cxx_name = "syncFailed", READ, NOTIFY = sync_changed)]
        /// Local changes not yet on the server.
        #[qproperty(i32, pending, READ, NOTIFY = sync_changed)]
        #[qproperty(i32, conflict_count, cxx_name = "conflictCount", READ, NOTIFY = sync_changed)]
        type CalendarModel = super::CalendarModelRust;

        #[qsignal]
        #[cxx_name = "syncChanged"]
        fn sync_changed(self: Pin<&mut CalendarModel>);

        /// Result of `testConnection`: calendars as JSON list of {url, name,
        /// color, writable}.
        #[qsignal]
        #[cxx_name = "connectionTested"]
        fn connection_tested(self: Pin<&mut CalendarModel>, ok: bool, message: QString, calendars: QString);
        /// Google sign-in: the address to open in the browser.
        #[qsignal]
        #[cxx_name = "googleSignInUrl"]
        fn google_sign_in_url(self: Pin<&mut CalendarModel>, url: QString);
        /// Google sign-in finished (the calendars come with
        /// `connectionTested`).
        #[qsignal]
        #[cxx_name = "googleSignedIn"]
        fn google_signed_in(self: Pin<&mut CalendarModel>, ok: bool, message: QString);

        #[qsignal]
        #[cxx_name = "calendarCreated"]
        fn calendar_created(self: Pin<&mut CalendarModel>, ok: bool, message: QString, url: QString);

        /// A synchronisation ended (`message` describes the result).
        #[qsignal]
        #[cxx_name = "syncFinished"]
        fn sync_finished(self: Pin<&mut CalendarModel>, ok: bool, message: QString);

        /// Occurrences in `from..to`: JSON list of {uid, kind, title,
        /// status, start, end, allDay, recurring, instance, tags, files,
        /// session, checklist: [done, total], calendar, color, readOnly}.
        #[qinvokable]
        fn occurrences(self: &CalendarModel, from: &QString, to: &QString) -> QString;

        /// Unscheduled tasks (JSON list of {uid, title, status, estimate,
        /// tags, files, calendar, color}).
        #[qinvokable]
        fn tasks(self: &CalendarModel) -> QString;

        /// One entry with all fields (JSON, "" if unknown).
        #[qinvokable]
        fn entry(self: &CalendarModel, uid: &QString) -> QString;

        /// Creates (`uid` empty) or updates an entry from JSON; returns the
        /// UID ("" if it could not be saved). `calendar` picks the calendar.
        #[qinvokable]
        fn save(self: Pin<&mut CalendarModel>, json: &QString) -> QString;

        #[qinvokable]
        fn remove(self: Pin<&mut CalendarModel>, uid: &QString) -> bool;

        /// Moves an entry (a whole series) so it starts at `start` and ends
        /// at `end`; `all_day` switches between timed and all-day. For a series
        /// `instance` is the moved occurrence's original start.
        #[qinvokable]
        #[cxx_name = "moveTo"]
        fn move_to(
            self: Pin<&mut CalendarModel>,
            uid: &QString,
            instance: &QString,
            start: &QString,
            end: &QString,
            all_day: bool,
        ) -> bool;

        /// Plans a task: it becomes a work block from `start` to `end`.
        #[qinvokable]
        fn schedule(self: Pin<&mut CalendarModel>, uid: &QString, start: &QString, end: &QString) -> bool;

        /// Turns a work block back into an unscheduled task.
        #[qinvokable]
        fn unschedule(self: Pin<&mut CalendarModel>, uid: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "setStatus"]
        fn set_status(self: Pin<&mut CalendarModel>, uid: &QString, status: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "toggleCheck"]
        fn toggle_check(self: Pin<&mut CalendarModel>, uid: &QString, index: i32) -> bool;

        /// Links a project file (absolute path) with an entry.
        #[qinvokable]
        #[cxx_name = "linkFile"]
        fn link_file(self: Pin<&mut CalendarModel>, uid: &QString, path: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "unlinkFile"]
        fn unlink_file(self: Pin<&mut CalendarModel>, uid: &QString, file: &QString) -> bool;

        /// Links of entries follow files moved or renamed in Atlas.
        #[qinvokable]
        #[cxx_name = "filesMoved"]
        fn files_moved(self: Pin<&mut CalendarModel>, from: &QString, to: &QString);

        /// A work block for a file dropped on a free time; returns the UID.
        #[qinvokable]
        #[cxx_name = "blockForFile"]
        fn block_for_file(self: Pin<&mut CalendarModel>, path: &QString, start: &QString, end: &QString) -> QString;

        /// A task (unscheduled) for a file; returns the UID.
        #[qinvokable]
        #[cxx_name = "taskForFile"]
        fn task_for_file(self: Pin<&mut CalendarModel>, path: &QString) -> QString;

        /// Task list items of a Markdown file: JSON list of {line, done,
        /// text, planned (a task with this text and file exists)}.
        #[qinvokable]
        #[cxx_name = "checklistOf"]
        fn checklist_of(self: &CalendarModel, path: &QString) -> QString;

        /// Creates tasks for chosen items (JSON list of {line, text}) of the
        /// Markdown file `path`; returns how many.
        #[qinvokable]
        #[cxx_name = "addTasks"]
        fn add_tasks(self: Pin<&mut CalendarModel>, path: &QString, items: &QString) -> i32;

        /// Entries matching `query` (JSON list of {uid, title, kind, start}).
        #[qinvokable]
        fn search(self: &CalendarModel, query: &QString, limit: i32) -> QString;

        /// Reminders due since the last call (JSON list of {uid, title, start,
        /// minutes}).
        #[qinvokable]
        #[cxx_name = "dueReminders"]
        fn due_reminders(self: Pin<&mut CalendarModel>) -> QString;

        #[qinvokable]
        #[cxx_name = "viewState"]
        fn view_state(self: &CalendarModel) -> QString;

        #[qinvokable]
        #[cxx_name = "setViewState"]
        fn set_view_state(self: &CalendarModel, json: &QString);

        /// Reads the calendar files and the account settings again.
        #[qinvokable]
        fn reload(self: Pin<&mut CalendarModel>);

        // ------------------------------------------------------ Nextcloud

        /// All calendars: JSON list of {id, name, color, writable, visible,
        /// remote, projectOnly, missing, isDefault, projectDefault}. The
        /// project calendar has id "local".
        #[qinvokable]
        fn calendars(self: &CalendarModel) -> QString;

        /// Calendar for new entries (id).
        #[qinvokable]
        #[cxx_name = "defaultCalendar"]
        fn default_calendar(self: &CalendarModel) -> QString;

        /// Account settings: JSON {server, user, davUrl, passwordCommand,
        /// lastSync, home}.
        #[qinvokable]
        fn account(self: &CalendarModel) -> QString;

        /// Checks the connection in the background (`connectionTested`).
        /// Without `password`: the known one, else `password_command`'s.
        #[qinvokable]
        #[cxx_name = "testConnection"]
        fn test_connection(
            self: &CalendarModel,
            server: &QString,
            user: &QString,
            password: &QString,
            dav_url: &QString,
            password_command: &QString,
        );

        /// Saves the account and the chosen calendars (JSON list of {url,
        /// name, visible}), keeps the password for this Atlas start and
        /// synchronises.
        #[qinvokable]
        #[cxx_name = "saveAccount"]
        fn save_account(
            self: Pin<&mut CalendarModel>,
            server: &QString,
            user: &QString,
            password: &QString,
            dav_url: &QString,
            calendars: &QString,
            password_command: &QString,
        );

        /// Signs in with Google in the browser (OAuth for installed apps,
        /// with the user's own client): `googleSignInUrl`, then
        /// `googleSignedIn` and the calendars with `connectionTested`.
        #[qinvokable]
        #[cxx_name = "googleSignIn"]
        fn google_sign_in(self: Pin<&mut CalendarModel>, client_id: &QString, client_secret: &QString);

        /// Switches between "" (Nextcloud/CalDAV) and "google"; a change
        /// forgets the other account's sign-in, calendars and cache.
        #[qinvokable]
        #[cxx_name = "setProvider"]
        fn set_provider(self: Pin<&mut CalendarModel>, provider: &QString);

        /// The app password for this Atlas start (only in memory).
        #[qinvokable]
        #[cxx_name = "setPassword"]
        fn set_password(self: Pin<&mut CalendarModel>, password: &QString);

        /// Remembers the password on this machine (now and after it is
        /// entered) or forgets it.
        #[qinvokable]
        #[cxx_name = "rememberPassword"]
        fn remember_password(self: Pin<&mut CalendarModel>, remember: bool);

        /// Runs the password command again (after it failed).
        #[qinvokable]
        #[cxx_name = "retryPasswordCommand"]
        fn retry_password_command(self: Pin<&mut CalendarModel>);

        #[qinvokable]
        #[cxx_name = "setCalendarVisible"]
        fn set_calendar_visible(self: Pin<&mut CalendarModel>, id: &QString, visible: bool);

        /// Limits a calendar to this project (or shows it everywhere again).
        #[qinvokable]
        #[cxx_name = "setCalendarProjectOnly"]
        fn set_calendar_project_only(self: Pin<&mut CalendarModel>, id: &QString, only: bool);

        /// Default calendar for new entries: for all projects or only this one.
        #[qinvokable]
        #[cxx_name = "setDefaultCalendar"]
        fn set_default_calendar(self: Pin<&mut CalendarModel>, id: &QString, project: bool);

        /// Synchronises now (in the background).
        #[qinvokable]
        #[cxx_name = "syncNow"]
        fn sync_now(self: Pin<&mut CalendarModel>);

        /// Creates a calendar in Nextcloud (after the user confirmed);
        /// `calendarCreated` reports the result.
        #[qinvokable]
        #[cxx_name = "createCalendar"]
        fn create_calendar(self: Pin<&mut CalendarModel>, name: &QString);

        /// Removes the local cache (pending changes included).
        #[qinvokable]
        #[cxx_name = "clearCache"]
        fn clear_cache(self: Pin<&mut CalendarModel>);

        /// Forgets account, password and cache; nothing is deleted on the
        /// server.
        #[qinvokable]
        #[cxx_name = "disconnectAccount"]
        fn disconnect_account(self: Pin<&mut CalendarModel>);

        /// Open conflicts: JSON list of {id, title, calendar, local, remote,
        /// message}, versions described as text.
        #[qinvokable]
        fn conflicts(self: &CalendarModel) -> QString;

        /// `choice`: "remote", "local" or "both".
        #[qinvokable]
        #[cxx_name = "resolveConflict"]
        fn resolve_conflict(self: Pin<&mut CalendarModel>, id: i32, choice: &QString) -> bool;
    }

    impl cxx_qt::Initialize for CalendarModel {}
    impl cxx_qt::Threading for CalendarModel {}
}

/// The app password, for this Atlas start only.
static PASSWORD: Mutex<Option<String>> = Mutex::new(None);

fn password() -> Option<String> {
    PASSWORD.lock().ok().and_then(|p| p.clone())
}

fn password_now() -> Option<String> {
    password()
}

/// The password command runs once per Atlas start (and again after it or
/// the account changed, or on request).
struct CommandState {
    /// The command line last run ("" = none yet).
    tried: String,
    running: bool,
    error: String,
}

static COMMAND: Mutex<CommandState> =
    Mutex::new(CommandState { tried: String::new(), running: false, error: String::new() });

/// Time for the command, including typing a passphrase into pinentry.
const COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);

/// Where the remembered secret lives: the Nextcloud app password, or for
/// Google the refresh token of the sign-in.
fn secret_store() -> SecretStore {
    SecretStore::default_for(if Config::load().calendar.is_google() { "google" } else { "nextcloud" })
}

/// Storing and forgetting the remembered password happen one after another;
/// a newer request makes a still waiting older store obsolete (so the latest
/// password wins).
static SECRET_LOCK: Mutex<()> = Mutex::new(());
static SECRET_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn forget_secret() -> anyhow::Result<()> {
    SECRET_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let _guard = SECRET_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    secret_store().forget()
}

fn reset_command() {
    if let Ok(mut c) = COMMAND.lock() {
        c.tried.clear();
        c.error.clear();
    }
}

pub struct CalendarModelRust {
    atlas_dir: QString,
    project_root: QString,
    revision: i32,
    error: QString,
    zone: QString,
    connected: bool,
    google: bool,
    unlocked: bool,
    fetching_password: bool,
    password_error: QString,
    remembered: QString,
    syncing: bool,
    sync_status: QString,
    sync_failed: bool,
    pending: i32,
    conflict_count: i32,

    tz: Tz,
    entries: Vec<Entry>,
    config: CalendarConfig,
    last_reminder_check: chrono::DateTime<Utc>,
    /// Another synchronisation was requested while one ran.
    sync_again: bool,
    /// Remote calendars as last read from the cache: URL → (name, colour,
    /// writable and present).
    meta: std::collections::BTreeMap<String, (String, String, bool)>,
    /// (calendar, UID) of entries with changes waiting for the server.
    waiting: std::collections::BTreeSet<(String, String)>,
}

impl Default for CalendarModelRust {
    fn default() -> Self {
        let tz = calendar::local_zone();
        Self {
            atlas_dir: QString::default(),
            project_root: QString::default(),
            revision: 0,
            error: QString::default(),
            zone: QString::from(tz.name()),
            connected: false,
            google: false,
            unlocked: false,
            fetching_password: false,
            password_error: QString::default(),
            remembered: QString::default(),
            syncing: false,
            sync_status: QString::default(),
            sync_failed: false,
            pending: 0,
            conflict_count: 0,
            tz,
            entries: Vec::new(),
            config: CalendarConfig::default(),
            last_reminder_check: Utc::now(),
            sync_again: false,
            meta: Default::default(),
            waiting: Default::default(),
        }
    }
}

const ISO: &str = "%Y-%m-%dT%H:%M";
const SETTINGS: &str = "settings.json";
/// Colour of the project calendar.
const LOCAL_COLOR: &str = "#4fa3ff";

fn iso(t: NaiveDateTime) -> String {
    t.format(ISO).to_string()
}

fn account_of(c: &CalendarConfig) -> Account {
    Account { server: c.server.clone(), user: c.user.clone(), dav_url: c.dav_url.clone() }
}

/// The client for the configured account; for Google `password` is the
/// refresh token.
fn client(c: &CalendarConfig, password: &str) -> Client {
    if c.is_google() {
        return Client::google(&c.user, oauth_client(c), password, google::Endpoints::default(), true);
    }
    Client::new(account_of(c), password, true)
}

fn oauth_client(c: &CalendarConfig) -> google::OAuthClient {
    google::OAuthClient { id: c.google_client_id.clone(), secret: c.google_client_secret.clone() }
}

fn describe_time(t: i64) -> String {
    chrono::DateTime::from_timestamp(t, 0)
        .map(|d| d.with_timezone(&calendar::local_zone()).format("%d.%m. %H:%M").to_string())
        .unwrap_or_default()
}

impl cxx_qt::Initialize for qobject::CalendarModel {
    fn initialize(mut self: Pin<&mut Self>) {
        self.as_mut().on_atlas_dir_changed(|m| m.reload()).release();
        self.as_mut().on_project_root_changed(|m| m.reload()).release();
    }
}

impl qobject::CalendarModel {
    fn store(&self) -> Store {
        let atlas = self.atlas_dir.to_string();
        if atlas.is_empty() {
            Store::new(atlas_core::paths::state_root().join(calendar::DIR))
        } else {
            Store::for_atlas_dir(Path::new(&atlas))
        }
    }

    fn sync_store(&self) -> SyncStore {
        SyncStore::new(self.store().dir())
    }

    fn set_error(mut self: Pin<&mut Self>, message: String) {
        if self.error.to_string() != message {
            self.as_mut().rust_mut().error = qs(message);
            self.error_changed();
        }
    }

    fn bump(mut self: Pin<&mut Self>) {
        let r = self.revision.wrapping_add(1);
        self.as_mut().rust_mut().revision = r;
        self.revision_changed();
    }

    /// Remote calendars shown in this project.
    fn visible_remote(&self) -> Vec<String> {
        if !self.config.connected() {
            return Vec::new();
        }
        let root = self.project_root.to_string();
        self.config.calendars.iter().filter(|c| c.visible_in(&root)).map(|c| c.url.clone()).collect()
    }

    /// Calendars to synchronise: the visible ones.
    fn synced(&self) -> Vec<String> {
        self.visible_remote()
    }

    fn project_settings(&self) -> Value {
        std::fs::read_to_string(self.store().dir().join(SETTINGS))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(json!({}))
    }

    fn writable(&self, calendar: &str) -> bool {
        if calendar == LOCAL {
            return true;
        }
        self.meta.get(calendar).is_none_or(|m| m.2)
    }

    pub fn reload(mut self: Pin<&mut Self>) {
        let config = Config::load().calendar;
        self.as_mut().rust_mut().config = config;
        let mut problem = String::new();
        let store = self.store();
        let mut entries = match store.load() {
            Ok(e) => e,
            Err(err) => {
                log::warn!("calendar: {err:#}");
                problem = format!("{err:#}");
                Vec::new()
            }
        };
        let remote = self.visible_remote();
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
        self.as_mut().rust_mut().entries = entries;
        self.as_mut().set_error(problem);
        self.as_mut().update_sync_state();
        self.as_mut().fetch_password();
        self.bump();
    }

    /// Refreshes the sync properties from the cache.
    fn update_sync_state(mut self: Pin<&mut Self>) {
        let state = self.sync_store().load();
        let connected = self.config.connected();
        let unlocked = connected && password().is_some();
        let (fetching, command_error) =
            COMMAND.lock().map(|c| (c.running, c.error.clone())).unwrap_or((false, String::new()));
        let synced = self.synced();
        let pending = state.queue.iter().filter(|o| synced.contains(&o.calendar)).count() as i32;
        let failed = !state.error.is_empty();
        let status = if !connected {
            String::new()
        } else if self.syncing {
            "Synchronisiere …".to_owned()
        } else if failed {
            state.error.clone()
        } else if !unlocked && fetching {
            "Hole App-Passwort …".to_owned()
        } else if !unlocked && self.config.is_google() {
            "Bei Google neu anmelden".to_owned()
        } else if !unlocked && !command_error.is_empty() {
            format!("Passwort-Befehl: {command_error}")
        } else if !unlocked {
            "App-Passwort für diese Sitzung fehlt".to_owned()
        } else if let Some(t) = state.last_sync {
            format!("Zuletzt synchronisiert {}", describe_time(t))
        } else {
            "Noch nicht synchronisiert".to_owned()
        };
        let meta = state
            .calendars
            .iter()
            .map(|(url, c)| {
                let name = Some(c.info.name.clone())
                    .filter(|n| !n.is_empty())
                    .or_else(|| self.config.calendars.iter().find(|x| x.url == *url).map(|x| x.name.clone()))
                    .unwrap_or_else(|| url.clone());
                let color = Some(c.info.color.clone()).filter(|c| !c.is_empty()).unwrap_or_else(|| "#0082c9".into());
                (url.clone(), (name, color, c.info.writable && !c.missing))
            })
            .collect();
        let waiting = state.queue.iter().map(|o| (o.calendar.clone(), o.uid.clone())).collect();
        let mut r = self.as_mut().rust_mut();
        r.meta = meta;
        r.waiting = waiting;
        r.connected = connected;
        r.google = r.config.is_google();
        r.unlocked = unlocked;
        r.fetching_password = fetching && !unlocked;
        r.password_error = qs(if unlocked { "" } else { &command_error });
        r.remembered = qs(secret_store().kind().map(|k| k.as_str()).unwrap_or(""));
        r.pending = pending;
        r.conflict_count = state.conflicts.len() as i32;
        r.sync_status = qs(status);
        r.sync_failed = failed;
        self.as_mut().sync_changed();
    }

    /// Rewrites `events.ics` if `calendar` is the project calendar (remote
    /// entries go through the queue) and records the links of all entries.
    fn commit(mut self: Pin<&mut Self>, calendar: &str) -> bool {
        let store = self.store();
        let result = if calendar == LOCAL {
            let local: Vec<Entry> = self.entries.iter().filter(|e| e.calendar == LOCAL).cloned().collect();
            store.save(&local)
        } else {
            Ok(())
        };
        let result = result.and_then(|()| store.save_links(self.entries.iter()));
        let ok = match result {
            Ok(()) => {
                self.as_mut().set_error(String::new());
                true
            }
            Err(err) => {
                self.as_mut().set_error(format!("Kalender kann nicht gespeichert werden: {err:#}"));
                false
            }
        };
        self.bump();
        ok
    }

    /// Queues the object of `uid` in remote `calendar` and sends it.
    fn push_remote(mut self: Pin<&mut Self>, calendar: &str, uid: &str) {
        let parts: Vec<&Entry> = self.entries.iter().filter(|e| e.uid == uid && e.calendar == calendar).collect();
        let result = if parts.is_empty() {
            self.sync_store().update(|s| s.local_delete(calendar, uid))
        } else {
            let data = sync::object_data(&parts);
            self.sync_store().update(|s| s.local_put(calendar, uid, data))
        };
        if let Err(err) = result {
            self.as_mut().set_error(format!("Kalender-Cache kann nicht geschrieben werden: {err:#}"));
        }
        self.as_mut().update_sync_state();
        self.sync_now();
    }

    /// Stores a changed entry `uid` of `calendar`.
    fn store_entry(mut self: Pin<&mut Self>, calendar: &str, uid: &str) -> bool {
        let ok = self.as_mut().commit(calendar);
        if calendar != LOCAL {
            self.push_remote(calendar, uid);
        }
        ok
    }

    fn index_of(&self, uid: &str) -> Option<usize> {
        // The master entry (not an override of a series).
        self.entries.iter().position(|e| e.uid == uid && e.recurrence_id.is_none())
    }

    fn relative(&self, path: &str) -> String {
        let root = self.project_root.to_string();
        match Path::new(path).strip_prefix(&root) {
            Ok(rel) if !root.is_empty() => format!("/{}", rel.to_string_lossy()),
            _ => path.to_owned(),
        }
    }

    fn new_time(&self, local: NaiveDateTime, all_day: bool) -> Time {
        if all_day { Time::Date(local.date()) } else { Time::Zoned(local, self.tz) }
    }

    /// Name, colour and writability of a calendar.
    fn calendar_info(&self, id: &str) -> (String, String, bool) {
        if id == LOCAL {
            return ("Projekt (nur lokal)".into(), LOCAL_COLOR.into(), true);
        }
        self.meta.get(id).cloned().unwrap_or_else(|| {
            let name = self
                .config
                .calendars
                .iter()
                .find(|c| c.url == id)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| id.to_owned());
            (name, "#0082c9".into(), true)
        })
    }

    fn entry_json(&self, e: &Entry) -> Value {
        let tz = self.tz;
        let start = e.start.map(|s| s.local(tz));
        let end = e.end.map(|s| s.local(tz));
        let all_day = e.all_day();
        // All-day ends are exclusive in iCalendar; the UI shows the last day.
        let end_date = match (all_day, end, start) {
            (true, Some(end), _) => Some((end - Duration::days(1)).date()),
            (true, None, Some(start)) => Some(start.date()),
            _ => None,
        };
        let rule = e.rrule.as_deref().and_then(recur::Rule::parse);
        let (name, color, writable) = self.calendar_info(&e.calendar);
        json!({
            "uid": e.uid,
            "kind": e.kind.as_str(),
            "title": e.title,
            "description": e.description,
            "start": start.map(iso),
            "end": end.map(iso),
            "endDate": end_date.map(|d| d.format("%Y-%m-%d").to_string()),
            "allDay": all_day,
            "status": e.status.as_str(),
            "tags": e.tags,
            "checklist": e.checklist.iter().map(|(d, t)| json!({ "done": d, "text": t })).collect::<Vec<_>>(),
            "files": e.files,
            "session": e.session,
            "estimate": e.estimate_minutes,
            "reminders": e.reminders,
            "rrule": rule.as_ref().map(|r| json!({
                "freq": r.freq.as_str().to_lowercase(),
                "interval": r.interval,
                "count": r.count,
                "until": r.until.map(|u| u.format("%Y-%m-%d").to_string()),
                "text": r.describe(),
                "supported": r.supported,
            })),
            "calendar": e.calendar,
            "calendarName": name,
            "color": color,
            "readOnly": !writable,
            "pending": self.waiting.contains(&(e.calendar.clone(), e.uid.clone())),
        })
    }

    pub fn occurrences(&self, from: &QString, to: &QString) -> QString {
        let (Some(from), Some(to)) = (calendar::parse_local(&from.to_string()), calendar::parse_local(&to.to_string()))
        else {
            return qs("[]");
        };
        let mut infos = std::collections::BTreeMap::new();
        let list: Vec<Value> = calendar::occurrences(&self.entries, self.tz, from, to)
            .into_iter()
            .map(|o| {
                let e = self.entries.iter().find(|e| e.uid == o.uid && e.recurrence_id.is_none());
                let done = e.map(|e| e.checklist.iter().filter(|c| c.0).count()).unwrap_or(0);
                let cal = e.map(|e| e.calendar.clone()).unwrap_or_else(|| LOCAL.into());
                let (_, color, writable) = infos.entry(cal.clone()).or_insert_with(|| self.calendar_info(&cal)).clone();
                json!({
                    "uid": o.uid,
                    "kind": o.kind.as_str(),
                    "title": o.title,
                    "status": o.status.as_str(),
                    "start": iso(o.start),
                    "end": iso(o.end),
                    "allDay": o.all_day,
                    "recurring": o.recurring,
                    "instance": o.instance.map(iso),
                    "tags": e.map(|e| e.tags.clone()).unwrap_or_default(),
                    "files": e.map(|e| e.files.clone()).unwrap_or_default(),
                    "session": e.map(|e| e.session.clone()).unwrap_or_default(),
                    "checklist": [done, e.map(|e| e.checklist.len()).unwrap_or(0)],
                    "calendar": cal,
                    "color": color,
                    "readOnly": !writable,
                })
            })
            .collect();
        qs(Value::from(list).to_string())
    }

    pub fn tasks(&self) -> QString {
        let list: Vec<Value> = self
            .entries
            .iter()
            .filter(|e| e.kind == Kind::Task && e.start.is_none())
            .map(|e| {
                let (_, color, writable) = self.calendar_info(&e.calendar);
                json!({
                    "uid": e.uid, "title": e.title, "status": e.status.as_str(), "estimate": e.estimate_minutes,
                    "tags": e.tags, "files": e.files, "calendar": e.calendar, "color": color, "readOnly": !writable,
                })
            })
            .collect();
        qs(Value::from(list).to_string())
    }

    pub fn entry(&self, uid: &QString) -> QString {
        self.index_of(&uid.to_string()).map(|i| qs(self.entry_json(&self.entries[i]).to_string())).unwrap_or_default()
    }

    pub fn search(&self, query: &QString, limit: i32) -> QString {
        let query = query.to_string();
        let tz = self.tz;
        let mut found: Vec<(i32, Value)> = self
            .entries
            .iter()
            .filter(|e| e.recurrence_id.is_none())
            .filter_map(|e| {
                let hay = format!("{} {} {}", e.title, e.tags.join(" "), e.description);
                let score = atlas_core::finder::score(query.trim(), &hay)?;
                Some((
                    score,
                    json!({ "uid": e.uid, "title": e.title, "kind": e.kind.as_str(), "start": e.start.map(|s| iso(s.local(tz))) }),
                ))
            })
            .collect();
        found.sort_by_key(|f| std::cmp::Reverse(f.0));
        found.truncate(limit.max(1) as usize);
        qs(Value::from(found.into_iter().map(|f| f.1).collect::<Vec<_>>()).to_string())
    }

    pub fn due_reminders(mut self: Pin<&mut Self>) -> QString {
        let now = Utc::now();
        let from = self.last_reminder_check;
        self.as_mut().rust_mut().last_reminder_check = now;
        let list: Vec<Value> = calendar::due_reminders(&self.entries, self.tz, from, now)
            .into_iter()
            .map(|(uid, title, start, minutes)| json!({ "uid": uid, "title": title, "start": iso(start), "minutes": minutes }))
            .collect();
        qs(Value::from(list).to_string())
    }

    pub fn view_state(&self) -> QString {
        qs(self.store().state().to_string())
    }

    pub fn set_view_state(&self, json: &QString) {
        if let Ok(v) = serde_json::from_str::<Value>(&json.to_string())
            && let Err(err) = self.store().save_state(&v)
        {
            log::warn!("calendar state: {err:#}");
        }
    }

    // ------------------------------------------------------------ Nextcloud

    pub fn calendars(&self) -> QString {
        let default = self.default_calendar().to_string();
        let project_default = self.project_settings()["default_calendar"].as_str().unwrap_or_default().to_owned();
        let mut list = vec![json!({
            "id": LOCAL, "name": "Projekt (nur lokal)", "color": LOCAL_COLOR, "writable": true, "visible": true,
            "remote": false, "projectOnly": true, "missing": false, "isDefault": default == LOCAL,
            "projectDefault": project_default == LOCAL,
        })];
        if self.config.connected() {
            let state = self.sync_store().load();
            let root = self.project_root.to_string();
            for c in &self.config.calendars {
                let cached = state.calendars.get(&c.url);
                let (name, color, writable) = self.calendar_info(&c.url);
                list.push(json!({
                    "id": c.url, "name": if name == c.url && !c.name.is_empty() { c.name.clone() } else { name }, "color": color,
                    "writable": writable, "visible": c.visible_in(&root), "remote": true,
                    "projectOnly": !c.projects.is_empty(), "missing": cached.is_some_and(|x| x.missing),
                    "isDefault": default == c.url, "projectDefault": project_default == c.url, "globalDefault": self.config.default == c.url,
                }));
            }
        }
        qs(Value::from(list).to_string())
    }

    pub fn default_calendar(&self) -> QString {
        let visible = self.visible_remote();
        let usable = |id: &str| id == LOCAL || (visible.iter().any(|v| v == id) && self.writable(id));
        let project = self.project_settings()["default_calendar"].as_str().unwrap_or_default().to_owned();
        if !project.is_empty() && usable(&project) {
            return qs(project);
        }
        if !self.config.default.is_empty() && usable(&self.config.default) {
            return qs(&self.config.default);
        }
        qs(LOCAL)
    }
}
