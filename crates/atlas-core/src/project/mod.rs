// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! The project's `.atlas/` folder.
//!
//! ```text
//! Projektarbeit/
//! ├ .atlas/
//! │  ├ project.toml                  durable  – format version, project id
//! │  ├ sessions/<device-id>/
//! │  │  └ active.json                durable  – layout, open documents, tree state
//! │  ├ metadata.sqlite               durable  – file identities
//! │  ├ index.sqlite                  rebuildable – file list (path, inode, size)
//! │  └ cache/                        rebuildable
//! └ … user documents (never touched)
//! ```
//!
//! Durable files are written atomically ([`crate::paths::write_atomic`],
//! SQLite transactions with a rollback journal). Rebuildable data may be
//! deleted or corrupted at any time; [`index::FileIndex::ensure`] detects that
//! and rebuilds it from the project files.
//!
//! All paths stored in `.atlas/` are relative to the project root, so a
//! project can be moved or synced to another machine as a whole.

pub mod index;
pub mod metadata;
pub mod sessions;
pub mod writer;

use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::paths;

pub const ATLAS_DIR: &str = ".atlas";
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub format_version: u32,
    /// Random, stable ID of the project (survives renames and moves).
    pub id: String,
    pub created_unix: u64,
}

#[derive(Debug, Clone)]
pub struct Project {
    root: PathBuf,
    config: ProjectConfig,
    device: String,
}

impl Project {
    /// Opens a project and creates its `.atlas/` structure if needed.
    pub fn open(root: &Path) -> Result<Self> {
        Self::open_as(root, &paths::device_id())
    }

    /// Opens `root` only if it already has sessions of this device; unlike
    /// [`Project::open`] it never creates `.atlas/` in a folder.
    pub fn open_existing(root: &Path) -> Option<Self> {
        let device = paths::device_id();
        if !root.join(ATLAS_DIR).join("sessions").join(&device).is_dir() {
            return None;
        }
        Self::open_as(root, &device).ok()
    }

    /// Like [`Project::open`] with an explicit device ID.
    pub fn open_as(root: &Path, device: &str) -> Result<Self> {
        let root = root.canonicalize().with_context(|| format!("cannot open {}", root.display()))?;
        if !root.is_dir() {
            bail!("{} is not a folder", root.display());
        }
        let device = device.to_owned();
        let dir = root.join(ATLAS_DIR);
        for sub in [dir.join("sessions").join(&device), dir.join("cache")] {
            std::fs::create_dir_all(&sub).with_context(|| format!("cannot create {}", sub.display()))?;
        }
        let config_file = dir.join("project.toml");
        let config = match std::fs::read_to_string(&config_file) {
            Ok(text) => {
                toml::from_str::<ProjectConfig>(&text).with_context(|| format!("invalid {}", config_file.display()))?
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let config = ProjectConfig {
                    format_version: FORMAT_VERSION,
                    id: uuid::Uuid::new_v4().to_string(),
                    created_unix: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs()),
                };
                paths::write_atomic(&config_file, toml::to_string_pretty(&config)?.as_bytes())?;
                config
            }
            Err(err) => return Err(err).with_context(|| format!("cannot read {}", config_file.display())),
        };
        if config.format_version > FORMAT_VERSION {
            bail!(
                "{} was written by a newer Atlas (format {}, supported {})",
                config_file.display(),
                config.format_version,
                FORMAT_VERSION
            );
        }
        Ok(Self { root, config, device })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config(&self) -> &ProjectConfig {
        &self.config
    }

    pub fn device(&self) -> &str {
        &self.device
    }

    pub fn atlas_dir(&self) -> PathBuf {
        self.root.join(ATLAS_DIR)
    }

    pub fn metadata_file(&self) -> PathBuf {
        self.atlas_dir().join("metadata.sqlite")
    }

    pub fn index_file(&self) -> PathBuf {
        self.atlas_dir().join("index.sqlite")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.atlas_dir().join("cache")
    }

    pub fn metadata(&self) -> Result<metadata::Metadata> {
        metadata::Metadata::open(&self.metadata_file())
    }

    /// Current absolute location of a document from a stored session: its
    /// old path if it still exists, otherwise where it was moved to
    /// (looked up by inode, then by fingerprint among index entries of the
    /// same size), or `None` if it is gone.
    pub fn locate(
        &self,
        metadata: Option<&metadata::Metadata>,
        index: Option<&index::FileIndex>,
        relative: &Path,
    ) -> Option<PathBuf> {
        let path = self.absolute(relative)?;
        if path.exists() {
            return Some(path);
        }
        let id = metadata?.document(&self.root, relative).ok()??;
        let index = index?;
        let mut candidates: Vec<PathBuf> = Vec::new();
        candidates.extend(index.by_inode(id.dev, id.ino));
        if let Some(fp) = &id.fingerprint {
            candidates.extend(index.with_size(fp.size));
        }
        let candidates: Vec<PathBuf> = candidates.iter().filter_map(|rel| self.absolute(rel)).collect();
        match id.resolve(candidates.iter().map(PathBuf::as_path)) {
            crate::identity::Resolution::Moved(new) => Some(new),
            _ => None,
        }
    }

    /// Path relative to the project root, if `path` is inside the project.
    pub fn relative(&self, path: &Path) -> Option<PathBuf> {
        path.strip_prefix(&self.root).ok().map(Path::to_path_buf)
    }

    /// Absolute path for a stored relative path. Refuses paths that would
    /// leave the project (`..`, absolute paths), so a crafted `.atlas/`
    /// cannot point Atlas at files elsewhere.
    pub fn absolute(&self, relative: &Path) -> Option<PathBuf> {
        let safe = relative.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
        safe.then(|| self.root.join(relative))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(root: &Path) -> Project {
        Project::open_as(root, "test-device").unwrap()
    }

    #[test]
    fn creates_layout_and_keeps_id() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("paper.pdf"), "x").unwrap();
        let p = open(dir.path());
        let atlas = dir.path().join(".atlas");
        assert!(atlas.join("project.toml").is_file());
        assert!(atlas.join("cache").is_dir());
        assert!(atlas.join("sessions/test-device").is_dir());
        assert_eq!(p.config().id, open(dir.path()).config().id);
        // User files are untouched.
        assert_eq!(std::fs::read_to_string(dir.path().join("paper.pdf")).unwrap(), "x");
    }

    #[test]
    fn refuses_paths_outside_project() {
        let dir = tempfile::tempdir().unwrap();
        let p = open(dir.path());
        assert!(p.absolute(Path::new("docs/a.pdf")).is_some());
        assert!(p.absolute(Path::new("../etc/passwd")).is_none());
        assert!(p.absolute(Path::new("/etc/passwd")).is_none());
    }

    #[test]
    fn locates_documents_moved_outside_of_atlas() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/paper.pdf"), "%PDF-1.4 content").unwrap();
        let p = open(&root);
        let mut meta = p.metadata().unwrap();
        let id = crate::identity::FileId::of(&root.join("docs/paper.pdf")).unwrap();
        meta.remember(&root, &[id]).unwrap();

        std::fs::create_dir(root.join("archive")).unwrap();
        std::fs::rename(root.join("docs/paper.pdf"), root.join("archive/renamed.pdf")).unwrap();
        let (index, _) = index::FileIndex::ensure(&root, &p.index_file()).unwrap();

        let found = p.locate(Some(&meta), Some(&index), Path::new("docs/paper.pdf"));
        assert_eq!(found, Some(root.join("archive/renamed.pdf")));
        assert_eq!(p.locate(Some(&meta), Some(&index), Path::new("docs/none.pdf")), None);
    }

    #[test]
    fn rejects_newer_format() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".atlas")).unwrap();
        std::fs::write(dir.path().join(".atlas/project.toml"), "format_version = 99\nid = \"x\"\ncreated_unix = 0\n")
            .unwrap();
        assert!(Project::open_as(dir.path(), "test-device").is_err());
    }
}
