// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Files and folders of a project: listings for the tree and the folder
//! view, watching, file operations, trash, properties, quick open.
//! Template: `file_ops.rs`, `tree_model.rs`, `folder_model.rs`,
//! `trash_model.rs` and the file functions of `app.rs`.

use crate::{Result, todo_task};

/// One entry of a folder.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub is_hidden: bool,
    pub is_symlink: bool,
    /// Bytes (files only).
    pub size: Option<u64>,
    /// Unix seconds.
    pub modified: i64,
    /// Atlas file kind (`markdown`, `pdf`, `image`, `code`, `folder`, …), drives icons and viewers.
    pub kind: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SortKey {
    Name,
    Size,
    Modified,
    Kind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ListOptions {
    pub show_hidden: bool,
    pub folders_only: bool,
    pub sort: SortKey,
    pub ascending: bool,
}

/// What a copy or move does when the target name exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ConflictPolicy {
    /// Stop with an error before touching anything.
    Abort,
    Skip,
    Replace,
    /// Keep both: `Name (2).ext`.
    KeepBoth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Transfer {
    Copy,
    Move,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TrashItem {
    /// Opaque id for restore and purge.
    pub id: String,
    pub name: String,
    /// Empty when unknown (macOS).
    pub original_path: String,
    /// Unix seconds.
    pub deleted: i64,
    pub is_dir: bool,
    pub size: Option<u64>,
}

/// Owner, group and mode of a file (properties dialog).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FileProperties {
    pub path: String,
    pub kind: String,
    pub size: Option<u64>,
    pub created: Option<i64>,
    pub modified: i64,
    pub accessed: Option<i64>,
    pub owner: String,
    pub group: String,
    pub gid: u32,
    /// Unix permission bits (`0o644`).
    pub mode: u32,
    pub can_change_mode: bool,
    pub can_change_owner: bool,
    pub symlink_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Group {
    pub gid: u32,
    pub name: String,
}

/// Size of a folder tree.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TreeSize {
    pub bytes: u64,
    pub files: u32,
    pub dirs: u32,
}

/// A quick-open (⌘P) hit.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct QuickOpenHit {
    pub path: String,
    /// Project-relative, for display.
    pub relative: String,
    pub score: i32,
    /// Line from `datei.md:42`, 0 if none.
    pub line: u32,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Files {}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Files {
    /// Entries of `dir`, sorted (natural sort if switched on in the settings).
    pub fn list(&self, dir: String, options: ListOptions) -> Result<Vec<FileEntry>> {
        let _ = (dir, options);
        todo_task!("P2.2")
    }

    /// Watches these folders (replaces the previous set): changes arrive as
    /// `Event::FilesChanged` / `Event::FileModified`.
    pub fn watch(&self, dirs: Vec<String>) {
        let _ = dirs;
        todo_task!("P2.2")
    }

    pub fn exists(&self, path: String) -> bool {
        crate::util::path(&path).exists()
    }

    pub fn is_dir(&self, path: String) -> bool {
        crate::util::path(&path).is_dir()
    }

    pub fn file_kind(&self, path: String) -> String {
        let _ = path;
        todo_task!("P2.2")
    }

    /// UTF-8 text of a file (lossy), at most `max_bytes`.
    pub fn read_text(&self, path: String, max_bytes: u64) -> Result<String> {
        let _ = (path, max_bytes);
        todo_task!("P2.2")
    }

    /// Writes atomically (temp file + rename), keeps mode and line endings.
    pub fn write_text(&self, path: String, text: String) -> Result<()> {
        let _ = (path, text);
        todo_task!("P2.2")
    }

    /// Error text if `name` is not a valid file name, else `None`.
    pub fn check_name(&self, name: String) -> Option<String> {
        let _ = name;
        todo_task!("P2.2")
    }

    /// Creates an empty file; returns its path.
    pub fn new_file(&self, dir: String, name: String) -> Result<String> {
        let _ = (dir, name);
        todo_task!("P2.2")
    }

    pub fn new_folder(&self, dir: String, name: String) -> Result<String> {
        let _ = (dir, name);
        todo_task!("P2.2")
    }

    /// Renames in place; links, annotations, relations and calendar links follow.
    pub fn rename(&self, path: String, new_name: String) -> Result<String> {
        let _ = (path, new_name);
        todo_task!("P2.2")
    }

    /// Names in `dest` that `paths` would collide with.
    pub fn conflicts(&self, paths: Vec<String>, dest: String) -> Vec<String> {
        let _ = (paths, dest);
        todo_task!("P2.2")
    }

    /// Copies or moves in the background; progress and end arrive as
    /// `Event::FileOperation` with the returned id.
    pub fn transfer(&self, kind: Transfer, paths: Vec<String>, dest: String, policy: ConflictPolicy) -> u64 {
        let _ = (kind, paths, dest, policy);
        todo_task!("P2.2")
    }

    /// Moves to the system trash (background, `Event::FileOperation`).
    pub fn move_to_trash(&self, paths: Vec<String>) -> u64 {
        let _ = paths;
        todo_task!("P2.2")
    }

    pub fn trash_items(&self) -> Result<Vec<TrashItem>> {
        todo_task!("P2.2")
    }

    pub fn restore_from_trash(&self, ids: Vec<String>) -> Result<()> {
        let _ = ids;
        todo_task!("P2.2")
    }

    pub fn delete_from_trash(&self, ids: Vec<String>) -> Result<()> {
        let _ = ids;
        todo_task!("P2.2")
    }

    pub fn properties(&self, path: String) -> Result<FileProperties> {
        let _ = path;
        todo_task!("P2.2")
    }

    pub fn own_groups(&self) -> Vec<Group> {
        todo_task!("P2.2")
    }

    pub fn set_mode(&self, path: String, mode: u32) -> Result<()> {
        let _ = (path, mode);
        todo_task!("P2.2")
    }

    pub fn set_group(&self, path: String, gid: u32) -> Result<()> {
        let _ = (path, gid);
        todo_task!("P2.2")
    }

    pub async fn tree_size(&self, path: String) -> Result<TreeSize> {
        let _ = path;
        todo_task!("P2.2")
    }

    /// Fuzzy file search over the project (⌘P); `query` may end in `:line`.
    pub fn quick_open(&self, query: String, limit: u32) -> Vec<QuickOpenHit> {
        let _ = (query, limit);
        todo_task!("P2.2")
    }

    /// Re-reads the file list used by quick open (after big changes).
    pub fn refresh_file_list(&self) {
        todo_task!("P2.2")
    }
}
