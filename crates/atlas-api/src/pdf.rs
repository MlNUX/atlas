// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! PDF data of one document: Atlas annotations (`.atlas/annotations/`,
//! the same files on Linux and macOS, E12), page text for search and
//! index, quotes into notes, export. Display, selection and drawing are
//! the UI's (PDFKit on macOS). Template: `annotations_model.rs`,
//! `pdf_items.rs`.
//!
//! Coordinates are PDF points, origin top left of the page as displayed
//! (CropBox, after `/Rotate`), see `docs/data-format.md`; PDFKit converts
//! with `y' = height − y` (`Features/PDF/Coordinates.swift`).

use crate::{Result, todo_task};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum AnnotationKind {
    Highlight,
    Underline,
    Strikeout,
    /// A comment pinned to a point.
    Note,
    /// Freehand strokes.
    Ink,
    /// A bookmark for the whole page.
    Bookmark,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Point {
    pub x: f64,
    pub y: f64,
    /// Pen pressure 0…1, −1 if unknown.
    pub pressure: f64,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Stroke {
    pub points: Vec<Point>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Annotation {
    /// Empty when adding (assigned by [`PdfDocument::add`]).
    pub id: String,
    pub kind: AnnotationKind,
    /// 1-based.
    pub page: u32,
    /// Text markup: one per line part; note: the icon.
    pub rects: Vec<Rect>,
    /// Marked text (text markup).
    pub text: String,
    pub comment: String,
    /// `#rrggbb`.
    pub color: String,
    pub ink: Vec<Stroke>,
    /// Stroke width in points (ink).
    pub width: Option<f64>,
    /// Bookmark title.
    pub title: String,
    /// Shared by the parts of a markup over several pages.
    pub group: String,
    /// Unix seconds.
    pub created: i64,
    pub modified: i64,
}

/// Changes to an annotation; `None` keeps the value.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct AnnotationPatch {
    pub comment: Option<String>,
    pub color: Option<String>,
    pub title: Option<String>,
    pub rects: Option<Vec<Rect>>,
    pub ink: Option<Vec<Stroke>>,
}

/// How the stored annotations were matched to this file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum MatchState {
    /// No annotations yet.
    New,
    /// Same path, same file.
    Path,
    /// Found by fingerprint after a move or rename.
    Moved,
    /// Same path but the file changed (pages may not fit any more).
    Changed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PdfStatus {
    pub state: MatchState,
    /// Explanation for `Moved` / `Changed`, empty otherwise.
    pub message: String,
    pub count: u32,
}

/// Opaque copy of an annotation (or a group) for undo.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Snapshot {
    pub data: String,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct PdfDocument {}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl PdfDocument {
    /// Tells the core what the UI read from the file (page count, PDF
    /// `/ID`), so it can match the stored annotations. Call once after
    /// opening, before the other methods.
    pub fn attach(&self, page_count: u32, pdf_id: Option<String>) -> Result<PdfStatus> {
        let _ = (page_count, pdf_id);
        todo_task!("P2.6")
    }

    pub fn status(&self) -> PdfStatus {
        todo_task!("P2.6")
    }

    /// The user has seen the `Moved` / `Changed` message.
    pub fn acknowledge_change(&self) {
        todo_task!("P2.6")
    }

    /// Hands the page text to the core (search, index, quotes): one string
    /// per page, lines separated by `\n`. Wraps `pdf::set_page_texts`.
    pub fn set_page_texts(&self, pages: Vec<String>) {
        let _ = pages;
        todo_task!("P2.6")
    }

    pub fn annotations(&self) -> Vec<Annotation> {
        todo_task!("P2.6")
    }

    pub fn page_annotations(&self, page: u32) -> Vec<Annotation> {
        let _ = page;
        todo_task!("P2.6")
    }

    pub fn annotation(&self, id: String) -> Option<Annotation> {
        let _ = id;
        todo_task!("P2.6")
    }

    /// Adds and saves; returns the id. Text markup over several pages: one
    /// call per page with the same `group`.
    pub fn add(&self, annotation: Annotation) -> Result<String> {
        let _ = annotation;
        todo_task!("P2.6")
    }

    pub fn update(&self, id: String, patch: AnnotationPatch) -> Result<()> {
        let _ = (id, patch);
        todo_task!("P2.6")
    }

    /// Removes the annotation and the other parts of its group.
    pub fn remove(&self, id: String) -> Result<()> {
        let _ = id;
        todo_task!("P2.6")
    }

    pub fn snapshot(&self, id: String) -> Option<Snapshot> {
        let _ = id;
        todo_task!("P2.6")
    }

    pub fn restore(&self, snapshot: Snapshot) -> Result<()> {
        let _ = snapshot;
        todo_task!("P2.6")
    }

    /// Markdown quote of an annotation for `note` (relative link back).
    pub fn quote_markdown(&self, id: String, note: String) -> Result<String> {
        let _ = (id, note);
        todo_task!("P2.6")
    }

    /// `atlas:` link to the page / annotation, for the clipboard.
    pub fn link(&self, page: u32, id: Option<String>) -> String {
        let _ = (page, id);
        todo_task!("P2.6")
    }

    /// Suggested file name for an annotated copy.
    pub fn default_export_path(&self) -> String {
        todo_task!("P2.6")
    }

    /// Re-reads the annotation file (changed by sync or another window).
    pub fn reload(&self) -> Result<PdfStatus> {
        todo_task!("P2.6")
    }
}
