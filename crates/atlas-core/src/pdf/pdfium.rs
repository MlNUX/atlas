// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! PDFium backend (pdfium-render, library loaded at runtime).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use pdfium_render::prelude::*;

use super::{Backend, scaled_height};

pub struct PdfiumBackend {
    pdfium: &'static Pdfium,
    docs: HashMap<PathBuf, PdfDocument<'static>>,
}

/// Location of `libpdfium.so`: `ATLAS_PDFIUM_LIB` at runtime, then the path
/// baked in by the Nix build, then the system library search path.
fn bind() -> Result<Box<dyn PdfiumLibraryBindings>> {
    let explicit =
        std::env::var("ATLAS_PDFIUM_LIB").ok().or_else(|| option_env!("ATLAS_PDFIUM_LIB").map(str::to_owned));
    match explicit {
        Some(path) => Pdfium::bind_to_library(&path).with_context(|| format!("cannot load PDFium library {path}")),
        None => Pdfium::bind_to_system_library().context("cannot load PDFium system library"),
    }
}

impl PdfiumBackend {
    pub fn new() -> Result<Self> {
        // The worker thread lives as long as the process, so leaking the
        // library handle gives documents a 'static lifetime.
        let pdfium: &'static Pdfium = Box::leak(Box::new(Pdfium::new(bind()?)));
        Ok(Self { pdfium, docs: HashMap::new() })
    }

    fn doc(&mut self, path: &Path) -> Result<&PdfDocument<'static>> {
        if !self.docs.contains_key(path) {
            let doc = self.pdfium.load_pdf_from_file(path, None).map_err(|e| anyhow::anyhow!("pdfium: {e}"))?;
            self.docs.insert(path.to_path_buf(), doc);
        }
        Ok(&self.docs[path])
    }
}

impl Backend for PdfiumBackend {
    fn open(&mut self, path: &Path) -> Result<Vec<(f64, f64)>> {
        self.docs.remove(path);
        let doc = self.doc(path)?;
        let pages = doc.pages();
        Ok((0..pages.len())
            .map(|i| {
                pages.page_size(i).map_or((595.0, 842.0), |r| (f64::from(r.width().value), f64::from(r.height().value)))
            })
            .collect())
    }

    fn render(&mut self, path: &Path, page: usize, width_px: u32) -> Result<(u32, u32, Vec<u8>)> {
        let doc = self.doc(path)?;
        let page = doc.pages().get(page as PdfPageIndex).map_err(|e| anyhow::anyhow!("pdfium: {e}"))?;
        let size = (f64::from(page.width().value), f64::from(page.height().value));
        let height_px = scaled_height(size, width_px);
        let config = PdfRenderConfig::new()
            .set_target_size(width_px as Pixels, height_px as Pixels)
            .set_format(PdfBitmapFormat::BGRA)
            // pdfium-render defaults to RGBA output (FPDF_REVERSE_BYTE_ORDER);
            // Atlas works with BGRA throughout.
            .set_reverse_byte_order(false)
            .render_form_data(true);
        let bitmap = page.render_with_config(&config).map_err(|e| anyhow::anyhow!("pdfium: {e}"))?;
        let (w, h) = (bitmap.width() as u32, bitmap.height() as u32);
        let raw = bitmap.as_raw_bytes();
        let row = w as usize * 4;
        let stride = raw.len() / h.max(1) as usize;
        let bgra = if stride == row { raw } else { raw.chunks(stride).flat_map(|r| &r[..row]).copied().collect() };
        Ok((w, h, bgra))
    }

    fn text(&mut self, path: &Path, page: usize) -> Result<String> {
        let doc = self.doc(path)?;
        let page = doc.pages().get(page as PdfPageIndex).map_err(|e| anyhow::anyhow!("pdfium: {e}"))?;
        let text = page.text().map_err(|e| anyhow::anyhow!("pdfium: {e}"))?;
        Ok(text.all())
    }

    fn close(&mut self, path: &Path) {
        self.docs.remove(path);
    }
}
