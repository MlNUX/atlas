// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! PDF rendering (Poppler, chosen in Phase 0, see docs/development/phase0.md).
//!
//! The library is not thread safe in the way the UI needs, so a worker
//! thread ("actor") owns it and all open documents. Requests are queued; the
//! result is handed to a callback on the worker thread. The engine sits
//! behind the [`Backend`] trait so that it can be exchanged later.
//!
//! Rendered pages are BGRA, premultiplied, 4 bytes per pixel, rows without
//! padding – i.e. `QImage::Format_ARGB32_Premultiplied` on little endian.

pub mod formulas;
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

/// Darkest shade of a page in dark mode: paper becomes dark grey, not black.
pub const DARK_PAPER: u8 = 0x26;

impl RenderedPage {
    /// Dark mode: inverts the colours, white paper becomes [`DARK_PAPER`]
    /// (per channel `max(255 − v, DARK_PAPER)`, the same as the viewer's
    /// painter fallback). Rendered pages are opaque, alpha stays.
    pub fn darken(&mut self) {
        let lut: [u8; 256] = std::array::from_fn(|v| (255 - v as u8).max(DARK_PAPER));
        for px in self.bgra.as_chunks_mut::<4>().0 {
            px[0] = lut[px[0] as usize];
            px[1] = lut[px[1] as usize];
            px[2] = lut[px[2] as usize];
        }
    }
}

/// A link on a page: its area in points (origin top left) and its target.
#[derive(Debug, Clone, PartialEq)]
pub struct PageLink {
    pub page: usize,
    pub rect: [f64; 4],
    pub target: LinkTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LinkTarget {
    /// A page of this document (0-based), optionally a position on it in
    /// points from the top.
    Page { page: usize, top: Option<f64> },
    /// A web address or other URI (`mailto:` …).
    Uri(String),
}

/// One entry of the document outline (table of contents).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineItem {
    pub title: String,
    /// 0-based target page, if the entry points into this document.
    pub page: Option<usize>,
    pub depth: usize,
}

/// A text match: page and rectangle in PDF points, origin top left.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub page: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Upper bound for search hits in one document.
pub const MAX_SEARCH_HITS: usize = 5000;
/// Pages a selection may span (longer drags are cut off).
pub const MAX_SELECTION_PAGES: usize = 30;

/// How text selection snaps (drag, double click, triple click).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionStyle {
    Glyph,
    Word,
    Line,
}

/// Selected text on one page: line rectangles (points, origin top left).
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub page: usize,
    pub rects: Vec<[f64; 4]>,
    pub text: String,
}

/// Engine specific implementation; lives on the worker thread.
trait Backend {
    fn open(&mut self, path: &Path) -> Result<Vec<(f64, f64)>>;
    fn render(&mut self, path: &Path, page: usize, width_px: u32) -> Result<(u32, u32, Vec<u8>)>;
    fn text(&mut self, path: &Path, page: usize) -> Result<String>;
    fn outline(&mut self, path: &Path) -> Result<Vec<OutlineItem>>;
    fn links(&mut self, path: &Path) -> Result<Vec<PageLink>>;
    fn search(&mut self, path: &Path, query: &str, limit: usize) -> Result<Vec<SearchHit>>;
    /// Text in `area` (`[x1, y1, x2, y2]`: start and end of the drag).
    fn select(
        &mut self,
        path: &Path,
        page: usize,
        area: [f64; 4],
        style: SelectionStyle,
    ) -> Result<(Vec<[f64; 4]>, String)>;
    fn document_id(&mut self, path: &Path) -> Result<Option<String>>;
    /// Renders `area` (`[x0, y0, x1, y1]`, points, origin top left) of
    /// `page` at `scale` pixels per point on white, in greyscale: width,
    /// height, one byte per pixel.
    fn clip_gray(&mut self, path: &Path, page: usize, area: [f64; 4], scale: f64) -> Result<(u32, u32, Vec<u8>)>;
    /// Like `clip_gray`, in colour: RGB, three bytes per pixel.
    fn clip_rgb(&mut self, path: &Path, page: usize, area: [f64; 4], scale: f64) -> Result<(u32, u32, Vec<u8>)>;
    /// The characters of the text of `page` with rectangle and font.
    fn glyphs(&mut self, path: &Path, page: usize) -> Result<Vec<formulas::Glyph>>;
    fn close(&mut self, path: &Path);
}

/// The plain text of every page (lines separated by `\n`), read with a
/// document of its own: callable from any thread, e.g. the project search.
pub fn page_texts(path: &Path) -> Result<Vec<String>> {
    let mut backend = poppler::PopplerBackend::default();
    let pages = backend.open(path)?.len();
    (0..pages).map(|page| backend.text(path, page)).collect()
}

/// Splits a drag from `start` to `end` (`(page, x, y)`, points) into one
/// selection area per page: the first page from the start point to its
/// end, pages in between completely, the last page up to the end point.
pub fn selection_spans(
    page_sizes: &[(f64, f64)],
    start: (usize, f64, f64),
    end: (usize, f64, f64),
) -> Vec<(usize, [f64; 4])> {
    let (a, b) = if (end.0, end.2, end.1) < (start.0, start.2, start.1) && end.0 != start.0 || end.0 < start.0 {
        (end, start)
    } else {
        (start, end)
    };
    let last = page_sizes.len().saturating_sub(1);
    let (first_page, last_page) = (a.0.min(last), b.0.min(last).min(a.0 + MAX_SELECTION_PAGES - 1));
    if first_page == last_page {
        return vec![(first_page, [a.1, a.2, b.1, b.2])];
    }
    (first_page..=last_page)
        .map(|page| {
            let (w, h) = page_sizes[page];
            let area = if page == first_page {
                [a.1, a.2, w, h]
            } else if page == b.0 {
                [0.0, 0.0, b.1, b.2]
            } else {
                [0.0, 0.0, w, h]
            };
            (page, area)
        })
        .collect()
}

type Job = Box<dyn FnOnce(&mut dyn Backend, &mut HashMap<PathBuf, DocInfo>) + Send>;

/// Handle to the PDF worker threads: one for everything but rendering,
/// a few for rendering pages in parallel (each with documents of its own:
/// a Poppler document must not be used by two threads at once).
pub struct PdfService {
    jobs: Sender<Job>,
    renders: Sender<RenderJob>,
    /// Bumped by [`PdfService::close`]: render threads reopen a document
    /// whose number changed (it changed on disk).
    closed: Arc<std::sync::Mutex<HashMap<PathBuf, u64>>>,
}

struct RenderJob {
    path: PathBuf,
    page: usize,
    width_px: u32,
    cancel: CancelToken,
    done: Box<dyn FnOnce(Result<RenderedPage>) + Send>,
}

/// Render threads: up to 3, one fewer than the cores (the UI needs one).
fn render_threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().saturating_sub(1)).clamp(1, 3)
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
        let closed: Arc<std::sync::Mutex<HashMap<PathBuf, u64>>> = Arc::default();
        let (renders, render_rx) = unbounded::<RenderJob>();
        for i in 0..render_threads() {
            let rx = render_rx.clone();
            let closed = closed.clone();
            std::thread::Builder::new()
                .name(format!("atlas-pdf-render-{i}"))
                .spawn(move || {
                    let mut backend = poppler::PopplerBackend::default();
                    // Documents open here, with the close number they were opened at.
                    let mut open: HashMap<PathBuf, u64> = HashMap::new();
                    for job in rx {
                        if job.cancel.is_cancelled() {
                            continue;
                        }
                        let epoch = closed.lock().map(|c| c.get(&job.path).copied().unwrap_or(0)).unwrap_or(0);
                        if open.get(&job.path) != Some(&epoch) {
                            backend.close(&job.path);
                            if let Err(err) = backend.open(&job.path) {
                                (job.done)(Err(err));
                                continue;
                            }
                            open.insert(job.path.clone(), epoch);
                        }
                        let start = Instant::now();
                        let result =
                            backend.render(&job.path, job.page, job.width_px.max(1)).map(|(width, height, bgra)| {
                                RenderedPage { width, height, bgra, elapsed: start.elapsed() }
                            });
                        (job.done)(result);
                    }
                })
                .expect("spawn pdf render worker");
        }
        Self { jobs: tx, renders, closed }
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
        if let Ok(mut closed) = self.closed.lock() {
            *closed.entry(path.clone()).or_default() += 1;
        }
        self.submit(Box::new(move |backend, docs| {
            docs.remove(&path);
            backend.close(&path);
        }));
    }

    /// Renders `page` (0 based) at `width_px` pixels width (on one of the
    /// render threads; `done` runs there).
    pub fn render(
        &self,
        path: PathBuf,
        page: usize,
        width_px: u32,
        cancel: CancelToken,
        done: impl FnOnce(Result<RenderedPage>) + Send + 'static,
    ) {
        let _ = self.renders.send(RenderJob { path, page, width_px, cancel, done: Box::new(done) });
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

    /// Renders a part of a page in greyscale (e.g. a formula for the LaTeX
    /// recognition); see [`Backend::clip_gray`].
    pub fn clip_gray(
        &self,
        path: PathBuf,
        page: usize,
        area: [f64; 4],
        scale: f64,
        done: impl FnOnce(Result<(u32, u32, Vec<u8>)>) + Send + 'static,
    ) {
        self.submit(Box::new(move |backend, docs| {
            if !docs.contains_key(&path)
                && let Err(err) = backend.open(&path)
            {
                return done(Err(err));
            }
            done(backend.clip_gray(&path, page, area, scale))
        }));
    }

    /// Like [`PdfService::page_formulas`], for the text inside `rects`
    /// (`[x0, y0, x1, y1]`, e.g. a selection) only.
    pub fn area_formulas(
        &self,
        path: PathBuf,
        page: usize,
        rects: Vec<[f64; 4]>,
        done: impl FnOnce(Result<(formulas::PageText, Vec<(u32, u32, Vec<u8>)>)>) + Send + 'static,
    ) {
        self.submit(Box::new(move |backend, docs| {
            if !docs.contains_key(&path)
                && let Err(err) = backend.open(&path)
            {
                return done(Err(err));
            }
            let text = match backend.glyphs(&path, page) {
                Ok(glyphs) => formulas::PageText::new(formulas::subset(&glyphs, &rects)),
                Err(err) => return done(Err(err)),
            };
            let clips = (0..text.formulas.len())
                .map(|i| {
                    backend.clip_gray(&path, page, text.clip_area(i), text.clip_scale(i)).unwrap_or((0, 0, Vec::new()))
                })
                .collect();
            done(Ok((text, clips)))
        }));
    }

    /// Saves `area` (`[x0, y0, x1, y1]`, points) of `page` in colour as PNG
    /// at `out`, `scale` pixels per point (e.g. a figure quoted in a note).
    pub fn save_area_png(&self, path: &Path, page: usize, area: [f64; 4], scale: f64, out: &Path) -> Result<()> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let pdf = path.to_path_buf();
        self.submit(Box::new(move |backend, docs| {
            if !docs.contains_key(&pdf)
                && let Err(err) = backend.open(&pdf)
            {
                let _ = tx.send(Err(err));
                return;
            }
            let _ = tx.send(backend.clip_rgb(&pdf, page, area, scale));
        }));
        let (w, h, rgb) = rx.recv()??;
        let image = image::RgbImage::from_raw(w, h, rgb).ok_or_else(|| anyhow::anyhow!("Bildgröße passt nicht"))?;
        image.save_with_format(out, image::ImageFormat::Png)?;
        Ok(())
    }

    /// The text of `page` with its formulas (see [`formulas`]), each
    /// formula cut out in greyscale at the size the formula model reads best
    /// (an empty picture if that failed).
    pub fn page_formulas(
        &self,
        path: PathBuf,
        page: usize,
        done: impl FnOnce(Result<(formulas::PageText, Vec<(u32, u32, Vec<u8>)>)>) + Send + 'static,
    ) {
        self.submit(Box::new(move |backend, docs| {
            if !docs.contains_key(&path)
                && let Err(err) = backend.open(&path)
            {
                return done(Err(err));
            }
            let text = match backend.glyphs(&path, page) {
                Ok(glyphs) => formulas::PageText::new(glyphs),
                Err(err) => return done(Err(err)),
            };
            let clips = (0..text.formulas.len())
                .map(|i| {
                    backend.clip_gray(&path, page, text.clip_area(i), text.clip_scale(i)).unwrap_or((0, 0, Vec::new()))
                })
                .collect();
            done(Ok((text, clips)))
        }));
    }

    /// Cuts out `area` of `page` (a box drawn around a formula) for the
    /// formula model, at the size it reads best for the text in there.
    pub fn formula_clip(
        &self,
        path: PathBuf,
        page: usize,
        area: [f64; 4],
        done: impl FnOnce(Result<(u32, u32, Vec<u8>)>) + Send + 'static,
    ) {
        self.submit(Box::new(move |backend, docs| {
            if !docs.contains_key(&path)
                && let Err(err) = backend.open(&path)
            {
                return done(Err(err));
            }
            let size = backend.glyphs(&path, page).ok().and_then(|g| formulas::font_size_in(&g, area));
            done(backend.clip_gray(&path, page, area, formulas::scale_for(size.unwrap_or(12.0))))
        }));
    }

    /// Reads the outline (table of contents).
    pub fn outline(&self, path: PathBuf, done: impl FnOnce(Result<Vec<OutlineItem>>) + Send + 'static) {
        self.submit(Box::new(move |backend, _| done(backend.outline(&path))));
    }

    /// Reads the links of all pages (internal jumps and web addresses).
    pub fn links(&self, path: PathBuf, done: impl FnOnce(Result<Vec<PageLink>>) + Send + 'static) {
        self.submit(Box::new(move |backend, _| done(backend.links(&path))));
    }

    /// Finds `query` (case insensitive) in all pages.
    pub fn search(
        &self,
        path: PathBuf,
        query: String,
        cancel: CancelToken,
        done: impl FnOnce(Result<Vec<SearchHit>>) + Send + 'static,
    ) {
        self.submit(Box::new(move |backend, _| {
            if !cancel.is_cancelled() {
                done(backend.search(&path, &query, MAX_SEARCH_HITS))
            }
        }));
    }

    /// Selects text along `spans` (see [`selection_spans`]).
    pub fn select(
        &self,
        path: PathBuf,
        spans: Vec<(usize, [f64; 4])>,
        style: SelectionStyle,
        cancel: CancelToken,
        done: impl FnOnce(Result<Vec<Selection>>) + Send + 'static,
    ) {
        self.submit(Box::new(move |backend, _| {
            if cancel.is_cancelled() {
                return;
            }
            let result = spans
                .into_iter()
                .map(|(page, area)| {
                    backend.select(&path, page, area, style).map(|(rects, text)| Selection { page, rects, text })
                })
                .filter(|s| !matches!(s, Ok(sel) if sel.rects.is_empty()))
                .collect();
            done(result)
        }));
    }

    /// Permanent ID from the PDF trailer (hex), if present.
    pub fn document_id(&self, path: PathBuf, done: impl FnOnce(Result<Option<String>>) + Send + 'static) {
        self.submit(Box::new(move |backend, _| done(backend.document_id(&path))));
    }

    /// Writes a copy of `src` with the annotations embedded to `dst` (on
    /// the worker thread; the original is never written).
    pub fn export_annotated(
        &self,
        src: PathBuf,
        dst: PathBuf,
        annotations: Vec<crate::annotations::Annotation>,
        done: impl FnOnce(Result<usize>) + Send + 'static,
    ) {
        self.submit(Box::new(move |_, _| done(poppler::export_annotated(&src, &dst, &annotations))));
    }

    /// Annotations stored inside a PDF: (0-based page, poppler type).
    pub fn list_annotations(&self, path: &Path) -> Result<Vec<(usize, i32)>> {
        poppler::list_annotations(path)
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
        pdf_with("1 0 0 rg 0 0 100 100 re f 0 g BT /F1 24 Tf 72 720 Td (Hello Atlas) Tj ET", &["Helvetica"])
    }

    /// One page (A4) with `content`, fonts `/F1`, `/F2`, … named `fonts`
    /// (Type 1, not embedded).
    pub(crate) fn pdf_with(content: &str, fonts: &[&str]) -> Vec<u8> {
        let refs: String = (0..fonts.len()).map(|i| format!("/F{} {} 0 R ", i + 1, i + 5)).collect();
        let mut objects = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << {refs}>> >> >>"
            ),
            format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        ];
        objects.extend(fonts.iter().map(|f| format!("<< /Type /Font /Subtype /Type1 /BaseFont /{f} >>")));
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
    fn dark_mode_inverts_onto_dark_paper() {
        // White, black, red (BGRA).
        let mut page = RenderedPage {
            width: 3,
            height: 1,
            bgra: vec![255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 255, 255],
            elapsed: Duration::ZERO,
        };
        page.darken();
        let d = DARK_PAPER;
        assert_eq!(page.bgra, [d, d, d, 255, 255, 255, 255, 255, 255, 255, d, 255]);
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
        // A page the document does not have: an error, not a Poppler
        // assertion on stderr.
        assert!(service.render_blocking(&path, 5, 100).is_err());
        assert!(service.text_blocking(&path, 1).is_err());
        // Background is white, the text made some pixels dark.
        assert_eq!(&page.bgra[0..4], &[255, 255, 255, 255]);
        assert!(page.bgra.chunks(4).any(|px| px[0] < 128));
        // The bottom left corner is pure red, in BGRA byte order.
        let row = (page.height as usize - 2) * page.width as usize * 4;
        assert_eq!(&page.bgra[row + 8..row + 12], &[0, 0, 255, 255], "output must be BGRA");
        let text = service.text_blocking(&path, 0).unwrap();
        assert!(text.contains("Hello Atlas"), "text: {text:?}");
    }

    /// By hand: rendering on the render threads while the main worker
    /// extracts text, glyphs and searches, on several documents at once.
    /// `ATLAS_BENCH_PDF=a.pdf ATLAS_BENCH_PDF2=b.pdf cargo test -p atlas-core -- --ignored stress_pdf_threads`
    #[test]
    #[ignore]
    fn stress_pdf_threads() {
        let a = PathBuf::from(std::env::var("ATLAS_BENCH_PDF").expect("ATLAS_BENCH_PDF"));
        let b = PathBuf::from(std::env::var("ATLAS_BENCH_PDF2").expect("ATLAS_BENCH_PDF2"));
        let service = PdfService::get();
        let pages_a = service.open_blocking(&a).unwrap().page_sizes.len();
        let pages_b = service.open_blocking(&b).unwrap().page_sizes.len();
        let until = Instant::now()
            + Duration::from_secs(std::env::var("STRESS_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(30));
        let mut rounds = 0;
        while Instant::now() < until {
            let (tx, rx) = crossbeam_channel::unbounded();
            for i in 0..12 {
                let (path, n) = if i % 2 == 0 { (&a, pages_a) } else { (&b, pages_b) };
                let tx = tx.clone();
                service.render(
                    path.clone(),
                    (rounds * 7 + i) % n,
                    600 + (i as u32 % 3) * 300,
                    CancelToken::default(),
                    move |r| {
                        let _ = tx.send(r.map(|_| ()));
                    },
                );
            }
            for page in 0..3 {
                let _ = service.text_blocking(&a, page % pages_a);
                let (t2, r2) = crossbeam_channel::bounded(1);
                service.page_formulas(b.clone(), page % pages_b, move |r| {
                    let _ = t2.send(r.map(|_| ()));
                });
                let _ = r2.recv();
                let (t3, r3) = crossbeam_channel::bounded(1);
                service.search(a.clone(), "die".into(), CancelToken::default(), move |r| {
                    let _ = t3.send(r.map(|_| ()));
                });
                let _ = r3.recv();
            }
            if rounds % 5 == 0 {
                service.close(a.clone());
            }
            drop(tx);
            for r in rx {
                r.unwrap();
            }
            rounds += 1;
        }
        println!("{rounds} rounds without a crash");
    }

    /// By hand: several engines rendering the same document at the same
    /// time give the same pixels as one (Poppler's font handling is shared).
    #[test]
    #[ignore]
    fn parallel_rendering_is_identical() {
        let path = PathBuf::from(std::env::var("ATLAS_BENCH_PDF").expect("ATLAS_BENCH_PDF"));
        let mut backend = poppler::PopplerBackend::default();
        let n = backend.open(&path).unwrap().len();
        let hash =
            |bgra: &[u8]| bgra.iter().fold(0u64, |h, &b| h.wrapping_mul(1_099_511_628_211).wrapping_add(u64::from(b)));
        let expected: Vec<u64> = (0..n).map(|p| hash(&backend.render(&path, p, 900).unwrap().2)).collect();
        for round in 0..3 {
            let got: Vec<(usize, u64)> = std::thread::scope(|scope| {
                let workers: Vec<_> = (0..4)
                    .map(|t| {
                        let path = path.clone();
                        scope.spawn(move || {
                            let mut backend = poppler::PopplerBackend::default();
                            backend.open(&path).unwrap();
                            (t..n)
                                .step_by(4)
                                .map(|p| (p, hash(&backend.render(&path, p, 900).unwrap().2)))
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
                workers.into_iter().flat_map(|w| w.join().unwrap()).collect()
            });
            for (p, h) in got {
                assert_eq!(h, expected[p], "round {round}, page {p}");
            }
        }
        println!("{n} pages identical in 3 rounds of 4 threads");
    }

    /// By hand: render times of a real document, one engine and several in
    /// parallel. `ATLAS_BENCH_PDF=lang.pdf cargo test --release -p atlas-core -- --ignored --nocapture render_benchmark`
    #[test]
    #[ignore]
    fn render_benchmark() {
        let path = PathBuf::from(std::env::var("ATLAS_BENCH_PDF").expect("ATLAS_BENCH_PDF"));
        let width: u32 = std::env::var("ATLAS_BENCH_WIDTH").ok().and_then(|w| w.parse().ok()).unwrap_or(1600);
        let mut backend = poppler::PopplerBackend::default();
        let started = Instant::now();
        let sizes = backend.open(&path).unwrap();
        println!("open: {} pages, {} ms", sizes.len(), started.elapsed().as_millis());
        let n = sizes.len().min(40);
        let started = Instant::now();
        let mut darken = Duration::ZERO;
        for page in 0..n {
            let (w, h, bgra) = backend.render(&path, page, width).unwrap();
            let mut p = RenderedPage { width: w, height: h, bgra, elapsed: Duration::ZERO };
            let t = Instant::now();
            p.darken();
            darken += t.elapsed();
        }
        let one = started.elapsed() - darken;
        println!(
            "render {n} pages at {width} px, one engine: {} ms per page; darken {} ms per page",
            one.as_millis() / n as u128,
            darken.as_millis() / n as u128
        );
        for threads in [2usize, 4] {
            let started = Instant::now();
            std::thread::scope(|scope| {
                for t in 0..threads {
                    let path = path.clone();
                    scope.spawn(move || {
                        let mut backend = poppler::PopplerBackend::default();
                        backend.open(&path).unwrap();
                        for page in (t..n).step_by(threads) {
                            backend.render(&path, page, width).unwrap();
                        }
                    });
                }
            });
            println!("{threads} engines: {} ms per page", started.elapsed().as_millis() / n as u128);
        }
    }

    /// By hand, with the real formula model:
    /// `ATLAS_FORMULA_PDF=folie.pdf ATLAS_LATEX_OCR_MODELS=… cargo test -p atlas-core -- --ignored --nocapture page_text_with_real_model`
    #[test]
    #[ignore]
    fn page_text_with_real_model() {
        let path = PathBuf::from(std::env::var("ATLAS_FORMULA_PDF").expect("ATLAS_FORMULA_PDF"));
        let page: usize = std::env::var("ATLAS_FORMULA_PAGE").ok().and_then(|p| p.parse().ok()).unwrap_or(0);
        let (tx, rx) = crossbeam_channel::bounded(1);
        PdfService::get().page_formulas(path, page, move |r| {
            let _ = tx.send(r);
        });
        let (text, clips) = rx.recv().unwrap().unwrap();
        println!("--- Text ---\n{}", text.plain());
        if std::env::var("ATLAS_FORMULA_GLYPHS").is_ok() {
            for g in &text.glyphs {
                println!("{:?} {:?} {:.1} {:?}", g.ch, g.rect.map(|v| v.round()), g.size, g.font);
            }
        }
        let mut latex = Vec::new();
        for (i, (w, h, pixels)) in clips.into_iter().enumerate() {
            if let Ok(dir) = std::env::var("ATLAS_FORMULA_DUMP") {
                let mut pgm = format!("P5 {w} {h} 255\n").into_bytes();
                pgm.extend_from_slice(&pixels);
                std::fs::write(format!("{dir}/formel-{i}.pgm"), pgm).unwrap();
            }
            let started = Instant::now();
            let tex = crate::latex_ocr::recognize_gray(w, h, pixels);
            println!("Formel {i} {:?} {w}×{h} ({} ms): {tex:?}", text.formulas[i].rect, started.elapsed().as_millis());
            latex.push(tex.ok());
        }
        println!("--- Mit LaTeX ---\n{}", text.with_latex(&latex));
    }

    #[test]
    fn saves_an_area_as_png_and_finds_formulas_in_a_selection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.pdf");
        std::fs::write(&path, sample_pdf()).unwrap();
        let out = dir.path().join("bild.png");
        PdfService::get().save_area_png(&path, 0, [0.0, 700.0, 120.0, 842.0], 2.0, &out).unwrap();
        let png = std::fs::read(&out).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!((img.width(), img.height()), (240, 284));
        // The red square (bottom left corner of the page) is in the picture.
        let px = img.to_rgb8().get_pixel(20, 270).0;
        assert_eq!(px, [255, 0, 0]);

        let path = dir.path().join("formula.pdf");
        let content = "BT /F1 12 Tf 1 0 0 1 72 720 Tm (Flaeche) Tj /F2 12 Tf 1 0 0 1 122 720 Tm (x) Tj \
                       /F3 12 Tf 1 0 0 1 132 720 Tm (=) Tj /F2 12 Tf 1 0 0 1 142 720 Tm (y) Tj ET \
                       BT /F2 12 Tf 1 0 0 1 72 600 Tm (a) Tj /F3 12 Tf 1 0 0 1 82 600 Tm (+) Tj /F2 12 Tf 1 0 0 1 92 600 Tm (b) Tj ET";
        std::fs::write(&path, pdf_with(content, &["Helvetica", "CMMI10", "CMR10"])).unwrap();
        let (tx, rx) = crossbeam_channel::bounded(1);
        // Only the first line is selected: one formula, not two.
        PdfService::get().area_formulas(path, 0, vec![[60.0, 105.0, 200.0, 130.0]], move |r| {
            let _ = tx.send(r);
        });
        let (text, clips) = rx.recv().unwrap().unwrap();
        assert_eq!(text.formulas.len(), 1, "{:?}", text.plain());
        assert_eq!(clips.len(), 1);
        assert!(text.plain().starts_with("Flaeche"), "{:?}", text.plain());
    }

    #[test]
    fn finds_formulas_by_their_fonts_and_cuts_them_out() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("formula.pdf");
        // "Fläche x = y2 gesamt" with x, y in CMMI10 and = 2 in CMR10.
        let content = "BT /F1 12 Tf 1 0 0 1 72 720 Tm (Flaeche) Tj /F2 12 Tf 1 0 0 1 122 720 Tm (x) Tj \
                       /F3 12 Tf 1 0 0 1 132 720 Tm (=) Tj /F2 12 Tf 1 0 0 1 142 720 Tm (y) Tj \
                       /F3 8 Tf 1 0 0 1 149 725 Tm (2) Tj /F1 12 Tf 1 0 0 1 159 720 Tm (gesamt) Tj ET";
        std::fs::write(&path, pdf_with(content, &["Helvetica", "CMMI10", "CMR10"])).unwrap();
        let (tx, rx) = crossbeam_channel::bounded(1);
        PdfService::get().page_formulas(path.clone(), 0, move |r| {
            let _ = tx.send(r);
        });
        let (text, clips) = rx.recv().unwrap().unwrap();
        let fonts: Vec<_> = text.glyphs.iter().map(|g| (g.ch, g.font.clone())).collect();
        assert!(text.plain().contains("Flaeche"), "{:?}", text.plain());
        assert_eq!(text.formulas.len(), 1, "{fonts:?} {:?}", text.formulas);
        let f = &text.formulas[0];
        // Origin top left: the line sits about 110 points from the top.
        assert!(f.rect[1] > 90.0 && f.rect[3] < 130.0 && f.rect[0] > 100.0, "{:?}", f.rect);
        assert_eq!(clips.len(), 1);
        let (w, h, pixels) = &clips[0];
        assert!(*w > 20 && *h > 15 && pixels.iter().any(|&p| p < 128), "{w}×{h}");
        let latex = text.with_latex(&[Some("x = y^2".into())]);
        assert_eq!(latex, "Flaeche $x = y^2$ gesamt", "{:?}", text.plain());
    }
}

#[cfg(test)]
mod outline_tests {
    use super::*;

    /// Three pages, an outline with a nested entry, and "Suchwort" on page 2
    /// at (100, 200) from the top left.
    fn outlined_pdf(path: &Path) {
        let surface = cairo::PdfSurface::new(595.0, 842.0, path).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        surface.add_outline(0, "Kapitel 1", "page=1", cairo::PdfOutline::empty()).unwrap();
        let k2 = surface.add_outline(0, "Kapitel 2", "page=2", cairo::PdfOutline::OPEN).unwrap();
        surface.add_outline(k2, "Abschnitt 2.1", "page=3", cairo::PdfOutline::empty()).unwrap();
        cr.set_font_size(20.0);
        for page in 0..3 {
            cr.move_to(100.0, 200.0);
            cr.show_text(if page == 1 { "Ein Suchwort hier" } else { "Andere Seite" }).unwrap();
            cr.show_page().unwrap();
        }
        surface.finish();
    }

    #[test]
    fn reads_links() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("links.pdf");
        {
            let surface = cairo::PdfSurface::new(595.0, 842.0, &path).unwrap();
            let cr = cairo::Context::new(&surface).unwrap();
            cr.set_font_size(20.0);
            cr.tag_begin("Link", "uri='https://example.org/'");
            cr.move_to(100.0, 200.0);
            cr.show_text("Webseite").unwrap();
            cr.tag_end("Link");
            cr.tag_begin("Link", "page=2 pos=[0 300]");
            cr.move_to(100.0, 400.0);
            cr.show_text("Zu Seite 2").unwrap();
            cr.tag_end("Link");
            cr.show_page().unwrap();
            cr.show_text("Seite 2").unwrap();
            cr.show_page().unwrap();
            surface.finish();
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        PdfService::get().links(path.clone(), move |r| tx.send(r).unwrap());
        let links = rx.recv().unwrap().unwrap();
        assert_eq!(links.len(), 2, "{links:?}");
        let web = links.iter().find(|l| l.target == LinkTarget::Uri("https://example.org/".into())).unwrap();
        assert_eq!(web.page, 0);
        // Area around the text, origin top left.
        assert!(web.rect[0] <= 101.0 && web.rect[1] < 200.0 && web.rect[1] + web.rect[3] > 190.0, "{:?}", web.rect);
        let jump = links.iter().find(|l| matches!(l.target, LinkTarget::Page { .. })).unwrap();
        let LinkTarget::Page { page, top } = jump.target else { unreachable!() };
        assert_eq!(page, 1);
        assert!(top.is_none_or(|t| (t - 300.0).abs() < 1.0), "{top:?}");
        assert!(jump.rect[1] < 400.0 && jump.rect[1] > 300.0, "{:?}", jump.rect);
    }

    #[test]
    fn reads_outline_and_finds_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("outline.pdf");
        outlined_pdf(&path);
        let service = PdfService::get();
        let (tx, rx) = crossbeam_channel::bounded(1);
        service.outline(path.clone(), move |r| tx.send(r).unwrap());
        let outline = rx.recv().unwrap().unwrap();
        let summary: Vec<_> = outline.iter().map(|o| (o.title.as_str(), o.page, o.depth)).collect();
        assert_eq!(summary, [("Kapitel 1", Some(0), 0), ("Kapitel 2", Some(1), 0), ("Abschnitt 2.1", Some(2), 1)]);

        let (tx, rx) = crossbeam_channel::bounded(1);
        service.search(path, "suchwort".into(), CancelToken::default(), move |r| tx.send(r).unwrap());
        let hits = rx.recv().unwrap().unwrap();
        assert_eq!(hits.len(), 1);
        let hit = &hits[0];
        assert_eq!(hit.page, 1);
        // Baseline at y = 200 from the top: the box spans just above it.
        assert!(hit.y > 170.0 && hit.y < 200.0 && hit.y + hit.height > 195.0, "{hit:?}");
        assert!(hit.x > 100.0 && hit.width > 40.0, "{hit:?}");
    }
}

#[cfg(test)]
mod annotation_tests {
    use super::*;
    use crate::annotations::{Annotation, Kind};

    fn three_pages(path: &Path) {
        let surface = cairo::PdfSurface::new(595.0, 842.0, path).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        cr.set_font_size(20.0);
        for page in 0..3 {
            cr.move_to(100.0, 200.0);
            cr.show_text(&format!("Zeile eins Seite {}", page + 1)).unwrap();
            cr.move_to(100.0, 230.0);
            cr.show_text("Zeile zwei").unwrap();
            cr.show_page().unwrap();
        }
        surface.finish();
    }

    fn blocking<T: Send + 'static>(f: impl FnOnce(crossbeam_channel::Sender<T>)) -> T {
        let (tx, rx) = crossbeam_channel::bounded(1);
        f(tx);
        rx.recv().unwrap()
    }

    #[test]
    fn spans_cover_pages_between_start_and_end() {
        let sizes = vec![(100.0, 200.0); 4];
        assert_eq!(selection_spans(&sizes, (1, 10.0, 20.0), (1, 50.0, 60.0)), vec![(1, [10.0, 20.0, 50.0, 60.0])]);
        let spans = selection_spans(&sizes, (2, 50.0, 60.0), (0, 10.0, 20.0));
        assert_eq!(
            spans,
            vec![(0, [10.0, 20.0, 100.0, 200.0]), (1, [0.0, 0.0, 100.0, 200.0]), (2, [0.0, 0.0, 50.0, 60.0])],
            "dragging upwards selects the same range"
        );
    }

    #[test]
    fn selects_text_and_exports_a_copy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("quelle.pdf");
        three_pages(&path);
        let original = std::fs::read(&path).unwrap();
        let service = PdfService::get();
        let info = service.open_blocking(&path).unwrap();

        // Drag over both lines of page 2.
        let spans = selection_spans(&info.page_sizes, (1, 95.0, 185.0), (1, 200.0, 232.0));
        let sel = blocking(|tx| {
            service.select(path.clone(), spans, SelectionStyle::Glyph, CancelToken::default(), move |r| {
                tx.send(r).unwrap()
            })
        })
        .unwrap();
        assert_eq!(sel.len(), 1);
        assert_eq!(sel[0].page, 1);
        assert!(sel[0].text.contains("Seite 2") && sel[0].text.contains("Zeile zwei"), "{:?}", sel[0].text);
        assert_eq!(sel[0].rects.len(), 2, "one bar per line: {:?}", sel[0].rects);
        let first = sel[0].rects[0];
        assert!(first[0] >= 99.0 && first[1] > 170.0 && first[1] < 200.0, "{first:?}");

        // A selection over a page break yields one part per page.
        let spans = selection_spans(&info.page_sizes, (0, 95.0, 225.0), (1, 150.0, 205.0));
        let parts = blocking(|tx| {
            service
                .select(path.clone(), spans, SelectionStyle::Word, CancelToken::default(), move |r| tx.send(r).unwrap())
        })
        .unwrap();
        assert_eq!(parts.iter().map(|p| p.page).collect::<Vec<_>>(), [0, 1], "{parts:?}");

        let id = blocking(|tx| service.document_id(path.clone(), move |r| tx.send(r).unwrap())).unwrap();
        assert!(id.is_none_or(|id| id.len() == 32), "permanent ID is hex");

        let annotation = |kind, page| Annotation {
            id: crate::annotations::new_id(),
            kind,
            page,
            rects: sel[0].rects.clone(),
            text: sel[0].text.clone(),
            comment: "Kommentar".into(),
            color: "#ffd400".into(),
            ink: if kind == Kind::Ink { vec![vec![[10.0, 10.0], [50.0, 60.0], [90.0, 20.0]]] } else { Vec::new() },
            width: Some(2.0),
            title: String::new(),
            group: String::new(),
            created: 0,
            modified: 0,
        };
        let list = vec![
            annotation(Kind::Highlight, 2),
            annotation(Kind::Underline, 2),
            annotation(Kind::Note, 1),
            annotation(Kind::Ink, 3),
            annotation(Kind::Bookmark, 3),
        ];
        let out = dir.path().join("quelle (annotiert).pdf");
        let added = blocking(|tx| {
            service.export_annotated(path.clone(), out.clone(), list.clone(), move |r| tx.send(r).unwrap())
        })
        .unwrap();
        assert_eq!(added, 4, "bookmarks are not PDF annotations");
        let embedded = service.list_annotations(&out).unwrap();
        assert_eq!(embedded.len(), 4, "{embedded:?}");
        assert_eq!(std::fs::read(&path).unwrap(), original, "the original stays untouched");
        assert_eq!(service.list_annotations(&path).unwrap().len(), 0);

        let err =
            blocking(|tx| service.export_annotated(path.clone(), path.clone(), list, move |r| tx.send(r).unwrap()));
        assert!(err.is_err(), "exporting over the original is refused");
    }
}
