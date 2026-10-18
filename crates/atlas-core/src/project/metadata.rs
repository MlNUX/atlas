// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! `.atlas/metadata.sqlite`: durable metadata about project files.
//!
//! Phase 0 stores the identities of documents referenced by sessions, so
//! they can be re-found after being moved or renamed outside of Atlas.
//! Every change runs in a transaction; the database uses SQLite's rollback
//! journal with `synchronous = FULL`, which is atomic and – unlike WAL – keeps
//! no extra files around that sync tools could tear apart.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};

use crate::identity::{FileId, Fingerprint};

const SCHEMA_VERSION: i64 = 2;

pub struct Metadata {
    conn: Connection,
}

impl Metadata {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).with_context(|| format!("cannot open {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "DELETE")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.busy_timeout(std::time::Duration::from_secs(2))?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        // Migrations run in order; each step is one transaction.
        if version > SCHEMA_VERSION {
            bail!("{} has a newer schema ({version})", path.display());
        }
        if version < 1 {
            conn.execute_batch(
                "BEGIN;
                 CREATE TABLE documents (
                     path        TEXT PRIMARY KEY,  -- relative to the project root
                     dev         INTEGER NOT NULL,
                     ino         INTEGER NOT NULL,
                     size        INTEGER,
                     head        TEXT,              -- BLAKE3 of the first 64 KiB
                     seen_unix   INTEGER NOT NULL
                 );
                 PRAGMA user_version = 1;
                 COMMIT;",
            )?;
        }
        if version < 2 {
            conn.execute_batch(
                "BEGIN;
                 CREATE TABLE document_state (
                     path        TEXT PRIMARY KEY,  -- relative to the project root
                     state       TEXT NOT NULL,     -- JSON owned by the viewer (page, zoom, …)
                     updated_unix INTEGER NOT NULL
                 );
                 PRAGMA user_version = 2;
                 COMMIT;",
            )?;
        }
        Ok(Self { conn })
    }

    /// Records the identities of `docs` (absolute paths inside `root`).
    pub fn remember(&mut self, root: &Path, docs: &[FileId]) -> Result<()> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO documents (path, dev, ino, size, head, seen_unix) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(path) DO UPDATE SET dev = ?2, ino = ?3, size = ?4, head = ?5, seen_unix = ?6",
            )?;
            for doc in docs {
                let Ok(rel) = doc.path.strip_prefix(root) else { continue };
                let (size, head) = match &doc.fingerprint {
                    Some(fp) => (Some(fp.size as i64), Some(fp.head.as_str())),
                    None => (None, None),
                };
                stmt.execute(params![rel.to_string_lossy(), doc.dev as i64, doc.ino as i64, size, head, now as i64])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// The stored identity of a document (path made absolute with `root`).
    pub fn document(&self, root: &Path, relative: &Path) -> Result<Option<FileId>> {
        let row = self
            .conn
            .query_row(
                "SELECT dev, ino, size, head FROM documents WHERE path = ?1",
                params![relative.to_string_lossy()],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                        r.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()?;
        Ok(row.map(|(dev, ino, size, head)| FileId {
            path: root.join(relative),
            dev: dev as u64,
            ino: ino as u64,
            fingerprint: match (size, head) {
                (Some(size), Some(head)) => Some(Fingerprint { size: size as u64, head }),
                _ => None,
            },
        }))
    }

    /// Viewer state of a document (page, zoom …) as JSON.
    pub fn document_state(&self, relative: &Path) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT state FROM document_state WHERE path = ?1", params![relative.to_string_lossy()], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn set_document_state(&mut self, relative: &Path, state: &str) -> Result<()> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        self.conn.execute(
            "INSERT INTO document_state (path, state, updated_unix) VALUES (?1, ?2, ?3)
             ON CONFLICT(path) DO UPDATE SET state = ?2, updated_unix = ?3",
            params![relative.to_string_lossy(), state, now as i64],
        )?;
        Ok(())
    }

    pub fn document_paths(&self) -> Result<Vec<PathBuf>> {
        let mut stmt = self.conn.prepare("SELECT path FROM documents ORDER BY path")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok()).map(PathBuf::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_identities_relative_to_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/a.pdf"), "pdf").unwrap();
        let db = root.join("meta.sqlite");
        let id = FileId::of(&root.join("docs/a.pdf")).unwrap();
        Metadata::open(&db).unwrap().remember(root, std::slice::from_ref(&id)).unwrap();

        let meta = Metadata::open(&db).unwrap();
        assert_eq!(meta.document_paths().unwrap(), vec![PathBuf::from("docs/a.pdf")]);
        let stored = meta.document(root, Path::new("docs/a.pdf")).unwrap().unwrap();
        assert_eq!(stored, id);
        assert!(meta.document(root, Path::new("missing")).unwrap().is_none());
    }

    #[test]
    fn stores_document_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut meta = Metadata::open(&dir.path().join("m.sqlite")).unwrap();
        assert_eq!(meta.document_state(Path::new("a.pdf")).unwrap(), None);
        meta.set_document_state(Path::new("a.pdf"), r#"{"page":3}"#).unwrap();
        meta.set_document_state(Path::new("a.pdf"), r#"{"page":4}"#).unwrap();
        assert_eq!(meta.document_state(Path::new("a.pdf")).unwrap().as_deref(), Some(r#"{"page":4}"#));
    }
}
