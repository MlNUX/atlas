// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Editor logic for the Markdown and code editors (NSTextView on macOS):
//! syntax highlighting (P1.5, `syntect`), list and table editing (P1.4),
//! wiki-link completion, symbols, Markdown preview HTML, external editor.
//! The UI only draws and forwards keys. Template: `app/editing.rs`,
//! `TextEditor.qml`, `text_document.rs`.
//!
//! Offsets are UTF-8 byte offsets into the text passed in; the UI converts
//! to UTF-16 for NSTextView.

use crate::{Result, todo_task};

/// A coloured span (highlighting).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Span {
    pub start: u32,
    pub end: u32,
    /// Style name from the Atlas palette (`keyword`, `string`, `comment`,
    /// `heading`, `emphasis`, `link`, `code`, …); colours are the UI's.
    pub style: String,
}

/// A change the editor applies: replace `[start, end)` with `text`, then
/// put the cursor (and selection) at `cursor_start..cursor_end`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Edit {
    pub start: u32,
    pub end: u32,
    pub text: String,
    pub cursor_start: u32,
    pub cursor_end: u32,
}

/// Keys the core handles in Markdown (everything else is the editor's).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum EditorKey {
    /// Return: continue lists and tables.
    Enter,
    /// Tab / Shift-Tab: indent list items, next/previous table cell.
    Tab,
    BackTab,
    /// Typing `|`: align the table.
    Pipe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Completion {
    /// Text to insert in place of the typed prefix.
    pub insert: String,
    pub label: String,
    /// Project-relative path for display.
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Symbol {
    pub name: String,
    /// `heading`, `function`, `class`, …
    pub kind: String,
    /// 1-based.
    pub line: u32,
    pub level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ExternalEditor {
    pub command: String,
    pub in_terminal: bool,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Editor {}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Editor {
    /// Language id for a file (`markdown`, `rust`, `python`, `matlab`,
    /// `latex`, …), `None` for plain text.
    pub fn language(&self, path: String) -> Option<String> {
        let _ = path;
        todo_task!("P2.8")
    }

    /// Highlighting of `text` in `language` (Markdown includes its code blocks).
    pub fn highlight(&self, text: String, language: String) -> Vec<Span> {
        let _ = (text, language);
        todo_task!("P2.8")
    }

    /// The core's reaction to `key` at the selection, `None`: let the editor
    /// handle it.
    pub fn key(&self, text: String, key: EditorKey, selection_start: u32, selection_end: u32) -> Option<Edit> {
        let _ = (text, key, selection_start, selection_end);
        todo_task!("P2.8")
    }

    /// Aligns the Markdown table around `offset`.
    pub fn align_table(&self, text: String, offset: u32) -> Option<Edit> {
        let _ = (text, offset);
        todo_task!("P2.8")
    }

    /// Completions for the `[[…` before `offset`.
    pub fn complete_wiki_link(&self, text: String, offset: u32, limit: u32) -> Vec<Completion> {
        let _ = (text, offset, limit);
        todo_task!("P2.8")
    }

    pub fn symbols(&self, path: String, text: String) -> Vec<Symbol> {
        let _ = (path, text);
        todo_task!("P2.8")
    }

    /// HTML for the preview page (`crates/atlas-app/web/preview.*`): links
    /// resolved against `path`, math and diagrams left to KaTeX/Mermaid.
    pub fn render_markdown(&self, text: String, path: String) -> String {
        let _ = (text, path);
        todo_task!("P2.8")
    }

    /// A fenced code block of `text` (for „Als Code einfügen“).
    pub fn code_block(&self, text: String, language: String) -> String {
        let _ = (text, language);
        todo_task!("P2.8")
    }

    pub fn external_editor(&self) -> ExternalEditor {
        todo_task!("P2.8")
    }

    pub fn set_external_editor(&self, editor: ExternalEditor) -> Result<()> {
        let _ = editor;
        todo_task!("P2.8")
    }

    /// Opens `path` at `line` in the external editor (outside a terminal);
    /// with `in_terminal` the UI runs [`crate::actions::Prepared`] itself.
    pub fn open_in_external_editor(&self, path: String, line: u32) -> Result<Option<crate::actions::Prepared>> {
        let _ = (path, line);
        todo_task!("P2.8")
    }
}
