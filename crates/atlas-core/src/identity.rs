// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! File identity.
//!
//! Files are tracked primarily by path plus device ID and inode. A light
//! fingerprint (size + hash of the first 64 KiB) helps to re-find a file that
//! was moved or renamed outside of Atlas, when the inode changed as well
//! (e.g. editors that save via "write new file, rename over old").

use std::fs::File;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const FINGERPRINT_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileId {
    pub path: PathBuf,
    pub dev: u64,
    pub ino: u64,
    pub fingerprint: Option<Fingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub size: u64,
    /// BLAKE3 of the first 64 KiB, hex encoded.
    pub head: String,
}

/// How well a stored [`FileId`] matches the file system right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Same path, same inode.
    Unchanged,
    /// Same path, but the inode changed (file replaced in place).
    Replaced,
    /// Found at a different path (same inode or same fingerprint).
    Moved(PathBuf),
    /// Not found.
    Missing,
}

impl FileId {
    /// Identity of an existing file. Regular files get a fingerprint,
    /// directories do not.
    pub fn of(path: &Path) -> std::io::Result<Self> {
        let meta = std::fs::metadata(path)?;
        let fingerprint = if meta.is_file() { Fingerprint::of(path).ok() } else { None };
        Ok(Self { path: path.to_path_buf(), dev: meta.dev(), ino: meta.ino(), fingerprint })
    }

    /// Resolves this identity against the file system. `candidates` are
    /// paths to consider when the file is no longer at its old location
    /// (typically the entries of the project tree).
    pub fn resolve<'a>(&self, candidates: impl IntoIterator<Item = &'a Path>) -> Resolution {
        if let Ok(meta) = std::fs::metadata(&self.path) {
            return if meta.dev() == self.dev && meta.ino() == self.ino {
                Resolution::Unchanged
            } else {
                Resolution::Replaced
            };
        }
        let mut by_fingerprint = None;
        for candidate in candidates {
            let Ok(meta) = std::fs::metadata(candidate) else { continue };
            if meta.dev() == self.dev && meta.ino() == self.ino {
                return Resolution::Moved(candidate.to_path_buf());
            }
            if by_fingerprint.is_none()
                && let Some(fp) = &self.fingerprint
                && meta.is_file()
                && meta.len() == fp.size
                && Fingerprint::of(candidate).is_ok_and(|c| &c == fp)
            {
                by_fingerprint = Some(candidate.to_path_buf());
            }
        }
        by_fingerprint.map_or(Resolution::Missing, Resolution::Moved)
    }
}

impl Fingerprint {
    pub fn of(path: &Path) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let size = file.metadata()?.len();
        let mut head = Vec::with_capacity(FINGERPRINT_BYTES.min(size) as usize);
        file.take(FINGERPRINT_BYTES).read_to_end(&mut head)?;
        Ok(Self { size, head: blake3::hash(&head).to_hex().to_string() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rename_by_inode() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, "hello").unwrap();
        let id = FileId::of(&a).unwrap();
        assert_eq!(id.resolve([]), Resolution::Unchanged);
        std::fs::rename(&a, &b).unwrap();
        assert_eq!(id.resolve([b.as_path()]), Resolution::Moved(b.clone()));
    }

    #[test]
    fn detects_copy_by_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, "same content").unwrap();
        let id = FileId::of(&a).unwrap();
        std::fs::write(&b, "same content").unwrap();
        std::fs::remove_file(&a).unwrap();
        assert_eq!(id.resolve([b.as_path()]), Resolution::Moved(b.clone()));
    }

    #[test]
    fn detects_replace_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let tmp = dir.path().join("a.txt.new");
        std::fs::write(&a, "v1").unwrap();
        let id = FileId::of(&a).unwrap();
        std::fs::write(&tmp, "v2").unwrap();
        std::fs::rename(&tmp, &a).unwrap();
        assert_eq!(id.resolve([]), Resolution::Replaced);
    }
}
