// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! ILIAS: settings, remembered login, favourites as folders, downloads.
//! The login itself happens in a web view of the UI (WKWebView on macOS);
//! the UI hands its session cookies to the core, which does everything
//! else (crate `atlas-ilias`). Template: `ilias_backend.rs`,
//! `IliasService.qml`.

use crate::{Result, todo_task};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct IliasConfig {
    pub base_url: String,
    pub download_dir: String,
    pub show_in_places: bool,
    pub user: String,
    /// Host of the university login page allowed in the login window.
    pub login_host: String,
    pub login_saved: bool,
}

/// A cookie of the login web view.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    /// Unix seconds, `None` for session cookies.
    pub expires: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct IliasItem {
    pub title: String,
    /// `crs`, `grp`, `fold`, `file`, `webr`, …
    pub kind: String,
    /// ILIAS address of the item.
    pub url: String,
    /// Address sending the file (files only).
    pub download: String,
    pub info: String,
    pub children: Vec<IliasItem>,
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
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Ilias {}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Ilias {
    pub fn config(&self) -> IliasConfig {
        todo_task!("P2.10")
    }

    pub fn save_config(&self, config: IliasConfig) -> Result<()> {
        let _ = config;
        todo_task!("P2.10")
    }

    /// Replaces the session cookies used for reading and downloading.
    pub fn set_cookies(&self, cookies: Vec<Cookie>) {
        let _ = cookies;
        todo_task!("P2.10")
    }

    /// The favourites with everything in them (also saved as the last
    /// index). Fails with `AtlasError::Auth` when the login expired.
    pub async fn scan(&self) -> Result<Vec<IliasItem>> {
        todo_task!("P2.10")
    }

    /// The last scan result, without network.
    pub fn last_index(&self) -> Vec<IliasItem> {
        todo_task!("P2.10")
    }

    /// Downloads into `dir`, named as the server says or `title`; a new
    /// version replaces its earlier download there. Returns the path.
    pub async fn download(
        &self,
        url: String,
        download: String,
        title: String,
        info: String,
        dir: String,
    ) -> Result<String> {
        let _ = (url, download, title, info, dir);
        todo_task!("P2.10")
    }

    pub fn downloads(&self) -> Vec<IliasDownload> {
        todo_task!("P2.10")
    }

    /// Remembers the login (password only protected: keychain / systemd-creds).
    pub fn remember_login(&self, login: IliasLogin) -> Result<()> {
        let _ = login;
        todo_task!("P2.10")
    }

    pub fn remembered_login(&self) -> Result<Option<IliasLogin>> {
        todo_task!("P2.10")
    }

    pub fn forget_login(&self) -> Result<()> {
        todo_task!("P2.10")
    }

    pub fn trace(&self, line: String) {
        let _ = line;
        todo_task!("P2.10")
    }

    /// Path of the ILIAS debug log.
    pub fn debug_log(&self) -> String {
        todo_task!("P2.10")
    }
}
