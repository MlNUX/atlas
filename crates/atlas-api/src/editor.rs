// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Editor logic for the Markdown and code editors (NSTextView on macOS):
//! syntax highlighting (P1.5, `atlas_core::highlight`), list, table and
//! task editing and completion (P1.4, `atlas_core::markdown_edit`),
//! symbols, Markdown preview HTML, external editor. The UI only draws,
//! decides *when* a key applies (read-only, Markdown mode) and applies the
//! returned [`Edit`]s. Template: `app/editing.rs`, `TextEditor.qml`,
//! `text_document.rs`.
//!
//! All positions are UTF-16 code units (like `NSString` and QML strings).

use std::sync::Arc;

use crate::{Result, todo_task};

/// Kind of a highlighted span (`atlas_core::highlight::Kind`); colours are the UI's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum HighlightKind {
    Comment,
    String,
    Escape,
    Number,
    Constant,
    Keyword,
    Operator,
    Function,
    Type,
    Variable,
    Tag,
    Attribute,
    Punctuation,
    Heading,
    Emphasis,
    Strong,
    Link,
    Code,
    Quote,
    List,
    Math,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Span {
    pub start: u32,
    pub end: u32,
    pub kind: HighlightKind,
}

/// A change to apply: replace `[start, end)` with `text`, then select from
/// `anchor` to `cursor` (the direction of the selection matters). Common
/// start and end are left out, so undo stays fine-grained.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Edit {
    pub start: u32,
    pub end: u32,
    pub text: String,
    pub anchor: u32,
    pub cursor: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum CompletionKind {
    /// `[[Notiz…` → note names (quick open).
    WikiLink,
    /// `#wi…` → the project's tags by frequency.
    Tag,
}

/// What is being completed at the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct CompletionContext {
    pub kind: CompletionKind,
    /// The text typed so far (after `[[` or `#`).
    pub typed: String,
    /// Where the typed text starts.
    pub start: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Completion {
    /// Note: project-relative path; tag: the tag as written.
    pub value: String,
    pub label: String,
    /// Project-relative path or the tag's count, for display.
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

/// Incremental highlighting of one document (`atlas_core::highlight::Session`):
/// remembers the parser state per line, so a keystroke re-highlights from
/// the changed line on (also across Markdown code blocks).
#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Highlighter {}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Highlighter {
    /// The language is known (otherwise everything is plain text).
    pub fn known(&self) -> bool {
        todo_task!("P2.8")
    }

    /// Spans of the whole `text` after a change.
    pub fn update(&self, text: String) -> Vec<Span> {
        let _ = text;
        todo_task!("P2.8")
    }
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Editor {}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Editor {
    /// Language of a file (syntect name like `Rust`, `MATLAB`; opaque for
    /// the UI, passed back to [`Editor::highlighter`]), `None`: plain text.
    pub fn language(&self, path: String) -> Option<String> {
        let _ = path;
        todo_task!("P2.8")
    }

    /// Language of a code block's info string (```` ```py ````).
    pub fn language_for_fence(&self, info: String) -> Option<String> {
        let _ = info;
        todo_task!("P2.8")
    }

    pub fn highlighter(&self, language: String) -> Arc<Highlighter> {
        let _ = language;
        todo_task!("P2.8")
    }

    // --- Markdown editing (P1.4); `None`: let the editor handle the key ---

    /// Return in a list: continue it (or end it on an empty item).
    pub fn continue_list(&self, text: String, position: u32) -> Option<Edit> {
        let _ = (text, position);
        todo_task!("P2.8")
    }

    /// Tab / Shift-Tab on list items (all lines of the selection).
    pub fn indent_list_items(&self, text: String, anchor: u32, cursor: u32, outdent: bool) -> Option<Edit> {
        let _ = (text, anchor, cursor, outdent);
        todo_task!("P2.8")
    }

    /// Tab / Shift-Tab in a table: next / previous cell (aligns the table).
    pub fn table_tab(&self, text: String, position: u32, back: bool) -> Option<Edit> {
        let _ = (text, position, back);
        todo_task!("P2.8")
    }

    /// Return in a table: new row below.
    pub fn table_enter(&self, text: String, position: u32) -> Option<Edit> {
        let _ = (text, position);
        todo_task!("P2.8")
    }

    /// Aligns the table at `position` (when the cursor leaves it), keeping
    /// `cursor` in its cell.
    pub fn align_table_at(&self, text: String, position: u32, cursor: u32) -> Option<Edit> {
        let _ = (text, position, cursor);
        todo_task!("P2.8")
    }

    /// Aligns all tables (menu „Tabellen ausrichten“); apply in order.
    pub fn align_tables(&self, text: String, cursor: u32) -> Vec<Edit> {
        let _ = (text, cursor);
        todo_task!("P2.8")
    }

    /// An aligned table as text (inserting a table).
    pub fn format_table(&self, markdown: String) -> String {
        let _ = markdown;
        todo_task!("P2.8")
    }

    /// Checks or unchecks the task on `line` (1-based; checkbox in the preview).
    pub fn toggle_task(&self, text: String, line: u32, done: bool) -> Option<Edit> {
        let _ = (text, line, done);
        todo_task!("P2.8")
    }

    // --- completion (P1.4) ---

    /// What is being completed at `position`, `None` if nothing.
    pub fn completion_at(&self, text: String, position: u32) -> Option<CompletionContext> {
        let _ = (text, position);
        todo_task!("P2.8")
    }

    /// Candidates: notes (quick open) for wiki links, the project's tags by
    /// frequency for `#`.
    pub fn completions(&self, context: CompletionContext, limit: u32) -> Vec<Completion> {
        let _ = (context, limit);
        todo_task!("P2.8")
    }

    /// Inserts `item` for the typed text up to `end` (closes `]]` unless
    /// already there; cursor after it).
    pub fn accept_completion(&self, text: String, context: CompletionContext, end: u32, item: Completion) -> Edit {
        let _ = (text, context, end, item);
        todo_task!("P2.8")
    }

    // --- other ---

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
    /// with `in_terminal` the UI runs the returned command itself.
    pub fn open_in_external_editor(&self, path: String, line: u32) -> Result<Option<crate::actions::Prepared>> {
        let _ = (path, line);
        todo_task!("P2.8")
    }
}
