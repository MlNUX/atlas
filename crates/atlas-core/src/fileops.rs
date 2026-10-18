// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! File operations: create, rename, copy, move and trash.
//!
//! Nothing here overwrites or deletes a file irrecoverably: name clashes are
//! errors unless the caller chose a [`Conflict`] policy, and "replace" moves
//! the existing file to the trash first. Deleting always goes to the
//! freedesktop.org trash (`~/.local/share/Trash` or a `.Trash-$UID` folder on
//! other file systems).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

/// What to do when the target of a copy or move already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conflict {
    /// Fail with an error (default; the UI asks first).
    Abort,
    /// Keep both: the new item gets a name like "Bericht (Kopie).pdf".
    KeepBoth,
    /// Leave the existing item and skip this one.
    Skip,
    /// Move the existing item to the trash, then copy/move.
    Replace,
}

impl Conflict {
    pub fn from_name(name: &str) -> Self {
        match name {
            "keep-both" => Conflict::KeepBoth,
            "skip" => Conflict::Skip,
            "replace" => Conflict::Replace,
            _ => Conflict::Abort,
        }
    }
}

/// Checks a single file name typed by the user.
pub fn validate_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        bail!("Der Name darf nicht leer sein.");
    }
    if name == "." || name == ".." {
        bail!("„{name}“ ist kein gültiger Name.");
    }
    if name.contains('/') || name.contains('\0') {
        bail!("Der Name darf kein „/“ enthalten.");
    }
    if name.len() > 255 {
        bail!("Der Name ist zu lang.");
    }
    Ok(())
}

pub fn create_file(dir: &Path, name: &str) -> Result<PathBuf> {
    validate_name(name)?;
    let path = dir.join(name);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .with_context(|| format!("„{name}“ kann nicht angelegt werden"))?;
    Ok(path)
}

pub fn create_dir(dir: &Path, name: &str) -> Result<PathBuf> {
    validate_name(name)?;
    let path = dir.join(name);
    std::fs::create_dir(&path).with_context(|| format!("Ordner „{name}“ kann nicht angelegt werden"))?;
    Ok(path)
}

/// Renames within the same folder; never replaces an existing entry.
pub fn rename(path: &Path, new_name: &str) -> Result<PathBuf> {
    validate_name(new_name)?;
    let parent = path.parent().context("kein übergeordneter Ordner")?;
    let target = parent.join(new_name);
    if target == path {
        return Ok(target);
    }
    // A case-only rename on a case-insensitive file system reports the
    // target as existing; same inode means it is the same entry.
    if target.symlink_metadata().is_ok() && !same_file(path, &target) {
        bail!("„{new_name}“ existiert bereits.");
    }
    std::fs::rename(path, &target).with_context(|| format!("Umbenennen in „{new_name}“ fehlgeschlagen"))?;
    Ok(target)
}

fn same_file(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (a.symlink_metadata(), b.symlink_metadata()) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

/// "Bericht.pdf" → "Bericht (Kopie).pdf", "Bericht (Kopie 2).pdf", …
pub fn free_name(dir: &Path, name: &OsStr) -> PathBuf {
    let path = Path::new(name);
    let (stem, ext) = match (path.file_stem(), path.extension()) {
        // Hidden files without extension (".bashrc") keep their full name as stem.
        (Some(stem), Some(ext)) if !stem.is_empty() => {
            (stem.to_string_lossy().into_owned(), format!(".{}", ext.to_string_lossy()))
        }
        _ => (name.to_string_lossy().into_owned(), String::new()),
    };
    for n in 1.. {
        let suffix = if n == 1 { " (Kopie)".to_owned() } else { format!(" (Kopie {n})") };
        let candidate = dir.join(format!("{stem}{suffix}{ext}"));
        if candidate.symlink_metadata().is_err() {
            return candidate;
        }
    }
    unreachable!()
}

/// Paths in `sources` whose name already exists in `dest`.
pub fn conflicts(sources: &[PathBuf], dest: &Path) -> Vec<PathBuf> {
    sources
        .iter()
        .filter_map(|s| s.file_name().map(|n| dest.join(n)))
        .filter(|t| t.symlink_metadata().is_ok())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transfer {
    Copy,
    Move,
}

/// Result of one transferred item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// Copies or moves `sources` into `dest`. Returns the new locations of all
/// items that were transferred (skipped items are left out).
///
/// `progress` is called with (done, total) item counts.
pub fn transfer(
    kind: Transfer,
    sources: &[PathBuf],
    dest: &Path,
    policy: Conflict,
    mut progress: impl FnMut(usize, usize),
) -> Result<Vec<Moved>> {
    if !dest.is_dir() {
        bail!("Ziel „{}“ ist kein Ordner.", dest.display());
    }
    let dest_canonical = dest.canonicalize()?;
    for source in sources {
        let source = source.canonicalize().with_context(|| format!("„{}“ nicht gefunden", source.display()))?;
        if source.is_dir() && dest_canonical.starts_with(&source) {
            bail!("„{}“ kann nicht in sich selbst kopiert oder verschoben werden.", display_name(&source));
        }
    }

    let mut done = Vec::new();
    for (i, source) in sources.iter().enumerate() {
        progress(i, sources.len());
        let name = source.file_name().ok_or_else(|| anyhow!("ungültige Quelle {}", source.display()))?;
        let mut target = dest.join(name);
        let same_place = source.parent() == Some(dest);
        if target.symlink_metadata().is_ok() {
            // Copying into the same folder always produces a copy.
            let policy = if same_place && kind == Transfer::Copy { Conflict::KeepBoth } else { policy };
            if same_place && kind == Transfer::Move {
                continue; // moving onto itself: nothing to do
            }
            match policy {
                Conflict::Abort => bail!("„{}“ existiert bereits im Ziel.", name.to_string_lossy()),
                Conflict::Skip => continue,
                Conflict::KeepBoth => target = free_name(dest, name),
                Conflict::Replace => {
                    trash(std::slice::from_ref(&target))
                        .with_context(|| format!("„{}“ konnte nicht ersetzt werden", name.to_string_lossy()))?;
                }
            }
        }
        match kind {
            Transfer::Copy => copy_recursive(source, &target)?,
            Transfer::Move => move_item(source, &target)?,
        }
        done.push(Moved { from: source.clone(), to: target });
    }
    progress(sources.len(), sources.len());
    Ok(done)
}

fn display_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

/// Copies a file, symlink (as link) or folder tree. Permissions are kept.
pub fn copy_recursive(source: &Path, target: &Path) -> Result<()> {
    let meta = source.symlink_metadata().with_context(|| format!("„{}“ nicht lesbar", source.display()))?;
    let file_type = meta.file_type();
    if file_type.is_symlink() {
        let link = std::fs::read_link(source)?;
        std::os::unix::fs::symlink(link, target)?;
    } else if file_type.is_dir() {
        std::fs::create_dir(target).with_context(|| format!("„{}“ kann nicht angelegt werden", target.display()))?;
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            copy_recursive(&entry.path(), &target.join(entry.file_name()))?;
        }
        std::fs::set_permissions(target, meta.permissions())?;
    } else {
        std::fs::copy(source, target)
            .with_context(|| format!("„{}“ konnte nicht kopiert werden", display_name(source)))?;
    }
    Ok(())
}

/// Renames if possible; across file systems copies and then trashes the
/// source, so a failure in the middle never loses data.
fn move_item(source: &Path, target: &Path) -> Result<()> {
    match std::fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(err) if err.raw_os_error() == Some(libc::EXDEV) => {
            copy_recursive(source, target)?;
            remove_permanently(source)
        }
        Err(err) => Err(err).with_context(|| format!("„{}“ konnte nicht verschoben werden", display_name(source))),
    }
}

/// Removes a source after a successful cross-device copy.
fn remove_permanently(path: &Path) -> Result<()> {
    if path.symlink_metadata()?.is_dir() {
        std::fs::remove_dir_all(path)?
    } else {
        std::fs::remove_file(path)?
    }
    Ok(())
}

/// Moves items to the trash.
pub fn trash(paths: &[PathBuf]) -> Result<()> {
    trash::delete_all(paths).map_err(|e| anyhow!("In den Papierkorb verschieben fehlgeschlagen: {e}"))
}

#[derive(Debug, Clone)]
pub struct TrashEntry {
    /// Opaque ID (path of the `.trashinfo` file).
    pub id: String,
    pub name: String,
    pub original_path: PathBuf,
    pub deleted_unix: i64,
    pub is_dir: bool,
    pub size: Option<u64>,
}

/// All items in all trash folders, newest first.
pub fn trash_list() -> Result<Vec<TrashEntry>> {
    let items = trash::os_limited::list().map_err(|e| anyhow!("Papierkorb nicht lesbar: {e}"))?;
    let mut entries: Vec<TrashEntry> = items
        .into_iter()
        .map(|item| {
            let meta = trash::os_limited::metadata(&item).ok();
            TrashEntry {
                id: item.id.to_string_lossy().into_owned(),
                name: item.name.to_string_lossy().into_owned(),
                original_path: item.original_path(),
                deleted_unix: item.time_deleted,
                is_dir: meta.as_ref().is_some_and(|m| m.size.entries().is_some()),
                size: meta.and_then(|m| m.size.size()),
            }
        })
        .collect();
    entries.sort_by(|a, b| b.deleted_unix.cmp(&a.deleted_unix).then_with(|| a.name.cmp(&b.name)));
    Ok(entries)
}

fn trash_items(ids: &[String]) -> Result<Vec<trash::TrashItem>> {
    let items = trash::os_limited::list().map_err(|e| anyhow!("Papierkorb nicht lesbar: {e}"))?;
    Ok(items.into_iter().filter(|i| ids.iter().any(|id| i.id.to_string_lossy() == id.as_str())).collect())
}

/// Restores items to their original location. A name clash at the original
/// location is reported as error and leaves the item in the trash.
pub fn trash_restore(ids: &[String]) -> Result<()> {
    let items = trash_items(ids)?;
    trash::os_limited::restore_all(items).map_err(|e| match e {
        trash::Error::RestoreCollision { path, .. } => {
            anyhow!("„{}“ existiert bereits; Wiederherstellen abgebrochen.", path.display())
        }
        other => anyhow!("Wiederherstellen fehlgeschlagen: {other}"),
    })
}

/// Deletes items from the trash irrecoverably.
pub fn trash_purge(ids: &[String]) -> Result<()> {
    let items = trash_items(ids)?;
    trash::os_limited::purge_all(items).map_err(|e| anyhow!("Endgültiges Löschen fehlgeschlagen: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn validates_names() {
        assert!(validate_name("Bericht.md").is_ok());
        for bad in ["", "  ", ".", "..", "a/b"] {
            assert!(validate_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn creates_and_renames_without_overwriting() {
        let d = tempfile::tempdir().unwrap();
        let file = create_file(d.path(), "a.md").unwrap();
        assert!(create_file(d.path(), "a.md").is_err());
        create_dir(d.path(), "dir").unwrap();
        fs::write(d.path().join("b.md"), "keep").unwrap();
        assert!(rename(&file, "b.md").is_err());
        assert_eq!(fs::read_to_string(d.path().join("b.md")).unwrap(), "keep");
        let renamed = rename(&file, "c.md").unwrap();
        assert!(renamed.exists() && !file.exists());
    }

    #[test]
    fn free_names() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("x.pdf"), "").unwrap();
        assert_eq!(free_name(d.path(), OsStr::new("x.pdf")), d.path().join("x (Kopie).pdf"));
        fs::write(d.path().join("x (Kopie).pdf"), "").unwrap();
        assert_eq!(free_name(d.path(), OsStr::new("x.pdf")), d.path().join("x (Kopie 2).pdf"));
        assert_eq!(free_name(d.path(), OsStr::new(".bashrc")), d.path().join(".bashrc (Kopie)"));
    }

    #[test]
    fn copies_trees_and_handles_conflicts() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("src");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("sub/a.txt"), "a").unwrap();
        std::os::unix::fs::symlink("sub/a.txt", src.join("link")).unwrap();
        let dest = d.path().join("dest");
        fs::create_dir(&dest).unwrap();

        let moved = transfer(Transfer::Copy, std::slice::from_ref(&src), &dest, Conflict::Abort, |_, _| {}).unwrap();
        assert_eq!(moved[0].to, dest.join("src"));
        assert_eq!(fs::read_to_string(dest.join("src/sub/a.txt")).unwrap(), "a");
        assert!(dest.join("src/link").symlink_metadata().unwrap().file_type().is_symlink());

        // Same again: abort, skip and keep-both behave as documented.
        assert!(transfer(Transfer::Copy, std::slice::from_ref(&src), &dest, Conflict::Abort, |_, _| {}).is_err());
        let skipped = transfer(Transfer::Copy, std::slice::from_ref(&src), &dest, Conflict::Skip, |_, _| {}).unwrap();
        assert!(skipped.is_empty());
        let both = transfer(Transfer::Copy, std::slice::from_ref(&src), &dest, Conflict::KeepBoth, |_, _| {}).unwrap();
        assert_eq!(both[0].to, dest.join("src (Kopie)"));

        // Copy into the same folder always creates a copy.
        let file = d.path().join("f.txt");
        fs::write(&file, "f").unwrap();
        let copy = transfer(Transfer::Copy, std::slice::from_ref(&file), d.path(), Conflict::Abort, |_, _| {}).unwrap();
        assert_eq!(copy[0].to, d.path().join("f (Kopie).txt"));
    }

    #[test]
    fn moves_and_refuses_moving_into_itself() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a");
        fs::create_dir_all(a.join("inner")).unwrap();
        assert!(
            transfer(Transfer::Move, std::slice::from_ref(&a), &a.join("inner"), Conflict::Abort, |_, _| {}).is_err()
        );
        let b = d.path().join("b");
        fs::create_dir(&b).unwrap();
        let moved = transfer(Transfer::Move, std::slice::from_ref(&a), &b, Conflict::Abort, |_, _| {}).unwrap();
        assert_eq!(moved[0].to, b.join("a"));
        assert!(!a.exists() && b.join("a/inner").is_dir());
    }
}
