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

use std::sync::{Arc, Mutex};

use atlas_core::{highlight, markdown, markdown_edit as me};

use crate::inner::Shared;
use crate::{AtlasError, Result};

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
    /// Template with `{file}`, `{line}`, `{column}`, `{dir}`; empty: `$VISUAL`/`$EDITOR`.
    pub command: String,
    pub in_terminal: bool,
}

/// How to open a file in the external editor; the UI does it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ExternalLaunch {
    /// Open with the system's default application (NSWorkspace).
    System,
    /// Start this program detached (own session).
    Detached { argv: Vec<String> },
    /// Run this program in an Atlas terminal.
    Terminal { argv: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub slug: String,
    /// 1-based.
    pub line: u32,
}

/// Preview of a Markdown text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RenderedMarkdown {
    /// Body for the preview page (`crates/atlas-app/web/preview.*`).
    pub html: String,
    pub title: Option<String>,
    /// Tags from the front matter, as written.
    pub tags: Vec<String>,
    pub headings: Vec<Heading>,
    /// Links whose target does not exist.
    pub broken_links: u32,
}

/// Incremental highlighting of one document (`atlas_core::highlight::Session`):
/// remembers the parser state per line, so a keystroke re-highlights from
/// the changed line on (also across Markdown code blocks).
#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Highlighter {
    session: Mutex<highlight::Session>,
}

fn kind(k: highlight::Kind) -> HighlightKind {
    use highlight::Kind as K;
    match k {
        K::Comment => HighlightKind::Comment,
        K::String => HighlightKind::String,
        K::Escape => HighlightKind::Escape,
        K::Number => HighlightKind::Number,
        K::Constant => HighlightKind::Constant,
        K::Keyword => HighlightKind::Keyword,
        K::Operator => HighlightKind::Operator,
        K::Function => HighlightKind::Function,
        K::Type => HighlightKind::Type,
        K::Variable => HighlightKind::Variable,
        K::Tag => HighlightKind::Tag,
        K::Attribute => HighlightKind::Attribute,
        K::Punctuation => HighlightKind::Punctuation,
        K::Heading => HighlightKind::Heading,
        K::Emphasis => HighlightKind::Emphasis,
        K::Strong => HighlightKind::Strong,
        K::Link => HighlightKind::Link,
        K::Code => HighlightKind::Code,
        K::Quote => HighlightKind::Quote,
        K::List => HighlightKind::List,
        K::Math => HighlightKind::Math,
    }
}

fn span(s: &highlight::Span) -> Span {
    Span { start: s.start as u32, end: s.end as u32, kind: kind(s.kind) }
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Highlighter {
    /// The language is known (otherwise everything is plain text).
    pub fn known(&self) -> bool {
        self.session.lock().unwrap().known()
    }

    /// Spans of the whole `text` after a change.
    pub fn update(&self, text: String) -> Vec<Span> {
        self.session.lock().unwrap().update(&text).iter().map(span).collect()
    }
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Editor {
    shared: Arc<Shared>,
}

impl Editor {
    pub(crate) fn new(shared: Arc<Shared>) -> Self {
        Self { shared }
    }
}

fn edit(e: me::Edit) -> Edit {
    Edit { start: e.start as u32, end: e.end as u32, text: e.text, anchor: e.anchor as u32, cursor: e.cursor as u32 }
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Editor {
    /// Language of a file (syntect name like `Rust`, `MATLAB`; opaque for
    /// the UI, passed back to [`Editor::highlighter`]), `None`: plain text.
    pub fn language(&self, path: String) -> Option<String> {
        highlight::language_for_path(&crate::util::path(&path))
    }

    /// Language of a code block's info string (```` ```py ````).
    pub fn language_for_fence(&self, info: String) -> Option<String> {
        highlight::language_for_fence(&info)
    }

    pub fn highlighter(&self, language: String) -> Arc<Highlighter> {
        Arc::new(Highlighter { session: Mutex::new(highlight::Session::new(&language)) })
    }

    // --- Markdown editing (P1.4); `None`: let the editor handle the key ---

    /// Return in a list: continue it (or end it on an empty item).
    pub fn continue_list(&self, text: String, position: u32) -> Option<Edit> {
        me::continue_list(&text, position as usize).map(edit)
    }

    /// Tab / Shift-Tab on list items (all lines of the selection).
    pub fn indent_list_items(&self, text: String, anchor: u32, cursor: u32, outdent: bool) -> Option<Edit> {
        me::indent_list_items(&text, anchor as usize, cursor as usize, outdent).map(edit)
    }

    /// Tab / Shift-Tab in a table: next / previous cell (aligns the table).
    pub fn table_tab(&self, text: String, position: u32, back: bool) -> Option<Edit> {
        me::table_tab(&text, position as usize, back).map(edit)
    }

    /// Return in a table: new row below.
    pub fn table_enter(&self, text: String, position: u32) -> Option<Edit> {
        me::table_enter(&text, position as usize).map(edit)
    }

    /// Aligns the table at `position` (when the cursor leaves it), keeping
    /// `cursor` in its cell.
    pub fn align_table_at(&self, text: String, position: u32, cursor: u32) -> Option<Edit> {
        me::align_table_at(&text, position as usize, cursor as usize).map(edit)
    }

    /// Aligns all tables (menu „Tabellen ausrichten“); apply in order.
    pub fn align_tables(&self, text: String, cursor: u32) -> Vec<Edit> {
        me::align_tables(&text, cursor as usize).into_iter().map(edit).collect()
    }

    /// An aligned table as text (inserting a table).
    pub fn format_table(&self, markdown: String) -> String {
        me::align_table(&markdown)
    }

    /// Checks or unchecks the task on `line` (1-based; checkbox in the preview).
    pub fn toggle_task(&self, text: String, line: u32, done: bool) -> Option<Edit> {
        me::toggle_task(&text, line as usize, done).map(edit)
    }

    // --- completion (P1.4) ---

    /// What is being completed at `position`, `None` if nothing.
    pub fn completion_at(&self, text: String, position: u32) -> Option<CompletionContext> {
        let c = me::completion_at(&text, position as usize);
        if let Some(typed) = c.wiki {
            return Some(CompletionContext { kind: CompletionKind::WikiLink, typed, start: c.wiki_start as u32 });
        }
        c.tag.map(|typed| CompletionContext { kind: CompletionKind::Tag, typed, start: c.tag_start as u32 })
    }

    /// Candidates: notes (quick open) for wiki links, the project's tags by
    /// frequency for `#`.
    pub fn completions(&self, context: CompletionContext, limit: u32) -> Vec<Completion> {
        let limit = limit.max(1) as usize;
        match context.kind {
            CompletionKind::Tag => {
                let tags: Vec<me::Tag> = self
                    .shared
                    .notes
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map(|n| n.tags())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|t| me::Tag { key: t.key, display: t.display, count: t.count })
                    .collect();
                me::tag_suggestions(&tags, &context.typed.to_lowercase())
                    .into_iter()
                    .take(limit)
                    .map(|t| Completion { value: t.display.clone(), label: t.display, detail: t.count.to_string() })
                    .collect()
            }
            CompletionKind::WikiLink => {
                let files = self.shared.files.read().unwrap().clone();
                let notes: Vec<atlas_core::finder::Entry> = files
                    .iter()
                    .filter(|e| !e.is_dir && atlas_core::notes::is_markdown(std::path::Path::new(&e.relative)))
                    .cloned()
                    .collect();
                atlas_core::finder::best(&notes, &context.typed, limit)
                    .into_iter()
                    .map(|(e, _)| {
                        let label = std::path::Path::new(&e.relative)
                            .file_stem()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        Completion { value: e.relative.clone(), label, detail: e.relative.clone() }
                    })
                    .collect()
            }
        }
    }

    /// Inserts `item` for the typed text up to `end` (closes `]]` unless
    /// already there; cursor after it).
    pub fn accept_completion(&self, text: String, context: CompletionContext, end: u32, item: Completion) -> Edit {
        let (start, end) = (context.start as usize, end as usize);
        edit(match context.kind {
            CompletionKind::Tag => me::accept_tag(&text, start, end, &item.value),
            CompletionKind::WikiLink => me::accept_wiki(&text, start, end, &item.value),
        })
    }

    // --- other ---

    pub fn symbols(&self, path: String, text: String) -> Vec<Symbol> {
        atlas_core::symbols::symbols(&crate::util::path(&path), &text)
            .into_iter()
            .map(|s| Symbol {
                name: s.name,
                kind: s.kind.to_owned(),
                line: s.line as u32,
                level: s.depth.min(255) as u8,
            })
            .collect()
    }

    /// The preview of `text` (links resolved against `path` and the project).
    pub fn render_markdown(&self, text: String, path: String) -> RenderedMarkdown {
        let r = markdown::render(&text, &crate::util::path(&path), Some(&self.shared.root));
        RenderedMarkdown {
            broken_links: r.links.iter().filter(|l| l.kind != markdown::LinkKind::Definition && l.is_broken()).count()
                as u32,
            html: r.html,
            title: r.title,
            tags: r.tags,
            headings: r
                .headings
                .into_iter()
                .map(|h| Heading { level: h.level, text: h.text, slug: h.slug, line: h.line as u32 })
                .collect(),
        }
    }

    /// A fenced code block of `text` (for „Als Code einfügen“).
    pub fn code_block(&self, text: String, language: String) -> String {
        markdown::code_block(&text, &language)
    }

    /// The tree of folder `path` as text (three levels, at most 300
    /// entries, hidden files and build folders left out), for „Ordnerbaum
    /// einfügen“; the preview draws it with lines.
    pub fn folder_tree(&self, path: String) -> String {
        atlas_core::text_tree::of_folder(std::path::Path::new(&path), 3, 300)
    }

    pub fn external_editor(&self) -> ExternalEditor {
        let editor = atlas_core::config::Config::load().editor;
        ExternalEditor { command: editor.command, in_terminal: editor.terminal.unwrap_or(false) }
    }

    pub fn set_external_editor(&self, editor: ExternalEditor) -> Result<()> {
        atlas_core::config::split_words(&editor.command)
            .map_err(|e| AtlasError::Invalid { message: format!("{e:#}") })?;
        let config = atlas_core::config::EditorConfig {
            command: editor.command,
            terminal: Some(editor.in_terminal),
            ..atlas_core::config::Config::load().editor
        };
        Ok(atlas_core::config::Config::save_editor(&config)?)
    }

    /// How to open `path` at `line` in the external editor.
    pub fn external_launch(&self, path: String, line: u32) -> Result<ExternalLaunch> {
        let config = atlas_core::config::Config::load().editor;
        Ok(match atlas_core::config::editor_launch(&config, &crate::util::path(&path), line, 0)? {
            atlas_core::config::EditorLaunch::System => ExternalLaunch::System,
            atlas_core::config::EditorLaunch::Detached(argv) => ExternalLaunch::Detached { argv },
            atlas_core::config::EditorLaunch::Terminal(argv) => ExternalLaunch::Terminal { argv },
        })
    }
}
