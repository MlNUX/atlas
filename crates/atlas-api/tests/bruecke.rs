// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! P2.13: the rest of the Linux bridge – marked passages of the project,
//! pictures and files in notes, folder tree, `.atlasnote` pages – against a
//! temporary project and private state/config/cache folders (own test binary).

use std::io::Write;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use atlas_api::app::Atlas;
use atlas_api::atlasnote::{AtlasNote, NoteMark, PatternKind};
use atlas_api::events::{Event, EventSink};
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
            std::env::set_var("XDG_CACHE_HOME", dir.path().join("cache"));
        }
        dir
    });
    guard
}

fn write(path: impl AsRef<std::path::Path>, data: &[u8]) {
    let path = path.as_ref();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, data).unwrap();
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

    /// Waits until the note index is built.
    fn notes_ready(&self) {
        loop {
            match self.events.recv_timeout(Duration::from_secs(10)).expect("index in time") {
                Event::NotesChanged { .. } => return,
                _ => continue,
            }
        }
    }
}

const ANNOTATIONS: &str = r##"{"version":1,"document":{"path":"docs/paper.pdf","blake3":"x","size":3,"pages":3},"annotations":[
 {"id":"aaaaaaaaaaaa","kind":"highlight","page":2,"rects":[[1,2,3,4]],"text":"Erster Teil","comment":"wichtig","color":"#ffd400","group":"g","created":1,"modified":1},
 {"id":"bbbbbbbbbbbb","kind":"highlight","page":3,"rects":[[1,2,3,4]],"text":"zweiter Teil","color":"#ffd400","group":"g","created":1,"modified":1},
 {"id":"cccccccccccc","kind":"underline","page":1,"rects":[[1,2,3,4]],"text":"Vorne","color":"#00ff00","created":1,"modified":1},
 {"id":"dddddddddddd","kind":"bookmark","page":1,"title":"Merk","created":1,"modified":1}]}"##;

fn fixture() -> Fixture {
    let serial = serial();
    let dir = tempfile::tempdir().unwrap();
    write(dir.path().join("docs/paper.pdf"), b"pdf");
    write(dir.path().join(".atlas/annotations/paper-1234.json"), ANNOTATIONS.as_bytes());
    write(
        dir.path().join("notes/lesen.md"),
        b"---\ntitle: Lesenotiz\n---\n# Lesen\n\n> Erster Teil\n>\n> \xe2\x80\x94 [paper.pdf, S. 2](/docs/paper.pdf#page=2&annot=aaaaaaaaaaaa)\n",
    );
    write(dir.path().join("src/lib.rs"), b"");
    write(dir.path().join("src/sub/mod.rs"), b"");
    let (tx, rx) = channel();
    let sink: Arc<dyn EventSink> = Arc::new(Collect(Mutex::new(tx)));
    let atlas = Atlas::new(sink.clone());
    let project = atlas.open_project(dir.path().to_string_lossy().into_owned(), sink).unwrap();
    Fixture { _serial: serial, dir, _atlas: atlas, project, events: rx }
}

#[test]
fn marked_passages_grouped_with_citations() {
    let f = fixture();
    f.notes_ready();
    let list = f.project.notes().marked_passages();
    assert_eq!(list.len(), 2, "{list:?}");
    // Sorted by page; the group is one passage with its text joined.
    assert_eq!((list[0].page, list[0].text.as_str()), (1, "Vorne"));
    assert!(list[0].cited.is_empty());
    assert_eq!(list[1].text, "Erster Teil zweiter Teil");
    assert_eq!(list[1].comment, "wichtig");
    assert_eq!(list[1].pdf, f.path("docs/paper.pdf"));
    assert_eq!(list[1].cited.len(), 1);
    assert_eq!(list[1].cited[0].note, f.path("notes/lesen.md"));
    assert_eq!(list[1].cited[0].title.as_deref(), Some("Lesenotiz"));
}

#[test]
fn pictures_and_files_into_notes() {
    let f = fixture();
    let notes = f.project.notes();
    let note = f.path("notes/lesen.md");

    let target = notes.image_target(note.clone(), f.path("docs/paper.pdf"), 4).unwrap();
    assert!(target.starts_with(&f.path("assets/paper-s4-")) && target.ends_with(".png"), "{target}");
    write(&target, b"png");
    let quote = notes.image_quote(note.clone(), target.clone(), f.path("docs/paper.pdf"), 4);
    let name = std::path::Path::new(&target).file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(quote, format!("![paper.pdf, S. 4](../assets/{name})\n*[paper.pdf, S. 4](/docs/paper.pdf#page=4)*"));

    let pasted = notes.new_image_path(note.clone(), "".into()).unwrap();
    assert!(pasted.starts_with(&f.path("assets/lesen-")) && pasted.ends_with(".png"), "{pasted}");
    assert!(notes.image_markdown(note.clone(), pasted.clone()).starts_with("![lesen-"));

    let outside = tempfile::tempdir().unwrap();
    let foto = outside.path().join("Foto 1.jpg");
    write(&foto, b"jpg");
    let md = notes.import_files(
        note.clone(),
        vec![
            format!("file://{}", foto.to_string_lossy().replace(' ', "%20")),
            f.path("docs/paper.pdf"),
            "/fehlt".into(),
        ],
    );
    let lines: Vec<&str> = md.lines().collect();
    assert_eq!(lines.len(), 2, "{md}");
    assert!(lines[0].starts_with("![") && lines[0].contains("../assets/"), "{md}");
    assert!(lines[1].contains("../docs/paper.pdf"), "project files are linked where they are: {md}");
    assert!(
        std::path::Path::new(&f.path("assets")).read_dir().unwrap().any(|e| e.unwrap().file_name() == "Foto 1.jpg")
    );
}

#[test]
fn folder_tree_for_notes() {
    let f = fixture();
    let tree = f.project.editor().folder_tree(f.path("src"));
    assert!(tree.contains("lib.rs") && tree.contains("sub") && tree.contains("mod.rs"), "{tree}");
}

fn notebook(dir: &std::path::Path) -> String {
    let json = r#"{"version":3,"pages":[
      {"id":"a","paper":"LINED","dark":false,"height":1414,"marks":[
        {"style":{"tool":"PEN","color":-16776961,"width":6},"points":[[1,100,100,0,0,0.8,0,0],[1,400,100,1,0,0.6,0,0]]},
        {"style":{"tool":"HIGHLIGHTER","color":-10923,"width":22},"points":[[1,100,300,0,0,-1,-1,-1],[1,600,300,1,0,-1,-1,-1]]},
        {"style":{"tool":"TEXT","color":-16777216,"width":20},"text":"Hallo\nWelt","rotation":90,"points":[[1,100,500,0,0,-1,-1,-1]]},
        {"style":{"tool":"PEN","color":-16777216,"width":3},"image":"pic-1.png",
         "points":[[1,500,700,0,0,-1,-1,-1],[1,900,700,1,0,-1,-1,-1],[1,500,900,2,0,-1,-1,-1]]}]},
      {"id":"b","paper":"DOTS","dark":true,"height":1000,"marks":[]},
      {"id":"c","paper":"PLAIN","dark":false,"height":1414,"pdf":{"file":"doc-1.pdf","page":2},"marks":[]}]}"#;
    let path = dir.join("Tablet.atlasnote");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    for (name, data) in [("notebook.json", json.as_bytes()), ("images/pic-1.png", b"png"), ("pdf/doc-1.pdf", b"pdf")] {
        zip.start_file(name, options).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn atlasnote_pages_for_drawing() {
    let _serial = serial();
    let dir = tempfile::tempdir().unwrap();
    let note = AtlasNote::open(notebook(dir.path())).unwrap();
    assert_eq!((note.page_width(), note.page_count()), (1000.0, 3));
    let pages = note.pages();

    let first = &pages[0];
    let lines = first.pattern.expect("lined paper");
    assert_eq!((lines.kind, lines.step, lines.start), (PatternKind::Lines, 32.0, 48.0));
    assert!(first.background.r > 0.99 && first.pdf.is_none());
    let NoteMark::Stroke { pressure, color, points, .. } = &first.marks[0] else { panic!("stroke") };
    assert!(*pressure && color.b > 0.99 && color.a > 0.99);
    assert_eq!((points[1].x, points[1].y, points[1].pressure), (400.0, 100.0, 0.6));
    let NoteMark::Stroke { pressure, color, .. } = &first.marks[1] else { panic!("highlighter") };
    assert!(!*pressure && (color.a - 0x55 as f64 / 255.0).abs() < 1e-9, "highlighter is translucent");
    let NoteMark::Text { lines, size, rotation, .. } = &first.marks[2] else { panic!("text") };
    assert_eq!((lines.clone(), *size, *rotation), (vec!["Hallo".to_owned(), "Welt".to_owned()], 20.0, 90.0));
    let NoteMark::Image { path, top_right, .. } = &first.marks[3] else { panic!("image") };
    assert!(std::path::Path::new(path).is_file() && path.ends_with("images/pic-1.png"));
    assert_eq!((top_right.x, top_right.y), (900.0, 700.0));

    let dark = &pages[1];
    assert!(dark.dark && dark.background.r < 0.2);
    assert_eq!(dark.pattern.unwrap().kind, PatternKind::Dots);

    let pdf = pages[2].pdf.as_ref().expect("pdf background");
    assert!(std::path::Path::new(&pdf.path).is_file() && pdf.page == 2);
    assert!(pages[2].pattern.is_none());

    assert!(note.page(9).is_none());
    assert!(AtlasNote::open(dir.path().join("fehlt.atlasnote").to_string_lossy().into_owned()).is_err());
}
