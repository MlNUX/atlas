// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Extensions: list, trust, enable, settings (secrets in the keychain /
//! systemd-creds), bundled ones, views (web pages in a `WebView` talking to
//! the core through [`Extensions::call`]). Template: `app/extensions.rs`.
//!
//! Requests from a view's page and their results stay JSON: they are
//! defined by the extension, not by Atlas (`docs/extensions.md`).

use crate::{Result, todo_task};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ExtensionLocation {
    User,
    Project,
    Bundled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ExtensionStatus {
    Active,
    Disabled,
    /// A project extension not trusted in its current form.
    Untrusted,
    /// The manifest has errors.
    Broken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SettingKind {
    Text,
    Secret,
    Bool,
    Number,
    Choice,
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ExtensionSetting {
    pub key: String,
    pub title: String,
    pub description: String,
    pub kind: SettingKind,
    /// Allowed values of `Choice`.
    pub choices: Vec<String>,
    /// Current value as TOML text (`"abc"`, `true`, `3`); secrets: `set` or empty.
    pub value: String,
    pub default_value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ExtensionInfo {
    pub name: String,
    pub title: String,
    pub version: String,
    pub description: String,
    pub dir: String,
    pub manifest: String,
    pub location: ExtensionLocation,
    pub status: ExtensionStatus,
    pub errors: Vec<String>,
    /// Ids of its actions, processors, exporters (for the details).
    pub actions: Vec<String>,
    pub processors: Vec<String>,
    pub exporters: Vec<String>,
    pub settings: Vec<ExtensionSetting>,
    /// Requested permissions, as shown in the trust dialog.
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct BundledExtension {
    pub name: String,
    pub title: String,
    pub description: String,
    pub installed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ViewInfo {
    /// `view:<extension>/<id>`
    pub id: String,
    pub name: String,
    pub extension: String,
    pub keybinding: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ViewPage {
    /// `file://` URL of the page.
    pub url: String,
    pub name: String,
    pub extension: String,
    /// The page may load from the network.
    pub network: bool,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Extensions {}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Extensions {
    pub fn list(&self) -> Vec<ExtensionInfo> {
        todo_task!("P2.7")
    }

    /// Project extensions waiting for trust.
    pub fn untrusted(&self) -> Vec<String> {
        todo_task!("P2.7")
    }

    pub fn trust(&self, name: String) -> Result<()> {
        let _ = name;
        todo_task!("P2.7")
    }

    pub fn set_enabled(&self, name: String, enabled: bool) -> Result<()> {
        let _ = (name, enabled);
        todo_task!("P2.7")
    }

    /// `value` as TOML text; `None` resets it. Secrets go to the secret store.
    pub fn set_setting(&self, name: String, key: String, value: Option<String>) -> Result<()> {
        let _ = (name, key, value);
        todo_task!("P2.7")
    }

    /// New user extension from the template; returns the manifest path.
    pub fn create(&self, title: String) -> Result<String> {
        let _ = title;
        todo_task!("P2.7")
    }

    pub fn bundled(&self) -> Vec<BundledExtension> {
        todo_task!("P2.7")
    }

    pub fn install_bundled(&self, name: String) -> Result<()> {
        let _ = name;
        todo_task!("P2.7")
    }

    /// The folder goes to the trash; settings, secrets and trust are forgotten.
    pub fn uninstall(&self, dir: String) -> Result<()> {
        let _ = dir;
        todo_task!("P2.7")
    }

    pub fn views(&self) -> Vec<ViewInfo> {
        todo_task!("P2.7")
    }

    pub fn view(&self, id: String) -> Result<ViewPage> {
        let _ = id;
        todo_task!("P2.7")
    }

    /// A request of a view's page: `params` and the result are JSON.
    pub async fn call(&self, extension: String, method: String, params: String) -> Result<String> {
        let _ = (extension, method, params);
        todo_task!("P2.7")
    }

    pub fn reload(&self) {
        todo_task!("P2.7")
    }
}
