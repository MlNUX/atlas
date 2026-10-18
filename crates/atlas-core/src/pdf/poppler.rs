// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Poppler backend (poppler-glib + cairo).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use glib::translate::ToGlibPtr;
use poppler_sys as ffi;

use super::{Backend, OutlineItem, SearchHit, scaled_height};

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

    fn outline(&mut self, path: &Path) -> Result<Vec<OutlineItem>> {
        self.outline_of(path)
    }

    fn search(&mut self, path: &Path, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        self.search_in(path, query, limit)
    }

    fn close(&mut self, path: &Path) {
        self.docs.remove(path);
    }
}

impl PopplerBackend {
    pub(super) fn outline_of(&mut self, path: &Path) -> Result<Vec<OutlineItem>> {
        let doc = self.doc(path)?;
        let doc_ptr: *mut ffi::PopplerDocument = doc.to_glib_none().0;
        let mut out = Vec::new();
        // SAFETY: plain poppler-glib calls on a live document; every iterator
        // and action obtained here is freed before returning.
        unsafe {
            let iter = ffi::poppler_index_iter_new(doc_ptr);
            if !iter.is_null() {
                walk(doc_ptr, iter, 0, &mut out);
                ffi::poppler_index_iter_free(iter);
            }
        }
        Ok(out)
    }

    pub(super) fn search_in(&mut self, path: &Path, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        let doc = self.doc(path)?;
        let mut hits = Vec::new();
        for index in 0..doc.n_pages() {
            let Some(page) = doc.page(index) else { continue };
            let height = page.size().1;
            for rect in page.find_text_with_options(query, poppler::FindFlags::DEFAULT) {
                // poppler reports PDF coordinates (origin bottom left).
                let (x1, x2) = (rect.x1().min(rect.x2()), rect.x1().max(rect.x2()));
                let (y1, y2) = (rect.y1().min(rect.y2()), rect.y1().max(rect.y2()));
                hits.push(SearchHit { page: index as usize, x: x1, y: height - y2, width: x2 - x1, height: y2 - y1 });
                if hits.len() >= limit {
                    return Ok(hits);
                }
            }
        }
        Ok(hits)
    }
}

/// Walks one level of the outline (and its children).
///
/// # Safety
/// `doc` and `iter` must be valid poppler objects.
unsafe fn walk(
    doc: *mut ffi::PopplerDocument,
    iter: *mut ffi::PopplerIndexIter,
    depth: usize,
    out: &mut Vec<OutlineItem>,
) {
    if depth > 32 {
        return;
    }
    loop {
        unsafe {
            let action = ffi::poppler_index_iter_get_action(iter);
            if !action.is_null() {
                let title_ptr = (*action).any.title;
                let title = if title_ptr.is_null() {
                    String::new()
                } else {
                    std::ffi::CStr::from_ptr(title_ptr).to_string_lossy().into_owned()
                };
                let page = if (*action).type_ == ffi::POPPLER_ACTION_GOTO_DEST {
                    dest_page(doc, (*action).goto_dest.dest)
                } else {
                    None
                };
                out.push(OutlineItem { title, page, depth });
                ffi::poppler_action_free(action);
            }
            let child = ffi::poppler_index_iter_get_child(iter);
            if !child.is_null() {
                walk(doc, child, depth + 1, out);
                ffi::poppler_index_iter_free(child);
            }
            if ffi::poppler_index_iter_next(iter) == 0 {
                break;
            }
        }
    }
}

/// 0-based page of a destination, resolving named destinations.
unsafe fn dest_page(doc: *mut ffi::PopplerDocument, dest: *mut ffi::PopplerDest) -> Option<usize> {
    unsafe {
        if dest.is_null() {
            return None;
        }
        let page = if (*dest).type_ == ffi::POPPLER_DEST_NAMED && !(*dest).named_dest.is_null() {
            let named = ffi::poppler_document_find_dest(doc, (*dest).named_dest);
            if named.is_null() {
                return None;
            }
            let page = (*named).page_num;
            ffi::poppler_dest_free(named);
            page
        } else {
            (*dest).page_num
        };
        (page > 0).then(|| page as usize - 1)
    }
}
