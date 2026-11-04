// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! macOS.

use std::path::PathBuf;

/// `~/Library/Application Support/Atlas` (decision E13 of the macOS port).
pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("Atlas")
}

/// `~/.Trash` (the Finder's trash on the home volume).
pub fn trash_files_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".Trash"))
}

/// The Finder's standard folders. Finder's own sidebar favourites are not
/// read: they live in a binary, privacy-protected `.sfl3` archive.
pub fn fixed_places() -> Vec<(&'static str, &'static str, Option<PathBuf>)> {
    let home = dirs::home_dir();
    vec![
        ("Persönlicher Ordner", "home", home.clone()),
        ("Schreibtisch", "desktop", dirs::desktop_dir()),
        ("Dokumente", "documents", dirs::document_dir()),
        ("Downloads", "downloads", dirs::download_dir()),
        ("Programme", "applications", Some(PathBuf::from("/Applications"))),
        ("Bilder", "pictures", dirs::picture_dir()),
        ("Musik", "music", dirs::audio_dir()),
        ("Filme", "videos", dirs::video_dir()),
        ("Papierkorb", "trash", trash_files_dir()),
    ]
}

/// macOS has no `PR_SET_PDEATHSIG`; jobs end through
/// [`crate::background::stop_all`] when Atlas quits normally.
pub fn end_with_parent() {}

/// Generic passwords in the user's login keychain, service `de.minux.atlas`.
pub mod keychain {
    use anyhow::{Context, Result};
    use security_framework::passwords;

    pub const SERVICE: &str = "de.minux.atlas";

    /// errSecItemNotFound
    const NOT_FOUND: i32 = -25300;

    pub fn set(account: &str, secret: &str) -> Result<()> {
        passwords::set_generic_password(SERVICE, account, secret.as_bytes())
            .with_context(|| format!("Schlüsselbund: „{account}“ kann nicht gespeichert werden"))
    }

    /// The secret, `None` if there is none.
    pub fn get(account: &str) -> Result<Option<String>> {
        match passwords::get_generic_password(SERVICE, account) {
            Ok(bytes) => Ok(Some(String::from_utf8(bytes).context("Passwort im Schlüsselbund ist kein UTF-8")?)),
            Err(err) if err.code() == NOT_FOUND => Ok(None),
            Err(err) => Err(err).with_context(|| format!("Schlüsselbund: „{account}“ kann nicht gelesen werden")),
        }
    }

    /// Whether an entry exists (does not read the secret, so no prompt).
    pub fn contains(account: &str) -> bool {
        use security_framework::item::{ItemClass, ItemSearchOptions};
        ItemSearchOptions::new()
            .class(ItemClass::generic_password())
            .service(SERVICE)
            .account(account)
            .search()
            .is_ok_and(|found| !found.is_empty())
    }

    /// Removes the entry; nothing if there is none.
    pub fn delete(account: &str) -> Result<()> {
        match passwords::delete_generic_password(SERVICE, account) {
            Ok(()) => Ok(()),
            Err(err) if err.code() == NOT_FOUND => Ok(()),
            Err(err) => Err(err).with_context(|| format!("Schlüsselbund: „{account}“ kann nicht gelöscht werden")),
        }
    }
}
