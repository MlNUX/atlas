// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Extracting zip archives ("Hier entpacken" in the file view), as Dolphin
//! does with "Unterordner automatisch erkennen": an archive with exactly one
//! folder at its top comes out as that folder, anything else goes into a new
//! folder named after the archive. Nothing existing is overwritten (a taken
//! name gets a free one, see [`crate::fileops::free_name`]), no entry may
//! leave the target (absolute paths, `..`), links are not created and the
//! macOS extras (`__MACOSX/`, `.DS_Store`) are left out.

use std::ffi::OsStr;
use std::fs::File;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

/// Archives that can be extracted.
pub fn is_zip(path: &Path) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    // Notebooks from Atlas Tablet are zips too, but opened as notebooks.
    name.ends_with(".zip") && !name.ends_with(".atlasnote.zip")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extracted {
    /// The folder (or the single file) that was created.
    pub target: PathBuf,
    /// Files written.
    pub files: usize,
    /// Entries left out: unsafe paths, links.
    pub skipped: usize,
}

/// `dir/name` if that is free, else a free name next to it.
fn free_target(dir: &Path, name: &OsStr) -> PathBuf {
    let plain = dir.join(name);
    if plain.symlink_metadata().is_err() { plain } else { crate::fileops::free_name(dir, name) }
}

fn is_junk(path: &Path) -> bool {
    path.components().next().is_some_and(|c| c.as_os_str() == "__MACOSX")
        || path.file_name().is_some_and(|n| n == ".DS_Store")
}

/// Extracts `archive` into `dir`; `progress(done, total)` after each entry.
pub fn extract_zip(archive: &Path, dir: &Path, mut progress: impl FnMut(usize, usize)) -> Result<Extracted> {
    let file = File::open(archive).with_context(|| format!("{} ist nicht lesbar", archive.display()))?;
    let mut zip = zip::ZipArchive::new(file).with_context(|| format!("{} ist kein Zip-Archiv", archive.display()))?;

    // The entries with safe paths inside the archive.
    let mut entries: Vec<(usize, PathBuf, bool)> = Vec::new();
    let mut skipped = 0;
    for i in 0..zip.len() {
        let entry = zip.by_index_raw(i)?;
        let Some(path) = entry.enclosed_name() else {
            skipped += 1;
            continue;
        };
        if path.as_os_str().is_empty() || is_junk(&path) {
            continue;
        }
        if entry.is_symlink() {
            skipped += 1;
            continue;
        }
        if entry.encrypted() {
            bail!(
                "„{}“ ist verschlüsselt – das kann Atlas nicht entpacken",
                archive.file_name().unwrap_or_default().to_string_lossy()
            );
        }
        entries.push((i, path, entry.is_dir()));
    }
    if entries.is_empty() {
        bail!("Das Archiv ist leer");
    }

    // One folder at the top: that folder; else a folder named after the archive.
    let top = |p: &Path| p.components().next().map(|c| c.as_os_str().to_owned());
    let first = top(&entries[0].1);
    let single_folder = first.is_some()
        && entries.iter().all(|(_, p, _)| top(p) == first)
        && entries.iter().any(|(_, p, is_dir)| p.components().count() > 1 || *is_dir);
    let (target, strip) = if single_folder {
        let name = first.clone().unwrap_or_default();
        (free_target(dir, &name), true)
    } else {
        let stem = archive.file_stem().unwrap_or(OsStr::new("Archiv")).to_owned();
        (free_target(dir, &stem), false)
    };
    std::fs::create_dir_all(&target).with_context(|| format!("{} kann nicht angelegt werden", target.display()))?;

    let result = (|| -> Result<usize> {
        let total = entries.len();
        let mut files = 0;
        for (done, (index, path, is_dir)) in entries.iter().enumerate() {
            let relative: PathBuf = if strip { path.components().skip(1).collect() } else { path.clone() };
            // (enclosed_name already refuses "..", this keeps it so.)
            if relative.components().any(|c| !matches!(c, Component::Normal(_))) {
                continue;
            }
            let out = target.join(&relative);
            if *is_dir || relative.as_os_str().is_empty() {
                std::fs::create_dir_all(&out)?;
            } else {
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut entry = zip.by_index(*index)?;
                let mut writer =
                    File::create(&out).with_context(|| format!("{} kann nicht geschrieben werden", out.display()))?;
                std::io::copy(&mut entry, &mut writer)
                    .with_context(|| format!("„{}“ im Archiv ist nicht lesbar", path.display()))?;
                #[cfg(unix)]
                if let Some(mode) = entry.unix_mode() {
                    use std::os::unix::fs::PermissionsExt;
                    // Executable bits survive, set-uid and friends do not.
                    let _ = std::fs::set_permissions(&out, std::fs::Permissions::from_mode((mode & 0o777) | 0o600));
                }
                files += 1;
            }
            progress(done + 1, total);
        }
        Ok(files)
    })();
    match result {
        Ok(files) => Ok(Extracted { target, files, skipped }),
        Err(err) => {
            // The folder was new: nothing half-extracted stays behind.
            let _ = std::fs::remove_dir_all(&target);
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make(path: &Path, entries: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default().unix_permissions(0o755);
        for (name, bytes) in entries {
            if name.ends_with('/') {
                zip.add_directory(*name, options).unwrap();
            } else {
                zip.start_file(*name, options).unwrap();
                zip.write_all(bytes).unwrap();
            }
        }
        zip.finish().unwrap();
    }

    #[test]
    fn loose_files_go_into_a_folder_named_after_the_archive() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("Folien.zip");
        make(&archive, &[("a.txt", b"A"), ("sub/b.txt", b"B"), ("__MACOSX/._a.txt", b"x"), ("sub/.DS_Store", b"x")]);
        let mut steps = 0;
        let out = extract_zip(&archive, dir.path(), |_, _| steps += 1).unwrap();
        assert_eq!(out, Extracted { target: dir.path().join("Folien"), files: 2, skipped: 0 });
        assert_eq!(std::fs::read_to_string(out.target.join("sub/b.txt")).unwrap(), "B");
        assert!(!out.target.join("__MACOSX").exists() && !out.target.join("sub/.DS_Store").exists());
        assert_eq!(steps, 2);
        // Again: a new folder, nothing overwritten.
        let again = extract_zip(&archive, dir.path(), |_, _| {}).unwrap();
        assert_eq!(again.target, dir.path().join("Folien (Kopie)"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(out.target.join("a.txt")).unwrap().permissions().mode() & 0o777, 0o755);
        }
    }

    #[test]
    fn one_top_folder_comes_out_as_that_folder() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("projekt-main.zip");
        make(&archive, &[("projekt/", b""), ("projekt/README.md", b"# P"), ("projekt/src/main.rs", b"fn main() {}")]);
        let out = extract_zip(&archive, dir.path(), |_, _| {}).unwrap();
        assert_eq!(out.target, dir.path().join("projekt"));
        assert_eq!(std::fs::read_to_string(out.target.join("src/main.rs")).unwrap(), "fn main() {}");
        // A single file is not a folder: into "eins/".
        let archive = dir.path().join("eins.zip");
        make(&archive, &[("nur.txt", b"x")]);
        assert_eq!(extract_zip(&archive, dir.path(), |_, _| {}).unwrap().target, dir.path().join("eins"));
    }

    #[test]
    fn unsafe_paths_stay_inside() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("böse.zip");
        make(&archive, &[("../../draußen.txt", b"x"), ("/etc/passwort", b"x"), ("gut.txt", b"ok")]);
        let out = extract_zip(&archive, &dir.path().join("ziel"), |_, _| {}).unwrap();
        assert_eq!((out.files, out.skipped), (1, 2), "{out:?}");
        assert!(!dir.path().join("draußen.txt").exists() && !dir.path().parent().unwrap().join("draußen.txt").exists());
        let not_zip = dir.path().join("kaputt.zip");
        std::fs::write(&not_zip, b"kein zip").unwrap();
        assert!(extract_zip(&not_zip, dir.path(), |_, _| {}).is_err());
        assert!(
            is_zip(Path::new("a/B.ZIP")) && !is_zip(Path::new("Heft.atlasnote.zip")) && !is_zip(Path::new("a.zipx"))
        );
    }
}
