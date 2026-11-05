// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atlas notes from the tablet (`.atlasnote`), read only. Template:
//! `atlas_note_info`/`atlas_note_page`/`atlas_note_preview` in `app.rs`,
//! core `atlasnote.rs`.
//!
//! On Linux the core renders pages to PNG (cairo, poppler). The macOS build
//! has no such engine: there the UI draws the pages from [`AtlasNote::pages`]
//! (PDF backgrounds with PDFKit, strokes with Core Graphics). The page
//! records carry everything the drawing needs, decided here as on the
//! tablet: colours with the brushes' opacity, line patterns of the paper,
//! pressure widths, wrapped text lines.
//!
//! Coordinates: a page is [`AtlasNote::page_width`] (1000) units wide,
//! origin top left, y down.

use std::path::Path;
use std::sync::Arc;

use atlas_core::atlasnote::{self as core, NoteFile, PAGE_WIDTH};

use crate::util::background;
use crate::{AtlasError, Result};

/// Straight (not premultiplied) colour, each part 0…1.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct NoteColor {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PatternKind {
    /// Horizontal lines from `start` to the bottom, `margin` left and right.
    Lines,
    /// Vertical and horizontal lines every `step`, starting at `start`.
    Grid,
    /// Dots of `dot_radius` every `step`, from `start` to `width − start`.
    Dots,
}

/// The paper's pattern (none for plain paper and for backgrounds).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PaperPattern {
    pub kind: PatternKind,
    pub step: f64,
    pub start: f64,
    pub margin: f64,
    pub line_width: f64,
    pub dot_radius: f64,
    pub color: NoteColor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PdfBackground {
    /// Extracted PDF (absolute path).
    pub path: String,
    /// 0-based page, stretched over the whole note page.
    pub page: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct NotePoint {
    pub x: f64,
    pub y: f64,
    /// 0…1, negative if unknown.
    pub pressure: f64,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum NoteMark {
    /// A stroke with round caps and joins. With `pressure` each segment
    /// is `width × (0.4 + 0.8 × mean pressure of its two points)` wide,
    /// else the whole stroke `width`. One point: a dot of `width`.
    /// Translucent strokes (`color.a < 1`) are drawn opaque and faded as a
    /// whole, so overlapping segments do not darken.
    Stroke { tool: String, color: NoteColor, width: f64, pressure: bool, points: Vec<NotePoint> },
    /// Text in a sans-serif font of `size`, rotated by `rotation` degrees
    /// around `(x, y)`; line `i` has its baseline at `size × (1 + 1.3 i)`
    /// below that point. `lines` are wrapped as on the tablet.
    Text { color: NoteColor, size: f64, x: f64, y: f64, rotation: f64, lines: Vec<String> },
    /// A picture mapped onto three corners: top left, top right, bottom
    /// left (move, scale and rotation in one).
    Image { path: String, top_left: NotePoint, top_right: NotePoint, bottom_left: NotePoint },
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct NotePage {
    pub height: f64,
    pub dark: bool,
    pub background: NoteColor,
    pub pattern: Option<PaperPattern>,
    /// A PDF page as background (drawn on white).
    pub pdf: Option<PdfBackground>,
    /// A picture as background, stretched over the page (absolute path).
    pub image: Option<String>,
    pub marks: Vec<NoteMark>,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct AtlasNote {
    file: Arc<NoteFile>,
}

fn color((r, g, b): (f64, f64, f64), a: f64) -> NoteColor {
    NoteColor { r, g, b, a }
}

fn point(p: &[f64; 8]) -> NotePoint {
    NotePoint { x: p[1], y: p[2], pressure: p[5] }
}

/// Lines of a text box, wrapped after a fixed number of characters like
/// the tablet does.
fn wrap(text: &str, size: f64) -> Vec<String> {
    let limit = ((900.0 / (size * 0.65)) as usize).max(1);
    text.lines()
        .flat_map(|line| {
            let chars: Vec<char> = line.chars().collect();
            if chars.is_empty() {
                vec![String::new()]
            } else {
                chars.chunks(limit).map(|c| c.iter().collect()).collect()
            }
        })
        .collect()
}

fn pattern(page: &core::Page) -> Option<PaperPattern> {
    if page.pdf.is_some() || page.image.is_some() {
        return None;
    }
    let line = color(core::rgb(if page.dark { 0x424348 } else { 0xd9dfeb }), 1.0);
    let make = |kind, step, start, margin| PaperPattern {
        kind,
        step,
        start,
        margin,
        line_width: 0.65,
        dot_radius: 1.1,
        color: line,
    };
    match page.paper.as_str() {
        "LINED" => Some(make(PatternKind::Lines, 32.0, 48.0, 32.0)),
        "GRID" => Some(make(PatternKind::Grid, 48.0, core::paper_start(48.0, 0.0), 0.0)),
        "DOTS" => Some(make(PatternKind::Dots, 32.0, core::paper_start(32.0, 16.0), 16.0)),
        _ => None,
    }
}

impl AtlasNote {
    fn page_record(&self, page: &core::Page) -> NotePage {
        let source = |name: String| self.file.source_path(&name).to_string_lossy().into_owned();
        let background = if page.pdf.is_some() {
            color((1.0, 1.0, 1.0), 1.0)
        } else {
            color(core::rgb(if page.dark { 0x242528 } else { 0xfffdf7 }), 1.0)
        };
        let marks = page
            .marks
            .iter()
            .filter_map(|m| {
                let (r, g, b, a) = core::stroke_color(&m.style);
                if let Some(image) = &m.image {
                    let [tl, tr, bl] = [&m.points[0], &m.points[1], &m.points[2]];
                    return Some(NoteMark::Image {
                        path: source(format!("images/{image}")),
                        top_left: point(tl),
                        top_right: point(tr),
                        bottom_left: point(bl),
                    });
                }
                if let Some(text) = &m.text {
                    let origin = m.points.first()?;
                    return Some(NoteMark::Text {
                        color: color((r, g, b), a),
                        size: m.style.width,
                        x: origin[1],
                        y: origin[2],
                        rotation: m.rotation,
                        lines: wrap(text, m.style.width),
                    });
                }
                if m.points.is_empty() {
                    return None;
                }
                let pressure =
                    matches!(m.style.tool.as_str(), "PEN" | "PENCIL") && m.points.iter().any(|p| p[5] >= 0.0);
                Some(NoteMark::Stroke {
                    tool: m.style.tool.clone(),
                    color: color((r, g, b), a),
                    width: m.style.width,
                    pressure,
                    points: m.points.iter().map(point).collect(),
                })
            })
            .collect();
        NotePage {
            height: page.height,
            dark: page.dark,
            background,
            pattern: pattern(page),
            pdf: page
                .pdf
                .as_ref()
                .map(|p| PdfBackground { path: source(format!("pdf/{}", p.file)), page: p.page.max(0) as u32 }),
            image: page.image.clone().map(|i| source(format!("images/{i}"))),
            marks,
        }
    }
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl AtlasNote {
    /// Opens the notebook at `path` (extracted once into the cache; the
    /// file itself is never written).
    #[cfg_attr(feature = "ffi", uniffi::constructor)]
    pub fn open(path: String) -> Result<Arc<Self>> {
        let file =
            NoteFile::open_cached(Path::new(&path)).map_err(|e| AtlasError::Invalid { message: format!("{e:#}") })?;
        Ok(Arc::new(Self { file }))
    }

    /// Width of every page in note units.
    pub fn page_width(&self) -> f64 {
        PAGE_WIDTH
    }

    pub fn page_count(&self) -> u32 {
        self.file.notebook.pages.len() as u32
    }

    /// All pages with what the UI needs to draw them.
    pub fn pages(&self) -> Vec<NotePage> {
        self.file.notebook.pages.iter().map(|p| self.page_record(p)).collect()
    }

    pub fn page(&self, index: u32) -> Option<NotePage> {
        self.file.notebook.pages.get(index as usize).map(|p| self.page_record(p))
    }

    /// Whether this build renders pages itself ([`AtlasNote::render_png`]).
    pub fn can_render(&self) -> bool {
        core::CAN_RENDER
    }

    /// PNG of page `index`, `width` pixels wide (rendered once, then from
    /// the cache); `Unavailable` without a render engine (macOS).
    pub async fn render_png(&self, index: u32, width: u32) -> Result<String> {
        if !core::CAN_RENDER {
            return Err(AtlasError::Unavailable {
                message: "Dieser Build zeichnet Atlas-Notizen nicht selbst; die Oberfläche zeichnet sie aus pages()."
                    .into(),
            });
        }
        if index as usize >= self.file.notebook.pages.len() {
            return Err(AtlasError::NotFound { message: format!("Keine Seite {index}") });
        }
        let file = self.file.clone();
        background(move || {
            file.render_png(index as usize, width)
                .map(|p| p.to_string_lossy().into_owned())
                .map_err(|e| AtlasError::Failed { message: format!("{e:#}") })
        })
        .await
    }

    /// A page already rendered at any width, for a quick preview.
    pub fn preview(&self, index: u32) -> Option<String> {
        self.file.any_rendered(index as usize).map(|p| p.to_string_lossy().into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_wraps_like_the_tablet() {
        // 900 / (20 × 0.65) = 69 characters per line.
        let long = "a".repeat(100);
        let lines = wrap(&format!("{long}\n\nkurz"), 20.0);
        assert_eq!(lines.iter().map(String::len).collect::<Vec<_>>(), vec![69, 31, 0, 4]);
    }
}
