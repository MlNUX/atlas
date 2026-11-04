// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Project-wide text search (files with `rg`, PDFs from page text) and the
//! annotation search. Template: `search_model.rs`.

use crate::{Result, todo_task};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SearchQuery {
    pub pattern: String,
    pub regex: bool,
    /// `None`: smart case (sensitive only with capitals).
    pub case_sensitive: Option<bool>,
    pub whole_word: bool,
    /// Search text files.
    pub text: bool,
    /// Search PDFs (page text, see `pdf::set_page_texts` in the core).
    pub pdf: bool,
    /// Glob filters (`*.md`, `!build/`).
    pub globs: Vec<String>,
    /// File kinds from the type filter (`markdown`, `code`, …); empty = all.
    pub kinds: Vec<String>,
    /// Folder to search, default the project root.
    pub root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TextRange {
    /// Byte offsets in `SearchMatch::text`.
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SearchMatch {
    pub path: String,
    pub relative: String,
    /// Line (text) or page (PDF), 1-based.
    pub line: u64,
    /// The line, clipped around the match for long lines.
    pub text: String,
    pub ranges: Vec<TextRange>,
    pub pdf: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct KindCount {
    pub kind: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SearchSummary {
    pub matches: u32,
    pub files: u32,
    pub elapsed_ms: u32,
    /// Stopped at the match limit.
    pub truncated: bool,
    pub cancelled: bool,
    pub kinds: Vec<KindCount>,
    pub error: Option<String>,
}

/// Lines around a match, for the preview.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SearchContext {
    pub first_line: u64,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct AnnotationHit {
    pub pdf: String,
    pub page: u32,
    pub annotation_id: String,
    pub text: String,
    pub comment: String,
    pub color: String,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Search {}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Search {
    /// Starts a search (cancelling the previous one of this object); results
    /// stream as `Event::SearchMatches` and end with `Event::SearchFinished`,
    /// both with the returned id. Invalid patterns fail right away.
    pub fn start(&self, query: SearchQuery) -> Result<u64> {
        let _ = query;
        todo_task!("P2.4")
    }

    pub fn cancel(&self) {
        todo_task!("P2.4")
    }

    /// `radius` lines before and after `line` (PDF: the page's lines).
    pub fn context(&self, path: String, line: u64, radius: u32, pdf: bool) -> Result<SearchContext> {
        let _ = (path, line, radius, pdf);
        todo_task!("P2.4")
    }

    /// Searches text and comments of all annotations of the project.
    pub fn annotations(&self, query: String, limit: u32) -> Vec<AnnotationHit> {
        let _ = (query, limit);
        todo_task!("P2.4")
    }

    /// Fuzzy score of `text` for `query` (higher is better), `None` if no match.
    pub fn fuzzy_score(&self, query: String, text: String) -> Option<i32> {
        atlas_core::finder::score(&query, &text)
    }
}
