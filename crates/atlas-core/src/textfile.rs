// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Loading and saving edited text files, and recovery copies.
//!
//! Saving is atomic (temporary file in the same folder, fsync, rename,
//! fsync of the folder), keeps the permissions of the file and writes
//! through symbolic links to their target. It refuses to save if
//!
//! - the file is read-only (never silently overwritten), or
//! - the file on disk is no longer the version the edit started from
//!   (an external change) – the caller offers reload / keep / save as.
//!
//! While a file is being edited, a recovery copy is kept in
//! `.atlas/recovery/` (or Atlas' state folder for files outside a project);
//! it is removed after a successful save.

use std::ffi::CString;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Files above this size are opened read-only.
pub const MAX_EDIT_SIZE: u64 = 8 * 1024 * 1024;
/// Files above this size are not opened as text at all.
pub const MAX_OPEN_SIZE: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Loaded {
    pub text: String,
    /// BLAKE3 of the bytes on disk.
    pub hash: String,
    /// Why the file cannot be edited ("" if it can).
    pub read_only: String,
    /// The file was not valid UTF-8 (shown lossily, never saved back).
    pub lossy: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("{0}")]
    ReadOnly(String),
    /// The file changed (or vanished) on disk since it was loaded.
    #[error("die Datei wurde außerhalb von Atlas geändert")]
    Conflict { deleted: bool },
    #[error(transparent)]
    Io(#[from] anyhow::Error),
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// Hash of the file as it is on disk now, `None` if it cannot be read.
pub fn disk_hash(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|b| hash_bytes(&b))
}

fn access(path: &Path, mode: libc::c_int) -> bool {
    let Ok(c) = CString::new(path.as_os_str().as_bytes()) else { return false };
    // SAFETY: valid NUL-terminated path.
    unsafe { libc::access(c.as_ptr(), mode) == 0 }
}

/// Why `path` cannot be saved, or `None` if it can.
pub fn read_only_reason(path: &Path) -> Option<String> {
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if target.exists() && !access(&target, libc::W_OK) {
        return Some("Die Datei ist schreibgeschützt.".into());
    }
    let dir = target.parent().unwrap_or(Path::new("/"));
    if !access(dir, libc::W_OK) {
        return Some("Der Ordner ist schreibgeschützt.".into());
    }
    None
}

pub fn load(path: &Path) -> Result<Loaded> {
    let meta = std::fs::metadata(path).with_context(|| format!("cannot read {}", path.display()))?;
    if meta.len() > MAX_OPEN_SIZE {
        anyhow::bail!("{} is too large to open as text ({} bytes)", path.display(), meta.len());
    }
    let too_large = meta.len() > MAX_EDIT_SIZE;
    let bytes = std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    let hash = hash_bytes(&bytes);
    let (text, lossy) = match String::from_utf8(bytes) {
        Ok(text) => (text, false),
        Err(err) => (String::from_utf8_lossy(err.as_bytes()).into_owned(), true),
    };
    let read_only = if lossy {
        "Die Datei ist kein gültiges UTF-8 und wird nur angezeigt.".into()
    } else if too_large {
        "Die Datei ist zu groß zum Bearbeiten.".into()
    } else {
        read_only_reason(path).unwrap_or_default()
    };
    Ok(Loaded { text, hash, read_only, lossy })
}

/// Saves `text` to `path` atomically.
///
/// `expected` is the hash of the version the edit is based on (`None` for
/// a new file, which must not exist yet, or to overwrite deliberately with
/// [`save_force`]). Returns the new hash.
pub fn save(path: &Path, text: &str, expected: Option<&str>) -> Result<String, SaveError> {
    let current = disk_hash(path);
    match (expected, &current) {
        (Some(_), None) if !path.exists() => return Err(SaveError::Conflict { deleted: true }),
        (Some(e), Some(c)) if e != c => return Err(SaveError::Conflict { deleted: false }),
        (None, Some(_)) => return Err(SaveError::Conflict { deleted: false }),
        _ => {}
    }
    write(path, text.as_bytes())
}

/// Saves regardless of external changes ("keep my version"), still
/// refusing read-only files.
pub fn save_force(path: &Path, text: &str) -> Result<String, SaveError> {
    write(path, text.as_bytes())
}

fn write(path: &Path, bytes: &[u8]) -> Result<String, SaveError> {
    if let Some(reason) = read_only_reason(path) {
        return Err(SaveError::ReadOnly(reason));
    }
    // Write through symbolic links: the link stays, its target changes.
    let target = if path.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false) {
        std::fs::canonicalize(path).map_err(anyhow::Error::from)?
    } else {
        path.to_path_buf()
    };
    let dir = target.parent().context("file without parent directory")?;
    let name = target.file_name().context("file without name")?.to_string_lossy();
    let tmp = dir.join(format!(".{name}.atlas-{}.tmp", std::process::id()));
    let old = std::fs::metadata(&target).ok();

    let result = (|| -> Result<()> {
        let mut file = std::fs::File::create(&tmp).with_context(|| format!("cannot create {}", tmp.display()))?;
        if let Some(old) = &old {
            file.set_permissions(std::fs::Permissions::from_mode(old.mode() & 0o7777))?;
            // Keep the owner/group where allowed (e.g. group-owned shared
            // folders); without the right to, the file becomes ours.
            let _ = std::os::unix::fs::fchown(&file, Some(old.uid()), Some(old.gid()));
        }
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, &target)?;
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
        Ok(())
    })();
    if let Err(err) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(SaveError::Io(err));
    }
    Ok(hash_bytes(bytes))
}

/// Unsaved state of an edited file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Recovery {
    pub version: u32,
    /// Absolute path of the edited file.
    pub path: PathBuf,
    /// Hash of the version on disk the edit started from.
    pub base_hash: String,
    /// Seconds since the epoch.
    pub saved_at: u64,
    pub text: String,
}

/// Recovery copies of one project (or of files outside projects).
#[derive(Debug, Clone)]
pub struct RecoveryStore {
    dir: PathBuf,
}

impl RecoveryStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn file_for(&self, path: &Path) -> PathBuf {
        let key = blake3::hash(path.as_os_str().as_bytes()).to_hex();
        self.dir.join(format!("{}.json", &key[..24]))
    }

    pub fn write(&self, path: &Path, base_hash: &str, text: &str) -> Result<()> {
        let entry = Recovery {
            version: 1,
            path: path.to_path_buf(),
            base_hash: base_hash.to_owned(),
            saved_at: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
            text: text.to_owned(),
        };
        crate::paths::write_atomic(&self.file_for(path), &serde_json::to_vec(&entry)?)
    }

    pub fn read(&self, path: &Path) -> Option<Recovery> {
        let bytes = std::fs::read(self.file_for(path)).ok()?;
        serde_json::from_slice::<Recovery>(&bytes).ok().filter(|r| r.path == path)
    }

    pub fn clear(&self, path: &Path) {
        let _ = std::fs::remove_file(self.file_for(path));
    }

    /// All recovery copies, newest first.
    pub fn list(&self) -> Vec<Recovery> {
        let mut out: Vec<Recovery> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| std::fs::read(e.path()).ok())
            .filter_map(|b| serde_json::from_slice(&b).ok())
            .collect();
        out.sort_by_key(|r: &Recovery| std::cmp::Reverse(r.saved_at));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_atomically_and_detects_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("notiz.md");
        std::fs::write(&file, "eins").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o640)).unwrap();

        let loaded = load(&file).unwrap();
        assert_eq!(loaded.text, "eins");
        assert!(loaded.read_only.is_empty());
        let h2 = save(&file, "zwei", Some(&loaded.hash)).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "zwei");
        assert_eq!(std::fs::metadata(&file).unwrap().mode() & 0o777, 0o640, "permissions kept");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1, "no temporary files left");

        // External change: saving on the old base is refused.
        std::fs::write(&file, "extern").unwrap();
        assert!(matches!(save(&file, "drei", Some(&h2)), Err(SaveError::Conflict { deleted: false })));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "extern");
        save_force(&file, "drei").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "drei");

        // Deleted externally.
        let h = disk_hash(&file).unwrap();
        std::fs::remove_file(&file).unwrap();
        assert!(matches!(save(&file, "x", Some(&h)), Err(SaveError::Conflict { deleted: true })));

        // New file must not exist yet.
        save(&file, "neu", None).unwrap();
        assert!(matches!(save(&file, "neu", None), Err(SaveError::Conflict { .. })));
    }

    #[test]
    fn writes_through_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.txt");
        let link = dir.path().join("link.txt");
        std::fs::write(&real, "a").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let loaded = load(&link).unwrap();
        save(&link, "b", Some(&loaded.hash)).unwrap();
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&real).unwrap(), "b");
    }

    #[test]
    fn never_overwrites_read_only_files() {
        if unsafe { libc::geteuid() } == 0 {
            return; // root may write anything
        }
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ro.txt");
        std::fs::write(&file, "fest").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444)).unwrap();
        let loaded = load(&file).unwrap();
        assert!(!loaded.read_only.is_empty());
        assert!(matches!(save(&file, "neu", Some(&loaded.hash)), Err(SaveError::ReadOnly(_))));
        assert!(matches!(save_force(&file, "neu"), Err(SaveError::ReadOnly(_))));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "fest");
    }

    #[test]
    fn non_utf8_is_read_only() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("latin1.txt");
        std::fs::write(&file, b"Gr\xfc\xdfe").unwrap();
        let loaded = load(&file).unwrap();
        assert!(loaded.lossy);
        assert!(!loaded.read_only.is_empty());
    }

    #[test]
    fn recovery_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = RecoveryStore::new(dir.path().join(".atlas/recovery"));
        let file = dir.path().join("a.md");
        assert!(store.read(&file).is_none());
        store.write(&file, "h", "ungespeichert").unwrap();
        let r = store.read(&file).unwrap();
        assert_eq!((r.text.as_str(), r.base_hash.as_str()), ("ungespeichert", "h"));
        assert_eq!(store.list().len(), 1);
        store.clear(&file);
        assert!(store.read(&file).is_none());
    }
}
