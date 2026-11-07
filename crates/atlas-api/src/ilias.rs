// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! ILIAS: settings, remembered login, favourites as folders, downloads.
//! The login itself happens in a web view of the UI (WKWebView on macOS);
//! the UI hands its session cookies to the core, which does everything
//! else (crate `atlas-ilias`). Template: `ilias_backend.rs`,
//! `IliasService.qml`.

use std::path::Path;
use std::sync::Mutex;

use atlas_core::config::Config;
use atlas_core::secret::{Kind, SecretStore};
use atlas_ilias::{self as ilias, IliasError};

use crate::{AtlasError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct IliasConfig {
    pub base_url: String,
    pub download_dir: String,
    pub show_in_places: bool,
    pub user: String,
    /// Host of the university login page allowed in the login window.
    pub login_host: String,
    /// A password is remembered (protected: keychain / systemd-creds).
    pub login_saved: bool,
}

/// A cookie of the login web view.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Cookie {
    pub name: String,
    pub value: String,
    /// Host the cookie belongs to (`ilias.studium.kit.edu`, `.kit.edu`).
    pub domain: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct IliasItem {
    /// `crs`, `grp`, `sess`, `fold`, `file`, …
    pub kind: String,
    pub id: String,
    pub title: String,
    /// ILIAS address of the item.
    pub url: String,
    /// Address sending the file (files only).
    pub download: Option<String>,
    /// Type, size, version, date as ILIAS shows them (a change = new version).
    pub info: String,
    /// Containers: what is inside (empty for files).
    pub children: Vec<IliasItem>,
    /// Containers that could not be read.
    pub error: String,
}

/// The last scan.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct IliasIndex {
    pub base_url: String,
    /// Unix milliseconds of the scan.
    pub scanned: u64,
    pub courses: Vec<IliasItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct IliasLogin {
    pub user: String,
    pub password: String,
}

/// A file downloaded before (to find new versions).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct IliasDownload {
    pub url: String,
    pub path: String,
    pub info: String,
    /// Unix milliseconds.
    pub time: u64,
}

/// The web view's session: cookies for the ILIAS host and its user agent.
#[derive(Default)]
struct Session {
    cookies: Vec<Cookie>,
    user_agent: String,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Ilias {
    session: Mutex<Session>,
}

impl Ilias {
    pub(crate) fn new() -> Self {
        Self { session: Mutex::default() }
    }

    /// `Cookie` header for `address` (cookies whose domain matches its host).
    fn cookie_header(&self, address: &str) -> (String, String) {
        let session = self.session.lock().unwrap();
        let host =
            ilias::Url::parse(address).ok().and_then(|u| u.host_str().map(str::to_lowercase)).unwrap_or_default();
        let header = session
            .cookies
            .iter()
            .filter(|c| {
                let domain = c.domain.trim_start_matches('.').to_lowercase();
                domain.is_empty() || host == domain || host.ends_with(&format!(".{domain}"))
            })
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ");
        (header, session.user_agent.clone())
    }
}

fn secret() -> SecretStore {
    SecretStore::default_for("ilias")
}

fn item(n: ilias::Node) -> IliasItem {
    IliasItem {
        kind: n.kind,
        id: n.id,
        title: n.name,
        url: n.url,
        download: n.download,
        info: n.info,
        children: n.children.unwrap_or_default().into_iter().map(item).collect(),
        error: n.error,
    }
}

fn error(err: IliasError) -> AtlasError {
    match err {
        IliasError::Login => AtlasError::Auth { message: "Anmeldung bei ILIAS nötig".into() },
        IliasError::Other(message) => AtlasError::Failed { message },
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Ilias {
    pub fn config(&self) -> IliasConfig {
        let c = Config::load().ilias;
        IliasConfig {
            base_url: c.base_url,
            download_dir: c.download_dir,
            show_in_places: c.show_in_places,
            user: c.user,
            login_host: c.login_host,
            login_saved: secret().kind() == Some(Kind::Encrypted),
        }
    }

    /// Saves `[ilias]` (the base URL gets a trailing `/`, the login host is
    /// lower case); `login_saved` is ignored.
    pub fn save_config(&self, config: IliasConfig) -> Result<()> {
        let url = config.base_url.trim();
        if ilias::Url::parse(url).is_err() {
            return Err(AtlasError::Invalid { message: format!("„{url}“ ist keine Adresse") });
        }
        let c = atlas_core::config::IliasConfig {
            base_url: if url.ends_with('/') { url.to_owned() } else { format!("{url}/") },
            download_dir: config.download_dir,
            show_in_places: config.show_in_places,
            user: config.user.trim().to_owned(),
            login_host: config.login_host.trim().to_lowercase(),
        };
        Ok(Config::save_ilias(&c)?)
    }

    /// Replaces the web view's session (all its cookies and its user agent)
    /// used for reading and downloading.
    pub fn set_session(&self, cookies: Vec<Cookie>, user_agent: String) {
        ilias::trace(&format!("Web-Sitzung: {} Cookies", cookies.len()));
        *self.session.lock().unwrap() = Session { cookies, user_agent };
    }

    /// The favourites with everything in them, saved as the last index.
    /// Fails with `AtlasError::Auth` when the login expired.
    pub async fn scan(&self) -> Result<IliasIndex> {
        let base = Config::load().ilias.base_url;
        let (cookies, agent) = self.cookie_header(&base);
        ilias::trace_start("Einlesen");
        crate::util::background(move || {
            let client = ilias::Client::new(&base, &cookies, &agent).map_err(error)?;
            let courses = ilias::scan(&client, &|_| {}).map_err(error)?;
            let scanned = now_ms();
            let index = serde_json::json!({ "base_url": base, "scanned": scanned, "courses": courses });
            ilias::save("index", &index.to_string())?;
            Ok(IliasIndex { base_url: base, scanned, courses: courses.into_iter().map(item).collect() })
        })
        .await
    }

    /// The last scan result, without network.
    pub fn last_index(&self) -> Option<IliasIndex> {
        #[derive(serde::Deserialize)]
        struct Stored {
            base_url: String,
            scanned: u64,
            courses: Vec<ilias::Node>,
        }
        let stored: Stored = serde_json::from_str(&ilias::load("index")).ok()?;
        Some(IliasIndex {
            base_url: stored.base_url,
            scanned: stored.scanned,
            courses: stored.courses.into_iter().map(item).collect(),
        })
    }

    /// Downloads file `url` (via `download`, the address sending it) into
    /// `dir`, named as the server says or `title`; a new version replaces
    /// its earlier download there. Returns the path.
    pub async fn download(
        &self,
        url: String,
        download: Option<String>,
        title: String,
        info: String,
        dir: String,
    ) -> Result<String> {
        let address = download.unwrap_or_else(|| url.clone());
        let (cookies, agent) = self.cookie_header(&address);
        let base = Config::load().ilias.base_url;
        crate::util::background(move || {
            let dir = ilias::target_dir(Path::new(&dir), &[])?;
            let client = ilias::Client::new(&base, &cookies, &agent).map_err(error)?;
            let previous = ilias::previous_download(&url);
            let path =
                ilias::download(&client, &address, &dir, &title, previous.as_deref(), &|_, _| {}).map_err(error)?;
            ilias::record_download(&url, &path, &info)?;
            Ok(crate::util::string(&path))
        })
        .await
    }

    pub fn downloads(&self) -> Vec<IliasDownload> {
        let all: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&ilias::load("downloads")).unwrap_or_default();
        all.into_iter()
            .map(|(url, v)| IliasDownload {
                url,
                path: v["path"].as_str().unwrap_or_default().to_owned(),
                info: v["info"].as_str().unwrap_or_default().to_owned(),
                time: v["time"].as_u64().unwrap_or(0),
            })
            .collect()
    }

    /// Remembers the login: user name in `[ilias]`, the password only
    /// protected (keychain / systemd-creds), otherwise nothing.
    pub fn remember_login(&self, login: IliasLogin) -> Result<()> {
        let user = login.user.trim().to_owned();
        if user.is_empty() {
            return Err(AtlasError::Invalid { message: "Benutzername fehlt".into() });
        }
        secret().store_encrypted(&login.password)?;
        let mut c = Config::load().ilias;
        c.user = user;
        Config::save_ilias(&c)?;
        ilias::trace("Anmeldedaten geschützt gespeichert");
        Ok(())
    }

    /// The remembered login, `None` if there is none.
    pub fn remembered_login(&self) -> Result<Option<IliasLogin>> {
        let user = Config::load().ilias.user;
        match secret().load()? {
            Some((password, Kind::Encrypted)) => Ok(Some(IliasLogin { user, password })),
            Some(_) => Err(AtlasError::Auth { message: "Passwort nicht geschützt gespeichert".into() }),
            None => Ok(None),
        }
    }

    /// Forgets the password and the allowed login page.
    pub fn forget_login(&self) -> Result<()> {
        secret().forget()?;
        let mut c = Config::load().ilias;
        c.login_host.clear();
        Config::save_ilias(&c)?;
        ilias::trace("Anmeldedaten vergessen");
        Ok(())
    }

    /// Forgets the web session (the UI also clears its web view's cookies).
    pub fn logout(&self) {
        ilias::trace("Abmelden");
        *self.session.lock().unwrap() = Session::default();
    }

    pub fn trace(&self, line: String) {
        ilias::trace(&line);
    }

    /// Path of the ILIAS debug log.
    pub fn debug_log(&self) -> String {
        let path = ilias::debug_log();
        if !path.exists() {
            ilias::trace("(noch nichts protokolliert)");
        }
        crate::util::string(&path)
    }
}
