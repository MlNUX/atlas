// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! What differs between Linux and macOS, behind one interface:
//!
//! | | Linux | macOS |
//! |---|---|---|
//! | settings | `$XDG_CONFIG_HOME/atlas` | `~/Library/Application Support/Atlas` |
//! | trash | `$XDG_DATA_HOME/Trash/files` | `~/.Trash` |
//! | fixed places | XDG user folders | home, desktop, documents, … (macOS names) |
//! | background jobs end with Atlas | `PR_SET_PDEATHSIG` | only via [`crate::background::stop_all`] |
//! | secrets | `systemd-creds` (see [`crate::secret`]) | keychain ([`keychain`]) |
//!
//! The working directory of another process (terminal) is in
//! `atlas-terminal`, which does not depend on this crate.

use std::path::PathBuf;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as imp;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;
#[cfg(target_os = "macos")]
pub use macos::keychain;

/// The folder with `config.toml`, actions and extension settings
/// (without `ATLAS_CONFIG_DIR`, see [`crate::config::config_dir`]).
pub fn config_dir() -> PathBuf {
    imp::config_dir()
}

/// The folder whose entries are the user's trash.
pub fn trash_files_dir() -> Option<PathBuf> {
    imp::trash_files_dir()
}

/// Display name, icon name and folder of the fixed places, in order (also
/// those that do not exist; the caller filters).
pub fn fixed_places() -> Vec<(&'static str, &'static str, Option<PathBuf>)> {
    imp::fixed_places()
}

/// For `Command::pre_exec`: lets the child end when Atlas ends (crash
/// included) where the system supports it; a no-op elsewhere.
///
/// Must stay async-signal-safe (runs between fork and exec).
pub fn end_with_parent() {
    imp::end_with_parent()
}

/// Whether [`end_with_parent`] does anything here.
pub const ENDS_WITH_PARENT: bool = cfg!(target_os = "linux");
