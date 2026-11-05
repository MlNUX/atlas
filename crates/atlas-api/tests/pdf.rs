// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! P2.6: annotations of a PDF through the facade (matching by path and
//! fingerprint, groups, undo, quotes, links) and the formula OCR status,
//! against a temporary project and private state/config folders (own test
//! binary). The PDF engine is not needed: the UI gives page count and `/ID`.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use atlas_api::AtlasError;
use atlas_api::app::Atlas;
use atlas_api::events::{Event, EventSink};
use atlas_api::ocr::{GrayImage, LatexOcr};
use atlas_api::pdf::{Annotation, AnnotationKind, AnnotationPatch, MatchState, Point, Rect, Stroke};
use atlas_api::project::Project;

struct Collect(Mutex<Sender<Event>>);

impl EventSink for Collect {
    fn on_event(&self, event: Event) {
        let _ = self.0.lock().unwrap().send(event);
    }
}

fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    static DIRS: OnceLock<tempfile::TempDir> = OnceLock::new();
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    DIRS.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        // SAFETY: set once, before any test of this binary reads them.
        unsafe {
            std::env::set_var("ATLAS_STATE_DIR", dir.path().join("state"));
            std::env::set_var("ATLAS_CONFIG_DIR", dir.path().join("config"));
        }
        dir
    });
    guard
}

struct Fixture {
    _serial: MutexGuard<'static, ()>,
    dir: tempfile::TempDir,
    _atlas: Arc<Atlas>,
    project: Arc<Project>,
    events: Receiver<Event>,
}

impl Fixture {
    fn path(&self, rel: &str) -> String {
        self.dir.path().canonicalize().unwrap().join(rel).to_string_lossy().into_owned()
    }
}

fn fixture() -> Fixture {
    let serial = serial();
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/paper.pdf"), b"%PDF-1.4 erstes Papier").unwrap();
    std::fs::write(dir.path().join("notiz.md"), "# Notiz\n").unwrap();
    let (tx, rx) = channel();
    let sink: Arc<dyn EventSink> = Arc::new(Collect(Mutex::new(tx)));
    let atlas = Atlas::new(sink.clone());
    let project = atlas.open_project(dir.path().to_string_lossy().into_owned(), sink).unwrap();
    Fixture { _serial: serial, dir, _atlas: atlas, project, events: rx }
}

fn markup(page: u32, text: &str, group: &str) -> Annotation {
    Annotation {
        id: String::new(),
        kind: AnnotationKind::Highlight,
        page,
        rects: vec![Rect { x: 72.0, y: 100.004, width: 200.0, height: 12.0 }],
        text: text.into(),
        comment: String::new(),
        color: String::new(),
        ink: Vec::new(),
        width: None,
        title: String::new(),
        group: group.into(),
        created: 0,
        modified: 0,
    }
}

#[test]
fn annotations_round_trip_and_groups() {
    let f = fixture();
    let pdf = f.project.pdf("docs/paper.pdf".into()).unwrap();
    assert_eq!(pdf.status().state, MatchState::Loading);
    assert!(matches!(pdf.add(markup(1, "x", "")), Err(AtlasError::Unavailable { .. })));
    assert!(matches!(f.project.pdf("fehlt.pdf".into()), Err(AtlasError::NotFound { .. })));

    let status = pdf.attach(3, Some("abc".into())).unwrap();
    assert_eq!((status.state, status.count), (MatchState::New, 0));

    let first = pdf.add(markup(1, "Ein ge-\ntrenntes Wort", "g1")).unwrap();
    let second = pdf.add(markup(2, "geht weiter", "g1")).unwrap();
    let mut ink = markup(3, "", "");
    ink.kind = AnnotationKind::Ink;
    ink.rects.clear();
    ink.ink = vec![Stroke { points: vec![Point { x: 1.0, y: 2.0 }, Point { x: 3.0, y: 4.0 }] }];
    ink.width = Some(2.0);
    let ink = pdf.add(ink).unwrap();
    assert!(f.events.try_iter().any(|e| matches!(e, Event::AnnotationsChanged { .. })));

    let a = pdf.annotation(first.clone()).unwrap();
    assert_eq!(a.text, "Ein getrenntes Wort", "markup text is cleaned");
    assert_eq!(a.color, "#ffd400");
    assert!(a.created > 0);
    assert_eq!(pdf.page_annotations(2).len(), 1);
    assert_eq!(pdf.status().state, MatchState::Path);

    // The file is the shared Atlas format, rounded to 1/100 pt.
    let dir = f.path(".atlas/annotations");
    let file = std::fs::read_dir(&dir).unwrap().next().unwrap().unwrap().path();
    let json = std::fs::read_to_string(&file).unwrap();
    assert!(json.contains("\"path\": \"docs/paper.pdf\""), "{json}");
    assert!(json.contains("100.0") && !json.contains("100.004"), "{json}");

    // Colour and comment go to all parts of the group.
    pdf.update(
        second.clone(),
        AnnotationPatch { color: Some("#ff0000".into()), comment: Some("wichtig".into()), ..Default::default() },
    )
    .unwrap();
    assert_eq!(pdf.annotation(first.clone()).unwrap().color, "#ff0000");
    assert_eq!(pdf.annotation(first.clone()).unwrap().comment, "wichtig");

    // Quote and link use the first part.
    let quote = pdf.quote_markdown(second.clone(), f.path("notiz.md")).unwrap();
    assert!(quote.contains("> Ein getrenntes Wort geht weiter"), "{quote}");
    assert!(quote.contains(&format!("(/docs/paper.pdf#page=1&annot={first})")), "{quote}");
    assert!(quote.ends_with("wichtig\n"), "{quote}");
    assert_eq!(pdf.link(2, Some(second.clone())), format!("/docs/paper.pdf#page=1&annot={first}"));
    assert_eq!(pdf.link(5, None), "/docs/paper.pdf#page=5");

    // Undo of a removal.
    let snap = pdf.snapshot(second.clone()).unwrap();
    pdf.remove(second.clone()).unwrap();
    assert_eq!(pdf.annotations().len(), 1, "the whole group goes");
    pdf.restore(snap).unwrap();
    assert_eq!(pdf.annotations().len(), 3);
    assert!(matches!(pdf.remove("fehlt".into()), Err(AtlasError::NotFound { .. })));

    // Another window sees the same annotations.
    let other = f.project.pdf(f.path("docs/paper.pdf")).unwrap();
    assert_eq!(other.attach(3, Some("abc".into())).unwrap().count, 3);
    other.remove(ink).unwrap();
    assert_eq!(pdf.reload().unwrap().count, 2);

    assert!(pdf.default_export_path().ends_with("docs/paper (annotiert).pdf"));
    pdf.set_page_texts(vec!["Seite eins".into(), "Seite zwei".into(), String::new()]);
}

#[test]
fn annotations_follow_moves_and_notice_changes() {
    let f = fixture();
    let pdf = f.project.pdf(f.path("docs/paper.pdf")).unwrap();
    pdf.attach(3, None).unwrap();
    pdf.add(markup(1, "Text", "")).unwrap();

    // Renamed outside of Atlas: found by content.
    std::fs::rename(f.path("docs/paper.pdf"), f.path("docs/umbenannt.pdf")).unwrap();
    let moved = f.project.pdf(f.path("docs/umbenannt.pdf")).unwrap();
    let status = moved.attach(3, None).unwrap();
    assert_eq!((status.state, status.count), (MatchState::Moved, 1));
    assert!(!status.message.is_empty());
    moved.acknowledge_change();
    assert_eq!(moved.status().state, MatchState::Path);

    // Changed in place: the positions may be off.
    std::fs::write(f.path("docs/umbenannt.pdf"), b"%PDF-1.4 neue Fassung").unwrap();
    let changed = f.project.pdf(f.path("docs/umbenannt.pdf")).unwrap();
    assert_eq!(changed.attach(3, None).unwrap().state, MatchState::Changed);
    changed.acknowledge_change();
    let again = f.project.pdf(f.path("docs/umbenannt.pdf")).unwrap();
    assert_eq!(again.attach(3, None).unwrap().state, MatchState::Path);
}

#[test]
fn formula_ocr_status_without_models() {
    let _serial = serial();
    let ocr = LatexOcr::new();
    let status = ocr.status();
    assert!(!status.models_dir.is_empty());
    assert!(!status.message.is_empty());
    let bad = GrayImage { width: 4, height: 4, pixels: vec![0; 3] };
    let result = futures_lite_block_on(ocr.recognize(bad));
    assert!(matches!(result, Err(AtlasError::Invalid { .. })));
}

/// Minimal executor for a future that is ready at once or after a thread.
fn futures_lite_block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Wake, Waker};
    struct Thread(std::thread::Thread);
    impl Wake for Thread {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Thread(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(out) = future.as_mut().poll(&mut cx) {
            return out;
        }
        std::thread::park();
    }
}

fn run(text: &str, x: f64, y: f64, font: &str, size: f64) -> Vec<atlas_api::pdf::PdfGlyph> {
    let w = 0.5 * size;
    text.chars()
        .enumerate()
        .map(|(i, ch)| atlas_api::pdf::PdfGlyph {
            text: ch.to_string(),
            rect: Rect { x: x + i as f64 * w, y, width: w, height: size },
            font: font.into(),
            size,
        })
        .collect()
}

#[test]
fn formulas_in_page_text_and_selections() {
    let f = fixture();
    let pdf = f.project.pdf("docs/paper.pdf".into()).unwrap();
    // "Fläche x = y2 und mehr", x and y in CMMI, = and 2 in CMR (as the core's test).
    let mut g = run("Fläche ", 0.0, 0.0, "Helvetica", 10.0);
    g.extend(run("x", 35.0, 0.0, "ABCDEF+CMMI10", 10.0));
    g.extend(run(" ", 40.0, 0.0, "", 10.0));
    g.extend(run("=", 42.0, 0.0, "ABCDEF+CMR10", 10.0));
    g.extend(run(" ", 47.0, 0.0, "", 10.0));
    g.extend(run("y", 49.0, 0.0, "ABCDEF+CMMI10", 10.0));
    g.extend(run("2", 54.0, -2.0, "ABCDEF+CMR7", 7.0));
    g.extend(run(" und mehr.", 58.0, 0.0, "Helvetica", 10.0));

    let clips = pdf.formula_clips(g.clone(), Vec::new());
    assert_eq!(clips.len(), 1, "{clips:?}");
    assert!(!clips[0].display && clips[0].scale > 0.0);
    let a = clips[0].area;
    assert!(a.x <= 35.0 && a.x + a.width >= 57.0, "the clip covers the formula: {a:?}");
    assert_eq!(
        pdf.text_with_latex(g.clone(), Vec::new(), vec![Some("x = y^2.".into())]),
        "Fläche $x = y^2$ und mehr.",
        "recognised LaTeX is tidied"
    );
    assert_eq!(pdf.text_with_latex(g.clone(), Vec::new(), vec![None]), "Fläche x = y2 und mehr.");
    // A selection without the formula finds none.
    let words = vec![Rect { x: 0.0, y: 0.0, width: 30.0, height: 10.0 }];
    assert!(pdf.formula_clips(g, words).is_empty());
}
