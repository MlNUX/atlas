// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Linux (freedesktop.org conventions).

use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("atlas")
}

/// `$XDG_DATA_HOME/Trash/files` (freedesktop.org trash specification).
pub fn trash_files_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("Trash/files"))
}

pub fn fixed_places() -> Vec<(&'static str, &'static str, Option<PathBuf>)> {
    vec![
        ("Persönlicher Ordner", "home", dirs::home_dir()),
        ("Schreibtisch", "desktop", dirs::desktop_dir()),
        ("Dokumente", "documents", dirs::document_dir()),
        ("Downloads", "downloads", dirs::download_dir()),
        ("Bilder", "pictures", dirs::picture_dir()),
        ("Musik", "music", dirs::audio_dir()),
        ("Videos", "videos", dirs::video_dir()),
        ("Papierkorb", "trash", trash_files_dir()),
    ]
}

pub fn end_with_parent() {
    // SAFETY: prctl is async-signal-safe.
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
    }
}
