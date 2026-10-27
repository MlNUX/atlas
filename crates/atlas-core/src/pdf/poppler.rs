// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Poppler backend (poppler-glib + cairo).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use glib::translate::ToGlibPtr;
use poppler_sys as ffi;

use super::formulas::Glyph;
use super::{Backend, LinkTarget, OutlineItem, PageLink, SearchHit, scaled_height};

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

/// Page `index` (0-based) if the document has it. poppler_document_get_page
/// itself returns NULL for others but logs a critical warning each time –
/// e.g. while a viewer still asks for pages of a PDF that got shorter.
fn page_at(doc: &poppler::Document, index: i64) -> Option<poppler::Page> {
    (0..i64::from(doc.n_pages())).contains(&index).then(|| doc.page(index as i32)).flatten()
}

impl Backend for PopplerBackend {
    fn open(&mut self, path: &Path) -> Result<Vec<(f64, f64)>> {
        self.docs.remove(path);
        let doc = self.doc(path)?;
        Ok((0..doc.n_pages()).map(|i| doc.page(i).map_or((595.0, 842.0), |p| p.size())).collect())
    }

    fn render(&mut self, path: &Path, page: usize, width_px: u32) -> Result<(u32, u32, Vec<u8>)> {
        let doc = self.doc(path)?;
        let page = page_at(doc, page as i64).context("page out of range")?;
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

    fn clip_gray(&mut self, path: &Path, page: usize, area: [f64; 4], scale: f64) -> Result<(u32, u32, Vec<u8>)> {
        let (w, h, rgb) = self.clip_rgb(path, page, area, scale)?;
        // Grey like OpenCV's RGB2GRAY (0.299 R + 0.587 G + 0.114 B).
        let gray = rgb
            .chunks(3)
            .map(|px| {
                let v = 0.299 * f32::from(px[0]) + 0.587 * f32::from(px[1]) + 0.114 * f32::from(px[2]);
                v.round().clamp(0.0, 255.0) as u8
            })
            .collect();
        Ok((w, h, gray))
    }

    fn clip_rgb(&mut self, path: &Path, page: usize, area: [f64; 4], scale: f64) -> Result<(u32, u32, Vec<u8>)> {
        let doc = self.doc(path)?;
        let page = page_at(doc, page as i64).context("page out of range")?;
        let (x0, x1) = (area[0].min(area[2]), area[0].max(area[2]));
        let (y0, y1) = (area[1].min(area[3]), area[1].max(area[3]));
        let (w, h) = (((x1 - x0) * scale).ceil() as i32, ((y1 - y0) * scale).ceil() as i32);
        if w < 2 || h < 2 || w > 8000 || h > 8000 {
            anyhow::bail!("Bereich zu klein oder zu groß ({w}×{h} Pixel)");
        }
        let mut surface = cairo::ImageSurface::create(cairo::Format::Rgb24, w, h)?;
        {
            let cr = cairo::Context::new(&surface)?;
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.paint()?;
            cr.scale(scale, scale);
            cr.translate(-x0, -y0);
            page.render(&cr);
        }
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().context("cairo surface data")?;
        // BGRx to RGB.
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for row in data.chunks(stride).take(h as usize) {
            for px in row[..w as usize * 4].chunks(4) {
                rgb.extend_from_slice(&[px[2], px[1], px[0]]);
            }
        }
        Ok((w as u32, h as u32, rgb))
    }

    fn text(&mut self, path: &Path, page: usize) -> Result<String> {
        let doc = self.doc(path)?;
        let page = page_at(doc, page as i64).context("page out of range")?;
        Ok(page.text().map(|t| t.to_string()).unwrap_or_default())
    }

    fn glyphs(&mut self, path: &Path, page: usize) -> Result<Vec<Glyph>> {
        let doc = self.doc(path)?;
        let page = page_at(doc, page as i64).context("page out of range")?;
        let chars: Vec<char> = page.text().map(|t| t.chars().collect()).unwrap_or_default();
        let page_ptr: *mut ffi::PopplerPage = page.to_glib_none().0;
        let mut rects: Vec<[f64; 4]> = Vec::new();
        let mut fonts: Vec<(String, f64)> = vec![(String::new(), 0.0); chars.len()];
        // SAFETY: poppler hands out a g_malloc'ed array of `n` rectangles
        // and a list of attributes (character ranges, inclusive) that
        // poppler_page_free_text_attributes frees.
        unsafe {
            let mut layout: *mut ffi::PopplerRectangle = std::ptr::null_mut();
            let mut n: std::ffi::c_uint = 0;
            if ffi::poppler_page_get_text_layout(page_ptr, &mut layout, &mut n) != 0 && !layout.is_null() {
                rects =
                    std::slice::from_raw_parts(layout, n as usize).iter().map(|r| [r.x1, r.y1, r.x2, r.y2]).collect();
            }
            glib::ffi::g_free(layout.cast());
            let list = ffi::poppler_page_get_text_attributes(page_ptr);
            let mut item = list;
            while !item.is_null() {
                let attr = (*item).data as *const ffi::PopplerTextAttributes;
                if !attr.is_null() {
                    let name = if (*attr).font_name.is_null() {
                        String::new()
                    } else {
                        std::ffi::CStr::from_ptr((*attr).font_name).to_string_lossy().into_owned()
                    };
                    let start = (*attr).start_index.max(0) as usize;
                    let end = ((*attr).end_index.max(0) as usize).min(chars.len().saturating_sub(1));
                    for slot in fonts.iter_mut().take(end + 1).skip(start) {
                        *slot = (name.clone(), (*attr).font_size);
                    }
                }
                item = (*item).next;
            }
            ffi::poppler_page_free_text_attributes(list);
        }
        // Without a rectangle for every character no formula can be placed.
        if rects.len() != chars.len() {
            log::debug!("pdf: {} characters but {} rectangles", chars.len(), rects.len());
            rects = vec![[0.0; 4]; chars.len()];
            fonts.iter_mut().for_each(|f| f.0.clear());
        }
        Ok(chars
            .into_iter()
            .zip(rects)
            .zip(fonts)
            .map(|((ch, rect), (font, size))| Glyph { ch, rect, font, size })
            .collect())
    }

    fn outline(&mut self, path: &Path) -> Result<Vec<OutlineItem>> {
        self.outline_of(path)
    }

    fn search(&mut self, path: &Path, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        self.search_in(path, query, limit)
    }

    fn links(&mut self, path: &Path) -> Result<Vec<PageLink>> {
        self.links_of(path)
    }

    fn select(
        &mut self,
        path: &Path,
        page: usize,
        area: [f64; 4],
        style: super::SelectionStyle,
    ) -> Result<(Vec<[f64; 4]>, String)> {
        self.select_in(path, page, area, style)
    }

    fn document_id(&mut self, path: &Path) -> Result<Option<String>> {
        self.document_id_of(path)
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

impl PopplerBackend {
    pub(super) fn links_of(&mut self, path: &Path) -> Result<Vec<PageLink>> {
        let doc = self.doc(path)?;
        let doc_ptr: *mut ffi::PopplerDocument = doc.to_glib_none().0;
        let heights: Vec<f64> = (0..doc.n_pages()).map(|i| doc.page(i).map_or(842.0, |p| p.size().1)).collect();
        let mut out = Vec::new();
        for (index, height) in heights.iter().enumerate() {
            let Some(page) = doc.page(index as i32) else { continue };
            let page_ptr: *mut ffi::PopplerPage = page.to_glib_none().0;
            // SAFETY: poppler-glib owns the list until it is freed below; the
            // actions in it stay valid as long as the list does.
            unsafe {
                let list = ffi::poppler_page_get_link_mapping(page_ptr);
                let mut node = list;
                while !node.is_null() {
                    let mapping = (*node).data as *const ffi::PopplerLinkMapping;
                    node = (*node).next;
                    if mapping.is_null() || (*mapping).action.is_null() {
                        continue;
                    }
                    let action = (*mapping).action;
                    let target = match (*action).type_ {
                        ffi::POPPLER_ACTION_GOTO_DEST => {
                            dest_target(doc_ptr, (*action).goto_dest.dest).map(|(page, top)| LinkTarget::Page {
                                page,
                                top: top.map(|t| heights.get(page).copied().unwrap_or(*height) - t),
                            })
                        }
                        ffi::POPPLER_ACTION_URI if !(*action).uri.uri.is_null() => Some(LinkTarget::Uri(
                            std::ffi::CStr::from_ptr((*action).uri.uri).to_string_lossy().into_owned(),
                        )),
                        _ => None,
                    };
                    let Some(target) = target else { continue };
                    // PDF coordinates: origin bottom left.
                    let a = &(*mapping).area;
                    let (x1, x2) = (a.x1.min(a.x2), a.x1.max(a.x2));
                    let (y1, y2) = (a.y1.min(a.y2), a.y1.max(a.y2));
                    out.push(PageLink { page: index, rect: [x1, height - y2, x2 - x1, y2 - y1], target });
                }
                ffi::poppler_page_free_link_mapping(list);
            }
        }
        Ok(out)
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
    unsafe { dest_target(doc, dest).map(|(page, _)| page) }
}

/// 0-based page of a destination and, if it names one, the position on the
/// page (PDF coordinates, from the bottom).
unsafe fn dest_target(doc: *mut ffi::PopplerDocument, dest: *mut ffi::PopplerDest) -> Option<(usize, Option<f64>)> {
    unsafe {
        if dest.is_null() {
            return None;
        }
        let read = |d: *const ffi::PopplerDest| {
            let top = matches!((*d).type_, ffi::POPPLER_DEST_XYZ | ffi::POPPLER_DEST_FITH | ffi::POPPLER_DEST_FITBH)
                && (*d).top > 0.0;
            ((*d).page_num, top.then_some((*d).top))
        };
        let (page, top) = if (*dest).type_ == ffi::POPPLER_DEST_NAMED && !(*dest).named_dest.is_null() {
            let named = ffi::poppler_document_find_dest(doc, (*dest).named_dest);
            if named.is_null() {
                return None;
            }
            let found = read(named);
            ffi::poppler_dest_free(named);
            found
        } else {
            read(dest)
        };
        (page > 0).then(|| (page as usize - 1, top))
    }
}

impl PopplerBackend {
    pub(super) fn select_in(
        &mut self,
        path: &Path,
        page: usize,
        area: [f64; 4],
        style: super::SelectionStyle,
    ) -> Result<(Vec<[f64; 4]>, String)> {
        // Regions come in integer pixels at `SCALE`; a quarter point is
        // precise enough for highlights.
        const SCALE: f64 = 4.0;
        let doc = self.doc(path)?;
        let page = page_at(doc, page as i64).context("page out of range")?;
        let mut rect = poppler::Rectangle::new();
        rect.set_x1(area[0]);
        rect.set_y1(area[1]);
        rect.set_x2(area[2]);
        rect.set_y2(area[3]);
        let style = match style {
            super::SelectionStyle::Glyph => poppler::SelectionStyle::Glyph,
            super::SelectionStyle::Word => poppler::SelectionStyle::Word,
            super::SelectionStyle::Line => poppler::SelectionStyle::Line,
        };
        let mut rects = Vec::new();
        if let Some(region) = page.selected_region(SCALE, style, &mut rect) {
            for i in 0..region.num_rectangles() {
                let r = region.rectangle(i);
                rects.push([
                    f64::from(r.x()) / SCALE,
                    f64::from(r.y()) / SCALE,
                    f64::from(r.width()) / SCALE,
                    f64::from(r.height()) / SCALE,
                ]);
            }
        }
        let text = page.selected_text(style, &mut rect).map(|t| t.to_string()).unwrap_or_default();
        Ok((merge_lines(rects), text))
    }

    pub(super) fn document_id_of(&mut self, path: &Path) -> Result<Option<String>> {
        let doc = self.doc(path)?;
        let doc_ptr: *mut ffi::PopplerDocument = doc.to_glib_none().0;
        let mut permanent: *mut std::ffi::c_char = std::ptr::null_mut();
        let mut update: *mut std::ffi::c_char = std::ptr::null_mut();
        // SAFETY: poppler returns two g_malloc'ed buffers of 32 bytes (the
        // hex encoded IDs, not NUL terminated) or FALSE.
        unsafe {
            let ok = ffi::poppler_document_get_id(doc_ptr, &mut permanent, &mut update);
            let id = (ok != 0 && !permanent.is_null()).then(|| {
                let bytes = std::slice::from_raw_parts(permanent as *const u8, 32);
                if bytes.iter().all(u8::is_ascii_hexdigit) {
                    String::from_utf8_lossy(bytes).to_ascii_lowercase()
                } else {
                    bytes.iter().map(|b| format!("{b:02x}")).collect()
                }
            });
            glib::ffi::g_free(permanent.cast());
            glib::ffi::g_free(update.cast());
            Ok(id)
        }
    }
}

/// Joins rectangles of one text line (same band) into one rectangle each,
/// so a highlight is drawn as one bar per line.
fn merge_lines(mut rects: Vec<[f64; 4]>) -> Vec<[f64; 4]> {
    rects.sort_by(|a, b| a[1].total_cmp(&b[1]).then(a[0].total_cmp(&b[0])));
    let mut out: Vec<[f64; 4]> = Vec::new();
    for r in rects {
        if let Some(last) = out.last_mut()
            && (last[1] - r[1]).abs() < 0.5
            && (last[3] - r[3]).abs() < 0.5
            && r[0] <= last[0] + last[2] + 2.0
        {
            let right = (last[0] + last[2]).max(r[0] + r[2]);
            last[2] = right - last[0];
            continue;
        }
        out.push(r);
    }
    out
}

fn parse_color(color: &str) -> ffi::PopplerColor {
    let hex = color.trim_start_matches('#');
    let channel = |i: usize| hex.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok()).unwrap_or(0) as u16 * 257;
    if hex.len() != 6 {
        return ffi::PopplerColor { red: 65535, green: 54484, blue: 0 };
    }
    ffi::PopplerColor { red: channel(0), green: channel(2), blue: channel(4) }
}

/// Writes a copy of `src` with `annotations` as real PDF annotations to
/// `dst` (bookmarks have no PDF equivalent and are skipped). The original is
/// opened separately and never written. Returns the number of added
/// annotations.
pub(super) fn export_annotated(
    src: &Path,
    dst: &Path,
    annotations: &[crate::annotations::Annotation],
) -> Result<usize> {
    use crate::annotations::Kind;
    anyhow::ensure!(
        src.canonicalize().ok() != dst.canonicalize().ok(),
        "Der Export darf die Original-PDF nicht überschreiben."
    );
    let uri = glib::filename_to_uri(src, None).context("invalid path")?;
    let doc = poppler::Document::from_file(&uri, None).map_err(|e| anyhow::anyhow!("poppler: {e}"))?;
    let doc_ptr: *mut ffi::PopplerDocument = doc.to_glib_none().0;
    let mut added = 0;
    for a in annotations {
        let Some(page) = page_at(&doc, i64::from(a.page) - 1) else { continue };
        let height = page.size().1;
        let page_ptr: *mut ffi::PopplerPage = page.to_glib_none().0;
        // Top-left points to PDF coordinates (origin bottom left).
        let pdf_rect = |r: &[f64; 4]| ffi::PopplerRectangle {
            x1: r[0],
            y1: height - (r[1] + r[3]),
            x2: r[0] + r[2],
            y2: height - r[1],
        };
        // SAFETY: plain poppler-glib calls on live objects; every created
        // object is released before the next iteration.
        unsafe {
            let annot: *mut ffi::PopplerAnnot = match a.kind {
                kind if kind.is_text_markup() => {
                    if a.rects.is_empty() {
                        continue;
                    }
                    let quads = glib::ffi::g_array_sized_new(
                        0,
                        0,
                        std::mem::size_of::<ffi::PopplerQuadrilateral>() as u32,
                        a.rects.len() as u32,
                    );
                    let mut bounds = pdf_rect(&a.rects[0]);
                    for r in &a.rects {
                        let p = pdf_rect(r);
                        bounds.x1 = bounds.x1.min(p.x1);
                        bounds.y1 = bounds.y1.min(p.y1);
                        bounds.x2 = bounds.x2.max(p.x2);
                        bounds.y2 = bounds.y2.max(p.y2);
                        let quad = ffi::PopplerQuadrilateral {
                            p1: ffi::PopplerPoint { x: p.x1, y: p.y2 },
                            p2: ffi::PopplerPoint { x: p.x2, y: p.y2 },
                            p3: ffi::PopplerPoint { x: p.x1, y: p.y1 },
                            p4: ffi::PopplerPoint { x: p.x2, y: p.y1 },
                        };
                        glib::ffi::g_array_append_vals(quads, (&quad as *const ffi::PopplerQuadrilateral).cast(), 1);
                    }
                    let annot = match kind {
                        Kind::Underline => ffi::poppler_annot_text_markup_new_underline(doc_ptr, &mut bounds, quads),
                        Kind::Strikeout => ffi::poppler_annot_text_markup_new_strikeout(doc_ptr, &mut bounds, quads),
                        _ => ffi::poppler_annot_text_markup_new_highlight(doc_ptr, &mut bounds, quads),
                    };
                    glib::ffi::g_array_unref(quads);
                    annot
                }
                Kind::Note => {
                    let r = a.rects.first().copied().unwrap_or([20.0, 20.0, 20.0, 20.0]);
                    let mut rect = pdf_rect(&[r[0], r[1], r[2].max(16.0), r[3].max(16.0)]);
                    ffi::poppler_annot_text_new(doc_ptr, &mut rect)
                }
                Kind::Ink => {
                    let width = a.width.unwrap_or(2.0);
                    let points: Vec<Vec<ffi::PopplerPoint>> = a
                        .ink
                        .iter()
                        .filter(|s| !s.is_empty())
                        .map(|s| s.iter().map(|p| ffi::PopplerPoint { x: p[0], y: height - p[1] }).collect())
                        .collect();
                    if points.is_empty() {
                        continue;
                    }
                    let all = points.iter().flatten();
                    let (mut x1, mut y1, mut x2, mut y2) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                    for p in all {
                        x1 = x1.min(p.x);
                        y1 = y1.min(p.y);
                        x2 = x2.max(p.x);
                        y2 = y2.max(p.y);
                    }
                    let mut rect =
                        ffi::PopplerRectangle { x1: x1 - width, y1: y1 - width, x2: x2 + width, y2: y2 + width };
                    let annot = ffi::poppler_annot_ink_new(doc_ptr, &mut rect);
                    if annot.is_null() {
                        continue;
                    }
                    // The ink list is converted with the page geometry, so
                    // the annotation must be on its page first.
                    ffi::poppler_page_add_annot(page_ptr, annot);
                    let mut paths: Vec<*mut ffi::PopplerPath> = points
                        .iter()
                        .map(|s| {
                            // The path takes ownership of the points and
                            // g_free()s them.
                            let buf = glib::ffi::g_malloc_n(s.len(), std::mem::size_of::<ffi::PopplerPoint>())
                                as *mut ffi::PopplerPoint;
                            std::ptr::copy_nonoverlapping(s.as_ptr(), buf, s.len());
                            ffi::poppler_path_new_from_array(buf, s.len())
                        })
                        .collect();
                    ffi::poppler_annot_ink_set_ink_list(annot.cast(), paths.as_mut_ptr(), paths.len());
                    for p in paths {
                        ffi::poppler_path_free(p);
                    }
                    ffi::poppler_annot_set_border_width(annot, width);
                    annot
                }
                _ => continue,
            };
            if annot.is_null() {
                continue;
            }
            let mut color = parse_color(&a.color);
            ffi::poppler_annot_set_color(annot, &mut color);
            let contents = if a.comment.is_empty() { a.text.clone() } else { a.comment.clone() };
            if let Ok(c) = std::ffi::CString::new(contents) {
                ffi::poppler_annot_set_contents(annot, c.as_ptr());
            }
            if a.kind != Kind::Ink {
                ffi::poppler_page_add_annot(page_ptr, annot);
            }
            glib::gobject_ffi::g_object_unref(annot.cast());
            added += 1;
        }
    }
    let dst_uri = glib::filename_to_uri(dst, None).context("invalid path")?;
    doc.save(&dst_uri).map_err(|e| anyhow::anyhow!("PDF kann nicht geschrieben werden: {e}"))?;
    Ok(added)
}

/// Annotations stored inside a PDF: (0-based page, poppler annotation type).
pub(super) fn list_annotations(path: &Path) -> Result<Vec<(usize, i32)>> {
    let uri = glib::filename_to_uri(path, None).context("invalid path")?;
    let doc = poppler::Document::from_file(&uri, None).map_err(|e| anyhow::anyhow!("poppler: {e}"))?;
    let mut out = Vec::new();
    for index in 0..doc.n_pages() {
        let Some(page) = doc.page(index) else { continue };
        let page_ptr: *mut ffi::PopplerPage = page.to_glib_none().0;
        // SAFETY: the list and its mappings are owned by us and freed below.
        unsafe {
            let list = ffi::poppler_page_get_annot_mapping(page_ptr);
            let mut node = list;
            while !node.is_null() {
                let mapping = (*node).data as *mut ffi::PopplerAnnotMapping;
                if !mapping.is_null() && !(*mapping).annot.is_null() {
                    out.push((index as usize, ffi::poppler_annot_get_annot_type((*mapping).annot)));
                }
                node = (*node).next;
            }
            ffi::poppler_page_free_annot_mapping(list);
        }
    }
    Ok(out)
}
