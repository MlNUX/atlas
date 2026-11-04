// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! P2.4: text search (ripgrep), PDF text handed in by the UI, context
//! lines and the annotation search through the facade.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use atlas_api::app::Atlas;
use atlas_api::events::{Event, EventSink};
use atlas_api::search::{SearchMatch, SearchQuery, SearchSummary};

struct Collect(Mutex<Sender<Event>>);

impl EventSink for Collect {
    fn on_event(&self, event: Event) {
        let _ = self.0.lock().unwrap().send(event);
    }
}

fn setup() -> (tempfile::TempDir, Arc<atlas_api::project::Project>, Receiver<Event>) {
    static DIRS: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIRS.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        // SAFETY: set once, before any test of this binary reads them.
        unsafe {
            std::env::set_var("ATLAS_STATE_DIR", dir.path().join("state"));
            std::env::set_var("ATLAS_CONFIG_DIR", dir.path().join("config"));
        }
        dir
    });
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.md"), "Die Fourier Transformation\nzweite Zeile\n").unwrap();
    std::fs::write(dir.path().join("b.rs"), "fn fourier() {}\n").unwrap();
    // UTF-16 positions: "Größe 🙂 " is 9 units before the match, 12 bytes.
    std::fs::write(dir.path().join("c.txt"), "Größe 🙂 FOURIERX\n").unwrap();
    std::fs::write(dir.path().join("skript.pdf"), b"%PDF stand-in").unwrap();
    let (tx, rx) = channel();
    let atlas = Atlas::new(Arc::new(Collect(Mutex::new(channel().0))));
    let project =
        atlas.open_project(dir.path().to_string_lossy().into_owned(), Arc::new(Collect(Mutex::new(tx)))).unwrap();
    (dir, project, rx)
}

fn query(pattern: &str) -> SearchQuery {
    SearchQuery {
        pattern: pattern.into(),
        regex: false,
        case_sensitive: false,
        whole_word: false,
        globs: Vec::new(),
        build_dirs: false,
        kinds: Vec::new(),
        root: None,
    }
}

fn collect(rx: &Receiver<Event>, id: u64) -> (Vec<SearchMatch>, SearchSummary) {
    let mut matches = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_secs(20)).expect("search event") {
            Event::SearchMatches { search_id, matches: m } if search_id == id => matches.extend(m),
            Event::SearchFinished { search_id, summary } if search_id == id => return (matches, summary),
            _ => {}
        }
    }
}

#[test]
fn finds_text_and_handed_in_pdf_text() {
    let (dir, project, rx) = setup();
    let pdf = dir.path().join("skript.pdf");
    // As the macOS app does after reading the PDF with PDFKit (core API;
    // PdfDocument::set_page_texts wraps it in P2.6).
    atlas_core::pdf::set_page_texts(&pdf.canonicalize().unwrap(), vec!["Einleitung".into(), "Fourier Reihen".into()]);
    // The handle may go away at once (as in Swift): the search goes on.
    let id = project.search().start(query("fourier")).unwrap();
    let search = project.search();
    let (matches, summary) = collect(&rx, id);
    assert_eq!(summary.matches, 4, "{matches:?}");
    assert_eq!(summary.files, 4);
    let c = matches.iter().find(|m| m.relative == "c.txt").unwrap();
    assert_eq!((c.ranges[0].start, c.ranges[0].end), (9, 16));
    assert!(summary.error.is_none());
    let pdf_hit = matches.iter().find(|m| m.pdf).expect("PDF match");
    assert_eq!(pdf_hit.line, 2, "page");
    let md = matches.iter().find(|m| m.relative == "a.md").unwrap();
    let r = &md.ranges[0];
    let utf16: Vec<u16> = md.text.encode_utf16().collect();
    assert_eq!(String::from_utf16(&utf16[r.start as usize..r.end as usize]).unwrap(), "Fourier");
    assert!(summary.kinds.iter().any(|k| k.kind == "markdown" && k.count == 1));

    // Type filter: only code.
    let id = search.start(SearchQuery { kinds: vec!["code".into()], ..query("fourier") }).unwrap();
    let (matches, _) = collect(&rx, id);
    assert!(matches.len() == 1 && matches[0].relative == "b.rs", "{matches:?}");

    // Context lines.
    let lines = search.context(md.path.clone(), 1, 1, false);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1].text, "zweite Zeile");
    let page = search.context(pdf_hit.path.clone(), 2, 0, true);
    assert_eq!(page[0].text, "Fourier Reihen");
}

#[test]
fn bad_patterns_fail_at_once() {
    let (_dir, project, _rx) = setup();
    let search = project.search();
    assert!(search.start(atlas_api::search::SearchQuery { regex: true, ..query("(") }).is_err());
    assert!(search.start(query("")).is_err());
    assert_eq!(search.fuzzy_score("fou".into(), "Fourier".into()).is_some(), true);
}

#[test]
fn searches_annotations() {
    let (dir, project, _rx) = setup();
    let store = atlas_core::annotations::Store::for_atlas_dir(&dir.path().canonicalize().unwrap().join(".atlas"));
    let file = atlas_core::annotations::AnnotationFile {
        document: atlas_core::annotations::DocumentRef { path: "skript.pdf".into(), ..Default::default() },
        annotations: vec![atlas_core::annotations::Annotation {
            id: "a1".into(),
            kind: atlas_core::annotations::Kind::Highlight,
            page: 3,
            text: "Satz von Parseval".into(),
            comment: "wichtig".into(),
            ..serde_json::from_str(r#"{"id":"x","kind":"highlight","page":1,"created":0,"modified":0}"#).unwrap()
        }],
        ..Default::default()
    };
    atlas_core::annotations::write(&store.dir().join("skript.json"), &file).unwrap();
    let hits = project.search().annotations("parseval".into(), 10);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].page, 3);
    assert_eq!(hits[0].comment, "wichtig");
    assert!(project.search().annotations("unbekannt".into(), 10).is_empty());
}
