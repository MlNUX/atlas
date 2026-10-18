// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Poppler backend (poppler-glib + cairo).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::{Backend, scaled_height};

#[derive(Default)]
pub struct PopplerBackend {
    docs: HashMap<PathBuf, poppler::Document>,
}

impl PopplerBackend {
    fn doc(&mut self, path: &Path) -> Result<&poppler::Document> {
        if !self.docs.contains_key(path) {
            let uri = glib::filename_to_uri(path, None).context("invalid path")?;
            let doc = poppler::Document::from_file(&uri, None).map_err(|e| anyhow::anyhow!("poppler: {e}"))?;
            self.docs.insert(path.to_path_buf(), doc);
        }
        Ok(&self.docs[path])
    }
}

impl Backend for PopplerBackend {
    fn open(&mut self, path: &Path) -> Result<Vec<(f64, f64)>> {
        self.docs.remove(path);
        let doc = self.doc(path)?;
        Ok((0..doc.n_pages()).map(|i| doc.page(i).map_or((595.0, 842.0), |p| p.size())).collect())
    }

    fn render(&mut self, path: &Path, page: usize, width_px: u32) -> Result<(u32, u32, Vec<u8>)> {
        let doc = self.doc(path)?;
        let page = doc.page(page as i32).context("page out of range")?;
        let size = page.size();
        let height_px = scaled_height(size, width_px);
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width_px as i32, height_px as i32)?;
        {
            let cr = cairo::Context::new(&surface)?;
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.paint()?;
            let scale = f64::from(width_px) / size.0;
            cr.scale(scale, scale);
            page.render(&cr);
        }
        surface.flush();
        let stride = surface.stride() as usize;
        let row = width_px as usize * 4;
        let data = surface.data().context("cairo surface data")?;
        let bgra =
            if stride == row { data.to_vec() } else { data.chunks(stride).flat_map(|r| &r[..row]).copied().collect() };
        Ok((width_px, height_px, bgra))
    }

    fn text(&mut self, path: &Path, page: usize) -> Result<String> {
        let doc = self.doc(path)?;
        let page = doc.page(page as i32).context("page out of range")?;
        Ok(page.text().map(|t| t.to_string()).unwrap_or_default())
    }

    fn close(&mut self, path: &Path) {
        self.docs.remove(path);
    }
}
