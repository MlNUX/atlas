// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! PDF data of one document: Atlas annotations (`.atlas/annotations/`,
//! the same files on Linux and macOS, E12), page text for search and
//! index, quotes into notes. Display, selection, drawing and the export of
//! an annotated copy are the UI's (PDFKit on macOS). Template:
//! `annotations_model.rs`.
//!
//! Coordinates are PDF points, origin top left of the page as displayed
//! (relative to the CropBox, after `/Rotate`), see `docs/data-format.md`.
//! The conversion to PDFKit's page space is `PageGeometry` from spike P0.4
//! (`docs/porting/spike-pdfkit.md`, later `Features/PDF/Coordinates.swift`);
//! a plain `y' = height − y` is only right without CropBox offset and rotation.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use atlas_core::annotations::{self as core, AnnotationFile, Fingerprint, Kind, Match, Store};

use crate::events::{Event, Sink};
use crate::{AtlasError, Result};

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

/// A rectangle as the file format stores it (`[x, y, width, height]`).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A point of a freehand stroke (the file format stores `[x, y]`).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Point {
    pub x: f64,
    pub y: f64,
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
    /// Switch between highlight, underline and strikeout.
    pub kind: Option<AnnotationKind>,
}

/// How the stored annotations were matched to this file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum MatchState {
    /// Not attached yet ([`PdfDocument::attach`]).
    Loading,
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
pub struct PdfDocument {
    path: PathBuf,
    /// Project root (stored paths are relative to it).
    root: PathBuf,
    store: Store,
    sink: Sink,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// Where the annotations are stored; `None` until attached.
    file: Option<PathBuf>,
    data: AnnotationFile,
    /// Fingerprint of the PDF as it is now (stored with the next write).
    current: Fingerprint,
    matched: Option<Match>,
    /// Page count and `/ID` given to `attach` (for `reload`).
    attached: Option<(u32, Option<String>)>,
}

impl PdfDocument {
    /// `atlas_dir` `None` (no writable `.atlas/`): Atlas' state folder.
    pub(crate) fn new(root: &Path, atlas_dir: Option<&Path>, sink: Sink, path: &Path) -> Result<Self> {
        if !path.is_file() {
            return Err(AtlasError::NotFound { message: format!("PDF {} nicht gefunden", path.display()) });
        }
        let store = match atlas_dir {
            Some(a) => Store::for_atlas_dir(a),
            None => Store::new(atlas_core::paths::state_root().join(core::DIR)),
        };
        Ok(Self {
            path: path.to_path_buf(),
            root: root.to_path_buf(),
            store,
            sink,
            state: Mutex::new(State::default()),
        })
    }
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl PdfDocument {
    /// Tells the core what the UI read from the file (page count, PDF
    /// `/ID` as hex), so it can match the stored annotations. Call once
    /// after opening (off the main thread: the file is hashed), before the
    /// other methods.
    pub fn attach(&self, page_count: u32, pdf_id: Option<String>) -> Result<PdfStatus> {
        let fingerprint = Fingerprint::of(&self.path, page_count, pdf_id.clone())?;
        let root = self.root.clone();
        let exists = |p: &str| {
            let p = Path::new(p);
            if p.is_absolute() { p.exists() } else { root.join(p).exists() }
        };
        let loaded = self.store.load(&self.key(), &fingerprint, exists)?;
        {
            let mut s = self.lock();
            s.file = Some(loaded.file);
            s.data = loaded.data;
            s.current = fingerprint;
            s.matched = Some(loaded.matched);
            s.attached = Some((page_count, pdf_id));
        }
        self.changed();
        Ok(self.status())
    }

    pub fn status(&self) -> PdfStatus {
        let s = self.lock();
        let state = match s.matched {
            None => MatchState::Loading,
            Some(Match::New) => MatchState::New,
            Some(Match::Exact) => MatchState::Path,
            Some(Match::Moved) => MatchState::Moved,
            Some(Match::Changed) => MatchState::Changed,
        };
        let message = match state {
            MatchState::Moved => "Die Annotationen sind der verschobenen oder umbenannten PDF gefolgt.".to_owned(),
            MatchState::Changed => "Die PDF hat sich seit den Annotationen geändert – Markierungen können \
                                    verrutscht sein."
                .to_owned(),
            _ => String::new(),
        };
        PdfStatus { state, message, count: s.data.annotations.len() as u32 }
    }

    /// The user has seen the `Moved` / `Changed` message: the stored
    /// fingerprint is updated to the file as it is now.
    pub fn acknowledge_change(&self) {
        let mut s = self.lock();
        if matches!(s.matched, Some(Match::Changed | Match::Moved)) {
            s.matched = Some(Match::Exact);
            if !s.data.annotations.is_empty()
                && let Err(err) = self.save(&mut s)
            {
                log::warn!("annotations: {err}");
            }
        }
    }

    /// Hands the page text to the core (search, index, quotes): one string
    /// per page, lines separated by `\n`. Used until the file changes.
    pub fn set_page_texts(&self, pages: Vec<String>) {
        atlas_core::pdf::set_page_texts(&self.path, pages);
    }

    pub fn annotations(&self) -> Vec<Annotation> {
        self.lock().data.annotations.iter().map(to_api).collect()
    }

    pub fn page_annotations(&self, page: u32) -> Vec<Annotation> {
        self.lock().data.annotations.iter().filter(|a| a.page == page).map(to_api).collect()
    }

    pub fn annotation(&self, id: String) -> Option<Annotation> {
        self.lock().data.annotations.iter().find(|a| a.id == id).map(to_api)
    }

    /// Adds and saves; returns the id. Text markup over several pages: one
    /// call per page with the same `group`.
    pub fn add(&self, annotation: Annotation) -> Result<String> {
        let mut a = from_api(&annotation);
        a.id = core::new_id();
        a.created = core::now();
        a.modified = a.created;
        if a.kind.is_text_markup() {
            a.text = core::clean_text(&a.text);
        }
        let id = a.id.clone();
        let mut s = self.attached()?;
        s.data.annotations.push(a);
        self.save(&mut s)?;
        drop(s);
        self.changed();
        Ok(id)
    }

    /// Changes one annotation; colour, comment and kind apply to all parts
    /// of a markup over several pages.
    pub fn update(&self, id: String, patch: AnnotationPatch) -> Result<()> {
        let mut s = self.attached()?;
        let index = s.data.annotations.iter().position(|a| a.id == id).ok_or_else(|| not_found(&id))?;
        let mut a = s.data.annotations[index].clone();
        if let Some(c) = patch.comment {
            a.comment = c;
        }
        if let Some(c) = patch.color {
            a.color = c;
        }
        if let Some(t) = patch.title {
            a.title = t;
        }
        if let Some(r) = patch.rects {
            a.rects = r.iter().map(rect_in).collect();
        }
        if let Some(i) = patch.ink {
            a.ink = i.iter().map(stroke_in).collect();
        }
        if let Some(k) = patch.kind {
            a.kind = k.into();
        }
        a.modified = core::now();
        if !a.group.is_empty() {
            for b in s.data.annotations.iter_mut().filter(|b| b.group == a.group && b.id != id) {
                b.color = a.color.clone();
                b.comment = a.comment.clone();
                b.kind = a.kind;
            }
        }
        s.data.annotations[index] = a;
        self.save(&mut s)?;
        drop(s);
        self.changed();
        Ok(())
    }

    /// Removes the annotation and the other parts of its group.
    pub fn remove(&self, id: String) -> Result<()> {
        let mut s = self.attached()?;
        let group =
            s.data.annotations.iter().find(|a| a.id == id).map(|a| a.group.clone()).ok_or_else(|| not_found(&id))?;
        s.data.annotations.retain(|a| a.id != id && (group.is_empty() || a.group != group));
        self.save(&mut s)?;
        drop(s);
        self.changed();
        Ok(())
    }

    /// The annotation and the other parts of its group, for undo.
    pub fn snapshot(&self, id: String) -> Option<Snapshot> {
        let s = self.lock();
        let parts = parts(&s.data.annotations, &id);
        (!parts.is_empty()).then(|| Snapshot { data: serde_json::to_string(&parts).unwrap_or_default() })
    }

    /// Puts a snapshot back (replacing parts with the same id).
    pub fn restore(&self, snapshot: Snapshot) -> Result<()> {
        let list: Vec<core::Annotation> = serde_json::from_str(&snapshot.data)
            .map_err(|e| AtlasError::Invalid { message: format!("Ungültiger Schnappschuss: {e}") })?;
        if list.is_empty() {
            return Ok(());
        }
        let mut s = self.attached()?;
        for a in list {
            match s.data.annotations.iter().position(|b| b.id == a.id) {
                Some(i) => s.data.annotations[i] = a,
                None => s.data.annotations.push(a),
            }
        }
        self.save(&mut s)?;
        drop(s);
        self.changed();
        Ok(())
    }

    /// Markdown quote of an annotation (all parts of its group) for the
    /// note at `note` (absolute; empty: a project-relative link).
    pub fn quote_markdown(&self, id: String, note: String) -> Result<String> {
        let s = self.lock();
        let parts = parts(&s.data.annotations, &id);
        let first = parts.first().ok_or_else(|| not_found(&id))?;
        let note = PathBuf::from(note);
        let in_project = |p: &Path| p.starts_with(&self.root);
        let link_path = if in_project(&self.path) && (note.as_os_str().is_empty() || in_project(&note)) {
            format!("/{}", self.key())
        } else {
            let dir = note.parent().unwrap_or(Path::new("/"));
            atlas_core::markdown::relative_path(dir, &self.path).to_string_lossy().into_owned()
        };
        let text = parts.iter().map(|p| p.text.as_str()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
        let text = if text.is_empty() { first.comment.clone() } else { text };
        let comment = if text == first.comment { String::new() } else { first.comment.clone() };
        let name = self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Ok(core::quote_markdown(&text, &comment, &name, &link_path, first.page, &first.id))
    }

    /// Link to the page (and annotation: the first part of its group), as
    /// written into notes: `/docs/paper.pdf#page=8&annot=…`.
    pub fn link(&self, page: u32, id: Option<String>) -> String {
        let path = if self.path.starts_with(&self.root) {
            format!("/{}", self.key())
        } else {
            self.path.to_string_lossy().into_owned()
        };
        let s = self.lock();
        match id.as_deref().and_then(|id| parts(&s.data.annotations, id).first().cloned()) {
            Some(a) => core::link(&path, a.page, Some(&a.id)),
            None => core::link(&path, page, None),
        }
    }

    /// Suggested file name for an annotated copy (next to the PDF, not
    /// taken yet).
    pub fn default_export_path(&self) -> String {
        let stem = self.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "dokument".into());
        let dir = self.path.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut candidate = dir.join(format!("{stem} (annotiert).pdf"));
        let mut n = 2;
        while candidate.exists() {
            candidate = dir.join(format!("{stem} (annotiert {n}).pdf"));
            n += 1;
        }
        candidate.to_string_lossy().into_owned()
    }

    /// Re-reads the annotation file (changed by sync or another window).
    pub fn reload(&self) -> Result<PdfStatus> {
        let attached = self.lock().attached.clone();
        match attached {
            Some((pages, id)) => self.attach(pages, id),
            None => Ok(self.status()),
        }
    }
}

impl PdfDocument {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn attached(&self) -> Result<MutexGuard<'_, State>> {
        let s = self.lock();
        if s.file.is_none() {
            return Err(AtlasError::Unavailable { message: "Die PDF ist noch nicht geladen.".into() });
        }
        Ok(s)
    }

    /// Key of the PDF in the store: project-relative (without `/`), or
    /// absolute outside the project – as on Linux.
    fn key(&self) -> String {
        match self.path.strip_prefix(&self.root) {
            Ok(rel) => rel.to_string_lossy().into_owned(),
            Err(_) => self.path.to_string_lossy().into_owned(),
        }
    }

    /// Writes the annotations with the current fingerprint.
    fn save(&self, s: &mut State) -> Result<()> {
        let Some(file) = s.file.clone() else { return Ok(()) };
        s.data.document.fingerprint = s.current.clone();
        core::write(&file, &s.data).map_err(|e| AtlasError::Failed {
            message: format!("Annotationen können nicht gespeichert werden: {e:#}"),
        })?;
        if s.matched == Some(Match::Changed) || s.matched == Some(Match::New) {
            // The stored fingerprint matches the PDF again.
            s.matched = Some(Match::Exact);
        }
        Ok(())
    }

    fn changed(&self) {
        self.sink.on_event(Event::AnnotationsChanged { path: self.path.to_string_lossy().into_owned() });
    }
}

/// Colour of the file format when none is given.
const DEFAULT_COLOR: &str = "#ffd400";

fn not_found(id: &str) -> AtlasError {
    AtlasError::NotFound { message: format!("Keine Annotation {id}") }
}

/// The annotation and the other parts of its group, in page order.
fn parts<'a>(all: &'a [core::Annotation], id: &str) -> Vec<&'a core::Annotation> {
    let Some(a) = all.iter().find(|a| a.id == id) else { return Vec::new() };
    if a.group.is_empty() {
        return vec![a];
    }
    let mut parts: Vec<_> = all.iter().filter(|b| b.group == a.group).collect();
    parts.sort_by_key(|b| b.page);
    parts
}

impl From<Kind> for AnnotationKind {
    fn from(k: Kind) -> Self {
        match k {
            Kind::Highlight => AnnotationKind::Highlight,
            Kind::Underline => AnnotationKind::Underline,
            Kind::Strikeout => AnnotationKind::Strikeout,
            Kind::Note => AnnotationKind::Note,
            Kind::Ink => AnnotationKind::Ink,
            Kind::Bookmark => AnnotationKind::Bookmark,
        }
    }
}

impl From<AnnotationKind> for Kind {
    fn from(k: AnnotationKind) -> Self {
        match k {
            AnnotationKind::Highlight => Kind::Highlight,
            AnnotationKind::Underline => Kind::Underline,
            AnnotationKind::Strikeout => Kind::Strikeout,
            AnnotationKind::Note => Kind::Note,
            AnnotationKind::Ink => Kind::Ink,
            AnnotationKind::Bookmark => Kind::Bookmark,
        }
    }
}

fn rect_in(r: &Rect) -> core::Rect {
    [r.x, r.y, r.width, r.height]
}

fn stroke_in(s: &Stroke) -> Vec<core::Point> {
    s.points.iter().map(|p| [p.x, p.y]).collect()
}

fn to_api(a: &core::Annotation) -> Annotation {
    Annotation {
        id: a.id.clone(),
        kind: a.kind.into(),
        page: a.page,
        rects: a.rects.iter().map(|r| Rect { x: r[0], y: r[1], width: r[2], height: r[3] }).collect(),
        text: a.text.clone(),
        comment: a.comment.clone(),
        color: a.color.clone(),
        ink: a.ink.iter().map(|s| Stroke { points: s.iter().map(|p| Point { x: p[0], y: p[1] }).collect() }).collect(),
        width: a.width,
        title: a.title.clone(),
        group: a.group.clone(),
        created: a.created,
        modified: a.modified,
    }
}

fn from_api(a: &Annotation) -> core::Annotation {
    core::Annotation {
        id: a.id.clone(),
        kind: a.kind.into(),
        page: a.page,
        rects: a.rects.iter().map(rect_in).collect(),
        text: a.text.clone(),
        comment: a.comment.clone(),
        color: if a.color.is_empty() { DEFAULT_COLOR.into() } else { a.color.clone() },
        ink: a.ink.iter().map(stroke_in).collect(),
        width: a.width,
        title: a.title.clone(),
        group: a.group.clone(),
        created: a.created,
        modified: a.modified,
    }
}
