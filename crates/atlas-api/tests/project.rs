// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! P2.2: project, sessions and files through the facade, against a
//! temporary project and private state/config folders (own test binary,
//! so the environment variables do not leak into other tests).

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use atlas_api::app::Atlas;
use atlas_api::events::{Event, EventSink};
use atlas_api::files::{ConflictPolicy, ListOptions, SortKey, Transfer};

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

/// Tests share `app.json` (load, change, save): one at a time.
fn private_dirs() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    dirs();
    guard
}

fn dirs() -> &'static tempfile::TempDir {
    static DIRS: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIRS.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        // SAFETY: set once, before any test of this binary reads them.
        unsafe {
            std::env::set_var("ATLAS_STATE_DIR", dir.path().join("state"));
            std::env::set_var("ATLAS_CONFIG_DIR", dir.path().join("config"));
        }
        dir
    })
}

fn wait_for(rx: &Receiver<Event>, what: impl Fn(&Event) -> bool) -> Event {
    loop {
        let event = rx.recv_timeout(Duration::from_secs(10)).expect("event in time");
        if what(&event) {
            return event;
        }
    }
}

fn project_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/Notiz.md"), "# Notiz\n[[Zweite]]\n").unwrap();
    std::fs::write(dir.path().join("docs/Zweite.md"), "zwei\n").unwrap();
    std::fs::write(dir.path().join("skript.pdf"), b"%PDF-1.4\n").unwrap();
    dir
}

#[test]
fn opens_a_project_and_builds_the_index() {
    let _serial = private_dirs();
    let dir = project_dir();
    let (app_sink, _) = sink();
    let atlas = Atlas::new(app_sink);
    let (window, events) = sink();
    let project = atlas.open_project(dir.path().to_string_lossy().into_owned(), window).unwrap();

    assert_eq!(project.name(), dir.path().file_name().unwrap().to_string_lossy());
    assert!(project.storage_error().is_none());
    assert!(std::path::Path::new(&project.atlas_dir()).is_dir());
    wait_for(&events, |e| matches!(e, Event::IndexStatus { ready: true, .. }));
    assert!(project.index_state().ready);

    // Quick open finds the notes after indexing.
    let hits = project.files().quick_open("zweit".into(), 5);
    assert_eq!(hits[0].relative, "docs/Zweite.md");
    let hits = project.files().quick_open("notiz:2".into(), 5);
    assert_eq!(hits[0].line, 2);

    // The project is remembered with its session.
    let known = atlas.known_projects();
    assert!(known.iter().any(|k| k.root == project.root() && k.session.is_some()));

    // Relative and absolute paths.
    let note = format!("{}/docs/Notiz.md", project.root());
    assert_eq!(project.relative_path(note.clone()).as_deref(), Some("docs/Notiz.md"));
    assert_eq!(project.absolute_path("/docs/Notiz.md".into()), note);
    assert_eq!(project.relative_path("/etc/hosts".into()), None);
}

#[test]
fn sessions_round_trip_and_switch() {
    let _serial = private_dirs();
    let dir = project_dir();
    let (s, _) = sink();
    let atlas = Atlas::new(s);
    let (w, _) = sink();
    let project = atlas.open_project(dir.path().to_string_lossy().into_owned(), w).unwrap();
    let root = project.root();

    let layout = r#"{"editor":{"groups":[{"tabs":[{"path":"docs/Notiz.md"},{"path":"skript.pdf"}]}]}}"#;
    project
        .save_session(
            layout.into(),
            vec![format!("{root}/docs")],
            vec![format!("{root}/docs/Notiz.md"), format!("{root}/skript.pdf")],
        )
        .unwrap();
    project.flush_session();
    let saved: serde_json::Value = serde_json::from_str(&project.session_layout()).unwrap();
    assert_eq!(saved, serde_json::from_str::<serde_json::Value>(layout).unwrap());
    assert_eq!(project.session_expanded(), vec![format!("{root}/docs")]);
    let summary = project.sessions().into_iter().find(|s| s.active).unwrap();
    assert_eq!(summary.tabs, 2);

    // A second, empty session; switching keeps the first one's layout.
    let id = project.create_session("Literatur".into(), None, Vec::new()).unwrap();
    project.switch_session(id.clone()).unwrap();
    assert!(project.session_layout().is_empty());
    assert_eq!(project.sessions().len(), 2);
    project.rename_session(id.clone(), "Recherche".into()).unwrap();
    assert!(project.sessions().iter().any(|s| s.name == "Recherche" && s.active));

    // Invalid layouts are rejected instead of stored.
    assert!(project.save_session("{kaputt".into(), Vec::new(), Vec::new()).is_err());

    // Document state is kept per file.
    let note = format!("{root}/docs/Notiz.md");
    project.save_document_state(note.clone(), r#"{"line":2}"#.into());
    project.flush_session();
    assert_eq!(project.document_state(note).as_deref(), Some(r#"{"line":2}"#));
}

#[test]
fn lists_creates_renames_and_transfers_files() {
    let _serial = private_dirs();
    let dir = project_dir();
    let (s, _) = sink();
    let atlas = Atlas::new(s);
    let (w, events) = sink();
    let project = atlas.open_project(dir.path().to_string_lossy().into_owned(), w).unwrap();
    let files = project.files();
    let root = project.root();
    let options = ListOptions { show_hidden: false, folders_only: false, sort: SortKey::Name, ascending: true };

    let names: Vec<String> = files.list(root.clone(), options.clone()).unwrap().into_iter().map(|e| e.name).collect();
    assert_eq!(names, vec!["docs", "skript.pdf"], "folders first, .atlas hidden");
    assert_eq!(files.file_kind(format!("{root}/skript.pdf")), "pdf");
    assert_eq!(files.file_kind(format!("{root}/docs/Notiz.md")), "markdown");
    assert_eq!(files.file_kind("terminal:/tmp#1".into()), "terminal");

    assert!(files.check_name("a/b".into()).is_some());
    let new = files.new_file(format!("{root}/docs"), "Neu.md".into()).unwrap();
    assert!(files.new_file(format!("{root}/docs"), "Neu.md".into()).is_err(), "no overwrite");
    let renamed = files.rename(new, "Umbenannt.md".into()).unwrap();
    files.write_text(renamed.clone(), "Inhalt\n".into()).unwrap();
    assert_eq!(files.read_text(renamed.clone(), 1024).unwrap(), "Inhalt\n");
    assert_eq!(files.read_text(renamed.clone(), 3).unwrap(), "Inh");

    let dest = files.new_folder(root.clone(), "Ziel".into()).unwrap();
    assert!(files.conflicts(vec![renamed.clone()], dest.clone()).is_empty());
    let id = files.transfer(Transfer::Copy, vec![renamed.clone()], dest.clone(), ConflictPolicy::Abort);
    let done = wait_for(
        &events,
        |e| matches!(e, Event::FileOperation { operation_id, done: true, .. } if *operation_id == id),
    );
    let Event::FileOperation { error, text, .. } = done else { unreachable!() };
    assert_eq!(error, None);
    assert!(text.contains("Umbenannt.md"));
    assert_eq!(files.conflicts(vec![renamed.clone()], dest.clone()).len(), 1);

    let props = files.properties(renamed).unwrap();
    assert_eq!(props.size, Some(7));
    assert!(props.mode & 0o400 != 0);
}

#[test]
fn places_and_urls() {
    let _serial = private_dirs();
    let (s, _) = sink();
    let atlas = Atlas::new(s);
    let places = atlas.places();
    assert!(places.iter().any(|p| p.icon == "home" && p.fixed));
    assert_eq!(atlas.url_to_path("file:///tmp/a%20b.md".into()).as_deref(), Some("/tmp/a b.md"));
    assert_eq!(atlas.url_to_path("file://localhost/tmp/x".into()).as_deref(), Some("/tmp/x"));
    assert_eq!(atlas.url_to_path("https://example.org".into()), None);
}
