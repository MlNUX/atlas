// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Folder contents with metadata for the file list, and the fixed system
//! places shown above the project tree.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub name: String,
    pub icon: String,
    pub path: PathBuf,
}

/// Home, desktop, documents, downloads and trash – only those that exist.
pub fn places() -> Vec<Place> {
    let candidates = [
        ("Persönlicher Ordner", "home", dirs::home_dir()),
        ("Schreibtisch", "desktop", dirs::desktop_dir()),
        ("Dokumente", "documents", dirs::document_dir()),
        ("Downloads", "downloads", dirs::download_dir()),
        ("Bilder", "pictures", dirs::picture_dir()),
        ("Musik", "music", dirs::audio_dir()),
        ("Videos", "videos", dirs::video_dir()),
        ("Papierkorb", "trash", trash_files_dir()),
    ];
    let mut seen = Vec::new();
    candidates
        .into_iter()
        .filter_map(|(name, icon, path)| {
            let path = path?;
            // XDG dirs fall back to $HOME when unset; do not list it twice.
            if !path.is_dir() || seen.contains(&path) {
                return None;
            }
            seen.push(path.clone());
            Some(Place { name: name.into(), icon: icon.into(), path })
        })
        .collect()
}

/// `$XDG_DATA_HOME/Trash/files` (freedesktop.org trash specification).
pub fn trash_files_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("Trash/files"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    /// Modification time in milliseconds since the Unix epoch.
    pub modified_ms: i64,
    pub kind: String,
}

impl FileInfo {
    pub fn is_hidden(&self) -> bool {
        self.name.starts_with('.')
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Size,
    Modified,
    Kind,
}

impl SortKey {
    pub fn from_name(name: &str) -> Self {
        match name {
            "size" => SortKey::Size,
            "modified" => SortKey::Modified,
            "kind" => SortKey::Kind,
            _ => SortKey::Name,
        }
    }
}

/// Human readable type of a file, e.g. "PDF-Dokument".
pub fn kind_of(name: &str, is_dir: bool) -> String {
    if is_dir {
        return "Ordner".to_owned();
    }
    let ext = Path::new(name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let known = match ext.as_str() {
        "pdf" => "PDF-Dokument",
        "md" | "markdown" => "Markdown",
        "txt" => "Text",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" => "Bild",
        "rs" => "Rust-Quelltext",
        "py" => "Python-Quelltext",
        "c" | "h" | "cpp" | "hpp" | "cc" => "C/C++-Quelltext",
        "js" | "ts" => "JavaScript/TypeScript",
        "qml" => "QML",
        "nix" => "Nix",
        "toml" | "yaml" | "yml" | "json" | "ini" => "Konfiguration",
        "sh" | "bash" | "zsh" | "fish" => "Shell-Skript",
        "zip" | "tar" | "gz" | "xz" | "zst" | "7z" => "Archiv",
        "" => "Datei",
        _ => return format!("{}-Datei", ext.to_uppercase()),
    };
    known.to_owned()
}

/// Lists `dir` with metadata. Unreadable entries are skipped; symlinks are
/// described by their target (a link to a folder counts as folder).
pub fn list(dir: &Path, show_hidden: bool) -> std::io::Result<Vec<FileInfo>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let Ok(link_meta) = std::fs::symlink_metadata(&path) else { continue };
        let is_symlink = link_meta.file_type().is_symlink();
        let meta = if is_symlink { std::fs::metadata(&path).unwrap_or(link_meta) } else { link_meta };
        let is_dir = meta.is_dir();
        out.push(FileInfo {
            kind: kind_of(&name, is_dir),
            name,
            path,
            is_dir,
            is_symlink,
            size: if is_dir { 0 } else { meta.len() },
            modified_ms: meta.mtime() * 1000 + meta.mtime_nsec() / 1_000_000,
        });
    }
    Ok(out)
}

/// Folders first, then by `key`; ties are broken by name.
pub fn compare(a: &FileInfo, b: &FileInfo, key: SortKey, ascending: bool) -> std::cmp::Ordering {
    let by_name = || a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.name.cmp(&b.name));
    let primary = match key {
        SortKey::Name => by_name(),
        SortKey::Size => a.size.cmp(&b.size).then_with(by_name),
        SortKey::Modified => a.modified_ms.cmp(&b.modified_ms).then_with(by_name),
        SortKey::Kind => a.kind.cmp(&b.kind).then_with(by_name),
    };
    let primary = if ascending { primary } else { primary.reverse() };
    b.is_dir.cmp(&a.is_dir).then(primary)
}

pub fn sort(entries: &mut [FileInfo], key: SortKey, ascending: bool) {
    entries.sort_by(|a, b| compare(a, b, key, ascending));
}

/// One step of turning one sorted listing into another; indices refer to
/// the list after all previous ops were applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListOp {
    Insert { at: usize, items: Vec<FileInfo> },
    Remove { at: usize, count: usize },
    Update { at: usize, item: FileInfo },
}

/// Minimal ops from `old` to `new` (both sorted with the same key), so a
/// view keeps its scroll position and selection when a folder changes.
pub fn diff(old: &[FileInfo], new: &[FileInfo], key: SortKey, ascending: bool) -> Vec<ListOp> {
    let mut ops: Vec<ListOp> = Vec::new();
    let (mut i, mut j, mut offset) = (0usize, 0usize, 0isize);
    let pos = |i: usize, offset: isize| (i as isize + offset) as usize;
    while i < old.len() || j < new.len() {
        let order = match (old.get(i), new.get(j)) {
            (Some(a), Some(b)) if a.path == b.path => None,
            (Some(a), Some(b)) => Some(compare(a, b, key, ascending)),
            (Some(_), None) => Some(std::cmp::Ordering::Less),
            (None, _) => Some(std::cmp::Ordering::Greater),
        };
        match order {
            None => {
                if old[i] != new[j] {
                    ops.push(ListOp::Update { at: pos(i, offset), item: new[j].clone() });
                }
                i += 1;
                j += 1;
            }
            Some(std::cmp::Ordering::Less) => {
                let at = pos(i, offset);
                match ops.last_mut() {
                    Some(ListOp::Remove { at: last, count }) if *last == at => *count += 1,
                    _ => ops.push(ListOp::Remove { at, count: 1 }),
                }
                offset -= 1;
                i += 1;
            }
            Some(_) => {
                let at = pos(i, offset);
                match ops.last_mut() {
                    Some(ListOp::Insert { at: last, items }) if *last + items.len() == at => items.push(new[j].clone()),
                    _ => ops.push(ListOp::Insert { at, items: vec![new[j].clone()] }),
                }
                offset += 1;
                j += 1;
            }
        }
    }
    ops
}

/// Applies ops produced by [`diff`].
pub fn apply(list: &mut Vec<FileInfo>, op: ListOp) {
    match op {
        ListOp::Insert { at, items } => {
            list.splice(at..at, items);
        }
        ListOp::Remove { at, count } => {
            list.drain(at..at + count);
        }
        ListOp::Update { at, item } => list[at] = item,
    }
}

/// "12 KB", "3,4 MB" – decimal units with German decimal comma.
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    let text = if value < 10.0 { format!("{value:.1}") } else { format!("{value:.0}") };
    format!("{} {}", text.replace('.', ","), UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_and_sorts_with_folders_first() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("b.pdf"), vec![0u8; 3000]).unwrap();
        std::fs::write(d.path().join("a.md"), "x").unwrap();
        std::fs::write(d.path().join(".hidden"), "").unwrap();
        std::fs::create_dir(d.path().join("zeta")).unwrap();
        let mut entries = list(d.path(), false).unwrap();
        sort(&mut entries, SortKey::Name, true);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["zeta", "a.md", "b.pdf"]);
        sort(&mut entries, SortKey::Size, false);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["zeta", "b.pdf", "a.md"]);
        assert_eq!(entries[1].kind, "PDF-Dokument");
        assert_eq!(list(d.path(), true).unwrap().len(), 4);
    }

    #[test]
    fn diff_turns_old_into_new() {
        let d = tempfile::tempdir().unwrap();
        for name in ["a", "c", "e", "g"] {
            std::fs::write(d.path().join(name), name).unwrap();
        }
        let mut old = list(d.path(), false).unwrap();
        sort(&mut old, SortKey::Name, true);
        std::fs::remove_file(d.path().join("c")).unwrap();
        std::fs::remove_file(d.path().join("e")).unwrap();
        std::fs::write(d.path().join("b"), "b").unwrap();
        std::fs::write(d.path().join("h"), "h").unwrap();
        std::fs::write(d.path().join("g"), "changed size").unwrap();
        std::fs::create_dir(d.path().join("dir")).unwrap();
        let mut new = list(d.path(), false).unwrap();
        sort(&mut new, SortKey::Name, true);

        let ops = diff(&old, &new, SortKey::Name, true);
        assert!(ops.iter().any(|op| matches!(op, ListOp::Remove { count: 2, .. })), "{ops:?}");
        assert!(ops.iter().any(|op| matches!(op, ListOp::Update { .. })));
        for op in ops {
            apply(&mut old, op);
        }
        assert_eq!(old, new);
    }

    #[test]
    fn formats_sizes() {
        assert_eq!(format_size(999), "999 B");
        assert_eq!(format_size(1500), "1,5 KB");
        assert_eq!(format_size(34_000_000), "34 MB");
    }

    #[test]
    fn kinds() {
        assert_eq!(kind_of("x.RS", false), "Rust-Quelltext");
        assert_eq!(kind_of("x.blend", false), "BLEND-Datei");
        assert_eq!(kind_of("docs", true), "Ordner");
    }
}
