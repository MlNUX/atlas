// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Trash round trip against a private trash (own process, so redirecting
//! `XDG_DATA_HOME` does not affect other tests or the user's trash).

use std::fs;

use atlas_core::fileops::{self, Conflict, Transfer};

#[test]
fn trash_restore_purge_and_replace() {
    let home = tempfile::tempdir().unwrap();
    // SAFETY: the only test in this binary; no other threads read the env.
    unsafe { std::env::set_var("XDG_DATA_HOME", home.path().join("data")) };
    let work = tempfile::tempdir_in(home.path()).unwrap();
    let file = work.path().join("Bericht.pdf");
    fs::write(&file, "v1").unwrap();

    fileops::trash(std::slice::from_ref(&file)).unwrap();
    assert!(!file.exists());
    let entries = fileops::trash_list().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].original_path, file.canonicalize().unwrap_or(file.clone()));
    assert!(entries[0].deleted_unix > 1_700_000_000, "deletion time: {}", entries[0].deleted_unix);

    // Restoring onto an existing file is refused and keeps both.
    fs::write(&file, "new").unwrap();
    assert!(fileops::trash_restore(&[entries[0].id.clone()]).is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), "new");
    fs::remove_file(&file).unwrap();
    fileops::trash_restore(&[entries[0].id.clone()]).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "v1");
    assert!(fileops::trash_list().unwrap().is_empty());

    // "Replace" moves the existing target to the trash instead of deleting it.
    let dest = work.path().join("dest");
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("Bericht.pdf"), "old").unwrap();
    fileops::transfer(Transfer::Copy, std::slice::from_ref(&file), &dest, Conflict::Replace, |_, _| {}).unwrap();
    assert_eq!(fs::read_to_string(dest.join("Bericht.pdf")).unwrap(), "v1");
    let entries = fileops::trash_list().unwrap();
    assert_eq!(entries.len(), 1);

    let ids: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
    fileops::trash_purge(&ids).unwrap();
    assert!(fileops::trash_list().unwrap().is_empty());
}
