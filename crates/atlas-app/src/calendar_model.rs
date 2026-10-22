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

fn qs(s: impl AsRef<str>) -> QString {
    QString::from(s.as_ref())
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

    pub fn save(mut self: Pin<&mut Self>, json: &QString) -> QString {
        let Ok(v) = serde_json::from_str::<Value>(&json.to_string()) else { return QString::default() };
        let tz = self.tz;
        let uid = v["uid"].as_str().unwrap_or_default().to_owned();
        let kind = v["kind"].as_str().and_then(Kind::parse).unwrap_or(Kind::Event);
        let existing = self.index_of(&uid);
        let mut entry = match existing {
            Some(i) => self.entries[i].clone(),
            None => {
                let mut e = Entry::new(kind, "");
                e.calendar = self.default_calendar().to_string();
                e
            }
        };
        let old_calendar = entry.calendar.clone();
        if let Some(cal) = v["calendar"].as_str().filter(|c| !c.is_empty()) {
            entry.calendar = cal.to_owned();
        }
        if !self.writable(&old_calendar) || !self.writable(&entry.calendar) {
            self.as_mut().set_error("Der Kalender ist schreibgeschützt.".into());
            return QString::default();
        }
        entry.kind = kind;
        // Google's calendars hold events only: tasks stay in the project.
        if kind == Kind::Task && self.config.is_google() && entry.calendar != LOCAL {
            entry.calendar = LOCAL.to_owned();
        }
        entry.title = v["title"].as_str().unwrap_or_default().trim().to_owned();
        if entry.title.is_empty() {
            entry.title = "Ohne Titel".into();
        }
        entry.description = v["description"].as_str().unwrap_or_default().to_owned();
        if let Some(s) = v["status"].as_str().and_then(Status::parse) {
            entry.status = s;
        }
        let strings = |key: &str| -> Vec<String> {
            v[key]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
                .unwrap_or_default()
        };
        if v.get("tags").is_some() {
            entry.tags = strings("tags")
                .into_iter()
                .map(|t| t.trim().trim_start_matches('#').to_owned())
                .filter(|t| !t.is_empty())
                .collect();
        }
        if v.get("files").is_some() {
            entry.files = strings("files");
        }
        if v.get("session").is_some() {
            entry.session = v["session"].as_str().unwrap_or_default().to_owned();
        }
        if v.get("checklist").is_some() {
            entry.checklist = v["checklist"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|c| {
                            Some((c["done"].as_bool().unwrap_or(false), c["text"].as_str()?.trim().to_owned()))
                        })
                        .filter(|c| !c.1.is_empty())
                        .collect()
                })
                .unwrap_or_default();
        }
        if v.get("estimate").is_some() {
            entry.estimate_minutes = v["estimate"]
                .as_u64()
                .map(|m| m as u32)
                .or_else(|| v["estimate"].as_str().and_then(calendar::parse_estimate));
        }
        if v.get("reminders").is_some() {
            entry.reminders =
                v["reminders"].as_array().map(|a| a.iter().filter_map(Value::as_i64).collect()).unwrap_or_default();
        }

        // Times: tasks have none; all-day entries use dates.
        let all_day = v["allDay"].as_bool().unwrap_or(false);
        let start = v["start"].as_str().and_then(calendar::parse_local);
        if kind == Kind::Task {
            entry.start = None;
            entry.end = None;
        } else if let Some(start) = start {
            let end = if all_day {
                v["endDate"]
                    .as_str()
                    .and_then(calendar::parse_local)
                    .map(|d| d + Duration::days(1))
                    .filter(|e| *e > start)
                    .unwrap_or(start + Duration::days(1))
            } else {
                v["end"]
                    .as_str()
                    .and_then(calendar::parse_local)
                    .filter(|e| *e >= start)
                    .unwrap_or(start + Duration::hours(1))
            };
            // Keep the zone kind of existing entries (UTC from a server stays UTC).
            entry.start = Some(match (entry.start, all_day) {
                (Some(old), false) if !old.is_date() => old.with_local(start, tz),
                _ => self.new_time(start, all_day),
            });
            entry.end = if kind == Kind::Deadline && !all_day {
                None
            } else {
                Some(match (entry.end, all_day) {
                    (Some(old), false) if !old.is_date() => old.with_local(end, tz),
                    _ => self.new_time(end, all_day),
                })
            };
        }
        // Recurrence: only replaced when the dialog changed it, so rules
        // Atlas cannot fully edit stay as they are.
        if v["rruleChanged"].as_bool().unwrap_or(false) {
            entry.rrule = match v["rrule"]["freq"].as_str() {
                Some(freq) if freq != "none" && entry.start.is_some() => {
                    let freq = match freq {
                        "daily" => recur::Freq::Daily,
                        "weekly" => recur::Freq::Weekly,
                        "monthly" => recur::Freq::Monthly,
                        _ => recur::Freq::Yearly,
                    };
                    let mut rule = recur::Rule::new(freq);
                    rule.interval = v["rrule"]["interval"].as_u64().unwrap_or(1).max(1) as u32;
                    rule.count = v["rrule"]["count"].as_u64().filter(|c| *c > 0).map(|c| c as u32);
                    rule.until = v["rrule"]["until"]
                        .as_str()
                        .and_then(calendar::parse_local)
                        .map(|d| d + Duration::hours(23) + Duration::minutes(59));
                    Some(rule.to_value())
                }
                _ => None,
            };
            entry.exdates.clear();
            // Changed occurrences of the old rule no longer fit.
            let uid = entry.uid.clone();
            self.as_mut().rust_mut().entries.retain(|e| !(e.uid == uid && e.recurrence_id.is_some()));
        }
        entry.sequence += 1;
        let uid = entry.uid.clone();
        let calendar = entry.calendar.clone();
        // Overrides of a series move with it to another calendar.
        for e in self.as_mut().rust_mut().entries.iter_mut().filter(|e| e.uid == uid && e.recurrence_id.is_some()) {
            e.calendar = calendar.clone();
        }
        match self.index_of(&uid) {
            Some(i) => self.as_mut().rust_mut().entries[i] = entry,
            None => self.as_mut().rust_mut().entries.push(entry),
        }
        if old_calendar != calendar && existing.is_some() {
            if old_calendar == LOCAL {
                self.as_mut().commit(LOCAL);
            } else {
                // Removed from the old Nextcloud calendar.
                let _ = self.sync_store().update(|s| s.local_delete(&old_calendar, &uid));
            }
        }
        self.store_entry(&calendar, &uid);
        qs(uid)
    }

    pub fn remove(mut self: Pin<&mut Self>, uid: &QString) -> bool {
        let uid = uid.to_string();
        let Some(i) = self.index_of(&uid) else { return false };
        let calendar = self.entries[i].calendar.clone();
        if !self.writable(&calendar) {
            self.as_mut().set_error("Der Kalender ist schreibgeschützt.".into());
            return false;
        }
        self.as_mut().rust_mut().entries.retain(|e| e.uid != uid);
        let _ = self.store().forget_links(&uid);
        self.store_entry(&calendar, &uid)
    }

    /// Changes the entry `uid` with `f` (false: nothing changed) and stores it.
    fn modify(mut self: Pin<&mut Self>, uid: &str, f: impl FnOnce(&mut Entry) -> bool) -> bool {
        let Some(i) = self.index_of(uid) else { return false };
        let calendar = self.entries[i].calendar.clone();
        if !self.writable(&calendar) {
            self.as_mut().set_error("Der Kalender ist schreibgeschützt.".into());
            return false;
        }
        let mut e = self.entries[i].clone();
        if !f(&mut e) {
            return false;
        }
        e.sequence += 1;
        self.as_mut().rust_mut().entries[i] = e;
        self.store_entry(&calendar, uid)
    }

    pub fn move_to(
        self: Pin<&mut Self>,
        uid: &QString,
        instance: &QString,
        start: &QString,
        end: &QString,
        all_day: bool,
    ) -> bool {
        let tz = self.tz;
        let (Some(start), Some(end)) =
            (calendar::parse_local(&start.to_string()), calendar::parse_local(&end.to_string()))
        else {
            return false;
        };
        let instance = calendar::parse_local(&instance.to_string());
        let zone = self.tz;
        self.modify(&uid.to_string(), |e| {
            let Some(old_start) = e.start else { return false };
            // A series moves as a whole: by the distance the occurrence moved.
            let reference = instance.unwrap_or_else(|| old_start.local(tz));
            let delta = start - reference;
            let length = (end - start).max(Duration::minutes(if all_day { 24 * 60 } else { 15 }));
            let new_start_local = old_start.local(tz) + delta;
            let new_time = |local: NaiveDateTime| Time::Zoned(local, zone);
            if all_day {
                e.start = Some(Time::Date(new_start_local.date()));
                e.end = Some(Time::Date(new_start_local.date() + Duration::days(length.num_days().max(1))));
            } else {
                e.start = Some(if old_start.is_date() {
                    new_time(new_start_local)
                } else {
                    old_start.with_local(new_start_local, tz)
                });
                e.end = match (e.kind, e.end) {
                    (Kind::Deadline, None) if !old_start.is_date() => None,
                    (_, Some(old)) if !old.is_date() => Some(old.with_local(new_start_local + length, tz)),
                    _ => Some(new_time(new_start_local + length)),
                };
            }
            true
        })
    }

    pub fn schedule(self: Pin<&mut Self>, uid: &QString, start: &QString, end: &QString) -> bool {
        let Some(start) = calendar::parse_local(&start.to_string()) else { return false };
        let end = calendar::parse_local(&end.to_string());
        let zone = self.tz;
        self.modify(&uid.to_string(), |e| {
            let length = e.estimate_minutes.map(|m| Duration::minutes(m as i64)).unwrap_or(Duration::hours(1));
            let end = end.filter(|e| *e > start).unwrap_or(start + length);
            e.kind = Kind::Block;
            e.start = Some(Time::Zoned(start, zone));
            e.end = Some(Time::Zoned(end, zone));
            true
        })
    }

    pub fn unschedule(self: Pin<&mut Self>, uid: &QString) -> bool {
        self.modify(&uid.to_string(), |e| {
            if e.start.is_none() || e.rrule.is_some() {
                return false;
            }
            if e.estimate_minutes.is_none()
                && let (Some(s), Some(end)) = (e.start, e.end)
                && !s.is_date()
            {
                let tz = calendar::local_zone();
                e.estimate_minutes = Some((end.local(tz) - s.local(tz)).num_minutes().max(0) as u32);
            }
            e.kind = Kind::Task;
            e.start = None;
            e.end = None;
            true
        })
    }

    pub fn set_status(self: Pin<&mut Self>, uid: &QString, status: &QString) -> bool {
        let Some(status) = Status::parse(&status.to_string()) else { return false };
        self.modify(&uid.to_string(), |e| {
            e.status = status;
            true
        })
    }

    pub fn toggle_check(self: Pin<&mut Self>, uid: &QString, index: i32) -> bool {
        self.modify(&uid.to_string(), |e| match e.checklist.get_mut(index.max(0) as usize) {
            Some(item) => {
                item.0 = !item.0;
                true
            }
            None => false,
        })
    }

    pub fn link_file(self: Pin<&mut Self>, uid: &QString, path: &QString) -> bool {
        let file = self.relative(&path.to_string());
        self.modify(&uid.to_string(), |e| {
            if e.files.contains(&file) {
                return false;
            }
            e.files.push(file);
            true
        })
    }

    pub fn unlink_file(self: Pin<&mut Self>, uid: &QString, file: &QString) -> bool {
        let file = file.to_string();
        self.modify(&uid.to_string(), |e| {
            let before = e.files.len();
            e.files.retain(|f| f != &file);
            e.files.len() != before
        })
    }

    pub fn files_moved(mut self: Pin<&mut Self>, from: &QString, to: &QString) {
        let (from, to) = (self.relative(&from.to_string()), self.relative(&to.to_string()));
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
        let touched: Vec<(String, String)> = self
            .entries
            .iter()
            .filter(|e| e.files.iter().any(|f| moved(f).is_some()))
            .map(|e| (e.uid.clone(), e.calendar.clone()))
            .collect();
        if touched.is_empty() {
            return;
        }
        for e in self.as_mut().rust_mut().entries.iter_mut() {
            for f in e.files.iter_mut() {
                if let Some(n) = moved(f) {
                    *f = n;
                }
            }
        }
        // Local entries are rewritten; links are recorded for all.
        let local = touched.iter().any(|t| t.1 == LOCAL);
        self.as_mut().commit(if local { LOCAL } else { "" });
        for (uid, cal) in touched.iter().filter(|t| t.1 != LOCAL) {
            self.as_mut().push_remote(cal, uid);
        }
    }

    fn title_for(path: &str) -> String {
        let p = Path::new(path);
        p.file_stem().or(p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_owned())
    }

    /// Adds a new entry in the default calendar.
    fn add(mut self: Pin<&mut Self>, mut e: Entry) -> QString {
        e.calendar = self.default_calendar().to_string();
        let (uid, calendar) = (e.uid.clone(), e.calendar.clone());
        self.as_mut().rust_mut().entries.push(e);
        self.store_entry(&calendar, &uid);
        qs(uid)
    }

    pub fn block_for_file(self: Pin<&mut Self>, path: &QString, start: &QString, end: &QString) -> QString {
        let path = path.to_string();
        let Some(start) = calendar::parse_local(&start.to_string()) else { return QString::default() };
        let end = calendar::parse_local(&end.to_string()).filter(|e| *e > start).unwrap_or(start + Duration::hours(1));
        let mut e = Entry::new(Kind::Block, &Self::title_for(&path));
        e.start = Some(self.new_time(start, false));
        e.end = Some(self.new_time(end, false));
        e.files = vec![self.relative(&path)];
        self.add(e)
    }

    pub fn task_for_file(self: Pin<&mut Self>, path: &QString) -> QString {
        let path = path.to_string();
        let mut e = Entry::new(Kind::Task, &Self::title_for(&path));
        e.files = vec![self.relative(&path)];
        self.add(e)
    }

    pub fn checklist_of(&self, path: &QString) -> QString {
        let path = path.to_string();
        let file = self.relative(&path);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let list: Vec<Value> = calendar::checklist_items(&text)
            .into_iter()
            .map(|(line, done, item)| {
                let planned = self.entries.iter().any(|e| e.title == item && e.files.iter().any(|f| f == &file));
                json!({ "line": line, "done": done, "text": item, "planned": planned })
            })
            .collect();
        qs(Value::from(list).to_string())
    }

    pub fn add_tasks(mut self: Pin<&mut Self>, path: &QString, items: &QString) -> i32 {
        let file = self.relative(&path.to_string());
        let Ok(Value::Array(items)) = serde_json::from_str(&items.to_string()) else { return 0 };
        let mut count = 0;
        for item in items {
            let Some(text) = item["text"].as_str() else { continue };
            let mut e = Entry::new(Kind::Task, text);
            e.files = vec![file.clone()];
            if let Some(line) = item["line"].as_u64() {
                e.description = format!("Aus {file}, Zeile {line}");
            }
            self.as_mut().add(e);
            count += 1;
        }
        count
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

    pub fn account(&self) -> QString {
        let state = self.sync_store().load();
        qs(json!({
            "provider": self.config.provider, "googleClientId": self.config.google_client_id,
            "googleClientSecret": self.config.google_client_secret,
            "server": self.config.server, "user": self.config.user, "davUrl": self.config.dav_url,
            "passwordCommand": self.config.password_command, "rememberPassword": self.config.remember_password,
            "lastSync": state.last_sync.map(describe_time), "error": state.error, "home": state.home,
        })
        .to_string())
    }

    pub fn test_connection(
        &self,
        server: &QString,
        user: &QString,
        password: &QString,
        dav_url: &QString,
        password_command: &QString,
    ) {
        let config = CalendarConfig {
            server: server.to_string(),
            user: user.to_string(),
            dav_url: dav_url.to_string(),
            ..Default::default()
        };
        // Empty: the password already given in this Atlas start, else the
        // command's.
        let password = Some(password.to_string()).filter(|p| !p.is_empty()).or_else(password_now);
        let command = password_command.to_string();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let password = match password {
                Some(p) => Ok(p),
                None if !command.trim().is_empty() => calendar::password_from_command(&command, COMMAND_TIMEOUT)
                    .map_err(|e| anyhow::anyhow!("Passwort-Befehl: {e:#}")),
                None => Ok(String::new()),
            };
            let result = password.and_then(|p| client(&config, &p).discover().map_err(|e| anyhow::anyhow!("{e}")));
            let _ = thread.queue(move |this| match result {
                Ok(d) => {
                    let list: Vec<Value> = d
                        .calendars
                        .iter()
                        .map(|c| json!({ "url": c.url, "name": c.name, "color": c.color, "writable": c.writable }))
                        .collect();
                    let message = format!("Verbunden – {} Kalender gefunden", list.len());
                    this.connection_tested(true, qs(message), qs(Value::from(list).to_string()));
                }
                Err(err) => this.connection_tested(false, qs(err.to_string()), qs("[]")),
            });
        });
    }

    pub fn save_account(
        mut self: Pin<&mut Self>,
        server: &QString,
        user: &QString,
        password: &QString,
        dav_url: &QString,
        calendars: &QString,
        password_command: &QString,
    ) {
        let mut config = Config::load().calendar;
        let command = password_command.to_string().trim().to_owned();
        if config.password_command != command {
            reset_command();
        }
        config.password_command = command;
        let changed_account = config.server != server.to_string()
            || config.user != user.to_string()
            || config.dav_url != dav_url.to_string();
        config.server = server.to_string().trim().to_owned();
        config.user = user.to_string().trim().to_owned();
        config.dav_url = dav_url.to_string().trim().to_owned();
        if let Ok(Value::Array(list)) = serde_json::from_str::<Value>(&calendars.to_string()) {
            let old = std::mem::take(&mut config.calendars);
            for c in list {
                let Some(url) = c["url"].as_str() else { continue };
                let mut choice = old
                    .iter()
                    .find(|o| o.url == url)
                    .cloned()
                    .unwrap_or(CalendarChoice { url: url.to_owned(), ..Default::default() });
                choice.name = c["name"].as_str().unwrap_or_default().to_owned();
                choice.hidden = !c["visible"].as_bool().unwrap_or(true);
                config.calendars.push(choice);
            }
        }
        if !config.calendars.iter().any(|c| c.url == config.default) {
            config.default.clear();
        }
        if let Err(err) = Config::save_calendar(&config) {
            self.as_mut().set_error(format!("Einstellungen können nicht gespeichert werden: {err:#}"));
            return;
        }
        if changed_account {
            let _ = self.sync_store().clear();
            let _ = forget_secret();
        }
        let password = password.to_string();
        if !password.is_empty()
            && let Ok(mut p) = PASSWORD.lock()
        {
            *p = Some(password.clone());
        }
        if !password.is_empty() && config.remember_password {
            self.as_mut().keep_password(password);
        }
        self.as_mut().reload();
        self.sync_now();
    }

    pub fn remember_password(mut self: Pin<&mut Self>, remember: bool) {
        let mut config = Config::load().calendar;
        if config.remember_password != remember {
            config.remember_password = remember;
            if let Err(err) = Config::save_calendar(&config) {
                self.as_mut().set_error(format!("Einstellungen können nicht gespeichert werden: {err:#}"));
                return;
            }
            self.as_mut().rust_mut().config.remember_password = remember;
        }
        if !remember {
            if let Err(err) = forget_secret() {
                self.as_mut().set_error(format!("Gemerktes Passwort kann nicht gelöscht werden: {err:#}"));
            }
            self.as_mut().update_sync_state();
        } else if let Some(password) = password() {
            self.as_mut().keep_password(password);
        }
    }

    /// Stores the password on this machine in the background.
    fn keep_password(self: Pin<&mut Self>, password: String) {
        let seq = SECRET_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let _guard = SECRET_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            // A newer store or a forget came after this one.
            let result = if SECRET_SEQ.load(std::sync::atomic::Ordering::SeqCst) == seq {
                secret_store().store(&password).map(|_| ())
            } else {
                Ok(())
            };
            let _ = thread.queue(move |mut this| {
                if let Err(err) = result {
                    this.as_mut().set_error(format!("Passwort kann nicht gemerkt werden: {err:#}"));
                }
                this.as_mut().update_sync_state();
            });
        });
    }

    pub fn retry_password_command(mut self: Pin<&mut Self>) {
        reset_command();
        self.as_mut().fetch_password();
    }

    /// Runs the password command in the background, once per command line,
    /// while no password is known; synchronises when it succeeded.
    fn fetch_password(mut self: Pin<&mut Self>) {
        let command = self.config.password_command.clone();
        let remembered = secret_store().kind().is_some();
        if !self.config.connected() || (command.is_empty() && !remembered) || password().is_some() {
            return;
        }
        // Marks this attempt (a changed command line tries again).
        let command = if command.is_empty() { "\u{0}remembered".to_owned() } else { command };
        {
            let Ok(mut c) = COMMAND.lock() else { return };
            if c.running || c.tried == command {
                return;
            }
            c.tried = command.clone();
            c.running = true;
            c.error.clear();
        }
        self.as_mut().update_sync_state();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            // The remembered password first, then the command.
            let result = match secret_store().load() {
                Ok(Some((password, _))) => Ok(password),
                Ok(None) | Err(_) if !command.starts_with('\u{0}') => {
                    calendar::password_from_command(&command, COMMAND_TIMEOUT)
                }
                Ok(None) => Err(anyhow::anyhow!("kein gemerktes Passwort")),
                Err(err) => Err(err.context("Gemerktes Passwort")),
            };
            let _ = thread.queue(move |mut this| {
                // Only a changed setting (other command, remembered ⇄ command)
                // makes the result stale; a retry meanwhile does not.
                let current = this.config.password_command.clone();
                let stale = current != command && !(current.is_empty() && command.starts_with('\u{0}'));
                let mut ok = false;
                if let Ok(mut c) = COMMAND.lock() {
                    c.running = false;
                    if !stale {
                        c.tried = command;
                        match result {
                            Ok(password) => {
                                ok = true;
                                if let Ok(mut p) = PASSWORD.lock() {
                                    p.get_or_insert(password);
                                }
                            }
                            Err(err) => {
                                log::warn!("password command failed: {err:#}");
                                c.error = format!("{err:#}");
                            }
                        }
                    }
                }
                this.as_mut().update_sync_state();
                if ok {
                    this.sync_now();
                } else if stale {
                    this.fetch_password();
                }
            });
        });
    }

    pub fn set_password(mut self: Pin<&mut Self>, password: &QString) {
        let password = password.to_string();
        if let Ok(mut p) = PASSWORD.lock() {
            *p = (!password.is_empty()).then(|| password.clone());
        }
        if !password.is_empty() && self.config.remember_password {
            self.as_mut().keep_password(password);
        }
        self.as_mut().update_sync_state();
        self.sync_now();
    }

    fn change_choice(mut self: Pin<&mut Self>, id: &str, f: impl FnOnce(&mut CalendarChoice, &str)) {
        let mut config = Config::load().calendar;
        let root = self.project_root.to_string();
        let Some(choice) = config.calendars.iter_mut().find(|c| c.url == id) else { return };
        f(choice, &root);
        if let Err(err) = Config::save_calendar(&config) {
            self.as_mut().set_error(format!("Einstellungen können nicht gespeichert werden: {err:#}"));
        }
        self.as_mut().reload();
        self.sync_now();
    }

    pub fn set_calendar_visible(self: Pin<&mut Self>, id: &QString, visible: bool) {
        self.change_choice(&id.to_string(), |c, root| {
            c.hidden = !visible;
            // Shown in this project again: also if it was limited to others.
            if visible && !c.projects.is_empty() && !c.projects.iter().any(|p| p == root) {
                c.projects.push(root.to_owned());
            }
        });
    }

    pub fn set_calendar_project_only(self: Pin<&mut Self>, id: &QString, only: bool) {
        self.change_choice(&id.to_string(), |c, root| {
            c.projects = if only { vec![root.to_owned()] } else { Vec::new() };
        });
    }

    pub fn set_default_calendar(mut self: Pin<&mut Self>, id: &QString, project: bool) {
        let id = id.to_string();
        let result = if project {
            let mut settings = self.project_settings();
            settings["default_calendar"] = json!(id);
            serde_json::to_string_pretty(&settings)
                .map_err(anyhow::Error::from)
                .and_then(|t| atlas_core::paths::write_atomic(&self.store().dir().join(SETTINGS), t.as_bytes()))
        } else {
            let mut config = Config::load().calendar;
            config.default = if id == LOCAL { String::new() } else { id };
            Config::save_calendar(&config)
        };
        if let Err(err) = result {
            self.as_mut().set_error(format!("Standardkalender kann nicht gespeichert werden: {err:#}"));
        }
        self.reload();
    }

    pub fn sync_now(mut self: Pin<&mut Self>) {
        if !self.config.connected() {
            return;
        }
        let Some(password) = password() else {
            self.as_mut().update_sync_state();
            self.fetch_password();
            return;
        };
        if self.syncing {
            self.as_mut().rust_mut().sync_again = true;
            return;
        }
        let calendars = self.synced();
        if calendars.is_empty() {
            return;
        }
        self.as_mut().rust_mut().syncing = true;
        self.as_mut().rust_mut().sync_again = false;
        self.as_mut().update_sync_state();
        let store = self.sync_store();
        let remote = client(&self.config, &password);
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = sync::run(&store, &remote, &calendars);
            let _ = thread.queue(move |mut this| {
                this.as_mut().rust_mut().syncing = false;
                let (ok, message) = match &result {
                    Ok(r) => {
                        let mut parts = Vec::new();
                        if r.uploaded + r.deleted > 0 {
                            parts.push(format!("{} gesendet", r.uploaded + r.deleted));
                        }
                        if r.downloaded + r.removed > 0 {
                            parts.push(format!("{} vom Server", r.downloaded + r.removed));
                        }
                        if r.conflicts > 0 {
                            parts.push(format!("{} Konflikt(e)", r.conflicts));
                        }
                        (
                            true,
                            if parts.is_empty() {
                                "Kalender aktuell".to_owned()
                            } else {
                                format!("Kalender: {}", parts.join(", "))
                            },
                        )
                    }
                    Err(err) => (false, err.to_string()),
                };
                this.as_mut().reload();
                this.as_mut().sync_finished(ok, qs(message));
                if this.sync_again {
                    this.sync_now();
                }
            });
        });
    }

    pub fn create_calendar(self: Pin<&mut Self>, name: &QString) {
        let Some(password) = password() else {
            self.calendar_created(false, qs("App-Passwort fehlt"), QString::default());
            return;
        };
        let name = name.to_string();
        let config = self.config.clone();
        let home = self.sync_store().load().home;
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let remote = client(&config, &password);
            // (Google needs no home: the Calendar API creates calendars.)
            let result = match home {
                Some(h) => Ok(h),
                None if config.is_google() => Ok(String::new()),
                None => remote.discover().and_then(|d| {
                    d.home.ok_or_else(|| {
                        atlas_core::calendar::caldav::DavError::Invalid("kein Kalenderverzeichnis".into())
                    })
                }),
            }
            .and_then(|home| remote.create_calendar(&home, &name, "#2ea043"));
            let _ = thread.queue(move |mut this| match result {
                Ok(cal) => {
                    let mut config = Config::load().calendar;
                    config.calendars.push(CalendarChoice {
                        url: cal.url.clone(),
                        name: cal.name.clone(),
                        ..Default::default()
                    });
                    if let Err(err) = Config::save_calendar(&config) {
                        this.as_mut().set_error(format!("{err:#}"));
                    }
                    this.as_mut().reload();
                    this.as_mut().calendar_created(true, qs(format!("Kalender „{}“ angelegt", cal.name)), qs(&cal.url));
                    this.sync_now();
                }
                Err(err) => this.calendar_created(false, qs(err.to_string()), QString::default()),
            });
        });
    }

    pub fn clear_cache(mut self: Pin<&mut Self>) {
        if let Err(err) = self.sync_store().clear() {
            self.as_mut().set_error(format!("Cache kann nicht gelöscht werden: {err:#}"));
        }
        self.as_mut().reload();
        self.sync_now();
    }

    pub fn google_sign_in(mut self: Pin<&mut Self>, client_id: &QString, client_secret: &QString) {
        let oauth = google::OAuthClient {
            id: client_id.to_string().trim().to_owned(),
            secret: client_secret.to_string().trim().to_owned(),
        };
        if oauth.id.is_empty() || oauth.secret.is_empty() {
            self.google_signed_in(false, qs("Client-ID und Client-Secret eintragen"));
            return;
        }
        // The browser comes back to this port (loopback redirect).
        let listener = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(err) => {
                self.google_signed_in(false, qs(format!("Kein lokaler Port frei: {err}")));
                return;
            }
        };
        let redirect = match listener.local_addr() {
            Ok(a) => format!("http://127.0.0.1:{}", a.port()),
            Err(err) => {
                self.google_signed_in(false, qs(err.to_string()));
                return;
            }
        };
        let (state, verifier) = (google::random_token(), google::random_token());
        let endpoints = google::Endpoints::default();
        self.as_mut().google_sign_in_url(qs(google::auth_url(&endpoints, &oauth, &redirect, &state, &verifier)));
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<(google::Tokens, Vec<Value>)> {
                let code = google::wait_for_code(&listener, &state, std::time::Duration::from_secs(300))?;
                let http = Client::new(Account::default(), "", true);
                let tokens = google::exchange_code(
                    |body| http.post_form(&endpoints.token, body).map_err(|e| anyhow::anyhow!("{e}")),
                    &oauth,
                    &code,
                    &redirect,
                    &verifier,
                )?;
                anyhow::ensure!(!tokens.email.is_empty(), "Google hat keine E-Mail-Adresse geschickt");
                let remote = Client::google(&tokens.email, oauth.clone(), &tokens.refresh, endpoints, true);
                let found = remote.discover().map_err(|e| anyhow::anyhow!("{e}"))?;
                let list = found
                    .calendars
                    .iter()
                    .map(|c| json!({ "url": c.url, "name": c.name, "color": c.color, "writable": c.writable }))
                    .collect();
                Ok((tokens, list))
            })();
            let _ = thread.queue(move |mut this| match result {
                Ok((tokens, list)) => {
                    this.as_mut().use_google_account(&oauth, &tokens.email, &tokens.refresh);
                    let message = format!("Angemeldet als {} – {} Kalender gefunden", tokens.email, list.len());
                    this.as_mut().connection_tested(true, qs(&message), qs(Value::from(list).to_string()));
                    this.google_signed_in(true, qs(message));
                }
                Err(err) => this.google_signed_in(false, qs(format!("{err:#}"))),
            });
        });
    }

    /// After the sign-in: the Google account in `config.toml`, the refresh
    /// token remembered (encrypted if possible) and for this Atlas start.
    fn use_google_account(mut self: Pin<&mut Self>, oauth: &google::OAuthClient, email: &str, refresh: &str) {
        let mut config = Config::load().calendar;
        if !(config.is_google() && config.user == email) {
            // Another account: its sign-in, calendars and cache go.
            let _ = forget_secret();
            let _ = self.sync_store().clear();
            config = CalendarConfig::default();
        }
        config.provider = "google".into();
        config.user = email.to_owned();
        config.google_client_id = oauth.id.clone();
        config.google_client_secret = oauth.secret.clone();
        // Without it every start would need the browser again.
        config.remember_password = true;
        if let Err(err) = Config::save_calendar(&config) {
            self.as_mut().set_error(format!("Einstellungen können nicht gespeichert werden: {err:#}"));
            return;
        }
        reset_command();
        if let Ok(mut p) = PASSWORD.lock() {
            *p = Some(refresh.to_owned());
        }
        self.as_mut().keep_password(refresh.to_owned());
        self.as_mut().reload();
        self.update_sync_state();
    }

    pub fn set_provider(mut self: Pin<&mut Self>, provider: &QString) {
        let provider = provider.to_string();
        let old = Config::load().calendar;
        if old.provider == provider {
            return;
        }
        if let Ok(mut p) = PASSWORD.lock() {
            *p = None;
        }
        reset_command();
        // (The store of the old provider, before the switch.)
        let _ = forget_secret();
        let config = CalendarConfig {
            provider,
            google_client_id: old.google_client_id,
            google_client_secret: old.google_client_secret,
            ..Default::default()
        };
        if let Err(err) = Config::save_calendar(&config) {
            self.as_mut().set_error(format!("Einstellungen können nicht gespeichert werden: {err:#}"));
        }
        let _ = self.sync_store().clear();
        self.reload();
    }

    pub fn disconnect_account(mut self: Pin<&mut Self>) {
        if let Ok(mut p) = PASSWORD.lock() {
            *p = None;
        }
        reset_command();
        let _ = forget_secret();
        if let Err(err) = Config::save_calendar(&CalendarConfig::default()) {
            self.as_mut().set_error(format!("Einstellungen können nicht gespeichert werden: {err:#}"));
        }
        let _ = self.sync_store().clear();
        self.reload();
    }

    pub fn conflicts(&self) -> QString {
        let tz = self.tz;
        let describe = |data: &Option<String>| -> Value {
            let Some(data) = data else { return Value::Null };
            let entries = sync::parse_object(data, "");
            let Some(e) = entries.iter().find(|e| e.recurrence_id.is_none()).or(entries.first()) else {
                return Value::Null;
            };
            let when = match e.start {
                Some(s) if s.is_date() => s.local(tz).format("%d.%m.%Y").to_string(),
                Some(s) => s.local(tz).format("%d.%m.%Y %H:%M").to_string(),
                None => String::new(),
            };
            json!({ "title": e.title, "when": when, "description": e.description, "status": e.status.as_str() })
        };
        let state = self.sync_store().load();
        let list: Vec<Value> = state
            .conflicts
            .iter()
            .map(|c| {
                json!({
                    "id": c.id, "title": c.title, "calendar": self.calendar_info(&c.calendar).0,
                    "local": describe(&c.local), "remote": describe(&c.remote), "message": c.message,
                })
            })
            .collect();
        qs(Value::from(list).to_string())
    }

    pub fn resolve_conflict(mut self: Pin<&mut Self>, id: i32, choice: &QString) -> bool {
        let choice = match choice.to_string().as_str() {
            "remote" => Choice::Remote,
            "local" => Choice::Local,
            _ => Choice::Both,
        };
        let state = self.sync_store().load();
        let Some(conflict) = state.conflicts.iter().find(|c| c.id == id as u64).cloned() else { return false };
        match self.sync_store().update(|s| s.resolve(conflict.id, choice)) {
            Ok(copy) => {
                // The copy keeps the links of the local version.
                if let Some(uid) = copy {
                    let links = self.store().links();
                    if let Some(l) = links.get(&conflict.uid) {
                        let mut e = Entry::new(Kind::Event, "");
                        e.uid = uid;
                        e.files = l.files.clone();
                        e.session = l.session.clone();
                        let _ = self.store().save_links([&e]);
                    }
                }
                self.as_mut().reload();
                self.sync_now();
                true
            }
            Err(err) => {
                self.as_mut().set_error(format!("{err:#}"));
                false
            }
        }
    }
}
