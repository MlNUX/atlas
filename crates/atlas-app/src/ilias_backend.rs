// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! ILIAS for QML (singleton `IliasBackend`): the `[ilias]` settings, the
//! remembered login, reading the favourites and downloads – all in the
//! background with the session of the login window's web profile
//! (atlas_core::ilias does the work).

use std::path::{Path, PathBuf};
use std::pin::Pin;

use atlas_core::config::{Config, IliasConfig};
use atlas_core::ilias::{self, IliasError};
use atlas_core::secret::{Kind, SecretStore};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use crate::util::qs;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        /// `[ilias]` as JSON {base_url, download_dir, show_in_places, user,
        /// login_host, login_saved}.
        #[qproperty(QString, config, READ, NOTIFY)]
        type IliasBackend = super::IliasBackendRust;

        /// Changes `[ilias]`: `base_url`, `download_dir`, `show_in_places`
        /// ("true"/"false") or `login_host`.
        #[qinvokable]
        #[cxx_name = "setConfig"]
        fn set_config(self: Pin<&mut IliasBackend>, key: &QString, value: &QString);

        /// Re-reads `[ilias]` into `config`.
        #[qinvokable]
        #[cxx_name = "reloadConfig"]
        fn reload_config(self: Pin<&mut IliasBackend>);

        /// State file "index" or "downloads" ("" if there is none).
        #[qinvokable]
        fn load(self: &IliasBackend, name: &QString) -> QString;

        /// The web profile (QML WebEngineProfile) whose login session
        /// reading and downloading use.
        #[qinvokable]
        #[cxx_name = "useProfile"]
        unsafe fn use_profile(self: &IliasBackend, profile: *mut QObject) -> bool;

        /// Reads the dashboard's favourites with everything in them, in the
        /// background; then `scanned` (the tree is in "index").
        #[qinvokable]
        fn scan(self: &IliasBackend);

        /// Reading goes on: what is read now.
        #[qsignal]
        fn progress(self: Pin<&mut IliasBackend>, text: QString);

        /// Reading ended: `status` "ok" (with the number of courses),
        /// "login" (log in again) or "error" (with the message).
        #[qsignal]
        fn scanned(self: Pin<&mut IliasBackend>, status: QString, message: QString);

        /// Downloads file `url` (ILIAS address; `download`: the address
        /// sending the file) straight into folder `dir`, named as the server
        /// says or else `title` (numbered if another file has that name; a
        /// new version replaces its earlier download there); remembers it
        /// with `info` in "downloads". Then `downloaded`.
        #[qinvokable]
        fn download(self: &IliasBackend, url: &QString, download: &QString, title: &QString, info: &QString, dir: &QString);

        #[qsignal]
        #[cxx_name = "downloadProgress"]
        fn download_progress(self: Pin<&mut IliasBackend>, url: QString, received: f64, total: f64);

        /// A download ended: `status` "ok" (with `path`), "login" or
        /// "error" (with the message).
        #[qsignal]
        fn downloaded(self: Pin<&mut IliasBackend>, url: QString, path: QString, status: QString, message: QString);

        /// Adds a line to the ILIAS log (`debug.log` in the ILIAS state
        /// folder; e.g. what the login window saw).
        #[qinvokable]
        fn trace(self: &IliasBackend, line: &QString);

        /// The ILIAS log file.
        #[qinvokable]
        #[cxx_name = "debugLog"]
        fn debug_log(self: &IliasBackend) -> QString;

        /// Remembers the university login: the user name in `[ilias]`, the
        /// password only encrypted with systemd-creds (otherwise nothing).
        /// Then `loginStored`.
        #[qinvokable]
        #[cxx_name = "rememberLogin"]
        fn remember_login(self: &IliasBackend, user: &QString, password: &QString);

        #[qsignal]
        #[cxx_name = "loginStored"]
        fn login_stored(self: Pin<&mut IliasBackend>, ok: bool, message: QString);

        /// Forgets the remembered login (password and allowed login page).
        #[qinvokable]
        #[cxx_name = "forgetLogin"]
        fn forget_login(self: Pin<&mut IliasBackend>);

        /// Decrypts the remembered login, in the background; then
        /// `loginReady` (empty password if none or on failure).
        #[qinvokable]
        #[cxx_name = "fetchLogin"]
        fn fetch_login(self: &IliasBackend);

        #[qsignal]
        #[cxx_name = "loginReady"]
        fn login_ready(self: Pin<&mut IliasBackend>, user: QString, password: QString, error: QString);

        /// Signs out: forgets the web session (all its cookies).
        #[qinvokable]
        fn logout(self: &IliasBackend);
    }

    impl cxx_qt::Threading for IliasBackend {}
}

pub struct IliasBackendRust {
    config: QString,
}

impl Default for IliasBackendRust {
    fn default() -> Self {
        Self { config: config_json(&Config::load().ilias) }
    }
}

/// The remembered university login (encrypted only).
fn secret() -> SecretStore {
    SecretStore::default_for("ilias")
}

fn config_json(c: &IliasConfig) -> QString {
    let saved = secret().kind() == Some(Kind::Encrypted);
    let json = serde_json::json!({
        "base_url": c.base_url, "download_dir": c.download_dir, "show_in_places": c.show_in_places,
        "user": c.user, "login_host": c.login_host, "login_saved": saved && !c.user.is_empty(),
    });
    qs(json.to_string())
}

/// What the background work needs from the web profile (read on the Qt
/// thread): the live cookies for `url`, the profile folder, the user agent.
fn web_session(url: &str) -> (String, PathBuf, String) {
    (
        crate::qt_helpers::web_cookie_header(&qs(url)).to_string(),
        PathBuf::from(crate::qt_helpers::web_profile_path().to_string()),
        crate::qt_helpers::web_user_agent().to_string(),
    )
}

impl qobject::IliasBackend {
    pub fn set_config(mut self: Pin<&mut Self>, key: &QString, value: &QString) {
        let mut config = Config::load().ilias;
        let value = value.to_string();
        match key.to_string().as_str() {
            "base_url" => {
                let url = value.trim();
                config.base_url = if url.ends_with('/') { url.to_owned() } else { format!("{url}/") };
            }
            "download_dir" => config.download_dir = value,
            "show_in_places" => config.show_in_places = value == "true",
            "login_host" => config.login_host = value.trim().to_lowercase(),
            other => {
                log::warn!("unknown ILIAS setting {other}");
                return;
            }
        }
        if let Err(err) = Config::save_ilias(&config) {
            log::warn!("cannot save [ilias]: {err:#}");
        }
        self.as_mut().set_config_json(config_json(&config));
    }

    pub fn reload_config(self: Pin<&mut Self>) {
        self.set_config_json(config_json(&Config::load().ilias));
    }

    fn set_config_json(mut self: Pin<&mut Self>, json: QString) {
        self.as_mut().rust_mut().config = json;
        self.config_changed();
    }

    pub fn load(&self, name: &QString) -> QString {
        qs(ilias::load(&name.to_string()))
    }

    /// # Safety
    /// `profile` must point to a live QObject (QML passes the profile).
    pub unsafe fn use_profile(&self, profile: *mut cxx_qt::QObject) -> bool {
        // SAFETY: `profile` comes from QML and outlives this call.
        unsafe { crate::qt_helpers::watch_web_cookies(profile) }
    }

    pub fn scan(&self) {
        let base = Config::load().ilias.base_url;
        let (live, profile, agent) = web_session(&base);
        ilias::trace_start("Einlesen");
        ilias::trace(&format!("Web-Profil (seit dem Start gemeldet): {}", crate::qt_helpers::web_cookie_summary()));
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let progress = |p: ilias::Progress| {
                let text = if p.pages == 0 {
                    format!("Lese {} …", p.text)
                } else {
                    format!("Lese … {} Seiten ({})", p.pages, p.text)
                };
                let _ = thread.queue(move |b| b.progress(qs(text)));
            };
            let cookies = ilias::Url::parse(&base).map(|u| ilias::session_cookies(&profile, &u, &live)).unwrap_or(live);
            let result = ilias::Client::new(&base, &cookies, &agent)
                .and_then(|client| ilias::scan(&client, &progress))
                .and_then(|courses| {
                    let scanned = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    let index = serde_json::json!({ "base_url": base, "scanned": scanned, "courses": courses });
                    ilias::save("index", &index.to_string()).map_err(|e| IliasError::Other(format!("{e:#}")))?;
                    Ok(courses.len())
                });
            let (status, message) = match result {
                Ok(n) => ("ok", n.to_string()),
                Err(IliasError::Login) => ("login", String::new()),
                Err(IliasError::Other(e)) => ("error", e),
            };
            let _ = thread.queue(move |b| b.scanned(qs(status), qs(message)));
        });
    }

    pub fn download(&self, url: &QString, download: &QString, title: &QString, info: &QString, dir: &QString) {
        let config = Config::load().ilias;
        let address = if download.is_empty() { url.to_string() } else { download.to_string() };
        let (live, profile, agent) = web_session(&address);
        let (url, title, info, dir) = (url.to_string(), title.to_string(), info.to_string(), dir.to_string());
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let last = std::sync::Mutex::new(std::time::Instant::now());
            let progress = |received: u64, total: Option<u64>| {
                let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
                if last.elapsed() < std::time::Duration::from_millis(100) {
                    return;
                }
                *last = std::time::Instant::now();
                let url = qs(&url);
                let total = total.unwrap_or(0) as f64;
                let _ = thread.queue(move |b| b.download_progress(url, received as f64, total));
            };
            let result = (|| {
                let other = |e: anyhow::Error| IliasError::Other(format!("{e:#}"));
                let dir = ilias::target_dir(Path::new(&dir), &[]).map_err(other)?;
                let cookies =
                    ilias::Url::parse(&address).map(|u| ilias::session_cookies(&profile, &u, &live)).unwrap_or(live);
                let client = ilias::Client::new(&config.base_url, &cookies, &agent)?;
                let previous = ilias::previous_download(&url);
                let path = ilias::download(&client, &address, &dir, &title, previous.as_deref(), &progress)?;
                ilias::record_download(&url, &path, &info).map_err(other)?;
                Ok(path)
            })();
            let (path, status, message) = match result {
                Ok(path) => (path.to_string_lossy().into_owned(), "ok", String::new()),
                Err(IliasError::Login) => (String::new(), "login", String::new()),
                Err(IliasError::Other(e)) => (String::new(), "error", e),
            };
            let _ = thread.queue(move |b| b.downloaded(qs(url), qs(path), qs(status), qs(message)));
        });
    }

    pub fn trace(&self, line: &QString) {
        ilias::trace(&line.to_string());
    }

    pub fn debug_log(&self) -> QString {
        let path = ilias::debug_log();
        if !path.exists() {
            ilias::trace("(noch nichts protokolliert)");
        }
        qs(path.to_string_lossy())
    }

    pub fn remember_login(&self, user: &QString, password: &QString) {
        let (user, password) = (user.to_string().trim().to_owned(), password.to_string());
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<()> {
                if user.is_empty() {
                    anyhow::bail!("Benutzername fehlt");
                }
                secret().store_encrypted(&password)?;
                let mut config = Config::load().ilias;
                config.user = user;
                Config::save_ilias(&config)
            })();
            let (ok, message) = match result {
                Ok(()) => (true, "Anmeldung verschlüsselt gespeichert (systemd-creds)".to_owned()),
                Err(err) => (false, format!("{err:#}")),
            };
            ilias::trace(&format!("Anmeldedaten merken: {message}"));
            let _ = thread.queue(move |mut b| {
                b.as_mut().reload_config();
                b.login_stored(ok, qs(message));
            });
        });
    }

    pub fn forget_login(self: Pin<&mut Self>) {
        if let Err(err) = secret().forget() {
            log::warn!("cannot forget the ILIAS login: {err:#}");
        }
        let mut config = Config::load().ilias;
        config.login_host.clear();
        if let Err(err) = Config::save_ilias(&config) {
            log::warn!("cannot save [ilias]: {err:#}");
        }
        ilias::trace("Anmeldedaten vergessen");
        self.reload_config();
    }

    pub fn fetch_login(&self) {
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let user = Config::load().ilias.user;
            let (password, error) = match secret().load() {
                Ok(Some((p, Kind::Encrypted))) => (p, String::new()),
                Ok(Some(_)) => (String::new(), "nicht verschlüsselt gespeichert".to_owned()),
                Ok(None) => (String::new(), String::new()),
                Err(err) => (String::new(), format!("{err:#}")),
            };
            ilias::trace(&if !error.is_empty() {
                format!("Anmeldedaten nicht lesbar: {error}")
            } else if password.is_empty() {
                "Anmeldedaten: kein gespeichertes Passwort".to_owned()
            } else {
                format!("Anmeldedaten entschlüsselt (Benutzer „{user}“)")
            });
            let _ = thread.queue(move |b| b.login_ready(qs(user), qs(password), qs(error)));
        });
    }

    pub fn logout(&self) {
        ilias::trace("Abmelden: alle Cookies des Web-Profils gelöscht");
        crate::qt_helpers::clear_web_cookies();
    }
}
