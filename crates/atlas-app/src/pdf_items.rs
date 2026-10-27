// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! QML types for PDFs: `PdfDocument` (page sizes,
//! open timing) and `PdfPage` (one asynchronously rendered page).

// `PdfDocument::select` takes a start and an end point with page each.
#![allow(clippy::too_many_arguments)]

use std::path::PathBuf;
use std::pin::Pin;
use std::time::Instant;

use atlas_core::pdf::{CancelToken, PdfService, RenderedPage, SelectionStyle, formulas, selection_spans};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QColor, QImage, QImageFormat, QPainterCompositionMode, QRect, QRectF, QSize, QString};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!(<QtQuick/QQuickPaintedItem>);
        type QQuickPaintedItem;
        include!("cxx-qt-lib/qpainter.h");
        type QPainter = cxx_qt_lib::QPainter;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qsize.h");
        type QSize = cxx_qt_lib::QSize;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, source, READ, WRITE, NOTIFY)]
        #[qproperty(i32, page_count, cxx_name = "pageCount", READ, NOTIFY)]
        #[qproperty(QString, status, READ, NOTIFY)]
        #[qproperty(QString, error, READ, NOTIFY)]
        #[qproperty(i32, open_ms, cxx_name = "openMs", READ, NOTIFY)]
        #[qproperty(f64, max_page_width, cxx_name = "maxPageWidth", READ, NOTIFY)]
        /// Outline as JSON: [{title, page, depth}], page -1 if unknown.
        #[qproperty(QString, outline_json, cxx_name = "outlineJson", READ, NOTIFY)]
        /// Links as JSON: [{page, rect: [x, y, w, h], target, top, uri}] in
        /// points from the top left; target -1 for web addresses, top -1 if
        /// the link names no position.
        #[qproperty(QString, links_json, cxx_name = "linksJson", READ, NOTIFY)]
        type PdfDocument = super::PdfDocumentRust;

        /// Page width in PDF points.
        #[qinvokable]
        #[cxx_name = "pageWidth"]
        fn page_width(self: &PdfDocument, page: i32) -> f64;

        /// Page height in PDF points.
        #[qinvokable]
        #[cxx_name = "pageHeight"]
        fn page_height(self: &PdfDocument, page: i32) -> f64;

        /// Extracts the text of `page`; the result arrives via `textReady`.
        #[qinvokable]
        #[cxx_name = "extractText"]
        fn extract_text(self: &PdfDocument, page: i32);

        /// Drops cached state and opens the file again.
        #[qinvokable]
        fn reload(self: Pin<&mut PdfDocument>);

        /// The text as the PDF has it; `formulas`: how many formulas it
        /// holds, which then come as LaTeX via `formulaText`.
        #[qsignal]
        #[cxx_name = "textReady"]
        fn text_ready(self: Pin<&mut PdfDocument>, page: i32, text: QString, ms: i32, formulas: i32);

        /// The text inside `rects` (JSON `[[x0, y0, x1, y1]]`, points) of
        /// `page` with its formulas as LaTeX, e.g. for a quote; arrives via
        /// `selectionLatexReady` (text "" if it holds no formula).
        #[qinvokable]
        #[cxx_name = "selectionLatex"]
        fn selection_latex(self: &PdfDocument, request: i32, page: i32, rects: &QString);

        #[qsignal]
        #[cxx_name = "selectionLatexReady"]
        fn selection_latex_ready(self: Pin<&mut PdfDocument>, request: i32, text: QString, formulas: i32);

        /// How far the LaTeX recognition of the formulas of `page` (-1: of
        /// a selection) is.
        #[qsignal]
        #[cxx_name = "formulaProgress"]
        fn formula_progress(self: Pin<&mut PdfDocument>, page: i32, text: QString);

        /// The text of `page` with its formulas as LaTeX (`$…$`, `$$…$$`),
        /// or `error` (then `text` is the plain text).
        #[qsignal]
        #[cxx_name = "formulaText"]
        fn formula_text(self: Pin<&mut PdfDocument>, page: i32, text: QString, error: QString);

        /// Searches the whole document; results arrive via `searchFinished`.
        #[qinvokable]
        fn search(self: Pin<&mut PdfDocument>, query: &QString);

        /// `hits_json`: [{page, x, y, w, h}] in PDF points, origin top left.
        #[qsignal]
        #[cxx_name = "searchFinished"]
        fn search_finished(self: Pin<&mut PdfDocument>, query: QString, hits_json: QString, ms: i32);

        /// Selects text from (page1, x1, y1) to (page2, x2, y2) – 0-based
        /// pages, points from the top left – snapping to glyphs (0), words
        /// (1) or lines (2). Returns a request number; the result arrives via
        /// `selectionReady` (older requests are dropped).
        #[qinvokable]
        fn select(
            self: Pin<&mut PdfDocument>,
            page1: i32,
            x1: f64,
            y1: f64,
            page2: i32,
            x2: f64,
            y2: f64,
            style: i32,
        ) -> i32;

        /// `json`: [{page, rects: [[x, y, w, h]], text}] (0-based pages).
        #[qsignal]
        #[cxx_name = "selectionReady"]
        fn selection_ready(self: Pin<&mut PdfDocument>, request: i32, json: QString);
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QQuickPaintedItem]
        #[qproperty(QString, source, READ, WRITE, NOTIFY)]
        #[qproperty(i32, page, READ, WRITE, NOTIFY)]
        #[qproperty(i32, render_width, cxx_name = "renderWidth", READ, WRITE, NOTIFY)]
        /// Dark mode: brightness inverted, hues kept.
        #[qproperty(bool, inverted, READ, WRITE, NOTIFY)]
        #[qproperty(bool, rendered, READ, NOTIFY)]
        #[qproperty(i32, render_ms, cxx_name = "renderMs", READ, NOTIFY)]
        #[qproperty(QString, error, READ, NOTIFY)]
        type PdfPage = super::PdfPageRust;

        #[cxx_override]
        unsafe fn paint(self: Pin<&mut PdfPage>, painter: *mut QPainter);

        #[inherit]
        fn update(self: Pin<&mut PdfPage>);
        /// Fixed to the rendered image: zooming or resizing then only scales
        /// the texture instead of painting the page anew on every frame.
        #[inherit]
        #[cxx_name = "setTextureSize"]
        fn set_texture_size(self: Pin<&mut PdfPage>, size: &QSize);
        #[inherit]
        fn width(self: &PdfPage) -> f64;
        #[inherit]
        fn height(self: &PdfPage) -> f64;
    }

    impl cxx_qt::Threading for PdfDocument {}
    impl cxx_qt::Initialize for PdfDocument {}
    impl cxx_qt::Threading for PdfPage {}
    impl cxx_qt::Initialize for PdfPage {}
}

pub struct PdfDocumentRust {
    source: QString,
    page_count: i32,
    status: QString,
    error: QString,
    open_ms: i32,
    max_page_width: f64,
    page_sizes: Vec<(f64, f64)>,
    generation: u64,
    outline_json: QString,
    links_json: QString,
    search_cancel: CancelToken,
    select_cancel: CancelToken,
    select_request: i32,
    /// The document this viewer keeps open in the PDF service.
    retained: Option<PathBuf>,
}

impl Drop for PdfDocumentRust {
    fn drop(&mut self) {
        if let Some(path) = self.retained.take() {
            PdfService::get().release(&path);
        }
    }
}

impl Default for PdfDocumentRust {
    fn default() -> Self {
        Self {
            source: QString::default(),
            page_count: 0,
            status: QString::default(),
            error: QString::default(),
            open_ms: 0,
            max_page_width: 0.0,
            page_sizes: Vec::new(),
            generation: 0,
            outline_json: QString::default(),
            links_json: QString::default(),
            search_cancel: CancelToken::default(),
            select_cancel: CancelToken::default(),
            select_request: 0,
            retained: None,
        }
    }
}

impl cxx_qt::Initialize for qobject::PdfDocument {
    fn initialize(mut self: Pin<&mut Self>) {
        self.as_mut().on_source_changed(|doc| doc.load()).release();
    }
}

impl qobject::PdfDocument {
    fn set_status(mut self: Pin<&mut Self>, status: &str, error: &str) {
        self.as_mut().rust_mut().status = QString::from(status);
        self.as_mut().rust_mut().error = QString::from(error);
        self.as_mut().status_changed();
        self.as_mut().error_changed();
    }

    fn load(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().generation += 1;
        let generation = self.generation;
        let path = PathBuf::from(self.source.to_string());
        // Keep the new document open, let go of the old one.
        if self.retained.as_deref() != Some(path.as_path()) {
            if !path.as_os_str().is_empty() {
                PdfService::get().retain(&path);
            }
            let old = std::mem::replace(
                &mut self.as_mut().rust_mut().retained,
                (!path.as_os_str().is_empty()).then(|| path.clone()),
            );
            if let Some(old) = old {
                PdfService::get().release(&old);
            }
        }
        if path.as_os_str().is_empty() {
            self.set_status("", "");
            return;
        }
        self.as_mut().set_status("loading", "");
        let thread = self.qt_thread();
        PdfService::get().open(path, move |result| {
            let _ = thread.queue(move |mut doc| {
                if doc.generation != generation {
                    return;
                }
                match result {
                    Ok(info) => {
                        {
                            let mut this = doc.as_mut().rust_mut();
                            this.page_count = info.page_sizes.len() as i32;
                            this.max_page_width = info.page_sizes.iter().map(|s| s.0).fold(0.0, f64::max);
                            this.page_sizes = info.page_sizes;
                            this.open_ms = info.open_time.as_millis() as i32;
                        }
                        doc.as_mut().page_count_changed();
                        doc.as_mut().max_page_width_changed();
                        doc.as_mut().open_ms_changed();
                        doc.as_mut().set_status("ready", "");
                        doc.as_mut().load_outline(generation);
                        doc.load_links(generation);
                    }
                    Err(err) => {
                        doc.as_mut().rust_mut().page_count = 0;
                        doc.as_mut().page_count_changed();
                        doc.set_status("error", &format!("{err:#}"));
                    }
                }
            });
        });
    }

    fn load_outline(self: Pin<&mut Self>, generation: u64) {
        let thread = self.qt_thread();
        PdfService::get().outline(PathBuf::from(self.source.to_string()), move |result| {
            let json = match result {
                Ok(items) => serde_json::Value::Array(
                    items
                        .into_iter()
                        .map(|i| serde_json::json!({ "title": i.title, "page": i.page.map_or(-1, |p| p as i64), "depth": i.depth }))
                        .collect(),
                )
                .to_string(),
                Err(_) => "[]".to_owned(),
            };
            let _ = thread.queue(move |mut doc| {
                if doc.generation == generation {
                    doc.as_mut().rust_mut().outline_json = QString::from(json.as_str());
                    doc.outline_json_changed();
                }
            });
        });
    }

    fn load_links(self: Pin<&mut Self>, generation: u64) {
        use atlas_core::pdf::LinkTarget;
        let thread = self.qt_thread();
        PdfService::get().links(PathBuf::from(self.source.to_string()), move |result| {
            let links: Vec<serde_json::Value> = result
                .unwrap_or_default()
                .into_iter()
                .map(|l| {
                    let (target, top, uri) = match l.target {
                        LinkTarget::Page { page, top } => (page as i64, top.unwrap_or(-1.0), String::new()),
                        LinkTarget::Uri(uri) => (-1, -1.0, uri),
                    };
                    serde_json::json!({ "page": l.page, "rect": l.rect, "target": target, "top": top, "uri": uri })
                })
                .collect();
            let json = serde_json::Value::Array(links).to_string();
            let _ = thread.queue(move |mut doc| {
                if doc.generation == generation {
                    doc.as_mut().rust_mut().links_json = QString::from(json.as_str());
                    doc.links_json_changed();
                }
            });
        });
    }

    pub fn search(mut self: Pin<&mut Self>, query: &QString) {
        self.search_cancel.cancel();
        let token = CancelToken::default();
        self.as_mut().rust_mut().search_cancel = token.clone();
        let text = query.to_string();
        let query = query.clone();
        if text.trim().is_empty() {
            self.search_finished(query, QString::from("[]"), 0);
            return;
        }
        let started = Instant::now();
        let thread = self.qt_thread();
        PdfService::get().search(PathBuf::from(self.source.to_string()), text, token, move |result| {
            let hits: Vec<serde_json::Value> = result
                .unwrap_or_default()
                .into_iter()
                .map(|h| serde_json::json!({ "page": h.page, "x": h.x, "y": h.y, "w": h.width, "h": h.height }))
                .collect();
            let json = serde_json::Value::Array(hits).to_string();
            let ms = started.elapsed().as_millis() as i32;
            let _ = thread.queue(move |doc| doc.search_finished(query, QString::from(json.as_str()), ms));
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn select(
        mut self: Pin<&mut Self>,
        page1: i32,
        x1: f64,
        y1: f64,
        page2: i32,
        x2: f64,
        y2: f64,
        style: i32,
    ) -> i32 {
        self.select_cancel.cancel();
        let token = CancelToken::default();
        let request = self.select_request.wrapping_add(1);
        {
            let mut this = self.as_mut().rust_mut();
            this.select_cancel = token.clone();
            this.select_request = request;
        }
        let style = match style {
            1 => SelectionStyle::Word,
            2 => SelectionStyle::Line,
            _ => SelectionStyle::Glyph,
        };
        let spans = selection_spans(&self.page_sizes, (page1.max(0) as usize, x1, y1), (page2.max(0) as usize, x2, y2));
        let thread = self.qt_thread();
        PdfService::get().select(PathBuf::from(self.source.to_string()), spans, style, token, move |result| {
            let list: Vec<serde_json::Value> = result
                .unwrap_or_default()
                .into_iter()
                .map(|s| serde_json::json!({ "page": s.page, "rects": s.rects, "text": s.text }))
                .collect();
            let json = serde_json::Value::Array(list).to_string();
            let _ = thread.queue(move |doc| {
                if doc.select_request == request {
                    doc.selection_ready(request, QString::from(json.as_str()));
                }
            });
        });
        request
    }

    pub fn page_width(&self, page: i32) -> f64 {
        self.page_sizes.get(page.max(0) as usize).map_or(595.0, |s| s.0)
    }

    pub fn page_height(&self, page: i32) -> f64 {
        self.page_sizes.get(page.max(0) as usize).map_or(842.0, |s| s.1)
    }

    pub fn extract_text(&self, page: i32) {
        let thread = self.qt_thread();
        let started = Instant::now();
        let path = PathBuf::from(self.source.to_string());
        PdfService::get().page_formulas(path, page.max(0) as usize, move |result| {
            let ms = started.elapsed().as_millis() as i32;
            let (text, clips) = match result {
                Ok(found) => found,
                Err(e) => {
                    let text = format!("[{e:#}]");
                    let _ = thread.queue(move |doc| doc.text_ready(page, QString::from(text.as_str()), ms, 0));
                    return;
                }
            };
            let count = text.formulas.len() as i32;
            let plain = text.plain();
            let _ = thread.queue(move |doc| doc.text_ready(page, QString::from(plain.as_str()), ms, count));
            if count == 0 {
                return;
            }
            // The recognition takes a while: not on the PDF thread.
            std::thread::spawn(move || {
                let progress = |message: String| {
                    let _ = thread.queue(move |doc| doc.formula_progress(page, QString::from(message.as_str())));
                };
                let (text, error) = recognize_formulas(&text, clips, page, &progress);
                let _ = thread.queue(move |doc| {
                    doc.formula_text(page, QString::from(text.as_str()), QString::from(error.as_str()))
                });
            });
        });
    }

    pub fn selection_latex(&self, request: i32, page: i32, rects: &QString) {
        let thread = self.qt_thread();
        let path = PathBuf::from(self.source.to_string());
        let rects: Vec<[f64; 4]> = serde_json::from_str(&rects.to_string()).unwrap_or_default();
        PdfService::get().area_formulas(path, page.max(0) as usize, rects, move |result| {
            let Ok((text, clips)) = result.inspect_err(|e| log::warn!("pdf: selection formulas: {e:#}")) else {
                let _ = thread.queue(move |doc| doc.selection_latex_ready(request, QString::default(), 0));
                return;
            };
            let count = text.formulas.len() as i32;
            if count == 0 {
                let _ = thread.queue(move |doc| doc.selection_latex_ready(request, QString::default(), 0));
                return;
            }
            std::thread::spawn(move || {
                let progress = |message: String| {
                    let _ = thread.queue(move |doc| doc.formula_progress(-1, QString::from(message.as_str())));
                };
                let (latex, error) = recognize_formulas(&text, clips, page, &progress);
                if !error.is_empty() {
                    log::warn!("pdf: quote formulas: {error}");
                }
                let _ =
                    thread.queue(move |doc| doc.selection_latex_ready(request, QString::from(latex.as_str()), count));
            });
        });
    }

    pub fn reload(self: Pin<&mut Self>) {
        let path = PathBuf::from(self.source.to_string());
        PdfService::get().close(path);
        self.load();
    }
}

/// Recognises the formulas of `text` (their pictures in `clips`) with the
/// built-in model, downloading it the first time: the text with LaTeX, and
/// an error ("" if all were recognised; the plain text if none could be).
fn recognize_formulas(
    text: &formulas::PageText,
    clips: Vec<(u32, u32, Vec<u8>)>,
    page: i32,
    progress: &dyn Fn(String),
) -> (String, String) {
    let count = clips.len();
    let outcome = crate::app::ensure_latex_models(progress).map(|()| {
        let mut latex = Vec::with_capacity(count);
        let mut failed = 0;
        for (i, (w, h, pixels)) in clips.into_iter().enumerate() {
            progress(format!("Erkenne Formel {} von {count} …", i + 1));
            let found = if w == 0 {
                Err(anyhow::anyhow!("nicht gerendert"))
            } else {
                atlas_core::latex_ocr::recognize_gray(w, h, pixels)
            };
            match found {
                Ok(tex) if !tex.trim().is_empty() => latex.push(Some(formulas::tidy(&tex))),
                other => {
                    if let Err(e) = other {
                        log::warn!("pdf: Formel {} auf Seite {}: {e:#}", i + 1, page + 1);
                    }
                    failed += 1;
                    latex.push(None);
                }
            }
        }
        (text.with_latex(&latex), failed)
    });
    match outcome {
        Ok((text, 0)) => (text, String::new()),
        Ok((text, failed)) => (text, format!("{failed} von {count} Formeln nicht erkannt")),
        Err(e) => (text.plain(), format!("{e:#}")),
    }
}

/// Rendered pages recently shown, so scrolling back (or reopening a PDF)
/// shows them at once instead of rendering again. Keyed by file, its
/// modification time (a changed file misses), page, width and dark mode;
/// the oldest go first beyond `CACHE_BYTES`. GUI thread only.
mod page_cache {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};
    use std::time::SystemTime;

    use cxx_qt_lib::QImage;

    /// Pixels kept for scrolling back (counted as 4 bytes each; images a
    /// viewer still shows share them).
    const CACHE_BYTES: usize = 128 << 20;

    #[derive(Clone, PartialEq)]
    pub struct Key {
        path: PathBuf,
        modified: Option<SystemTime>,
        page: i32,
        width: i32,
        inverted: bool,
    }

    impl Key {
        pub fn inverted(&self) -> bool {
            self.inverted
        }

        pub fn new(path: &Path, page: i32, width: i32, inverted: bool) -> Key {
            let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
            Key { path: path.to_path_buf(), modified, page, width, inverted }
        }
    }

    struct Entry {
        key: Key,
        image: QImage,
        height: i32,
        bytes: usize,
    }

    type Cache = (VecDeque<Entry>, usize);

    thread_local! {
        static CACHE: RefCell<Cache> = const { RefCell::new((VecDeque::new(), 0)) };
    }

    /// The image and its height for `key`; it becomes the newest.
    pub fn get(key: &Key) -> Option<(QImage, i32)> {
        CACHE.with(|c| {
            let mut c = c.borrow_mut();
            let at = c.0.iter().position(|e| &e.key == key)?;
            let entry = c.0.remove(at)?;
            let found = (entry.image.clone(), entry.height);
            c.0.push_back(entry);
            Some(found)
        })
    }

    pub fn put(key: Key, image: &QImage, height: i32) {
        let bytes = key.width.max(0) as usize * height.max(0) as usize * 4;
        if bytes > CACHE_BYTES / 4 {
            return;
        }
        CACHE.with(|c| {
            let mut c = c.borrow_mut();
            // The same page, and every page of an older version of the file.
            let mut freed = 0;
            c.0.retain(|e| {
                let stale = e.key.path == key.path
                    && (e.key.modified != key.modified || (e.key.page == key.page && e.key == key));
                if stale {
                    freed += e.bytes;
                }
                !stale
            });
            c.1 -= freed;
            // Room first, then the new one: the budget is never exceeded.
            while c.1 + bytes > CACHE_BYTES {
                let Some(old) = c.0.pop_front() else { break };
                c.1 -= old.bytes;
            }
            c.1 += bytes;
            c.0.push_back(Entry { key, image: image.clone(), height, bytes });
        })
    }
}

#[derive(Default)]
pub struct PdfPageRust {
    source: QString,
    page: i32,
    render_width: i32,
    inverted: bool,
    /// Whether `image` was rendered for dark mode.
    image_inverted: bool,
    rendered: bool,
    render_ms: i32,
    error: QString,
    image: Option<QImage>,
    cancel: CancelToken,
    generation: u64,
}

impl Drop for PdfPageRust {
    fn drop(&mut self) {
        // A page that scrolled out of view must not keep the engine busy.
        self.cancel.cancel();
    }
}

impl cxx_qt::Initialize for qobject::PdfPage {
    fn initialize(mut self: Pin<&mut Self>) {
        self.as_mut().on_source_changed(|p| p.schedule_render()).release();
        self.as_mut().on_page_changed(|p| p.schedule_render()).release();
        self.as_mut().on_render_width_changed(|p| p.schedule_render()).release();
        // Shown at once by the painter, then rendered anew in the new mode
        // (painting the conversion on every frame made resizing sluggish).
        self.as_mut()
            .on_inverted_changed(|mut p| {
                p.as_mut().update();
                p.schedule_render();
            })
            .release();
    }
}

impl qobject::PdfPage {
    fn schedule_render(mut self: Pin<&mut Self>) {
        self.cancel.cancel();
        let path = PathBuf::from(self.source.to_string());
        let width = self.render_width;
        if path.as_os_str().is_empty() || width <= 0 || self.page < 0 {
            return;
        }
        let token = CancelToken::default();
        {
            let mut this = self.as_mut().rust_mut();
            this.cancel = token.clone();
            this.generation += 1;
        }
        let generation = self.generation;
        let inverted = self.inverted;
        let key = page_cache::Key::new(&path, self.page, width, inverted);
        if let Some((image, height)) = page_cache::get(&key) {
            self.show_image(image, width, height, inverted, 0);
            return;
        }
        let thread = self.qt_thread();
        PdfService::get().render(path, self.page as usize, width as u32, token, move |mut result| {
            // Converted once here on the render thread, not on every paint.
            if inverted && let Ok(page) = result.as_mut() {
                page.darken();
            }
            let _ = thread.queue(move |page| page.apply_render(generation, key, result));
        });
    }

    fn apply_render(
        mut self: Pin<&mut Self>,
        generation: u64,
        key: page_cache::Key,
        result: anyhow::Result<RenderedPage>,
    ) {
        if generation != self.generation {
            return;
        }
        match result {
            Ok(page) => {
                let (width, height) = (page.width as i32, page.height as i32);
                let ms = page.elapsed.as_millis() as i32;
                // SAFETY: `bgra` holds width*height premultiplied BGRA pixels,
                // which is Format_ARGB32_Premultiplied on little endian.
                let image = unsafe {
                    QImage::from_raw_bytes(page.bgra, width, height, QImageFormat::Format_ARGB32_Premultiplied)
                };
                let inverted = key.inverted();
                page_cache::put(key, &image, height);
                self.show_image(image, width, height, inverted, ms);
            }
            Err(err) => {
                self.as_mut().rust_mut().error = QString::from(format!("{err:#}").as_str());
                self.as_mut().error_changed();
                self.update();
            }
        }
    }

    fn show_image(mut self: Pin<&mut Self>, image: QImage, width: i32, height: i32, inverted: bool, ms: i32) {
        {
            let mut this = self.as_mut().rust_mut();
            this.image = Some(image);
            this.image_inverted = inverted;
            this.render_ms = ms;
            this.rendered = true;
            this.error = QString::default();
        }
        self.as_mut().set_texture_size(&QSize::new(width, height));
        self.as_mut().render_ms_changed();
        self.as_mut().rendered_changed();
        self.as_mut().error_changed();
        self.update();
    }

    unsafe fn paint(self: Pin<&mut Self>, painter: *mut cxx_qt_lib::QPainter) {
        let Some(image) = self.image.as_ref() else { return };
        // SAFETY: Qt passes a valid, active painter for the duration of paint().
        let Some(painter) = (unsafe { painter.as_mut() }) else { return };
        let mut painter = unsafe { Pin::new_unchecked(painter) };
        painter.as_mut().set_render_hint(cxx_qt_lib::QPainterRenderHint::SmoothPixmapTransform, true);
        let rect = QRect::new(0, 0, self.width().round() as i32, self.height().round() as i32);
        painter.as_mut().draw_image(&rect, image);
        if self.inverted != self.image_inverted {
            // Just switched, the new render is on its way: convert while
            // painting (Difference with white inverts, Lighten lifts black
            // paper to dark grey; the other way round it is near enough).
            let area = QRectF::new(0.0, 0.0, f64::from(rect.width()), f64::from(rect.height()));
            painter.as_mut().set_composition_mode(QPainterCompositionMode::CompositionMode_Difference);
            painter.as_mut().fill_rect(&area, &QColor::from_rgb(255, 255, 255));
            painter.as_mut().set_composition_mode(QPainterCompositionMode::CompositionMode_Lighten);
            painter.as_mut().fill_rect(&area, &QColor::from_rgb(0x26, 0x26, 0x26));
            painter.as_mut().set_composition_mode(QPainterCompositionMode::CompositionMode_SourceOver);
        }
    }
}
