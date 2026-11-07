// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stand-in engine without the feature `pdf-poppler` (e.g. on macOS, where
//! the UI displays PDFs with PDFKit): every call fails with a clear message.
//! A real engine can still be installed with [`super::set_backend_factory`].

use std::path::Path;

use anyhow::{Result, bail};

use super::formulas::Glyph;
use super::{Backend, OutlineItem, PageLink, SearchHit, SelectionStyle};

const MESSAGE: &str = "PDF-Funktion in diesem Build nicht verfügbar (ohne pdf-poppler)";

pub(super) fn backend(_max_open: usize) -> Box<dyn Backend> {
    Box::new(Unavailable)
}

pub(super) fn export_annotated(
    _src: &Path,
    _dst: &Path,
    _annotations: &[crate::annotations::Annotation],
) -> Result<usize> {
    bail!(MESSAGE)
}

pub(super) fn list_annotations(_path: &Path) -> Result<Vec<(usize, i32)>> {
    bail!(MESSAGE)
}

struct Unavailable;

impl Backend for Unavailable {
    fn open(&mut self, _: &Path) -> Result<Vec<(f64, f64)>> {
        bail!(MESSAGE)
    }
    fn render(&mut self, _: &Path, _: usize, _: u32) -> Result<(u32, u32, Vec<u8>)> {
        bail!(MESSAGE)
    }
    fn text(&mut self, _: &Path, _: usize) -> Result<String> {
        bail!(MESSAGE)
    }
    fn outline(&mut self, _: &Path) -> Result<Vec<OutlineItem>> {
        bail!(MESSAGE)
    }
    fn links(&mut self, _: &Path) -> Result<Vec<PageLink>> {
        bail!(MESSAGE)
    }
    fn search(&mut self, _: &Path, _: &str, _: usize) -> Result<Vec<SearchHit>> {
        bail!(MESSAGE)
    }
    fn select(&mut self, _: &Path, _: usize, _: [f64; 4], _: SelectionStyle) -> Result<(Vec<[f64; 4]>, String)> {
        bail!(MESSAGE)
    }
    fn document_id(&mut self, _: &Path) -> Result<Option<String>> {
        bail!(MESSAGE)
    }
    fn clip_gray(&mut self, _: &Path, _: usize, _: [f64; 4], _: f64) -> Result<(u32, u32, Vec<u8>)> {
        bail!(MESSAGE)
    }
    fn clip_rgb(&mut self, _: &Path, _: usize, _: [f64; 4], _: f64) -> Result<(u32, u32, Vec<u8>)> {
        bail!(MESSAGE)
    }
    fn glyphs(&mut self, _: &Path, _: usize) -> Result<Vec<Glyph>> {
        bail!(MESSAGE)
    }
    fn close(&mut self, _: &Path) {}
}
