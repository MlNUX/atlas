// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Local index of the project's Markdown files: links, tags, backlinks.
//!
//! Notes are ordinary files anywhere in the project; the index is derived
//! from them (respecting `.gitignore`) and can always be rebuilt. It powers
//! backlinks, tag filters, link updates after moves inside Atlas and repair
//! suggestions for links broken by external moves.

use std::collections::BTreeMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::markdown::{self, Target};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteLink {
    /// Absolute, normalised target.
    pub target: PathBuf,
    pub raw: String,
    pub line: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Note {
    pub title: Option<String>,
    /// As written in the front matter.
    pub tags: Vec<String>,
    pub links: Vec<NoteLink>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backlink {
    pub source: PathBuf,
    pub line: usize,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// Lowercase, used for comparisons.
    pub key: String,
    /// First spelling found.
    pub display: String,
    pub count: usize,
}

/// A link rewrite prepared after a move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewrite {
    pub file: PathBuf,
    /// Hash of the content the rewrite is based on.
    pub base_hash: String,
    pub text: String,
    pub links: usize,
}

/// Proposed fix for a broken link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repair {
    pub source: PathBuf,
    pub line: usize,
    pub raw: String,
    pub broken: PathBuf,
    pub suggestion: PathBuf,
}

pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "md" | "markdown" | "mdown" | "mkd"))
}

fn walker(root: &Path) -> ignore::Walk {
    ignore::WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .parents(true)
        .require_git(false)
        .filter_entry(|e| e.file_name() != crate::project::ATLAS_DIR && e.file_name() != ".git")
        .build()
}

/// Where `path` is after `old` was moved to `new`.
pub fn moved_path(path: &Path, old: &Path, new: &Path) -> Option<PathBuf> {
    let rest = path.strip_prefix(old).ok()?;
    // `join("")` would append a trailing slash.
    Some(if rest.as_os_str().is_empty() { new.to_path_buf() } else { new.join(rest) })
}

#[derive(Debug, Clone, Default)]
pub struct NoteIndex {
    root: PathBuf,
    notes: BTreeMap<PathBuf, Note>,
}

impl NoteIndex {
    pub fn new(root: &Path) -> Self {
        Self { root: root.to_path_buf(), notes: BTreeMap::new() }
    }

    /// Scans all Markdown files of the project.
    pub fn build(root: &Path) -> Self {
        let mut index = Self::new(root);
        for entry in walker(root).flatten() {
            if entry.file_type().is_some_and(|t| t.is_file()) && is_markdown(entry.path()) {
                index.update(entry.path());
            }
        }
        index
    }

    pub fn len(&self) -> usize {
        self.notes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    pub fn note(&self, path: &Path) -> Option<&Note> {
        self.notes.get(path)
    }

    pub fn paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.notes.keys()
    }

    /// Re-reads one file (removes it if it is gone or no Markdown file).
    pub fn update(&mut self, path: &Path) {
        match std::fs::read_to_string(path) {
            Ok(text) if is_markdown(path) => self.update_text(path, &text),
            _ => {
                self.notes.remove(path);
            }
        }
    }

    pub fn update_text(&mut self, path: &Path, text: &str) {
        let analysis = markdown::analyze(text, path, Some(&self.root));
        let links = analysis
            .links
            .into_iter()
            .filter_map(|l| match l.target {
                Target::File { path, .. } => Some(NoteLink { target: path, raw: l.raw, line: l.line }),
                _ => None,
            })
            .collect();
        self.notes.insert(path.to_path_buf(), Note { title: analysis.title, tags: analysis.tags, links });
    }

    pub fn remove(&mut self, path: &Path) {
        self.notes.retain(|p, _| !p.starts_with(path));
    }

    /// Notes linking to `target` (a file, or anything below a folder).
    pub fn backlinks(&self, target: &Path) -> Vec<Backlink> {
        let mut out = Vec::new();
        for (source, note) in &self.notes {
            for link in &note.links {
                if link.target == target && source != target {
                    out.push(Backlink { source: source.clone(), line: link.line, raw: link.raw.clone() });
                }
            }
        }
        out
    }

    /// All tags, sorted by key.
    pub fn tags(&self) -> Vec<Tag> {
        let mut map: BTreeMap<String, Tag> = BTreeMap::new();
        for note in self.notes.values() {
            for tag in &note.tags {
                let key = markdown::normalize_tag(tag);
                map.entry(key.clone()).or_insert(Tag { key, display: tag.clone(), count: 0 }).count += 1;
            }
        }
        map.into_values().collect()
    }

    /// Notes with `tag`, including its sub tags (`architektur` matches
    /// `Architektur/Backend`).
    pub fn with_tag(&self, tag: &str) -> Vec<PathBuf> {
        let wanted = markdown::normalize_tag(tag);
        let prefix = format!("{wanted}/");
        self.notes
            .iter()
            .filter(|(_, n)| {
                n.tags.iter().map(|t| markdown::normalize_tag(t)).any(|t| t == wanted || t.starts_with(&prefix))
            })
            .map(|(p, _)| p.clone())
            .collect()
    }

    /// Links whose target does not exist.
    pub fn broken_links(&self, source: Option<&Path>) -> Vec<(PathBuf, NoteLink)> {
        self.notes
            .iter()
            .filter(|(p, _)| source.is_none_or(|s| s == p.as_path()))
            .flat_map(|(p, n)| n.links.iter().map(move |l| (p.clone(), l.clone())))
            .filter(|(_, l)| l.target.symlink_metadata().is_err())
            .collect()
    }

    /// Prepares link updates after Atlas moved or renamed `moves`
    /// (`(old, new)`, files or folders; already done on disk).
    ///
    /// Covers links pointing into moved items as well as relative links of
    /// moved notes themselves. Nothing is written; see [`apply`].
    pub fn plan_relink(&self, moves: &[(PathBuf, PathBuf)]) -> Vec<Rewrite> {
        let map = |p: &Path| moves.iter().find_map(|(old, new)| moved_path(p, old, new));
        let mut out = Vec::new();
        for (old_file, note) in &self.notes {
            let new_file = map(old_file).unwrap_or_else(|| old_file.clone());
            let file_moved = &new_file != old_file;
            if !file_moved && !note.links.iter().any(|l| map(&l.target).is_some()) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&new_file) else { continue };
            let (new_text, links) = markdown::rewrite_links(&text, old_file, &new_file, Some(&self.root), map);
            if links > 0 {
                out.push(Rewrite {
                    file: new_file,
                    base_hash: crate::textfile::hash_bytes(text.as_bytes()),
                    text: new_text,
                    links,
                });
            }
        }
        out
    }

    /// Writes prepared rewrites (skipping files changed in the meantime)
    /// and updates the index. Returns the rewritten files.
    pub fn apply(&mut self, moves: &[(PathBuf, PathBuf)], rewrites: &[Rewrite]) -> Vec<PathBuf> {
        let mut done = Vec::new();
        for rewrite in rewrites {
            match crate::textfile::save(&rewrite.file, &rewrite.text, Some(&rewrite.base_hash)) {
                Ok(_) => done.push(rewrite.file.clone()),
                Err(err) => log::warn!("cannot update links in {}: {err}", rewrite.file.display()),
            }
        }
        self.moved(moves);
        for file in &done {
            self.update(file);
        }
        done
    }

    /// Moves index entries after files were moved (without rewriting).
    pub fn moved(&mut self, moves: &[(PathBuf, PathBuf)]) {
        for (old, new) in moves {
            let moved: Vec<PathBuf> = self.notes.keys().filter(|p| p.starts_with(old)).cloned().collect();
            for path in moved {
                self.notes.remove(&path);
                if let Some(path) = moved_path(&path, old, new) {
                    self.update(&path);
                }
            }
            if is_markdown(new) && new.is_file() {
                self.update(new);
            }
        }
    }

    /// Suggestions for broken links (of `source`, or all notes): the file
    /// with the same inode as recorded in `known` (path → (dev, ino)), or the
    /// only file in the project with the same name. Walks the project, so
    /// call it off the UI thread.
    pub fn repair_suggestions(
        &self,
        source: Option<&Path>,
        known: impl Fn(&Path) -> Option<(u64, u64)>,
    ) -> Vec<Repair> {
        let broken = self.broken_links(source);
        if broken.is_empty() {
            return Vec::new();
        }
        let mut by_name: BTreeMap<std::ffi::OsString, Vec<PathBuf>> = BTreeMap::new();
        let mut by_inode: BTreeMap<(u64, u64), PathBuf> = BTreeMap::new();
        let wanted: Vec<_> = broken.iter().filter_map(|(_, l)| l.target.file_name().map(|n| n.to_owned())).collect();
        for entry in walker(&self.root).flatten() {
            let Some(name) = entry.path().file_name() else { continue };
            if let Ok(meta) = entry.metadata() {
                by_inode.insert((meta.dev(), meta.ino()), entry.path().to_path_buf());
            }
            if wanted.iter().any(|w| w == name) {
                by_name.entry(name.to_owned()).or_default().push(entry.path().to_path_buf());
            }
        }
        broken
            .into_iter()
            .filter_map(|(source, link)| {
                let suggestion = known(&link.target).and_then(|id| by_inode.get(&id).cloned()).or_else(|| {
                    let candidates = by_name.get(link.target.file_name()?)?;
                    (candidates.len() == 1).then(|| candidates[0].clone())
                })?;
                Some(Repair { source, line: link.line, raw: link.raw, broken: link.target, suggestion })
            })
            .collect()
    }

    /// Prepares the rewrite of `source` that replaces links to `broken` by
    /// links to `suggestion` (the user confirms before [`apply`]).
    pub fn plan_repair(&self, source: &Path, fixes: &[(PathBuf, PathBuf)]) -> Option<Rewrite> {
        let text = std::fs::read_to_string(source).ok()?;
        let map = |p: &Path| fixes.iter().find(|(b, _)| b == p).map(|(_, s)| s.clone());
        let (new_text, links) = markdown::rewrite_links(&text, source, source, Some(&self.root), map);
        (links > 0).then(|| Rewrite {
            file: source.to_path_buf(),
            base_hash: crate::textfile::hash_bytes(text.as_bytes()),
            text: new_text,
            links,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) -> PathBuf {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn backlinks_tags_and_relink() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let parser = write(root, "src/parser.rs", "fn parse() {}\n");
        let a = write(
            root,
            "notes/a.md",
            "---\ntags: [Architektur/Backend]\n---\n[P](/src/parser.rs#L1) und [B](../docs/b.md#start)\n",
        );
        let b = write(root, "docs/b.md", "---\ntags: [architektur]\n---\n# Start\n[A](../notes/a.md)\n");
        write(root, "target/ignored.md", "[P](/src/parser.rs)");
        write(root, ".gitignore", "target/\n");

        let mut index = NoteIndex::build(root);
        assert_eq!(index.len(), 2, "ignored folders are skipped");
        assert_eq!(
            index.backlinks(&parser),
            vec![Backlink { source: a.clone(), line: 4, raw: "/src/parser.rs#L1".into() }]
        );
        assert_eq!(index.backlinks(&a).len(), 1);
        let tags = index.tags();
        assert_eq!(tags.iter().map(|t| t.key.as_str()).collect::<Vec<_>>(), ["architektur", "architektur/backend"]);
        assert_eq!(index.with_tag("ARCHITEKTUR").len(), 2, "hierarchical and case-insensitive");
        assert_eq!(index.with_tag("architektur/backend"), vec![a.clone()]);

        // Atlas renames src/ to lib/ and moves b.md into docs/old/.
        let moves = vec![(root.join("src"), root.join("lib")), (b.clone(), root.join("docs/old/b.md"))];
        std::fs::rename(root.join("src"), root.join("lib")).unwrap();
        std::fs::create_dir_all(root.join("docs/old")).unwrap();
        std::fs::rename(&b, root.join("docs/old/b.md")).unwrap();
        let plan = index.plan_relink(&moves);
        assert_eq!(plan.len(), 2, "{plan:#?}");
        let done = index.apply(&moves, &plan);
        assert_eq!(done.len(), 2);
        let a_text = std::fs::read_to_string(&a).unwrap();
        assert!(a_text.contains("[P](/lib/parser.rs#L1)"), "{a_text}");
        assert!(a_text.contains("[B](../docs/old/b.md#start)"), "{a_text}");
        let b_text = std::fs::read_to_string(root.join("docs/old/b.md")).unwrap();
        assert!(b_text.contains("[A](../../notes/a.md)"), "{b_text}");
        assert!(index.broken_links(None).is_empty());
        assert_eq!(index.backlinks(&root.join("lib/parser.rs")).len(), 1);
    }

    #[test]
    fn suggests_repairs_after_external_moves() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let report = write(root, "docs/bericht.pdf", "%PDF");
        let note = write(root, "n.md", "[Bericht](docs/bericht.pdf#page=2)\n");
        let mut index = NoteIndex::build(root);
        let meta = std::fs::metadata(&report).unwrap();
        let known = (meta.dev(), meta.ino());

        // Moved and renamed outside of Atlas.
        std::fs::create_dir_all(root.join("archiv")).unwrap();
        std::fs::rename(&report, root.join("archiv/bericht-final.pdf")).unwrap();
        assert_eq!(index.broken_links(None).len(), 1);
        let repairs = index.repair_suggestions(None, |p| (p == report).then_some(known));
        assert_eq!(repairs.len(), 1);
        assert_eq!(repairs[0].suggestion, root.join("archiv/bericht-final.pdf"));
        // Without inode information only an unambiguous name helps.
        assert!(index.repair_suggestions(None, |_| None).is_empty());

        // Nothing changed until confirmed.
        assert_eq!(std::fs::read_to_string(&note).unwrap(), "[Bericht](docs/bericht.pdf#page=2)\n");
        let rewrite = index.plan_repair(&note, &[(repairs[0].broken.clone(), repairs[0].suggestion.clone())]).unwrap();
        index.apply(&[], &[rewrite]);
        assert_eq!(std::fs::read_to_string(&note).unwrap(), "[Bericht](archiv/bericht-final.pdf#page=2)\n");
        assert!(index.broken_links(None).is_empty());
    }
}
