// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! P2.3: notes, links, tags, quotes, relations, graph and link repair
//! through the facade, against a temporary project and private state/config
//! folders (own test binary).

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use atlas_api::app::Atlas;
use atlas_api::events::{Event, EventSink};
use atlas_api::notes::{LinkKind, Notes};
use atlas_api::project::Project;

struct Collect(Mutex<Sender<Event>>);

impl EventSink for Collect {
    fn on_event(&self, event: Event) {
        let _ = self.0.lock().unwrap().send(event);
    }
}

fn sink() -> (Arc<dyn EventSink>, Receiver<Event>) {
    let (tx, rx) = channel();
    (Arc::new(Collect(Mutex::new(tx))), rx)
}

/// Tests share `app.json`: one at a time, with private state/config folders.
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

fn wait_for(rx: &Receiver<Event>, what: impl Fn(&Event) -> bool) -> Event {
    loop {
        let event = rx.recv_timeout(Duration::from_secs(10)).expect("event in time");
        if what(&event) {
            return event;
        }
    }
}

struct Fixture {
    _serial: MutexGuard<'static, ()>,
    dir: tempfile::TempDir,
    _atlas: Arc<Atlas>,
    project: Arc<Project>,
    notes: Arc<Notes>,
    events: Receiver<Event>,
}

impl Fixture {
    fn path(&self, rel: &str) -> String {
        self.dir.path().canonicalize().unwrap().join(rel).to_string_lossy().into_owned()
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.path(rel)).unwrap()
    }
}

fn fixture() -> Fixture {
    let serial = serial();
    let dir = tempfile::tempdir().unwrap();
    let w = |rel: &str, text: &str| {
        let p = dir.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    };
    w(
        "notes/idee.md",
        "---\ntitle: Idee\ntags: [Rust, lernen]\n---\n# Einleitung\n\nSiehe [[plan]] und [Plan](plan.md#ziele).\n\n## Mehr\n\n> Wichtiger Satz — [skript.pdf, S. 3](/docs/skript.pdf#page=3&annot=abc123)\n\n![Bild](../bild.png)\n",
    );
    w(
        "notes/plan.md",
        "---\ndue: 2026-12-01\nstatus: planned\nestimate: 2h\ntags: [rust]\n---\n# Plan\n\n## Ziele\n\n- [ ] eins\n- [x] zwei\n",
    );
    w("notes/ohne.md", "Kein Link.\n");
    w("docs/skript.pdf", "%PDF-1.4\n");
    w("bild.png", "png");
    let (app_sink, _) = sink();
    let atlas = Atlas::new(app_sink);
    let (window, events) = sink();
    let project = atlas.open_project(dir.path().to_string_lossy().into_owned(), window).unwrap();
    wait_for(&events, |e| matches!(e, Event::IndexStatus { ready: true, .. }));
    let notes = project.notes();
    Fixture { _serial: serial, dir, _atlas: atlas, project, notes, events }
}

#[test]
fn lists_notes_backlinks_and_tags() {
    let f = fixture();
    let list = f.notes.list();
    assert_eq!(list, vec![f.path("notes/idee.md"), f.path("notes/ohne.md"), f.path("notes/plan.md")]);

    let back = f.notes.backlinks(f.path("notes/plan.md"));
    assert_eq!(back.len(), 2, "{back:?}");
    assert!(back.iter().any(|b| b.kind == LinkKind::Wiki && b.raw.contains("plan") && b.line == 7), "{back:?}");
    assert!(back.iter().any(|b| b.kind == LinkKind::Markdown && b.anchor == "ziele"));
    assert!(back.iter().all(|b| b.source == f.path("notes/idee.md") && b.context.contains("Siehe")));
    let pdf = f.notes.backlinks(f.path("docs/skript.pdf"));
    assert_eq!(pdf.len(), 1);
    assert_eq!((pdf[0].kind, pdf[0].anchor.as_str()), (LinkKind::Quote, "3"));
    assert_eq!(f.notes.backlinks(f.path("bild.png"))[0].kind, LinkKind::Embed);

    let tags = f.notes.tags();
    let rust = tags.iter().find(|t| t.key == "rust").unwrap();
    assert_eq!(rust.count, 2);
    assert_eq!(f.notes.tags_of(f.path("notes/idee.md")), vec!["Rust", "lernen"]);
    let mut with = f.notes.notes_with_tag("RUST".into());
    with.sort();
    assert_eq!(with, vec![f.path("notes/idee.md"), f.path("notes/plan.md")]);
}

#[test]
fn deadlines_quotes_checklists_headings() {
    let f = fixture();
    let d = f.notes.deadlines();
    assert_eq!(d.len(), 1);
    assert_eq!(
        (d[0].title.as_str(), d[0].due.as_str(), d[0].status.as_str(), d[0].estimate.as_str()),
        ("plan", "2026-12-01", "planned", "2h")
    );

    let q = f.notes.quotes();
    assert_eq!(q.len(), 1, "{q:?}");
    assert_eq!((q[0].page, q[0].annotation_id.as_str(), q[0].line), (3, "abc123", 11));
    assert_eq!(q[0].pdf, f.path("docs/skript.pdf"));
    assert_eq!(q[0].text, "Wichtiger Satz");

    assert!(f.notes.has_checklist(f.path("notes/plan.md")));
    assert!(!f.notes.has_checklist(f.path("notes/idee.md")));
    assert_eq!(f.notes.heading_line(f.path("notes/idee.md"), "mehr".into()), 9);
    assert_eq!(f.notes.heading_line(f.path("notes/idee.md"), "fehlt".into()), 0);
}

#[test]
fn note_changed_and_append_update_the_index() {
    let f = fixture();
    // The editor's text counts, not the disk.
    f.notes.note_changed(f.path("notes/ohne.md"), Some("Jetzt mit [[plan]].\n".into()));
    wait_for(&f.events, |e| matches!(e, Event::NotesChanged { .. }));
    assert_eq!(f.notes.backlinks(f.path("notes/plan.md")).len(), 3);

    let text = f.notes.append(f.path("notes/ohne.md"), "> Zitat\n".into()).unwrap();
    assert!(text.starts_with("Kein Link.") && text.contains("> Zitat"), "{text}");
    assert_eq!(f.read("notes/ohne.md"), text);
    // A new note is created.
    let new = f.notes.append(f.path("notes/neu.md"), "[[idee]]\n".into()).unwrap();
    assert_eq!(new, "[[idee]]\n");
    assert!(f.notes.list().contains(&f.path("notes/neu.md")));
    assert_eq!(f.notes.backlinks(f.path("notes/idee.md")).len(), 1);
}

#[test]
fn relations_folder_nodes_and_graph() {
    let f = fixture();
    let (a, b) = (f.path("docs/skript.pdf"), f.path("bild.png"));
    assert!(f.notes.relate(a.clone(), b.clone()).unwrap());
    assert!(!f.notes.relate(a.clone(), b.clone()).unwrap(), "already related");
    assert_eq!(f.notes.related(a.clone()), vec![b.clone()]);
    assert_eq!(f.notes.related(b.clone()), vec![a.clone()]);

    assert!(!f.notes.is_folder_node(f.path("docs")));
    assert!(f.notes.set_folder_node(f.path("docs"), true).unwrap());
    assert!(f.notes.is_folder_node(f.path("docs")));
    assert_eq!(f.notes.related(a.clone())[0], f.path("docs"), "folder node first");
    assert!(f.notes.set_folder_node(f.path("docs"), false).unwrap());

    let g = f.notes.graph(f.path("notes/idee.md"), 1);
    let names: Vec<_> = g.nodes.iter().map(|n| n.title.as_str()).collect();
    assert_eq!(g.nodes[0].title, "Idee", "{names:?}");
    // Without a title: the file name (as on Linux).
    assert!(names.contains(&"skript.pdf") && names.contains(&"plan.md"), "{names:?}");
    let pdf = g.nodes.iter().position(|n| n.path == a).unwrap() as u32;
    assert!(g.edges.iter().any(|e| e.to == pdf && e.quotes == 1));
    // The whole project: the unlinked note too, and the related picture.
    let all = f.notes.graph(f.path("notes/idee.md"), 0);
    assert!(all.nodes.iter().any(|n| n.path == f.path("notes/ohne.md")));
    assert!(all.edges.iter().any(|e| e.related));

    assert!(f.notes.unrelate(a.clone(), b.clone()).unwrap());
    assert!(f.notes.related(a).is_empty());
}

#[test]
fn moves_rewrite_links_and_follow_relations() {
    let f = fixture();
    f.notes.relate(f.path("notes/plan.md"), f.path("bild.png")).unwrap();
    std::fs::create_dir_all(f.path("archiv")).unwrap();
    std::fs::rename(f.path("notes/plan.md"), f.path("archiv/plan.md")).unwrap();
    let n = f.notes.files_moved(f.path("notes/plan.md"), f.path("archiv/plan.md")).unwrap();
    assert_eq!(n, 1);
    let idee = f.read("notes/idee.md");
    assert!(idee.contains("archiv/plan"), "{idee}");
    assert_eq!(f.notes.backlinks(f.path("archiv/plan.md")).len(), 2);
    assert_eq!(f.notes.related(f.path("bild.png")), vec![f.path("archiv/plan.md")]);
    wait_for(&f.events, |e| matches!(e, Event::Message { text, .. } if text.contains("Links in")));

    f.notes.files_trashed(vec![f.path("archiv/plan.md")]);
    assert!(f.notes.related(f.path("bild.png")).is_empty());
}

#[test]
fn finds_and_applies_link_repairs() {
    let f = fixture();
    // Moved outside Atlas: the link breaks.
    std::fs::create_dir_all(f.path("archiv")).unwrap();
    std::fs::rename(f.path("notes/plan.md"), f.path("archiv/plan.md")).unwrap();
    f.notes.find_link_repairs(f.path("notes/idee.md"));
    let Event::LinkRepairsFound { path, repairs } =
        wait_for(&f.events, |e| matches!(e, Event::LinkRepairsFound { .. }))
    else {
        unreachable!()
    };
    assert_eq!(path, f.path("notes/idee.md"));
    assert!(!repairs.is_empty(), "suggestion found by the file's identity");
    assert!(repairs.iter().all(|r| r.suggestion == f.path("archiv/plan.md")), "{repairs:?}");
    f.notes.apply_link_repairs(path, repairs).unwrap();
    assert!(f.read("notes/idee.md").contains("archiv/plan"));
    // Nothing left to repair.
    assert!(f.notes.apply_link_repairs(f.path("notes/idee.md"), Vec::new()).is_err());
    let _ = &f.project;
}
