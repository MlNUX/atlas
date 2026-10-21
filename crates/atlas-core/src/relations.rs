// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Links between two files made by hand ("Verknüpfen mit …"), e.g. two
//! PDFs. Kept in `.atlas/relations.json` with paths relative to the project
//! root; the files themselves stay unchanged. A relation has no direction.
//!
//! ```json
//! {"version": 1, "relations": [{"a": "docs/a.pdf", "b": "docs/b.pdf"}]}
//! ```

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::paths;

pub const FILE: &str = "relations.json";
const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    pub a: PathBuf,
    pub b: PathBuf,
}

impl Relation {
    fn joins(&self, x: &Path, y: &Path) -> bool {
        (self.a == x && self.b == y) || (self.a == y && self.b == x)
    }

    /// The other end, if `path` is one of them.
    pub fn other(&self, path: &Path) -> Option<&Path> {
        if self.a == path {
            Some(&self.b)
        } else if self.b == path {
            Some(&self.a)
        } else {
            None
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Data {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    relations: Vec<Relation>,
}

/// The relations of one project (paths relative to its root).
pub struct Relations {
    file: PathBuf,
}

impl Relations {
    /// `atlas_dir`: the project's `.atlas/` folder.
    pub fn new(atlas_dir: &Path) -> Self {
        Self { file: atlas_dir.join(FILE) }
    }

    fn read(&self) -> Result<Data> {
        match std::fs::read_to_string(&self.file) {
            Ok(text) => {
                let data: Data =
                    serde_json::from_str(&text).with_context(|| format!("invalid {}", self.file.display()))?;
                if data.version > FORMAT_VERSION {
                    bail!("{} has the newer format {}", self.file.display(), data.version);
                }
                Ok(data)
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Data::default()),
            Err(err) => Err(err).with_context(|| format!("cannot read {}", self.file.display())),
        }
    }

    fn write(&self, mut data: Data) -> Result<()> {
        data.version = FORMAT_VERSION;
        paths::write_atomic(&self.file, &serde_json::to_vec_pretty(&data)?)
    }

    /// All relations (none if the file is missing or unreadable).
    pub fn list(&self) -> Vec<Relation> {
        self.read().map(|d| d.relations).unwrap_or_default()
    }

    /// The files related to `path`.
    pub fn of(&self, path: &Path) -> Vec<PathBuf> {
        self.list().iter().filter_map(|r| r.other(path).map(Path::to_path_buf)).collect()
    }

    /// Relates `a` and `b`; `false` if they already are.
    pub fn add(&self, a: &Path, b: &Path) -> Result<bool> {
        if a == b {
            bail!("Eine Datei kann nicht mit sich selbst verknüpft werden.");
        }
        let mut data = self.read()?;
        if data.relations.iter().any(|r| r.joins(a, b)) {
            return Ok(false);
        }
        data.relations.push(Relation { a: a.to_path_buf(), b: b.to_path_buf() });
        self.write(data)?;
        Ok(true)
    }

    /// Removes the relation of `a` and `b`; `false` if there was none.
    pub fn remove(&self, a: &Path, b: &Path) -> Result<bool> {
        let mut data = self.read()?;
        let before = data.relations.len();
        data.relations.retain(|r| !r.joins(a, b));
        if data.relations.len() == before {
            return Ok(false);
        }
        self.write(data)?;
        Ok(true)
    }

    /// `old` (a file or folder) was moved to `new`: relations follow.
    pub fn moved(&self, old: &Path, new: &Path) -> Result<usize> {
        let mut data = self.read()?;
        let mut count = 0;
        for relation in &mut data.relations {
            for end in [&mut relation.a, &mut relation.b] {
                if let Ok(rest) = end.strip_prefix(old) {
                    *end = if rest.as_os_str().is_empty() { new.to_path_buf() } else { new.join(rest) };
                    count += 1;
                }
            }
        }
        // Moving one end onto the other leaves nothing to relate.
        data.relations.retain(|r| r.a != r.b);
        if count > 0 {
            self.write(data)?;
        }
        Ok(count)
    }

    /// `path` (a file or folder) was deleted: its relations go.
    pub fn removed(&self, path: &Path) -> Result<usize> {
        let mut data = self.read()?;
        let before = data.relations.len();
        data.relations.retain(|r| !r.a.starts_with(path) && !r.b.starts_with(path));
        let count = before - data.relations.len();
        if count > 0 {
            self.write(data)?;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_list_move_and_remove() {
        let dir = tempfile::tempdir().unwrap();
        let rel = Relations::new(dir.path());
        let (a, b, c) = (Path::new("docs/a.pdf"), Path::new("docs/b.pdf"), Path::new("notes/c.md"));
        assert!(rel.list().is_empty(), "no file yet");
        assert!(rel.add(a, b).unwrap());
        assert!(!rel.add(b, a).unwrap(), "no direction, no duplicate");
        assert!(rel.add(a, a).is_err());
        assert!(rel.add(c, a).unwrap());
        assert_eq!(rel.of(a), vec![PathBuf::from("docs/b.pdf"), PathBuf::from("notes/c.md")]);
        assert_eq!(rel.of(b), vec![PathBuf::from("docs/a.pdf")]);

        // Renaming a file and moving its folder.
        assert_eq!(rel.moved(b, Path::new("docs/b2.pdf")).unwrap(), 1);
        assert_eq!(rel.moved(Path::new("docs"), Path::new("literatur")).unwrap(), 3);
        assert_eq!(rel.of(c), vec![PathBuf::from("literatur/a.pdf")]);
        assert_eq!(rel.of(Path::new("literatur/b2.pdf")), vec![PathBuf::from("literatur/a.pdf")]);
        assert_eq!(rel.moved(Path::new("doc"), Path::new("x")).unwrap(), 0, "only whole path components");

        assert!(rel.remove(Path::new("literatur/b2.pdf"), Path::new("literatur/a.pdf")).unwrap());
        assert!(!rel.remove(Path::new("literatur/b2.pdf"), Path::new("literatur/a.pdf")).unwrap());
        assert_eq!(rel.removed(Path::new("notes")).unwrap(), 1);
        assert!(rel.list().is_empty());
    }
}
