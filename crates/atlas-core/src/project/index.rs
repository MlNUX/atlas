// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! `.atlas/index.sqlite`: a rebuildable list of the project's files.
//!
//! The index stores path, device/inode, size and modification time of every
//! file (respecting `.gitignore`), which lets Atlas re-find documents that
//! were moved outside of it. It never holds information that cannot be
//! recomputed from the project: a missing, outdated or corrupted index is
//! simply rebuilt. Rebuilding writes a temporary database and renames it over
//! the old one, so readers never see a half-built index.

use std::fs::File;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

use super::ATLAS_DIR;

const INDEX_VERSION: i64 = 1;

/// State of the index found on disk before [`FileIndex::ensure`] ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Previous {
    Valid,
    Missing,
    Outdated(i64),
    Corrupt(String),
}

#[derive(Debug, Clone)]
pub struct Report {
    pub files: usize,
    pub previous: Previous,
    pub elapsed: Duration,
}

pub struct FileIndex {
    conn: Connection,
}

impl FileIndex {
    /// Checks the index at `path` without modifying it.
    pub fn check(path: &Path) -> Previous {
        if !path.exists() {
            return Previous::Missing;
        }
        let result = (|| -> rusqlite::Result<Previous> {
            let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            let check: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
            if check != "ok" {
                return Ok(Previous::Corrupt(check));
            }
            let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
            if version != INDEX_VERSION {
                return Ok(Previous::Outdated(version));
            }
            conn.query_row("SELECT count(*) FROM files", [], |r| r.get::<_, i64>(0))?;
            Ok(Previous::Valid)
        })();
        result.unwrap_or_else(|err| Previous::Corrupt(err.to_string()))
    }

    /// Makes sure a usable index exists: the project is rescanned into a
    /// fresh database that atomically replaces the old one. Whatever was on
    /// disk before (valid, missing, corrupted) is reported in [`Report`].
    pub fn ensure(root: &Path, path: &Path) -> Result<(Self, Report)> {
        let started = Instant::now();
        let previous = Self::check(path);
        if let Previous::Corrupt(reason) = &previous {
            log::warn!("rebuilding corrupt index {}: {reason}", path.display());
        }
        let files = rebuild(root, path)?;
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Ok((Self { conn }, Report { files, previous, elapsed: started.elapsed() }))
    }

    pub fn len(&self) -> usize {
        self.conn.query_row("SELECT count(*) FROM files", [], |r| r.get::<_, i64>(0)).unwrap_or(0) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Relative path of the file with this device/inode pair.
    pub fn by_inode(&self, dev: u64, ino: u64) -> Option<PathBuf> {
        self.conn
            .query_row("SELECT path FROM files WHERE dev = ?1 AND ino = ?2", params![dev as i64, ino as i64], |r| {
                r.get::<_, String>(0)
            })
            .optional()
            .ok()
            .flatten()
            .map(PathBuf::from)
    }

    /// Device/inode of a relative path as recorded in the index.
    pub fn identity(&self, relative: &Path) -> Option<(u64, u64)> {
        self.conn
            .query_row("SELECT dev, ino FROM files WHERE path = ?1", params![relative.to_string_lossy()], |r| {
                Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)? as u64))
            })
            .optional()
            .ok()
            .flatten()
    }

    /// Relative paths of all files with exactly this size.
    pub fn with_size(&self, size: u64) -> Vec<PathBuf> {
        let Ok(mut stmt) = self.conn.prepare("SELECT path FROM files WHERE size = ?1 AND is_dir = 0") else {
            return Vec::new();
        };
        stmt.query_map(params![size as i64], |r| r.get::<_, String>(0))
            .map(|rows| rows.filter_map(|r| r.ok()).map(PathBuf::from).collect())
            .unwrap_or_default()
    }
}

fn rebuild(root: &Path, path: &Path) -> Result<usize> {
    let dir = path.parent().context("index without parent directory")?;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".index.sqlite.tmp-{}", std::process::id()));
    let _ = std::fs::remove_file(&tmp);

    let count = (|| -> Result<usize> {
        let mut conn = Connection::open(&tmp)?;
        // A temporary file that is renamed into place afterwards needs no
        // journal; durability comes from the fsync before the rename.
        conn.pragma_update(None, "journal_mode", "OFF")?;
        conn.pragma_update(None, "synchronous", "OFF")?;
        conn.execute_batch(
            "CREATE TABLE files (
                 path     TEXT PRIMARY KEY,   -- relative to the project root
                 dev      INTEGER NOT NULL,
                 ino      INTEGER NOT NULL,
                 size     INTEGER NOT NULL,
                 mtime_ns INTEGER NOT NULL,
                 is_dir   INTEGER NOT NULL
             );
             CREATE INDEX files_inode ON files (dev, ino);
             CREATE INDEX files_size ON files (size);",
        )?;
        let tx = conn.transaction()?;
        let mut count = 0;
        {
            let mut insert = tx.prepare("INSERT OR REPLACE INTO files VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
            let walker = ignore::WalkBuilder::new(root)
                .hidden(false)
                .require_git(false)
                .filter_entry(|e| e.depth() == 0 || (e.file_name() != ATLAS_DIR && e.file_name() != ".git"))
                .build();
            for entry in walker.flatten() {
                if entry.depth() == 0 {
                    continue;
                }
                let Ok(meta) = entry.metadata() else { continue };
                let Ok(rel) = entry.path().strip_prefix(root) else { continue };
                let mtime_ns = meta.mtime() * 1_000_000_000 + meta.mtime_nsec();
                insert.execute(params![
                    rel.to_string_lossy(),
                    meta.dev() as i64,
                    meta.ino() as i64,
                    meta.len() as i64,
                    mtime_ns,
                    meta.is_dir() as i64
                ])?;
                count += 1;
            }
        }
        tx.commit()?;
        conn.pragma_update(None, "user_version", INDEX_VERSION)?;
        drop(conn);
        File::open(&tmp)?.sync_all()?;
        std::fs::rename(&tmp, path)?;
        File::open(dir)?.sync_all()?;
        Ok(count)
    })();
    if count.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::create_dir_all(root.join(".atlas")).unwrap();
        std::fs::write(root.join("docs/a.pdf"), "aaaa").unwrap();
        std::fs::write(root.join("notes.md"), "# n").unwrap();
        std::fs::write(root.join(".atlas/session.json"), "{}").unwrap();
        dir
    }

    #[test]
    fn builds_index_without_atlas_dir() {
        let dir = project();
        let index_path = dir.path().join(".atlas/index.sqlite");
        let (index, report) = FileIndex::ensure(dir.path(), &index_path).unwrap();
        assert_eq!(report.previous, Previous::Missing);
        assert_eq!(report.files, 3); // docs, docs/a.pdf, notes.md
        let meta = std::fs::metadata(dir.path().join("docs/a.pdf")).unwrap();
        assert_eq!(index.by_inode(meta.dev(), meta.ino()), Some(PathBuf::from("docs/a.pdf")));
        assert_eq!(index.with_size(4), vec![PathBuf::from("docs/a.pdf")]);
    }

    #[test]
    fn corrupt_or_deleted_index_is_rebuilt_without_touching_documents() {
        let dir = project();
        let index_path = dir.path().join(".atlas/index.sqlite");
        FileIndex::ensure(dir.path(), &index_path).unwrap();
        assert_eq!(FileIndex::check(&index_path), Previous::Valid);

        // Garbage instead of a database.
        std::fs::write(&index_path, b"definitely not sqlite").unwrap();
        assert!(matches!(FileIndex::check(&index_path), Previous::Corrupt(_)));
        let (index, report) = FileIndex::ensure(dir.path(), &index_path).unwrap();
        assert!(matches!(report.previous, Previous::Corrupt(_)));
        assert_eq!(index.len(), 3);

        // Truncated database.
        let len = std::fs::metadata(&index_path).unwrap().len();
        std::fs::OpenOptions::new().write(true).open(&index_path).unwrap().set_len(len / 2).unwrap();
        let (index, report) = FileIndex::ensure(dir.path(), &index_path).unwrap();
        assert_ne!(report.previous, Previous::Valid);
        assert_eq!(index.len(), 3);

        // Deleted.
        std::fs::remove_file(&index_path).unwrap();
        let (_, report) = FileIndex::ensure(dir.path(), &index_path).unwrap();
        assert_eq!(report.previous, Previous::Missing);

        assert_eq!(std::fs::read_to_string(dir.path().join("docs/a.pdf")).unwrap(), "aaaa");
        assert_eq!(std::fs::read_to_string(dir.path().join("notes.md")).unwrap(), "# n");
        assert_eq!(std::fs::read_to_string(dir.path().join(".atlas/session.json")).unwrap(), "{}");
        // No temporary files are left behind.
        let leftovers: Vec<_> = std::fs::read_dir(dir.path().join(".atlas"))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains("tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn respects_gitignore() {
        let dir = project();
        std::fs::write(dir.path().join(".gitignore"), "build/\n").unwrap();
        std::fs::create_dir(dir.path().join("build")).unwrap();
        std::fs::write(dir.path().join("build/out.o"), "o").unwrap();
        let (index, report) = FileIndex::ensure(dir.path(), &dir.path().join(".atlas/index.sqlite")).unwrap();
        assert_eq!(report.files, 4); // + .gitignore
        assert!(index.with_size(1).is_empty());
    }
}
