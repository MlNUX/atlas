// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Settings (`config.toml`, shared by all windows), the context bar
//! profiles (logic from P1.3, `atlas_core::context_profiles`) and spell
//! checking (`atlas-spell`). Template: `app/settings.rs`, `spelling.rs`,
//! `SettingsDialog.qml`.

use crate::{Result, todo_task};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Theme {
    /// Follow the system (the only sensible choice on macOS, E8).
    System,
    Light,
    Dark,
}

/// The settings the UI shows and changes. Sections of their own
/// (calendar account, ILIAS, extensions) live with their services.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct AppSettings {
    pub theme: Theme,
    pub pdf_dark: bool,
    pub natural_sort: bool,
    pub spellcheck: bool,
    /// Spell checking languages (`de_DE`, `en_US`).
    pub spell_languages: Vec<String>,
    pub file_manager_places: bool,
    /// External editor command (`code -g {file}:{line}`), empty: none.
    pub editor_command: String,
    pub editor_in_terminal: bool,
    /// Shell for terminals, empty: login shell.
    pub shell: String,
    /// Further `config.toml` values the UI does not model, as TOML text of
    /// their tables (kept on save).
    pub other: String,
}

/// Remembered sort order of a folder view.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FolderSort {
    pub key: crate::files::SortKey,
    pub ascending: bool,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Settings {}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Settings {
    pub fn get(&self) -> AppSettings {
        todo_task!("P2.8")
    }

    /// Saves (atomically, keeping comments and unknown keys); other
    /// windows get `Event::SettingsChanged`.
    pub fn save(&self, settings: AppSettings) -> Result<()> {
        let _ = settings;
        todo_task!("P2.8")
    }

    /// Path of `config.toml` (for „In Editor öffnen“).
    pub fn file(&self) -> String {
        crate::util::string(&atlas_core::config::config_file())
    }

    /// Sort order for a folder (`key`: path or `*` for the default).
    pub fn folder_sort(&self, key: String) -> FolderSort {
        let _ = key;
        todo_task!("P2.8")
    }

    pub fn set_folder_sort(&self, key: String, sort: FolderSort) {
        let _ = (key, sort);
        todo_task!("P2.8")
    }

    /// The spell checker for the current languages.
    pub fn spelling(&self) -> std::sync::Arc<Spelling> {
        todo_task!("P2.8")
    }
}

// --- context bar ---

/// A building block of the context bar (display names belong to the UI).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ContextBlock {
    pub id: String,
    pub icon: String,
    /// Can be a tab of its own.
    pub tab: bool,
    /// Can be pinned to the bottom edge.
    pub pin: bool,
    /// `file`, `plan` or `ext`.
    pub group: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ContextPage {
    pub id: String,
    /// Empty when the UI supplies it (built-in tabs, single blocks).
    pub title: String,
    pub blocks: Vec<String>,
    /// A block filling the whole page (`links`, `graph`).
    pub full: bool,
}

/// How a profile lays out the bar for one file.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ContextLayout {
    pub profile: String,
    /// Tab ids after „Übersicht“, in order.
    pub tabs: Vec<String>,
    pub pages: Vec<ContextPage>,
    pub overview: Vec<String>,
    pub pinned: Vec<String>,
    pub collapsed: Vec<String>,
    pub hidden: Vec<String>,
}

/// Areas a block can be moved between in the edit mode.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ContextArea {
    Tabs,
    Overview,
    Pinned,
    Hidden,
    /// A page (`page:…` or a built-in tab id).
    Page {
        id: String,
    },
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Context {}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Context {
    pub fn catalog(&self) -> Vec<ContextBlock> {
        todo_task!("P2.8")
    }

    /// Profile names (`default`, `pdf`, `markdown`, `code`, `calendar`).
    pub fn profiles(&self) -> Vec<String> {
        todo_task!("P2.8")
    }

    /// The profile that applies to a tab of this kind (`pdf`, `markdown`, …).
    pub fn profile_for(&self, kind: String) -> String {
        let _ = kind;
        todo_task!("P2.8")
    }

    pub fn layout(&self, profile: String) -> ContextLayout {
        let _ = profile;
        todo_task!("P2.8")
    }

    /// Edit mode: moves `block` from one area to another (at the end).
    pub fn move_block(
        &self,
        profile: String,
        block: String,
        from: ContextArea,
        to: ContextArea,
    ) -> Result<ContextLayout> {
        let _ = (profile, block, from, to);
        todo_task!("P2.8")
    }

    /// Moves `block` within its area by `delta` places.
    pub fn move_by(&self, profile: String, area: ContextArea, block: String, delta: i64) -> Result<ContextLayout> {
        let _ = (profile, area, block, delta);
        todo_task!("P2.8")
    }

    /// New page of one's own with `title`; returns its id.
    pub fn new_page(&self, profile: String, title: String) -> Result<String> {
        let _ = (profile, title);
        todo_task!("P2.8")
    }

    pub fn set_collapsed(&self, profile: String, block: String, collapsed: bool) -> Result<()> {
        let _ = (profile, block, collapsed);
        todo_task!("P2.8")
    }

    /// Back to the suggestion for this profile.
    pub fn reset(&self, profile: String) -> Result<ContextLayout> {
        let _ = profile;
        todo_task!("P2.8")
    }
}

// --- spelling ---

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Misspelling {
    /// Byte offsets in the checked text.
    pub start: u32,
    pub end: u32,
    pub word: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SpellLanguage {
    pub code: String,
    pub name: String,
    pub available: bool,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Spelling {}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Spelling {
    /// Misspelled words of `text` (Markdown aware: code, links and math are skipped).
    pub fn check(&self, text: String) -> Vec<Misspelling> {
        let _ = text;
        todo_task!("P2.8")
    }

    pub fn suggest(&self, word: String) -> Vec<String> {
        let _ = word;
        todo_task!("P2.8")
    }

    /// Adds to the personal dictionary.
    pub fn add(&self, word: String) {
        let _ = word;
        todo_task!("P2.8")
    }

    pub fn languages(&self) -> Vec<SpellLanguage> {
        todo_task!("P2.8")
    }
}
