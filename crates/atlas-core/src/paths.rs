// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Device local state and safe file writing.
//!
//! Project data lives in the project's `.atlas/` folder (see
//! [`crate::project`]). Only state that belongs to this device and not to a
//! project stays below the XDG base directories:
//!
//! - `$XDG_STATE_HOME/atlas/app.json` – last opened project
//! - `$XDG_STATE_HOME/atlas/device-id` – stable ID of this device, used to
//!   keep sessions of several machines apart inside `.atlas/sessions/`

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const APP_DIR: &str = "atlas";

/// Root of the device local state (`$XDG_STATE_HOME/atlas`, usually
/// `~/.local/state/atlas`). `ATLAS_STATE_DIR` overrides the location.
pub fn state_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("ATLAS_STATE_DIR") {
        return PathBuf::from(dir);
    }
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| std::env::temp_dir().join("atlas-state"))
        .join(APP_DIR)
}

/// File holding application wide state.
pub fn app_state_file() -> PathBuf {
    state_root().join("app.json")
}

/// Stable identifier of this device: `<hostname>-<8 hex digits>`.
///
/// Generated once and stored in the state directory, so it survives
/// hostname changes and differs between machines that share a project
/// (e.g. via a synced folder).
pub fn device_id() -> String {
    let file = state_root().join("device-id");
    if let Ok(id) = std::fs::read_to_string(&file) {
        let id = id.trim();
        if is_valid_device_id(id) {
            return id.to_owned();
        }
    }
    let host = std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|h| h.trim().to_owned())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "device".to_owned());
    let host: String =
        host.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).take(32).collect();
    let id = format!("{host}-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
    if let Err(err) = write_atomic(&file, id.as_bytes()) {
        log::warn!("cannot store device id: {err:#}");
    }
    id
}

fn is_valid_device_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Writes `contents` to `path` so that readers see either the old or the new
/// file, never a partial one – also across crashes and power loss: the data
/// goes to a temporary file in the same directory, is flushed to disk,
/// renamed over the target, and the directory entry is flushed as well.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path.parent().context("file without parent directory")?;
    std::fs::create_dir_all(parent).with_context(|| format!("cannot create {}", parent.display()))?;
    let tmp =
        parent.join(format!(".{}.tmp-{}", path.file_name().unwrap_or_default().to_string_lossy(), std::process::id()));
    let result = (|| -> Result<()> {
        let mut file = File::create(&tmp).with_context(|| format!("cannot write {}", tmp.display()))?;
        file.write_all(contents)?;
        file.sync_all()?;
        std::fs::rename(&tmp, path).with_context(|| format!("cannot replace {}", path.display()))?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_and_leaves_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sub/state.json");
        write_atomic(&file, b"one").unwrap();
        write_atomic(&file, b"two").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "two");
        let names: Vec<_> =
            std::fs::read_dir(file.parent().unwrap()).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(names, vec![std::ffi::OsString::from("state.json")]);
    }

    #[test]
    fn device_ids_are_sanitised() {
        assert!(is_valid_device_id("nixos-1a2b3c4d"));
        assert!(!is_valid_device_id("../evil"));
        assert!(!is_valid_device_id(""));
    }
}
