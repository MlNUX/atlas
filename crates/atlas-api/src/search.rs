// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Project-wide text search (files with `rg`, PDFs from page text) and the
//! annotation search. Template: `search_model.rs`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use atlas_core::search as core;

use crate::events::Event;
use crate::inner::Shared;
use crate::util::{path, string};
use crate::{AtlasError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SearchQuery {
    pub pattern: String,
    pub regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    /// Glob filters (`*.md`, `!build/`).
    pub globs: Vec<String>,
    /// Also search build and dependency folders (`target/`, `node_modules/`, …).
    pub build_dirs: bool,
    /// Document types (`markdown`, `pdf`, `latex`, `code`, `text`); empty =
    /// all. PDFs are searched by their page text (see `PdfDocument::set_page_texts`).
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

/// A line around a match, for the preview.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ContextLine {
    /// 1-based (for PDFs: the line on the page).
    pub line: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct AnnotationHit {
    pub pdf: String,
    /// Project-relative key of the PDF.
    pub relative: String,
    pub page: u32,
    pub annotation_id: String,
    pub kind: crate::pdf::AnnotationKind,
    pub text: String,
    pub comment: String,
    pub title: String,
    pub color: String,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Search {
    shared: Arc<Shared>,
    running: Mutex<Option<core::SearchHandle>>,
}

impl Search {
    pub(crate) fn new(shared: Arc<Shared>) -> Self {
        Self { shared, running: Mutex::new(None) }
    }
}

fn to_match(m: core::Match) -> SearchMatch {
    SearchMatch {
        path: string(&m.path),
        relative: m.relative,
        line: m.line,
        text: m.text,
        ranges: m.ranges.into_iter().map(|(s, e)| TextRange { start: s as u32, end: e as u32 }).collect(),
        pdf: m.pdf,
    }
}

pub(crate) fn kind_of(kind: &atlas_core::annotations::Kind) -> crate::pdf::AnnotationKind {
    use crate::pdf::AnnotationKind as A;
    use atlas_core::annotations::Kind as K;
    match kind {
        K::Highlight => A::Highlight,
        K::Underline => A::Underline,
        K::Strikeout => A::Strikeout,
        K::Note => A::Note,
        K::Ink => A::Ink,
        K::Bookmark => A::Bookmark,
    }
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Search {
    /// Starts a search (cancelling the previous one of this object); results
    /// stream as `Event::SearchMatches` and end with `Event::SearchFinished`,
    /// both with the returned id. Invalid patterns fail right away.
    pub fn start(&self, query: SearchQuery) -> Result<u64> {
        self.cancel();
        if query.pattern.is_empty() {
            return Err(AtlasError::Invalid { message: "leeres Suchmuster".into() });
        }
        let options = core::Options {
            regex: query.regex,
            case_sensitive: query.case_sensitive,
            whole_word: query.whole_word,
            globs: query.globs,
            build_dirs: query.build_dirs,
            kinds: query.kinds,
        };
        let root: PathBuf = query.root.as_deref().map(path).unwrap_or_else(|| self.shared.root.clone());
        let id = self.shared.next_id();
        let counts: Arc<Mutex<(BTreeMap<&'static str, u32>, u32, Option<PathBuf>)>> = Arc::default();
        let (batch_shared, batch_counts) = (self.shared.clone(), counts.clone());
        let done_shared = self.shared.clone();
        let handle = core::start(
            &root,
            options.query(query.pattern),
            move |batch| {
                let mut c = batch_counts.lock().unwrap();
                for m in &batch {
                    *c.0.entry(core::kind_of(&m.path)).or_default() += 1;
                    if c.2.as_ref() != Some(&m.path) {
                        c.1 += 1;
                        c.2 = Some(m.path.clone());
                    }
                }
                drop(c);
                batch_shared
                    .emit(Event::SearchMatches { search_id: id, matches: batch.into_iter().map(to_match).collect() });
            },
            move |summary| {
                let c = counts.lock().unwrap();
                done_shared.emit(Event::SearchFinished {
                    search_id: id,
                    summary: SearchSummary {
                        matches: summary.matches as u32,
                        files: c.1,
                        elapsed_ms: summary.elapsed.as_millis() as u32,
                        truncated: summary.truncated,
                        cancelled: summary.cancelled,
                        kinds: c.0.iter().map(|(k, n)| KindCount { kind: (*k).to_owned(), count: *n }).collect(),
                        error: summary.error,
                    },
                });
            },
        )
        .map_err(|e| AtlasError::Invalid { message: format!("{e:#}") })?;
        *self.running.lock().unwrap() = Some(handle);
        Ok(id)
    }

    pub fn cancel(&self) {
        if let Some(handle) = self.running.lock().unwrap().take() {
            handle.cancel();
        }
    }

    /// `radius` lines before and after `line` (PDF: the lines of page `line`).
    pub fn context(&self, path: String, line: u64, radius: u32, pdf: bool) -> Vec<ContextLine> {
        let p = crate::util::path(&path);
        let lines = if pdf {
            core::pdf_context(&p, line as usize)
        } else {
            core::text_context(&p, line as usize, radius as usize)
        };
        lines.into_iter().map(|(line, text)| ContextLine { line: line as u64, text }).collect()
    }

    /// Searches text, comments and titles of all annotations of the project
    /// (fuzzy, best first; empty query: all).
    pub fn annotations(&self, query: String, limit: u32) -> Vec<AnnotationHit> {
        let dir = self.shared.root.join(".atlas").join(atlas_core::annotations::DIR);
        let store = atlas_core::annotations::Store::new(dir);
        atlas_core::annotations::search(&store, Some(&self.shared.root), &query, limit as usize)
            .into_iter()
            .map(|h| AnnotationHit {
                pdf: string(&h.path),
                relative: h.key,
                page: h.annotation.page,
                annotation_id: h.annotation.id.clone(),
                kind: kind_of(&h.annotation.kind),
                text: h.annotation.text,
                comment: h.annotation.comment,
                title: h.annotation.title,
                color: h.annotation.color,
            })
            .collect()
    }

    /// Fuzzy score of `text` for `query` (higher is better), `None` if no match.
    pub fn fuzzy_score(&self, query: String, text: String) -> Option<i32> {
        atlas_core::finder::score(&query, &text)
    }
}
