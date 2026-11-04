// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! P2.8: editor logic, settings, context bar and spelling through the
//! facade (private config/state folders; tests serialized because they
//! share `config.toml`).

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use atlas_api::app::Atlas;
use atlas_api::editor::{CompletionKind, HighlightKind};
use atlas_api::events::{Event, EventSink};
use atlas_api::settings::{ContextArea, ContextTarget, Theme};

struct Collect(Mutex<Sender<Event>>);

impl EventSink for Collect {
    fn on_event(&self, event: Event) {
        let _ = self.0.lock().unwrap().send(event);
    }
}

fn serial() -> MutexGuard<'static, ()> {
    static DIRS: OnceLock<tempfile::TempDir> = OnceLock::new();
    static LOCK: Mutex<()> = Mutex::new(());
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    DIRS.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        // SAFETY: set once, before any test of this binary reads them.
        unsafe {
            std::env::set_var("ATLAS_STATE_DIR", dir.path().join("state"));
            std::env::set_var("ATLAS_CONFIG_DIR", dir.path().join("config"));
        }
        std::fs::create_dir_all(dir.path().join("config")).unwrap();
        dir
    });
    guard
}

fn open() -> (tempfile::TempDir, Arc<Atlas>, Arc<atlas_api::project::Project>, Receiver<Event>) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Notiz.md"), "# Notiz\n\n#wichtig #wiki\n").unwrap();
    std::fs::write(dir.path().join("Wikipedia.md"), "tags: [wiki]\n#wiki\n").unwrap();
    let (tx, rx) = channel();
    let atlas = Atlas::new(Arc::new(Collect(Mutex::new(channel().0))));
    let project =
        atlas.open_project(dir.path().to_string_lossy().into_owned(), Arc::new(Collect(Mutex::new(tx)))).unwrap();
    // Wait until notes and files are indexed (completion uses them).
    loop {
        if let Event::IndexStatus { ready: true, .. } = rx.recv_timeout(Duration::from_secs(10)).unwrap() {
            break;
        }
    }
    (dir, atlas, project, rx)
}

fn apply(text: &str, e: &atlas_api::editor::Edit) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut out: Vec<u16> = units[..e.start as usize].to_vec();
    out.extend(e.text.encode_utf16());
    out.extend(&units[e.end as usize..]);
    String::from_utf16(&out).unwrap()
}

#[test]
fn lists_tables_and_tasks() {
    let _s = serial();
    let (_dir, _atlas, project, _rx) = open();
    let editor = project.editor();

    let text = "- eins";
    let e = editor.continue_list(text.into(), 6).unwrap();
    assert_eq!(apply(text, &e), "- eins\n- ");
    assert_eq!(e.cursor, 9);

    let text = "- eins\n- zwei";
    let e = editor.indent_list_items(text.into(), 9, 9, false).unwrap();
    assert!(apply(text, &e).contains("\n  - zwei"), "{:?}", apply(text, &e));

    let table = "| a | bb |\n|---|---|\n| ccc | d |";
    let aligned = editor.format_table(table.into());
    assert!(aligned.lines().all(|l| l.len() == aligned.lines().next().unwrap().len()), "{aligned}");
    assert_eq!(editor.align_tables(table.into(), 0).len(), 1);

    let text = "- [ ] Aufgabe";
    let e = editor.toggle_task(text.into(), 1, true).unwrap();
    assert_eq!(apply(text, &e), "- [x] Aufgabe");

    // UTF-16: an emoji before the list counts two units.
    let text = "🙂\n- eins";
    let e = editor.continue_list(text.into(), 9).unwrap();
    assert_eq!(apply(text, &e), "🙂\n- eins\n- ");
}

#[test]
fn completes_wiki_links_and_tags() {
    let _s = serial();
    let (_dir, _atlas, project, _rx) = open();
    let editor = project.editor();

    let text = "Siehe [[Wiki";
    let ctx = editor.completion_at(text.into(), 12).unwrap();
    assert_eq!(ctx.kind, CompletionKind::WikiLink);
    assert_eq!(ctx.typed, "Wiki");
    let items = editor.completions(ctx.clone(), 5);
    let item = items.iter().find(|i| i.value == "Wikipedia.md").expect("note candidate").clone();
    let e = editor.accept_completion(text.into(), ctx, 12, item);
    assert_eq!(apply(text, &e), "Siehe [[/Wikipedia.md]]");

    let text = "Text #wi";
    let ctx = editor.completion_at(text.into(), 8).unwrap();
    assert_eq!(ctx.kind, CompletionKind::Tag);
    let items = editor.completions(ctx.clone(), 5);
    assert_eq!(items[0].value, "wiki", "{items:?}");
    let e = editor.accept_completion(text.into(), ctx, 8, items[0].clone());
    assert_eq!(apply(text, &e), "Text #wiki");

    assert!(editor.completion_at("nichts".into(), 6).is_none());
}

#[test]
fn highlights_incrementally_and_renders() {
    let _s = serial();
    let (dir, _atlas, project, _rx) = open();
    let editor = project.editor();
    let language = editor.language(dir.path().join("a.rs").to_string_lossy().into_owned()).unwrap();
    let h = editor.highlighter(language);
    assert!(h.known());
    let spans = h.update("fn main() { let x = 1; } // c".into());
    assert!(spans.iter().any(|s| s.kind == HighlightKind::Keyword));
    assert!(spans.iter().any(|s| s.kind == HighlightKind::Comment));
    let again = h.update("fn main() { let x = 2; } // c".into());
    assert_eq!(again.len(), spans.len());

    let note = dir.path().join("Notiz.md").to_string_lossy().into_owned();
    let r = editor.render_markdown("# Titel\n\n[weg](fehlt.md) [da](Notiz.md)\n".into(), note.clone());
    assert!(r.html.contains("<h1"), "{}", r.html);
    assert_eq!(r.headings[0].text, "Titel");
    assert_eq!(r.broken_links, 1);
    let code = dir.path().join("a.rs").to_string_lossy().into_owned();
    let symbols = editor.symbols(code, "fn eins() {}\nstruct Zwei;\n".into());
    assert!(symbols.iter().any(|s| s.name == "eins" && s.line == 1), "{symbols:?}");
    assert!(editor.code_block("x = 1".into(), "python".into()).starts_with("```python"));
}

#[test]
fn settings_round_trip() {
    let _s = serial();
    let (tx, rx) = channel();
    let atlas = Atlas::new(Arc::new(Collect(Mutex::new(tx))));
    let settings = atlas.settings();
    let mut s = settings.get();
    s.theme = Theme::System;
    s.pdf_dark = !s.pdf_dark;
    s.shell = "/bin/sh".into();
    settings.save(s.clone()).unwrap();
    assert_eq!(settings.get(), s);
    assert!(rx.try_iter().any(|e| e == Event::SettingsChanged));
    assert!(std::fs::read_to_string(settings.file()).unwrap().contains("[terminal]"));
    let mut sort = settings.folder_sort();
    sort.ascending = !sort.ascending;
    settings.set_folder_sort(sort.clone()).unwrap();
    assert_eq!(settings.folder_sort(), sort);
    let mut ed = atlas
        .open_project(
            tempfile::tempdir().unwrap().keep().to_string_lossy().into_owned(),
            Arc::new(Collect(Mutex::new(channel().0))),
        )
        .unwrap()
        .editor()
        .external_editor();
    ed.command = "nvim +{line} {file}".into();
    ed.in_terminal = true;
    let project = atlas
        .open_project(
            tempfile::tempdir().unwrap().keep().to_string_lossy().into_owned(),
            Arc::new(Collect(Mutex::new(channel().0))),
        )
        .unwrap();
    project.editor().set_external_editor(ed).unwrap();
    match project.editor().external_launch("/tmp/a.md".into(), 7).unwrap() {
        atlas_api::editor::ExternalLaunch::Terminal { argv } => assert_eq!(argv, vec!["nvim", "+7", "/tmp/a.md"]),
        other => panic!("{other:?}"),
    }
}

#[test]
fn context_bar_edits_and_undo() {
    let _s = serial();
    let (_dir, _atlas, project, _rx) = open();
    let context = project.context();
    assert!(context.catalog().iter().any(|b| b.id == "links"));
    // Not set yet: everything uses „Allgemein“.
    assert_eq!(context.profile_for("pdf".into()), "default");
    let before = context.snapshot();
    let layout = context.layout("default".into());
    let first = layout.overview.first().cloned().expect("overview block");

    // Hide for all file types: also in the pdf profile.
    context.set_hidden(ContextTarget::All, first.clone(), true).unwrap();
    assert!(!context.layout("default".into()).overview.contains(&first));
    assert!(!context.layout("pdf".into()).overview.contains(&first));

    // Undo.
    context.restore(before).unwrap();
    assert!(context.layout("default".into()).overview.contains(&first));

    // Pin it in the markdown profile only.
    let target = ContextTarget::Profile { name: "markdown".into() };
    context.move_block(target.clone(), first.clone(), ContextArea::Overview, ContextArea::Pinned).unwrap();
    let md = context.layout("markdown".into());
    let pinnable = context.catalog().iter().find(|b| b.id == first).unwrap().pin;
    assert_eq!(md.pinned.contains(&first), pinnable);
    assert!(context.is_set("markdown".into()));
    assert_eq!(context.profile_for("markdown".into()), "markdown");

    // A page of one's own as a tab, renamed and removed again.
    let id = context.new_page(target.clone(), "Lernen".into()).unwrap();
    assert!(context.layout("markdown".into()).tabs.contains(&id));
    context.rename_page(target.clone(), id.clone(), "Prüfung".into()).unwrap();
    assert!(context.layout("markdown".into()).pages.iter().any(|p| p.id == id && p.title == "Prüfung"));
    context.remove_tab(target, id.clone()).unwrap();
    assert!(!context.layout("markdown".into()).tabs.contains(&id));

    let mut options = context.options();
    options.tab_labels = "icons".into();
    context.set_options(options.clone()).unwrap();
    assert_eq!(context.options(), options);
    options.empty = "kaputt".into();
    assert!(context.set_options(options).is_err());
}

#[test]
fn spelling_when_dictionaries_exist() {
    let _s = serial();
    let (tx, _) = channel();
    let spelling = Atlas::new(Arc::new(Collect(Mutex::new(tx)))).settings().spelling();
    if !spelling.available() {
        eprintln!("no dictionaries here; spelling not tested");
        return;
    }
    let text = "Größe 🙂 Hauss und Garten";
    let bad = spelling.check(text.into());
    let hauss = bad.iter().find(|m| m.word == "Hauss").expect("misspelled");
    assert_eq!((hauss.start, hauss.end), (9, 14), "UTF-16 positions");
    assert!(!spelling.languages().is_empty());
}
