// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! QML types for PDFs: `PdfDocument` (page sizes,
//! open timing) and `PdfPage` (one asynchronously rendered page).

use std::path::PathBuf;
use std::pin::Pin;
use std::time::Instant;

use atlas_core::pdf::{CancelToken, PdfService, RenderedPage};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QImage, QImageFormat, QRect, QString};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!(<QtQuick/QQuickPaintedItem>);
        type QQuickPaintedItem;
        include!("cxx-qt-lib/qpainter.h");
        type QPainter = cxx_qt_lib::QPainter;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
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

        #[qsignal]
        #[cxx_name = "textReady"]
        fn text_ready(self: Pin<&mut PdfDocument>, page: i32, text: QString, ms: i32);
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QQuickPaintedItem]
        #[qproperty(QString, source, READ, WRITE, NOTIFY)]
        #[qproperty(i32, page, READ, WRITE, NOTIFY)]
        #[qproperty(i32, render_width, cxx_name = "renderWidth", READ, WRITE, NOTIFY)]
        #[qproperty(bool, rendered, READ, NOTIFY)]
        #[qproperty(i32, render_ms, cxx_name = "renderMs", READ, NOTIFY)]
        #[qproperty(QString, error, READ, NOTIFY)]
        type PdfPage = super::PdfPageRust;

        #[cxx_override]
        unsafe fn paint(self: Pin<&mut PdfPage>, painter: *mut QPainter);

        #[inherit]
        fn update(self: Pin<&mut PdfPage>);
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
                        doc.set_status("ready", "");
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
        PdfService::get().text(path, page.max(0) as usize, move |result| {
            let ms = started.elapsed().as_millis() as i32;
            let text = result.unwrap_or_else(|e| format!("[{e:#}]"));
            let _ = thread.queue(move |doc| doc.text_ready(page, QString::from(text.as_str()), ms));
        });
    }

    pub fn reload(self: Pin<&mut Self>) {
        let path = PathBuf::from(self.source.to_string());
        PdfService::get().close(path);
        self.load();
    }
}

#[derive(Default)]
pub struct PdfPageRust {
    source: QString,
    page: i32,
    render_width: i32,
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
        let thread = self.qt_thread();
        PdfService::get().render(path, self.page as usize, width as u32, token, move |result| {
            let _ = thread.queue(move |page| page.apply_render(generation, result));
        });
    }

    fn apply_render(mut self: Pin<&mut Self>, generation: u64, result: anyhow::Result<RenderedPage>) {
        if generation != self.generation {
            return;
        }
        match result {
            Ok(page) => {
                // SAFETY: `bgra` holds width*height premultiplied BGRA pixels,
                // which is Format_ARGB32_Premultiplied on little endian.
                let image = unsafe {
                    QImage::from_raw_bytes(
                        page.bgra,
                        page.width as i32,
                        page.height as i32,
                        QImageFormat::Format_ARGB32_Premultiplied,
                    )
                };
                {
                    let mut this = self.as_mut().rust_mut();
                    this.image = Some(image);
                    this.render_ms = page.elapsed.as_millis() as i32;
                    this.rendered = true;
                    this.error = QString::default();
                }
                self.as_mut().render_ms_changed();
                self.as_mut().rendered_changed();
                self.as_mut().error_changed();
            }
            Err(err) => {
                self.as_mut().rust_mut().error = QString::from(format!("{err:#}").as_str());
                self.as_mut().error_changed();
            }
        }
        self.update();
    }

    unsafe fn paint(self: Pin<&mut Self>, painter: *mut cxx_qt_lib::QPainter) {
        let Some(image) = self.image.as_ref() else { return };
        // SAFETY: Qt passes a valid, active painter for the duration of paint().
        let Some(painter) = (unsafe { painter.as_mut() }) else { return };
        let mut painter = unsafe { Pin::new_unchecked(painter) };
        painter.as_mut().set_render_hint(cxx_qt_lib::QPainterRenderHint::SmoothPixmapTransform, true);
        let rect = QRect::new(0, 0, self.width().round() as i32, self.height().round() as i32);
        painter.draw_image(&rect, image);
    }
}
