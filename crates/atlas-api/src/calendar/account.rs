// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The account (CalDAV server or Google), the password (typed, remembered
//! or printed by a command), syncing and its conflicts.
//! Template: `calendar_model/{account,remote}.rs`.

use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use atlas_core::calendar::caldav::{self, Client, DavError, Remote};
use atlas_core::calendar::sync::{self, Choice};
use atlas_core::calendar::{self as core, Kind, google};
use atlas_core::config::{CalendarChoice, CalendarConfig, Config};
use atlas_core::secret::SecretStore;

use super::{
    Account, AtlasError, CalendarInfo, Conflict, ConflictChoice, ConnectionTest, Inner, Provider, Result, SyncState,
    failed,
};
use crate::events::{Event, Level};
use crate::util::background;

/// The password (Google: the refresh token) for this run only; one per
/// process like on Linux.
static PASSWORD: Mutex<Option<String>> = Mutex::new(None);

fn password() -> Option<String> {
    PASSWORD.lock().ok().and_then(|p| p.clone())
}

fn set_password_now(password: Option<String>) {
    if let Ok(mut p) = PASSWORD.lock() {
        *p = password;
    }
}

/// The password command runs once per command line (again after it or the
/// account changed, or on request).
struct CommandState {
    /// The command last run ("" = none yet).
    tried: String,
    running: bool,
    error: String,
}

static COMMAND: Mutex<CommandState> =
    Mutex::new(CommandState { tried: String::new(), running: false, error: String::new() });

/// Time for the command, including typing a passphrase into pinentry.
const COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);

/// Where the remembered secret lives.
fn secret_store() -> SecretStore {
    SecretStore::default_for(if Config::load().calendar.is_google() { "google" } else { "nextcloud" })
}

/// Storing and forgetting happen one after another; a newer request makes a
/// still waiting older store obsolete (the latest password wins).
static SECRET_LOCK: Mutex<()> = Mutex::new(());
static SECRET_SEQ: AtomicU64 = AtomicU64::new(0);

fn forget_secret() -> anyhow::Result<()> {
    SECRET_SEQ.fetch_add(1, Ordering::SeqCst);
    let _guard = SECRET_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    secret_store().forget()
}

pub(super) fn reset_command() {
    if let Ok(mut c) = COMMAND.lock() {
        c.tried.clear();
        c.error.clear();
    }
}

/// The client for the configured account; for Google `password` is the
/// refresh token.
fn client(c: &CalendarConfig, password: &str) -> Client {
    if c.is_google() {
        return Client::google(&c.user, oauth_client(c), password, google::Endpoints::default(), true);
    }
    Client::new(
        caldav::Account { server: c.server.clone(), user: c.user.clone(), dav_url: c.dav_url.clone() },
        password,
        true,
    )
}

fn oauth_client(c: &CalendarConfig) -> google::OAuthClient {
    google::OAuthClient { id: c.google_client_id.clone(), secret: c.google_client_secret.clone() }
}

fn describe_time(t: i64) -> String {
    chrono::DateTime::from_timestamp(t, 0)
        .map(|d| d.with_timezone(&core::local_zone()).format("%d.%m. %H:%M").to_string())
        .unwrap_or_default()
}

fn dav_error(err: DavError) -> AtlasError {
    match err {
        DavError::Auth => AtlasError::Auth { message: err.to_string() },
        DavError::Offline(_) => AtlasError::Unavailable { message: err.to_string() },
        _ => AtlasError::Failed { message: err.to_string() },
    }
}

fn found(calendars: &[caldav::RemoteCalendar]) -> Vec<CalendarInfo> {
    calendars
        .iter()
        .map(|c| CalendarInfo {
            id: c.url.clone(),
            name: c.name.clone(),
            color: c.color.clone(),
            visible: true,
            project_only: false,
            writable: c.writable,
            remote: true,
            missing: false,
            is_default: false,
            project_default: false,
            global_default: false,
        })
        .collect()
}

fn save_config(config: &CalendarConfig) -> Result<()> {
    Config::save_calendar(config).map_err(|e| failed("Einstellungen können nicht gespeichert werden", e))
}

/// A running Google sign-in.
pub(super) struct SignIn {
    /// Loopback port the browser comes back to.
    port: u16,
    result: Option<futures_channel::oneshot::Receiver<Result<String>>>,
}

impl Inner {
    fn provider(c: &CalendarConfig) -> Provider {
        if c.is_google() {
            Provider::Google
        } else if c.connected() {
            Provider::CalDav
        } else {
            Provider::None
        }
    }

    pub(super) fn account(&self) -> Account {
        let state = self.sync_store().load();
        let s = self.lock();
        let c = &s.config;
        Account {
            provider: Self::provider(c),
            server: c.server.clone(),
            dav_url: c.dav_url.clone(),
            user: c.user.clone(),
            password_command: c.password_command.clone(),
            remember_password: c.remember_password,
            google_client_id: c.google_client_id.clone(),
            google_client_secret: c.google_client_secret.clone(),
            last_sync: state.last_sync.map(describe_time),
            home: state.home.clone(),
            calendars: self.calendars(&s).into_iter().filter(|c| c.remote).collect(),
        }
    }

    pub(super) fn sync_state(&self) -> SyncState {
        let state = self.sync_store().load();
        let s = self.lock();
        let connected = s.config.connected();
        let unlocked = connected && password().is_some();
        let (fetching, command_error) =
            COMMAND.lock().map(|c| (c.running, c.error.clone())).unwrap_or((false, String::new()));
        let synced = self.visible_remote(&s);
        let failed = !state.error.is_empty();
        let status = if !connected {
            String::new()
        } else if s.syncing {
            "Synchronisiere …".to_owned()
        } else if failed {
            state.error.clone()
        } else if !unlocked && fetching {
            "Hole App-Passwort …".to_owned()
        } else if !unlocked && s.config.is_google() {
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
        SyncState {
            connected,
            google: s.config.is_google(),
            unlocked,
            fetching_password: fetching && !unlocked,
            remembered: secret_store().kind().map(|k| k.as_str().to_owned()).unwrap_or_default(),
            password_error: if unlocked { String::new() } else { command_error },
            syncing: s.syncing,
            status,
            failed,
            pending: state.queue.iter().filter(|o| synced.contains(&o.calendar)).count() as u32,
            conflicts: state.conflicts.len() as u32,
            problem: s.problem.clone(),
        }
    }

    /// Refreshes calendar names and waiting changes from the cache.
    pub(super) fn update_sync_state(&self) {
        let state = self.sync_store().load();
        {
            let mut s = self.lock();
            let meta = state
                .calendars
                .iter()
                .map(|(url, c)| {
                    let name = Some(c.info.name.clone())
                        .filter(|n| !n.is_empty())
                        .or_else(|| s.config.calendars.iter().find(|x| x.url == *url).map(|x| x.name.clone()))
                        .unwrap_or_else(|| url.clone());
                    let color = Some(c.info.color.clone())
                        .filter(|c| !c.is_empty())
                        .unwrap_or_else(|| super::REMOTE_COLOR.into());
                    (url.clone(), (name, color, c.info.writable && !c.missing))
                })
                .collect();
            s.meta = meta;
            s.waiting = state.queue.iter().map(|o| (o.calendar.clone(), o.uid.clone())).collect();
        }
        self.emit(Event::SyncChanged);
    }

    pub(super) async fn test_connection(&self, account: Account, password: String) -> Result<ConnectionTest> {
        let config = CalendarConfig {
            server: account.server.trim().to_owned(),
            user: account.user.trim().to_owned(),
            dav_url: account.dav_url.trim().to_owned(),
            ..Default::default()
        };
        // Empty: the password of this run, else the command's.
        let password = Some(password).filter(|p| !p.is_empty()).or_else(self::password);
        let command = account.password_command;
        background(move || {
            let password = match password {
                Some(p) => p,
                None if !command.trim().is_empty() => core::password_from_command(&command, COMMAND_TIMEOUT)
                    .map_err(|e| AtlasError::Auth { message: format!("Passwort-Befehl: {e:#}") })?,
                None => String::new(),
            };
            let d = client(&config, &password).discover().map_err(dav_error)?;
            Ok(ConnectionTest {
                message: format!("Verbunden – {} Kalender gefunden", d.calendars.len()),
                calendars: found(&d.calendars),
            })
        })
        .await
    }

    pub(super) fn save_account(self: &Arc<Self>, account: Account) -> Result<()> {
        let mut config = Config::load().calendar;
        let provider = match account.provider {
            Provider::None => return self.disconnect(),
            Provider::CalDav => "",
            Provider::Google => "google",
        };
        if config.provider != provider {
            // Another provider: the old one's sign-in, calendars and cache go.
            set_password_now(None);
            reset_command();
            let _ = forget_secret();
            let _ = self.sync_store().clear();
            config = CalendarConfig {
                provider: provider.into(),
                google_client_id: config.google_client_id,
                google_client_secret: config.google_client_secret,
                ..Default::default()
            };
        }
        let command = account.password_command.trim().to_owned();
        if config.password_command != command {
            reset_command();
        }
        config.password_command = command;
        if provider.is_empty() {
            let (server, user, dav_url) =
                (account.server.trim().to_owned(), account.user.trim().to_owned(), account.dav_url.trim().to_owned());
            if config.server != server || config.user != user || config.dav_url != dav_url {
                // Another account: its cache and remembered password go.
                let _ = self.sync_store().clear();
                let _ = forget_secret();
                set_password_now(None);
            }
            config.server = server;
            config.user = user;
            config.dav_url = dav_url;
        } else {
            config.google_client_id = account.google_client_id.trim().to_owned();
            config.google_client_secret = account.google_client_secret.trim().to_owned();
        }
        let old = std::mem::take(&mut config.calendars);
        for c in &account.calendars {
            let mut choice = old
                .iter()
                .find(|o| o.url == c.id)
                .cloned()
                .unwrap_or(CalendarChoice { url: c.id.clone(), ..Default::default() });
            choice.name = c.name.clone();
            choice.hidden = !c.visible;
            config.calendars.push(choice);
        }
        if !config.calendars.iter().any(|c| c.url == config.default) {
            config.default.clear();
        }
        let remember_changed = config.remember_password != account.remember_password;
        config.remember_password = account.remember_password;
        save_config(&config)?;
        if remember_changed && !account.remember_password {
            let _ = forget_secret();
        } else if remember_changed && let Some(p) = password() {
            self.keep_password(p);
        }
        self.reload();
        self.sync_now();
        Ok(())
    }

    pub(super) fn remember_password(self: &Arc<Self>, remember: bool) -> Result<()> {
        let mut config = Config::load().calendar;
        if config.remember_password != remember {
            config.remember_password = remember;
            save_config(&config)?;
            self.lock().config.remember_password = remember;
        }
        if !remember {
            forget_secret().map_err(|e| failed("Gemerktes Passwort kann nicht gelöscht werden", e))?;
            self.update_sync_state();
        } else if let Some(p) = password() {
            self.keep_password(p);
        }
        Ok(())
    }

    /// Stores the password on this machine in the background.
    fn keep_password(self: &Arc<Self>, password: String) {
        let seq = SECRET_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
        let this = self.clone();
        std::thread::spawn(move || {
            let result = {
                let _guard = SECRET_LOCK.lock().unwrap_or_else(|e| e.into_inner());
                // A newer store or a forget came after this one.
                if SECRET_SEQ.load(Ordering::SeqCst) == seq {
                    secret_store().store(&password).map(|_| ())
                } else {
                    Ok(())
                }
            };
            if let Err(err) = result {
                this.message(Level::Error, format!("Passwort kann nicht gemerkt werden: {err:#}"));
            }
            this.update_sync_state();
        });
    }

    pub(super) fn set_password(self: &Arc<Self>, password: String, remember: bool) -> Result<()> {
        set_password_now(Some(password.clone()).filter(|p| !p.is_empty()));
        let result = self.remember_password(remember);
        self.update_sync_state();
        self.sync_now();
        result
    }

    /// Runs the password command (or loads the remembered password) in the
    /// background, once per command line, while no password is known; syncs
    /// when it succeeded.
    pub(super) fn fetch_password(self: &Arc<Self>) {
        let (connected, command) = {
            let s = self.lock();
            (s.config.connected(), s.config.password_command.clone())
        };
        let remembered = secret_store().kind().is_some();
        if !connected || (command.is_empty() && !remembered) || password().is_some() {
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
        self.emit(Event::SyncChanged);
        let this = self.clone();
        std::thread::spawn(move || {
            // The remembered password first, then the command.
            let result = match secret_store().load() {
                Ok(Some((password, _))) => Ok(password),
                Ok(None) | Err(_) if !command.starts_with('\u{0}') => {
                    core::password_from_command(&command, COMMAND_TIMEOUT)
                }
                Ok(None) => Err(anyhow::anyhow!("kein gemerktes Passwort")),
                Err(err) => Err(err.context("Gemerktes Passwort")),
            };
            // Only a changed setting makes the result stale; a retry does not.
            let current = Config::load().calendar.password_command;
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
            this.update_sync_state();
            if ok {
                this.sync_now();
            } else if stale {
                this.fetch_password();
            }
        });
    }

    pub(super) fn google_sign_in(self: &Arc<Self>, client_id: String, client_secret: String) -> Result<String> {
        let oauth = google::OAuthClient { id: client_id.trim().to_owned(), secret: client_secret.trim().to_owned() };
        if oauth.id.is_empty() || oauth.secret.is_empty() {
            return Err(AtlasError::Invalid { message: "Client-ID und Client-Secret eintragen".into() });
        }
        self.cancel_google_sign_in();
        // The browser comes back to this port (loopback redirect).
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| AtlasError::Unavailable { message: format!("Kein lokaler Port frei: {e}") })?;
        let port = listener.local_addr()?.port();
        let redirect = format!("http://127.0.0.1:{port}");
        let (state, verifier) = (google::random_token(), google::random_token());
        let endpoints = google::Endpoints::default();
        let url = google::auth_url(&endpoints, &oauth, &redirect, &state, &verifier);
        let (tx, rx) = futures_channel::oneshot::channel();
        *self.sign_in.lock().unwrap_or_else(|e| e.into_inner()) = Some(SignIn { port, result: Some(rx) });
        let this = self.clone();
        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<(google::Tokens, Vec<caldav::RemoteCalendar>)> {
                let code = google::wait_for_code(&listener, &state, std::time::Duration::from_secs(300))?;
                let http = Client::new(caldav::Account::default(), "", true);
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
                Ok((tokens, found.calendars))
            })();
            let result = match result {
                Ok((tokens, calendars)) => this.use_google_account(&oauth, &tokens.email, &tokens.refresh, &calendars),
                Err(err) => Err(AtlasError::Auth { message: format!("{err:#}") }),
            };
            let _ = tx.send(result);
        });
        Ok(url)
    }

    /// After the sign-in: the Google account in `config.toml` with its
    /// calendars, the refresh token remembered and kept for this run.
    fn use_google_account(
        self: &Arc<Self>,
        oauth: &google::OAuthClient,
        email: &str,
        refresh: &str,
        calendars: &[caldav::RemoteCalendar],
    ) -> Result<String> {
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
        for c in calendars {
            if !config.calendars.iter().any(|x| x.url == c.url) {
                config.calendars.push(CalendarChoice {
                    url: c.url.clone(),
                    name: c.name.clone(),
                    ..Default::default()
                });
            }
        }
        save_config(&config)?;
        reset_command();
        set_password_now(Some(refresh.to_owned()));
        self.keep_password(refresh.to_owned());
        self.reload();
        self.sync_now();
        Ok(format!("Angemeldet als {email} – {} Kalender gefunden", calendars.len()))
    }

    pub(super) async fn google_sign_in_finished(self: Arc<Self>) -> Result<String> {
        let rx = self.sign_in.lock().unwrap_or_else(|e| e.into_inner()).as_mut().and_then(|s| s.result.take());
        let Some(rx) = rx else {
            return Err(AtlasError::NotFound { message: "Keine Google-Anmeldung läuft".into() });
        };
        let result = rx.await.unwrap_or_else(|_| Err(AtlasError::Failed { message: "Anmeldung abgebrochen".into() }));
        *self.sign_in.lock().unwrap_or_else(|e| e.into_inner()) = None;
        result
    }

    pub(super) fn cancel_google_sign_in(&self) {
        let port = self.sign_in.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|s| s.port);
        if let Some(port) = port {
            // The waiting listener ends on a declined answer.
            use std::io::Write;
            if let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", port)) {
                let _ = stream.write_all(b"GET /?error=abgebrochen HTTP/1.1\r\n\r\n");
            }
        }
    }

    pub(super) fn sync_now(self: &Arc<Self>) {
        let (config, calendars) = {
            let mut s = self.lock();
            if !s.config.connected() {
                return;
            }
            if password().is_none() {
                drop(s);
                self.update_sync_state();
                self.fetch_password();
                return;
            }
            if s.syncing {
                s.sync_again = true;
                return;
            }
            let calendars = self.visible_remote(&s);
            if calendars.is_empty() {
                return;
            }
            s.syncing = true;
            s.sync_again = false;
            (s.config.clone(), calendars)
        };
        let Some(password) = password() else { return };
        self.update_sync_state();
        let store = self.sync_store();
        let remote = client(&config, &password);
        let this = self.clone();
        std::thread::spawn(move || {
            let result = sync::run(&store, &remote, &calendars);
            let again = {
                let mut s = this.lock();
                s.syncing = false;
                s.sync_again
            };
            let (level, message) = match &result {
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
                    let text = if parts.is_empty() {
                        "Kalender aktuell".to_owned()
                    } else {
                        format!("Kalender: {}", parts.join(", "))
                    };
                    (Level::Info, text)
                }
                Err(err) => (Level::Warning, err.to_string()),
            };
            this.reload();
            this.message(level, message);
            if again {
                this.sync_now();
            }
        });
    }

    pub(super) async fn create_remote_calendar(self: Arc<Self>, name: String) -> Result<CalendarInfo> {
        let Some(password) = password() else {
            return Err(AtlasError::Auth { message: "App-Passwort fehlt".into() });
        };
        let config = self.lock().config.clone();
        let home = self.sync_store().load().home;
        let this = self.clone();
        background(move || {
            let remote = client(&config, &password);
            // (Google needs no home: the Calendar API creates calendars.)
            let home = match home {
                Some(h) => h,
                None if config.is_google() => String::new(),
                None => remote
                    .discover()
                    .and_then(|d| d.home.ok_or_else(|| DavError::Invalid("kein Kalenderverzeichnis".into())))
                    .map_err(dav_error)?,
            };
            let cal = remote.create_calendar(&home, &name, "#2ea043").map_err(dav_error)?;
            let mut config = Config::load().calendar;
            config.calendars.push(CalendarChoice {
                url: cal.url.clone(),
                name: cal.name.clone(),
                ..Default::default()
            });
            save_config(&config)?;
            this.reload();
            this.message(Level::Info, format!("Kalender „{}“ angelegt", cal.name));
            this.sync_now();
            let s = this.lock();
            Ok(this.calendars(&s).into_iter().find(|c| c.id == cal.url).unwrap_or_else(|| found(&[cal])[0].clone()))
        })
        .await
    }

    pub(super) fn conflicts(&self) -> Vec<Conflict> {
        let state = self.sync_store().load();
        let s = self.lock();
        let version = |data: &Option<String>, calendar: &str| {
            let entries = sync::parse_object(data.as_deref()?, calendar);
            let e = entries.iter().find(|e| e.recurrence_id.is_none()).or(entries.first())?;
            Some(self.entry_out(&s, e))
        };
        state
            .conflicts
            .iter()
            .map(|c| Conflict {
                id: c.id as u32,
                uid: c.uid.clone(),
                title: c.title.clone(),
                calendar: c.calendar.clone(),
                calendar_name: self.calendar_info(&s, &c.calendar).0,
                message: c.message.clone(),
                local: version(&c.local, &c.calendar),
                remote: version(&c.remote, &c.calendar),
            })
            .collect()
    }

    pub(super) fn resolve_conflict(self: &Arc<Self>, id: u32, choice: ConflictChoice) -> Result<()> {
        let choice = match choice {
            ConflictChoice::KeepRemote => Choice::Remote,
            ConflictChoice::KeepLocal => Choice::Local,
            ConflictChoice::KeepBoth => Choice::Both,
        };
        let state = self.sync_store().load();
        let conflict = state
            .conflicts
            .iter()
            .find(|c| c.id == id as u64)
            .cloned()
            .ok_or_else(|| AtlasError::NotFound { message: format!("Kein Konflikt {id}") })?;
        let copy = self
            .sync_store()
            .update(|s| s.resolve(conflict.id, choice))
            .map_err(|e| failed("Konflikt kann nicht gelöst werden", e))?;
        // The copy keeps the links of the local version.
        if let Some(uid) = copy {
            let store = self.store();
            if let Some(l) = store.links().get(&conflict.uid) {
                let mut e = core::Entry::new(Kind::Event, "");
                e.uid = uid;
                e.files = l.files.clone();
                e.session = l.session.clone();
                let _ = store.save_links([&e]);
            }
        }
        self.reload();
        self.sync_now();
        Ok(())
    }

    pub(super) fn disconnect(self: &Arc<Self>) -> Result<()> {
        set_password_now(None);
        reset_command();
        let _ = forget_secret();
        let result = save_config(&CalendarConfig::default());
        let _ = self.sync_store().clear();
        self.reload();
        result
    }
}
