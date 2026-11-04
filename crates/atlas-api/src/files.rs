// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Files and folders of a project: listings for the tree and the folder
//! view, watching, file operations, trash, properties, quick open.
//! Template: `file_ops.rs`, `tree_model.rs`, `folder_model.rs`,
//! `trash_model.rs` and the file functions of `app.rs`.

use std::path::PathBuf;
use std::sync::Arc;

use atlas_core::{fileops, finder, listing, perms};

use crate::events::Event;
use crate::inner::Shared;
use crate::util::{path, string};
use crate::{AtlasError, Result};

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
    /// Human readable type (`PDF-Dokument`, `Ordner`, …) for the folder view;
    /// the viewer kind comes from [`Files::file_kind`].
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
pub struct Files {
    shared: Arc<Shared>,
}

impl Files {
    pub(crate) fn new(shared: Arc<Shared>) -> Self {
        Self { shared }
    }
}

fn entry(info: listing::FileInfo) -> FileEntry {
    FileEntry {
        is_hidden: info.is_hidden(),
        path: string(&info.path),
        name: info.name,
        is_dir: info.is_dir,
        is_symlink: info.is_symlink,
        size: (!info.is_dir).then_some(info.size),
        modified: info.modified_ms / 1000,
        kind: info.kind,
    }
}

fn sort_key(key: SortKey) -> listing::SortKey {
    match key {
        SortKey::Name => listing::SortKey::Name,
        SortKey::Size => listing::SortKey::Size,
        SortKey::Modified => listing::SortKey::Modified,
        SortKey::Kind => listing::SortKey::Kind,
    }
}

fn paths(list: &[String]) -> Vec<PathBuf> {
    list.iter().map(|p| path(p)).collect()
}

fn invalid(err: anyhow::Error) -> AtlasError {
    AtlasError::Invalid { message: format!("{err:#}") }
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Files {
    /// Entries of `dir`, sorted (natural sort if switched on in the settings).
    pub fn list(&self, dir: String, options: ListOptions) -> Result<Vec<FileEntry>> {
        let mut entries = listing::list(&path(&dir), options.show_hidden)?;
        if options.folders_only {
            entries.retain(|e| e.is_dir);
        }
        listing::sort(&mut entries, sort_key(options.sort), options.ascending);
        Ok(entries.into_iter().map(entry).collect())
    }

    /// Watches these folders (replaces the previous set): changes arrive as
    /// `Event::FilesChanged`.
    pub fn watch(&self, dirs: Vec<String>) {
        let mut watcher = self.shared.watcher.lock().unwrap();
        if watcher.is_none() {
            let weak = Arc::downgrade(&self.shared);
            match atlas_core::watcher::DirWatcher::new(move |changed| {
                if let Some(shared) = weak.upgrade() {
                    shared.emit(Event::FilesChanged { dirs: changed.iter().map(|d| string(d)).collect() });
                }
            }) {
                Ok(w) => *watcher = Some(w),
                Err(err) => {
                    log::warn!("file watcher unavailable: {err:#}");
                    return;
                }
            }
        }
        if let Some(w) = watcher.as_ref() {
            w.sync(&paths(&dirs));
        }
    }

    pub fn exists(&self, path: String) -> bool {
        crate::util::path(&path).exists()
    }

    pub fn is_dir(&self, path: String) -> bool {
        crate::util::path(&path).is_dir()
    }

    /// Viewer kind: `pdf`, `markdown`, `image`, `atlasnote`, `text`,
    /// `binary`, `dir`, `trash`, `missing`, or for tabs that are no files
    /// `terminal`, `calendar`, `ilias`.
    pub fn file_kind(&self, path: String) -> String {
        listing::viewer_kind(&crate::util::path(&path)).to_owned()
    }

    /// UTF-8 text of a file (lossy), at most `max_bytes`.
    pub fn read_text(&self, path: String, max_bytes: u64) -> Result<String> {
        use std::io::Read;
        let mut data = Vec::new();
        std::fs::File::open(crate::util::path(&path))?.take(max_bytes).read_to_end(&mut data)?;
        Ok(String::from_utf8_lossy(&data).into_owned())
    }

    /// Writes atomically (temp file + rename).
    pub fn write_text(&self, path: String, text: String) -> Result<()> {
        Ok(atlas_core::paths::write_atomic(&crate::util::path(&path), text.as_bytes())?)
    }

    /// Error text if `name` is not a valid file name, else `None`.
    pub fn check_name(&self, name: String) -> Option<String> {
        fileops::validate_name(&name).err().map(|e| format!("{e:#}"))
    }

    /// Creates an empty file; returns its path.
    pub fn new_file(&self, dir: String, name: String) -> Result<String> {
        fileops::create_file(&path(&dir), &name).map(|p| string(&p)).map_err(invalid)
    }

    pub fn new_folder(&self, dir: String, name: String) -> Result<String> {
        fileops::create_dir(&path(&dir), &name).map(|p| string(&p)).map_err(invalid)
    }

    /// Renames in place; returns the new path. Links, annotations and
    /// relations follow through `Notes::files_moved` (the UI calls it).
    pub fn rename(&self, path: String, new_name: String) -> Result<String> {
        fileops::rename(&crate::util::path(&path), &new_name).map(|p| string(&p)).map_err(invalid)
    }

    /// Names in `dest` that `paths` would collide with.
    pub fn conflicts(&self, paths: Vec<String>, dest: String) -> Vec<String> {
        fileops::conflicts(&self::paths(&paths), &path(&dest)).iter().map(|p| string(p)).collect()
    }

    /// Copies or moves in the background; progress and end arrive as
    /// `Event::FileOperation` with the returned id. When done, `text` of
    /// the last event lists `from → to` per moved item (one per line).
    pub fn transfer(&self, kind: Transfer, paths: Vec<String>, dest: String, policy: ConflictPolicy) -> u64 {
        let id = self.shared.next_id();
        let shared = self.shared.clone();
        let (sources, dest) = (self::paths(&paths), path(&dest));
        std::thread::spawn(move || {
            let kind = match kind {
                Transfer::Copy => fileops::Transfer::Copy,
                Transfer::Move => fileops::Transfer::Move,
            };
            let policy = match policy {
                ConflictPolicy::Abort => fileops::Conflict::Abort,
                ConflictPolicy::Skip => fileops::Conflict::Skip,
                ConflictPolicy::Replace => fileops::Conflict::Replace,
                ConflictPolicy::KeepBoth => fileops::Conflict::KeepBoth,
            };
            let result = fileops::transfer(kind, &sources, &dest, policy, |done, total| {
                shared.emit(Event::FileOperation {
                    operation_id: id,
                    done: false,
                    text: format!("{done} von {total}"),
                    error: None,
                });
            });
            let (text, error) = match result {
                Ok(moved) => (
                    moved
                        .iter()
                        .map(|m| format!("{} → {}", m.from.display(), m.to.display()))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    None,
                ),
                Err(err) => (String::new(), Some(format!("{err:#}"))),
            };
            shared.emit(Event::FileOperation { operation_id: id, done: true, text, error });
        });
        id
    }

    /// Moves to the system trash (background, `Event::FileOperation`).
    pub fn move_to_trash(&self, paths: Vec<String>) -> u64 {
        let id = self.shared.next_id();
        let shared = self.shared.clone();
        let sources = self::paths(&paths);
        std::thread::spawn(move || {
            let error = fileops::trash(&sources).err().map(|e| format!("{e:#}"));
            let text = sources.iter().map(|p| string(p)).collect::<Vec<_>>().join("\n");
            shared.emit(Event::FileOperation { operation_id: id, done: true, text, error });
        });
        id
    }

    pub fn trash_items(&self) -> Result<Vec<TrashItem>> {
        Ok(fileops::trash_list()?
            .into_iter()
            .map(|e| TrashItem {
                id: e.id,
                name: e.name,
                original_path: string(&e.original_path),
                deleted: e.deleted_unix,
                is_dir: e.is_dir,
                size: e.size,
            })
            .collect())
    }

    pub fn restore_from_trash(&self, ids: Vec<String>) -> Result<()> {
        fileops::trash_restore(&ids).map_err(|e| AtlasError::Conflict { message: format!("{e:#}") })
    }

    pub fn delete_from_trash(&self, ids: Vec<String>) -> Result<()> {
        Ok(fileops::trash_purge(&ids)?)
    }

    pub fn properties(&self, path: String) -> Result<FileProperties> {
        let p = crate::util::path(&path);
        let info = perms::stat(&p)?;
        let created = std::fs::symlink_metadata(&p)
            .and_then(|m| m.created())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);
        Ok(FileProperties {
            kind: listing::kind_of(&p.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(), info.is_dir),
            path,
            size: (!info.is_dir).then_some(info.size),
            created,
            modified: info.modified,
            accessed: Some(info.accessed),
            can_change_mode: perms::can_change_mode(&info),
            can_change_owner: perms::can_change_owner(),
            owner: info.owner,
            group: info.group,
            gid: info.gid,
            mode: info.mode,
            symlink_target: info.link_target.as_deref().map(string),
        })
    }

    pub fn own_groups(&self) -> Vec<Group> {
        perms::own_groups().into_iter().map(|(gid, name)| Group { gid, name }).collect()
    }

    pub fn set_mode(&self, path: String, mode: u32) -> Result<()> {
        Ok(perms::set_mode(&crate::util::path(&path), mode)?)
    }

    pub fn set_group(&self, path: String, gid: u32) -> Result<()> {
        Ok(perms::set_owner(&crate::util::path(&path), None, Some(gid))?)
    }

    pub async fn tree_size(&self, path: String) -> Result<TreeSize> {
        let root = crate::util::path(&path);
        crate::util::background(move || {
            let mut size = TreeSize { bytes: 0, files: 0, dirs: 0 };
            for entry in ignore::WalkBuilder::new(&root).standard_filters(false).build().flatten().skip(1) {
                match entry.metadata() {
                    Ok(m) if m.is_dir() => size.dirs += 1,
                    Ok(m) => {
                        size.files += 1;
                        size.bytes += m.len();
                    }
                    Err(_) => {}
                }
            }
            Ok(size)
        })
        .await
    }

    /// Fuzzy file search over the project (⌘P); `query` may end in `:line`.
    pub fn quick_open(&self, query: String, limit: u32) -> Vec<QuickOpenHit> {
        let files = self.shared.files.read().unwrap().clone();
        let (query, line) = finder::split_location(query.trim());
        finder::best(&files, query, limit.max(1) as usize)
            .into_iter()
            .map(|(e, score)| QuickOpenHit {
                path: string(&finder::absolute(&self.shared.root, e)),
                relative: e.relative.clone(),
                score,
                line,
            })
            .collect()
    }

    /// Re-reads the file list used by quick open (after big changes).
    pub fn refresh_file_list(&self) {
        let weak = Arc::downgrade(&self.shared);
        let root = self.shared.root.clone();
        std::thread::spawn(move || {
            let files = Arc::new(finder::list(&root));
            if let Some(shared) = weak.upgrade() {
                *shared.files.write().unwrap() = files;
            }
        });
    }

    /// Free and total bytes of the file system holding `path`.
    pub fn disk_space(&self, path: String) -> Option<DiskSpace> {
        use std::os::unix::ffi::OsStrExt;
        let c = std::ffi::CString::new(crate::util::path(&path).as_os_str().as_bytes()).ok()?;
        // SAFETY: `c` is a valid C string and `st` a writable statvfs.
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
            return None;
        }
        let block = st.f_frsize as u64;
        Some(DiskSpace { free: st.f_bavail as u64 * block, total: st.f_blocks as u64 * block })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct DiskSpace {
    pub free: u64,
    pub total: u64,
}
