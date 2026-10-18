// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! PDF rendering (Poppler, chosen in Phase 0, see docs/phase0.md).
//!
//! The library is not thread safe in the way the UI needs, so a worker
//! thread ("actor") owns it and all open documents. Requests are queued; the
//! result is handed to a callback on the worker thread. The engine sits
//! behind the [`Backend`] trait so that it can be exchanged later.
//!
//! Rendered pages are BGRA, premultiplied, 4 bytes per pixel, rows without
//! padding – i.e. `QImage::Format_ARGB32_Premultiplied` on little endian.

mod poppler;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_channel::{Sender, unbounded};

#[derive(Debug, Clone)]
pub struct DocInfo {
    /// Page sizes in PDF points (1/72 inch).
    pub page_sizes: Vec<(f64, f64)>,
    pub open_time: Duration,
}

#[derive(Debug, Clone)]
pub struct RenderedPage {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
    pub elapsed: Duration,
}

/// Engine specific implementation; lives on the worker thread.
trait Backend {
    fn open(&mut self, path: &Path) -> Result<Vec<(f64, f64)>>;
    fn render(&mut self, path: &Path, page: usize, width_px: u32) -> Result<(u32, u32, Vec<u8>)>;
    fn text(&mut self, path: &Path, page: usize) -> Result<String>;
    fn close(&mut self, path: &Path);
}

type Job = Box<dyn FnOnce(&mut dyn Backend, &mut HashMap<PathBuf, DocInfo>) + Send>;

/// Handle to the PDF worker thread.
pub struct PdfService {
    jobs: Sender<Job>,
}

/// Lets a queued render be skipped when its result is no longer needed
/// (page scrolled out of view, zoom changed, item destroyed).
#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

impl PdfService {
    /// The process wide service (started on first use).
    pub fn get() -> &'static PdfService {
        static SERVICE: OnceLock<PdfService> = OnceLock::new();
        SERVICE.get_or_init(PdfService::spawn)
    }

    fn spawn() -> Self {
        let (tx, rx) = unbounded::<Job>();
        std::thread::Builder::new()
            .name("atlas-pdf".into())
            .spawn(move || {
                let mut backend = poppler::PopplerBackend::default();
                let mut docs = HashMap::new();
                for job in rx {
                    job(&mut backend, &mut docs);
                }
            })
            .expect("spawn pdf worker");
        Self { jobs: tx }
    }

    fn submit(&self, job: Job) {
        // The worker only stops with the process.
        let _ = self.jobs.send(job);
    }

    /// Opens (or reuses) a document and reports its page sizes.
    pub fn open(&self, path: PathBuf, done: impl FnOnce(Result<DocInfo>) + Send + 'static) {
        self.submit(Box::new(move |backend, docs| {
            if let Some(info) = docs.get(&path) {
                return done(Ok(info.clone()));
            }
            let start = Instant::now();
            let result = backend.open(&path).map(|page_sizes| {
                let info = DocInfo { page_sizes, open_time: start.elapsed() };
                docs.insert(path.clone(), info.clone());
                info
            });
            done(result)
        }));
    }

    /// Forgets a document, e.g. after it changed on disk.
    pub fn close(&self, path: PathBuf) {
        self.submit(Box::new(move |backend, docs| {
            docs.remove(&path);
            backend.close(&path);
        }));
    }

    /// Renders `page` (0 based) at `width_px` pixels width.
    pub fn render(
        &self,
        path: PathBuf,
        page: usize,
        width_px: u32,
        cancel: CancelToken,
        done: impl FnOnce(Result<RenderedPage>) + Send + 'static,
    ) {
        self.submit(Box::new(move |backend, docs| {
            if cancel.is_cancelled() {
                return;
            }
            if !docs.contains_key(&path) {
                let start = Instant::now();
                match backend.open(&path) {
                    Ok(page_sizes) => {
                        docs.insert(path.clone(), DocInfo { page_sizes, open_time: start.elapsed() });
                    }
                    Err(err) => return done(Err(err)),
                }
            }
            let start = Instant::now();
            let result = backend.render(&path, page, width_px.max(1)).map(|(width, height, bgra)| RenderedPage {
                width,
                height,
                bgra,
                elapsed: start.elapsed(),
            });
            done(result)
        }));
    }

    /// Extracts the plain text of a page.
    pub fn text(&self, path: PathBuf, page: usize, done: impl FnOnce(Result<String>) + Send + 'static) {
        self.submit(Box::new(move |backend, docs| {
            if !docs.contains_key(&path)
                && let Err(err) = backend.open(&path)
            {
                return done(Err(err));
            }
            done(backend.text(&path, page))
        }));
    }

    /// Synchronous helpers for benchmarks and tests.
    pub fn open_blocking(&self, path: &Path) -> Result<DocInfo> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.open(path.to_path_buf(), move |r| {
            let _ = tx.send(r);
        });
        rx.recv()?
    }

    pub fn render_blocking(&self, path: &Path, page: usize, width_px: u32) -> Result<RenderedPage> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.render(path.to_path_buf(), page, width_px, CancelToken::default(), move |r| {
            let _ = tx.send(r);
        });
        rx.recv()?
    }

    pub fn text_blocking(&self, path: &Path, page: usize) -> Result<String> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.text(path.to_path_buf(), page, move |r| {
            let _ = tx.send(r);
        });
        rx.recv()?
    }
}

/// Pixel height for a page rendered at `width_px`.
fn scaled_height(size: (f64, f64), width_px: u32) -> u32 {
    ((size.1 / size.0) * f64::from(width_px)).round().max(1.0) as u32
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Minimal one page PDF (A4) with a line of text, built by hand so that
    /// tests need no fixtures.
    pub(crate) fn sample_pdf() -> Vec<u8> {
        // A red square in the bottom left corner and a line of text.
        let content = b"1 0 0 rg 0 0 100 100 re f 0 g BT /F1 24 Tf 72 720 Td (Hello Atlas) Tj ET";
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R \
             /Resources << /Font << /F1 5 0 R >> >> >>"
                .to_string(),
            format!("<< /Length {} >>\nstream\n{}\nendstream", content.len(), std::str::from_utf8(content).unwrap()),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        ];
        let mut out = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (i, obj) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{obj}\nendobj\n", i + 1).as_bytes());
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes(),
        );
        out
    }

    #[test]
    fn renders_and_extracts_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.pdf");
        std::fs::write(&path, sample_pdf()).unwrap();
        let service = PdfService::get();
        let info = service.open_blocking(&path).unwrap();
        assert_eq!(info.page_sizes, vec![(595.0, 842.0)]);
        let page = service.render_blocking(&path, 0, 300).unwrap();
        assert_eq!(page.width, 300);
        assert_eq!(page.height, scaled_height((595.0, 842.0), 300));
        assert_eq!(page.bgra.len(), (page.width * page.height * 4) as usize);
        // Background is white, the text made some pixels dark.
        assert_eq!(&page.bgra[0..4], &[255, 255, 255, 255]);
        assert!(page.bgra.chunks(4).any(|px| px[0] < 128));
        // The bottom left corner is pure red, in BGRA byte order.
        let row = (page.height as usize - 2) * page.width as usize * 4;
        assert_eq!(&page.bgra[row + 8..row + 12], &[0, 0, 255, 255], "output must be BGRA");
        let text = service.text_blocking(&path, 0).unwrap();
        assert!(text.contains("Hello Atlas"), "text: {text:?}");
    }
}
