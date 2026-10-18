// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Project tree as a flat list of visible rows (VS Code explorer style).
//!
//! Directories are listed lazily: expanding a row only marks it as loading;
//! the caller lists the directory on a background thread with [`list_dir`]
//! and feeds the result back through [`FlatTree::plan_children`].
//!
//! Changes are expressed as a sequence of [`Op`]s so that a Qt item model can
//! wrap each one in `beginInsertRows`/`endInsertRows` etc. Re-listing a
//! directory that is already shown produces a minimal diff, which keeps
//! expanded subdirectories and the scroll position intact when files change
//! outside of Atlas.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
}

impl Entry {
    pub fn is_hidden(&self) -> bool {
        self.name.starts_with('.')
    }
}

/// Explorer order: directories first, then case-insensitive by name.
pub fn compare_entries(a: &Entry, b: &Entry) -> Ordering {
    b.is_dir
        .cmp(&a.is_dir)
        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        .then_with(|| a.name.cmp(&b.name))
}

/// Lists one directory, sorted in explorer order. Unreadable entries are
/// skipped. Symlinks to directories are shown as directories.
pub fn list_dir(dir: &Path, show_hidden: bool) -> std::io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for item in std::fs::read_dir(dir)? {
        let Ok(item) = item else { continue };
        let name = item.file_name().to_string_lossy().into_owned();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        let Ok(file_type) = item.file_type() else { continue };
        let is_symlink = file_type.is_symlink();
        let is_dir =
            if is_symlink { std::fs::metadata(item.path()).is_ok_and(|m| m.is_dir()) } else { file_type.is_dir() };
        entries.push(Entry { name, path: item.path(), is_dir, is_symlink });
    }
    entries.sort_by(compare_entries);
    Ok(entries)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub entry: Entry,
    pub depth: usize,
    pub expanded: bool,
    pub loading: bool,
}

impl Row {
    fn new(entry: Entry, depth: usize) -> Self {
        Self { entry, depth, expanded: false, loading: false }
    }
}

/// One step of a tree change. Indices refer to the row list *after* all
/// previous ops of the same plan have been applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Insert { at: usize, rows: Vec<Row> },
    Remove { at: usize, count: usize },
    Update { at: usize, row: Row },
}

#[derive(Debug, Default)]
pub struct FlatTree {
    root: PathBuf,
    rows: Vec<Row>,
}

/// Result of [`FlatTree::plan_toggle`].
#[derive(Debug)]
pub enum Toggle {
    /// The directory must be listed; feed the result to `plan_children`.
    Load { ops: Vec<Op>, dir: PathBuf },
    /// The subtree was collapsed.
    Collapse(Vec<Op>),
    /// Nothing to do (e.g. the row is a file).
    None,
}

impl FlatTree {
    pub fn new(root: PathBuf) -> Self {
        Self { root, rows: Vec::new() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn index_of(&self, path: &Path) -> Option<usize> {
        self.rows.iter().position(|r| r.entry.path == path)
    }

    /// Directories whose children are currently shown, including the root.
    /// This is exactly the set of directories that must be watched.
    pub fn expanded_dirs(&self) -> Vec<PathBuf> {
        std::iter::once(self.root.clone())
            .chain(self.rows.iter().filter(|r| r.expanded).map(|r| r.entry.path.clone()))
            .collect()
    }

    fn subtree_end(&self, index: usize) -> usize {
        let depth = self.rows[index].depth;
        self.rows[index + 1..].iter().position(|r| r.depth <= depth).map_or(self.rows.len(), |p| index + 1 + p)
    }

    /// `(first child, end of children, child depth, parent index)` of a shown
    /// directory.
    fn children_range(&self, dir: &Path) -> Option<(usize, usize, usize, Option<usize>)> {
        if dir == self.root {
            return Some((0, self.rows.len(), 0, None));
        }
        let parent = self.index_of(dir)?;
        let row = &self.rows[parent];
        if !row.expanded {
            return None;
        }
        Some((parent + 1, self.subtree_end(parent), row.depth + 1, Some(parent)))
    }

    /// Expand or collapse the directory row at `index`.
    pub fn plan_toggle(&self, index: usize) -> Toggle {
        let Some(row) = self.rows.get(index) else { return Toggle::None };
        if !row.entry.is_dir {
            return Toggle::None;
        }
        if row.expanded {
            let end = self.subtree_end(index);
            let mut ops = Vec::new();
            if end > index + 1 {
                ops.push(Op::Remove { at: index + 1, count: end - index - 1 });
            }
            ops.push(Op::Update { at: index, row: Row { expanded: false, loading: false, ..row.clone() } });
            Toggle::Collapse(ops)
        } else {
            let ops = vec![Op::Update { at: index, row: Row { expanded: true, loading: true, ..row.clone() } }];
            Toggle::Load { ops, dir: row.entry.path.clone() }
        }
    }

    /// Ops that make the children of `dir` equal to `entries` (which must be
    /// sorted with [`compare_entries`], as returned by [`list_dir`]).
    /// Children that stay keep their expanded subtrees. Returns no ops if
    /// `dir` is not (or no longer) shown as expanded.
    pub fn plan_children(&self, dir: &Path, entries: Vec<Entry>) -> Vec<Op> {
        let Some((start, end, depth, parent)) = self.children_range(dir) else {
            return Vec::new();
        };
        let mut plan = Plan::default();
        if let Some(p) = parent
            && self.rows[p].loading
        {
            plan.push(Op::Update { at: p, row: Row { loading: false, ..self.rows[p].clone() } });
        }

        let mut j = start;
        for entry in entries {
            loop {
                if j >= end {
                    plan.insert(j, Row::new(entry, depth));
                    break;
                }
                let current = &self.rows[j];
                let current_end = self.subtree_end(j);
                match compare_entries(&current.entry, &entry) {
                    Ordering::Equal => {
                        if current.entry != entry {
                            plan.update(j, Row { entry, ..current.clone() });
                        }
                        j = current_end;
                        break;
                    }
                    Ordering::Less => {
                        plan.remove(j, current_end - j);
                        j = current_end;
                    }
                    Ordering::Greater => {
                        plan.insert(j, Row::new(entry, depth));
                        break;
                    }
                }
            }
        }
        if j < end {
            plan.remove(j, end - j);
        }
        plan.ops
    }

    /// Ops that drop all rows below a directory that could not be listed
    /// (e.g. it was deleted while loading).
    pub fn plan_failed(&self, dir: &Path) -> Vec<Op> {
        match self.index_of(dir) {
            Some(i) => {
                let end = self.subtree_end(i);
                let mut ops = Vec::new();
                if end > i + 1 {
                    ops.push(Op::Remove { at: i + 1, count: end - i - 1 });
                }
                ops.push(Op::Update { at: i, row: Row { expanded: false, loading: false, ..self.rows[i].clone() } });
                ops
            }
            None => Vec::new(),
        }
    }

    pub fn apply(&mut self, op: Op) {
        match op {
            Op::Insert { at, rows } => {
                self.rows.splice(at..at, rows);
            }
            Op::Remove { at, count } => {
                self.rows.drain(at..at + count);
            }
            Op::Update { at, row } => self.rows[at] = row,
        }
    }

    pub fn apply_all(&mut self, ops: impl IntoIterator<Item = Op>) {
        for op in ops {
            self.apply(op);
        }
    }

    /// Replaces everything (new root). The caller resets its model.
    pub fn reset(&mut self, root: PathBuf) {
        self.root = root;
        self.rows.clear();
    }
}

/// Builds an op sequence while walking the *original* rows: tracks the index
/// shift caused by earlier ops and merges adjacent inserts/removes so that a
/// directory with 10 000 entries becomes a single model insert.
#[derive(Default)]
struct Plan {
    ops: Vec<Op>,
    offset: isize,
}

impl Plan {
    fn pos(&self, original: usize) -> usize {
        (original as isize + self.offset) as usize
    }

    fn push(&mut self, op: Op) {
        self.ops.push(op);
    }

    fn insert(&mut self, original: usize, row: Row) {
        let at = self.pos(original);
        self.offset += 1;
        if let Some(Op::Insert { at: last_at, rows }) = self.ops.last_mut()
            && *last_at + rows.len() == at
        {
            rows.push(row);
            return;
        }
        self.ops.push(Op::Insert { at, rows: vec![row] });
    }

    fn remove(&mut self, original: usize, count: usize) {
        let at = self.pos(original);
        self.offset -= count as isize;
        if let Some(Op::Remove { at: last_at, count: last_count }) = self.ops.last_mut()
            && *last_at == at
        {
            *last_count += count;
            return;
        }
        self.ops.push(Op::Remove { at, count });
    }

    fn update(&mut self, original: usize, row: Row) {
        let at = self.pos(original);
        self.ops.push(Op::Update { at, row });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn names(tree: &FlatTree) -> Vec<String> {
        tree.rows().iter().map(|r| format!("{}{}", "  ".repeat(r.depth), r.entry.name)).collect()
    }

    fn load(tree: &mut FlatTree, dir: &Path) {
        let ops = tree.plan_children(dir, list_dir(dir, false).unwrap());
        tree.apply_all(ops);
    }

    fn expand(tree: &mut FlatTree, path: &Path) {
        let i = tree.index_of(path).unwrap();
        match tree.plan_toggle(i) {
            Toggle::Load { ops, dir } => {
                tree.apply_all(ops);
                load(tree, &dir);
            }
            other => panic!("expected load, got {other:?}"),
        }
    }

    #[test]
    fn sorts_dirs_first_and_hides_dotfiles() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("b.txt"), "").unwrap();
        fs::write(d.path().join("A.md"), "").unwrap();
        fs::write(d.path().join(".hidden"), "").unwrap();
        fs::create_dir(d.path().join("zdir")).unwrap();
        let mut tree = FlatTree::new(d.path().to_path_buf());
        load(&mut tree, d.path());
        assert_eq!(names(&tree), ["zdir", "A.md", "b.txt"]);
    }

    #[test]
    fn rediff_keeps_expanded_subtrees() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "").unwrap();
        fs::write(root.join("a.txt"), "").unwrap();
        fs::write(root.join("c.txt"), "").unwrap();
        let mut tree = FlatTree::new(root.to_path_buf());
        load(&mut tree, root);
        expand(&mut tree, &root.join("src"));
        assert_eq!(names(&tree), ["src", "  main.rs", "a.txt", "c.txt"]);

        // External changes: add b.txt, remove c.txt, add a dir before src.
        fs::write(root.join("b.txt"), "").unwrap();
        fs::remove_file(root.join("c.txt")).unwrap();
        fs::create_dir(root.join("docs")).unwrap();
        load(&mut tree, root);
        assert_eq!(names(&tree), ["docs", "src", "  main.rs", "a.txt", "b.txt"]);
        assert!(tree.rows()[1].expanded);

        // Remove the expanded directory itself.
        fs::remove_dir_all(root.join("src")).unwrap();
        load(&mut tree, root);
        assert_eq!(names(&tree), ["docs", "a.txt", "b.txt"]);
    }

    #[test]
    fn collapse_removes_subtree() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::write(root.join("a/b/x"), "").unwrap();
        fs::write(root.join("z"), "").unwrap();
        let mut tree = FlatTree::new(root.to_path_buf());
        load(&mut tree, root);
        expand(&mut tree, &root.join("a"));
        expand(&mut tree, &root.join("a/b"));
        assert_eq!(names(&tree), ["a", "  b", "    x", "z"]);
        let Toggle::Collapse(ops) = tree.plan_toggle(0) else { panic!() };
        tree.apply_all(ops);
        assert_eq!(names(&tree), ["a", "z"]);
        assert_eq!(tree.expanded_dirs(), vec![root.to_path_buf()]);
    }

    #[test]
    fn large_directory_is_one_insert() {
        let d = tempfile::tempdir().unwrap();
        for i in 0..2000 {
            fs::write(d.path().join(format!("f{i:05}")), "").unwrap();
        }
        let tree = FlatTree::new(d.path().to_path_buf());
        let ops = tree.plan_children(d.path(), list_dir(d.path(), false).unwrap());
        assert_eq!(ops.len(), 1);
        assert!(matches!(&ops[0], Op::Insert { at: 0, rows } if rows.len() == 2000));
    }
}
