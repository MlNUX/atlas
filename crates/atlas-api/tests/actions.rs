// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! P2.7: actions, background jobs and extensions through the facade,
//! against a temporary project and private state/config folders (own test
//! binary). Commands are plain POSIX tools (`sh`, `echo`, `wc`).

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use atlas_api::AtlasError;
use atlas_api::actions::{ActionSource, Placement};
use atlas_api::app::Atlas;
use atlas_api::events::{Event, EventSink};
use atlas_api::extensions::{CardEntryKind, ExtensionLocation, ExtensionStatus, ProcessedKind, SettingKind};
use atlas_api::project::Project;

struct Collect(Mutex<Sender<Event>>);

impl EventSink for Collect {
    fn on_event(&self, event: Event) {
        let _ = self.0.lock().unwrap().send(event);
    }
}

fn config_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("ATLAS_CONFIG_DIR").unwrap())
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
    // Each test starts without user actions and extensions.
    let _ = std::fs::remove_dir_all(config_dir());
    guard
}

struct Fixture {
    _serial: MutexGuard<'static, ()>,
    dir: tempfile::TempDir,
    atlas: Arc<Atlas>,
    project: Arc<Project>,
    events: Receiver<Event>,
}

impl Fixture {
    fn path(&self, rel: &str) -> String {
        self.dir.path().canonicalize().unwrap().join(rel).to_string_lossy().into_owned()
    }
}

fn write(path: impl AsRef<std::path::Path>, text: &str) {
    let path = path.as_ref();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn fixture() -> Fixture {
    let serial = serial();
    let dir = tempfile::tempdir().unwrap();
    write(dir.path().join("text.md"), "eins zwei drei\n");
    write(dir.path().join("daten.bin"), "abc");
    write(dir.path().join("run.sh"), "echo hallo\n");
    write(
        dir.path().join(".atlas/actions.yaml"),
        r#"version: 1
actions:
  - name: Wörter zählen
    applies_to: "*.md"
    command: [wc, -w, "{file}"]
    keybinding: Ctrl+Alt+W
  - name: Server
    when: project
    terminal: background
    command: [sh, -c, "echo läuft; sleep 30"]
  - name: Aufräumen
    when: project
    command: [rm, -rf, build]
  - name: kaputt
"#,
    );
    let (tx, rx) = channel();
    let sink: Arc<dyn EventSink> = Arc::new(Collect(Mutex::new(tx)));
    let atlas = Atlas::new(sink.clone());
    let project = atlas.open_project(dir.path().to_string_lossy().into_owned(), sink).unwrap();
    Fixture { _serial: serial, dir, atlas, project, events: rx }
}

#[test]
fn actions_prepare_and_run_in_background() {
    let f = fixture();
    let actions = f.project.actions();
    assert!(!actions.errors().is_empty(), "the broken entry is reported");

    let for_file = actions.actions_for(vec![f.path("text.md")]);
    let count = for_file.iter().find(|a| a.name == "Wörter zählen").expect("file action");
    assert_eq!(count.source, ActionSource::Project);
    assert_eq!(count.keybinding, "Ctrl+Alt+W");
    let project = actions.actions_for(Vec::new());
    assert!(project.iter().any(|a| a.name == "Server"));
    assert!(!project.iter().any(|a| a.name == "Wörter zählen"));

    let p = actions.prepare(count.id.clone(), vec![f.path("text.md")], 0).unwrap();
    assert_eq!(p.placement, Placement::Panel);
    assert!(p.argv.contains(&f.path("text.md")), "{:?}", p.argv);
    assert_eq!(p.cwd, f.path("").trim_end_matches('/'));
    assert!(matches!(actions.prepare(count.id.clone(), Vec::new(), 0), Err(AtlasError::Invalid { .. })));
    assert!(matches!(actions.prepare("weg".into(), Vec::new(), 0), Err(AtlasError::NotFound { .. })));

    let clean = project.iter().find(|a| a.name == "Aufräumen").unwrap();
    let p = actions.prepare(clean.id.clone(), Vec::new(), 0).unwrap();
    assert!(p.destructive && p.confirm);

    assert_eq!(actions.keybindings().iter().map(|k| k.sequence.as_str()).collect::<Vec<_>>(), vec!["Ctrl+Alt+W"]);

    assert!(actions.is_runnable(f.path("run.sh")));
    assert!(!actions.is_runnable(f.path("text.md")));
    assert!(actions.prepare_script(f.path("run.sh")).unwrap().display.contains("run.sh"));

    // Background job: started, visible to the app, stopped.
    let server = project.iter().find(|a| a.name == "Server").unwrap();
    let p = actions.prepare(server.id.clone(), Vec::new(), 0).unwrap();
    assert_eq!(p.placement, Placement::Background);
    actions.start_background(server.id.clone(), p).unwrap();
    assert!(f.events.try_iter().any(|e| matches!(e, Event::BackgroundJobChanged { .. })));
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let job = f.atlas.background_job(server.id.clone()).expect("job");
        if job.log.contains("läuft") {
            assert!(job.running);
            break;
        }
        assert!(Instant::now() < until, "no output: {job:?}");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(f.atlas.background_jobs().iter().any(|j| j.key == server.id));
    assert!(f.atlas.stop_background_job(server.id.clone()));
    let until = Instant::now() + Duration::from_secs(10);
    while f.atlas.background_job(server.id.clone()).unwrap().running {
        assert!(Instant::now() < until, "job did not end");
        std::thread::sleep(Duration::from_millis(50));
    }

    let user = actions.actions_file(true).unwrap();
    assert!(std::fs::read_to_string(&user).unwrap().contains("version: 1"));
    assert_eq!(actions.actions_file(false).unwrap(), f.path(".atlas/actions.yaml"));
}

const MANIFEST: &str = r#"atlas_api: 2
name: zaehler
title: Zähler
version: 0.1.0
description: Zählt Wörter.
permissions:
  run: [wc, sh, cat]
  files: project
  network: false
commands:
  - name: Zählen (Erweiterung)
    applies_to: "*.md"
    command: [wc, -w, "{file}"]
    keybinding: Ctrl+Alt+W
settings:
  - key: format
    title: Format
    type: choice
    choices: [html, docx]
    default: html
  - key: token
    title: Zugang
    type: secret
processors:
  - name: Rohtext
    applies_to: ["*.bin"]
    output: text
    command: [cat, "{file}"]
exporters:
  - name: Kopie
    applies_to: "*.md"
    target: txt
    command: [sh, -c, "cat \"$0\" > \"$1\"", "{file}", "{output}"]
views:
  - id: tafel
    name: Tafel
    page: tafel.html
    keybinding: Ctrl+Alt+T
"#;

fn rt<F: std::future::Future>(future: F) -> F::Output {
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

#[test]
fn extensions_settings_views_processors_and_exporters() {
    let f = fixture();
    let ext = f.project.extensions();
    write(config_dir().join("extensions/zaehler/extension.yaml"), MANIFEST);
    write(config_dir().join("extensions/zaehler/tafel.html"), "<p>Tafel</p>");

    let list = ext.list();
    let info = list.iter().find(|e| e.name == "zaehler").expect("user extension");
    assert_eq!((info.location, info.status), (ExtensionLocation::User, ExtensionStatus::Active));
    assert!(info.permissions.iter().any(|p| p.starts_with("Programme: wc")), "{:?}", info.permissions);
    // The project action had Ctrl+Alt+W first.
    assert!(info.errors.iter().any(|e| e.contains("Ctrl+Alt+W")), "{:?}", info.errors);
    let format = info.settings.iter().find(|s| s.key == "format").unwrap();
    assert_eq!((format.kind, format.value.as_str()), (SettingKind::Choice, "\"html\""));

    ext.set_setting("zaehler".into(), "format".into(), Some("\"docx\"".into())).unwrap();
    assert!(matches!(
        ext.set_setting("zaehler".into(), "format".into(), Some("\"pdf\"".into())),
        Err(AtlasError::Invalid { .. })
    ));
    let info = ext.list().into_iter().find(|e| e.name == "zaehler").unwrap();
    assert_eq!(info.settings.iter().find(|s| s.key == "format").unwrap().value, "\"docx\"");
    ext.set_setting("zaehler".into(), "format".into(), None).unwrap();
    assert!(f.events.try_iter().any(|e| matches!(e, Event::ExtensionsChanged)));

    // Extension commands join the actions.
    let actions = f.project.actions().actions_for(vec![f.path("text.md")]);
    assert!(actions.iter().any(|a| a.source == ActionSource::Extension && a.extension == "zaehler"));
    assert!(f.project.actions().keybindings().iter().any(|k| k.sequence == "Ctrl+Alt+T"));

    // Views.
    let views = ext.views();
    assert_eq!(views.len(), 1);
    let page = ext.view(views[0].id.clone()).unwrap();
    assert!(page.url.starts_with("file://") && page.url.ends_with("/zaehler/tafel.html"), "{}", page.url);
    assert!(!page.network);
    let context = rt(ext.call("zaehler".into(), "context".into(), "{}".into())).unwrap();
    assert!(context.contains("\"extension\":\"zaehler\""), "{context}");
    assert!(matches!(rt(ext.call("fremd".into(), "context".into(), "{}".into())), Err(AtlasError::Unavailable { .. })));

    // Processors and exporters.
    let procs = ext.processors_for(f.path("daten.bin"));
    assert_eq!(procs.len(), 1);
    let out = rt(ext.process(procs[0].id.clone(), f.path("daten.bin"))).unwrap();
    assert_eq!(out.kind, ProcessedKind::Text);
    assert!(out.content.contains("abc"));
    let exporters = ext.exporters_for(f.path("text.md"));
    assert_eq!(exporters.len(), 1);
    assert!(exporters[0].output.ends_with("text.txt"));
    let done = rt(ext.export(exporters[0].id.clone(), f.path("text.md"), exporters[0].output.clone())).unwrap();
    assert!(done.message.contains("Kopie"));
    assert_eq!(std::fs::read_to_string(&exporters[0].output).unwrap(), "eins zwei drei\n");

    // Switched off: gone from views and actions.
    ext.set_enabled("zaehler".into(), false).unwrap();
    assert!(ext.views().is_empty());
    assert_eq!(ext.list().iter().find(|e| e.name == "zaehler").unwrap().status, ExtensionStatus::Disabled);
    ext.set_enabled("zaehler".into(), true).unwrap();

    // New and bundled extensions.
    let manifest = ext.create("Mein Werkzeug".into()).unwrap();
    assert!(manifest.ends_with("extension.yaml") && std::path::Path::new(&manifest).is_file());
    let bundled = ext.bundled();
    assert!(!bundled.is_empty() && bundled.iter().all(|b| !b.installed));
    ext.install_bundled(bundled[0].name.clone()).unwrap();
    let installed = ext.list().into_iter().find(|e| e.name == bundled[0].name).unwrap();
    assert_eq!(installed.location, ExtensionLocation::Bundled);
    assert!(matches!(ext.install_bundled("gibtsnicht".into()), Err(AtlasError::NotFound { .. })));
}

#[test]
fn project_extensions_need_trust() {
    let f = fixture();
    let manifest =
        MANIFEST.replace("name: zaehler", "name: projektzaehler").replace("    keybinding: Ctrl+Alt+W\n", "");
    write(f.path(".atlas/extensions/projektzaehler/extension.yaml"), &manifest);
    write(f.path(".atlas/extensions/projektzaehler/tafel.html"), "<p/>");
    let ext = f.project.extensions();
    assert_eq!(ext.untrusted(), vec!["projektzaehler"]);
    assert!(ext.views().is_empty());
    ext.trust("projektzaehler".into()).unwrap();
    assert!(ext.untrusted().is_empty());
    assert_eq!(ext.views().len(), 1);
    // Changed after trusting: asks again.
    write(f.path(".atlas/extensions/projektzaehler/tafel.html"), "<p>neu</p>");
    assert_eq!(ext.untrusted(), vec!["projektzaehler"]);
}

#[test]
fn extension_cards_in_the_context_bar() {
    let f = fixture();
    write(config_dir().join("extensions/zaehler/extension.yaml"), MANIFEST);
    write(config_dir().join("extensions/zaehler/tafel.html"), "<p>Tafel</p>");
    let ext = f.project.extensions();
    let cards = ext.context_cards();
    let card = cards.iter().find(|c| c.name == "zaehler").expect("card");
    assert_eq!(card.title, "Zähler");
    assert!(!card.shown, "not asked for (`context:`) and not chosen");
    assert!(card.entries.iter().any(|e| e.kind == CardEntryKind::View && e.id == "view:zaehler/tafel"));
    assert!(card.entries.iter().any(|e| e.kind == CardEntryKind::Command && e.name == "Zählen (Erweiterung)"));

    ext.set_card_shown("zaehler".into(), true).unwrap();
    assert!(ext.context_cards().iter().find(|c| c.name == "zaehler").unwrap().shown);
    // The bar knows the card as block `ext:zaehler`.
    let ctx = f.project.context();
    let layout = format!("{:?}", ctx.layout("default".into()));
    assert!(layout.contains("ext:zaehler"), "{layout}");

    ext.set_enabled("zaehler".into(), false).unwrap();
    assert!(ext.context_cards().is_empty());
    assert!(!format!("{:?}", ctx.layout("default".into())).contains("ext:zaehler"));
}
