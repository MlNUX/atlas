// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! P2.5: the project calendar through the facade (offline: entries, tasks,
//! series, links, calendars, settings), against a temporary project and
//! private state/config folders (own test binary).

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use atlas_api::AtlasError;
use atlas_api::app::Atlas;
use atlas_api::calendar::{Calendar, ChecklistItem, Entry, EntryKind, EntryStatus, Frequency, Provider, Recurrence};
use atlas_api::events::{Event, EventSink};
use atlas_api::project::Project;

struct Collect(Mutex<Sender<Event>>);

impl EventSink for Collect {
    fn on_event(&self, event: Event) {
        let _ = self.0.lock().unwrap().send(event);
    }
}

/// Tests share the config file: one at a time, with private state/config folders.
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
    cal: Arc<Calendar>,
    events: Receiver<Event>,
}

impl Fixture {
    fn path(&self, rel: &str) -> String {
        self.dir.path().canonicalize().unwrap().join(rel).to_string_lossy().into_owned()
    }

    fn changes(&self) -> usize {
        self.events.try_iter().filter(|e| matches!(e, Event::CalendarChanged { .. })).count()
    }
}

fn fixture() -> Fixture {
    let serial = serial();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("plan.md"),
        "# Plan\n\n- [ ] Gliederung\n- [x] Quellen\n```\n- [ ] kein Punkt\n```\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("kapitel")).unwrap();
    std::fs::write(dir.path().join("kapitel/eins.md"), "Text\n").unwrap();
    let (tx, rx) = channel();
    let sink: Arc<dyn EventSink> = Arc::new(Collect(Mutex::new(tx)));
    let atlas = Atlas::new(sink.clone());
    let project = atlas.open_project(dir.path().to_string_lossy().into_owned(), sink).unwrap();
    let cal = project.calendar();
    Fixture { _serial: serial, dir, _atlas: atlas, project, cal, events: rx }
}

fn event(title: &str, start: &str, end: &str) -> Entry {
    Entry {
        uid: String::new(),
        kind: EntryKind::Event,
        title: title.into(),
        description: String::new(),
        start: Some(start.into()),
        end: Some(end.into()),
        status: EntryStatus::Planned,
        tags: Vec::new(),
        checklist: Vec::new(),
        files: Vec::new(),
        session: String::new(),
        estimate_minutes: None,
        reminders: Vec::new(),
        recurrence: None,
        calendar: String::new(),
        read_only: false,
        pending: false,
    }
}

#[test]
fn one_calendar_per_project() {
    let f = fixture();
    assert!(Arc::ptr_eq(&f.cal, &f.project.calendar()));
    assert!(!f.cal.time_zone().is_empty());
}

#[test]
fn save_read_and_remove_events() {
    let f = fixture();
    let mut e = event("  Seminar ", "2026-11-04T09:30", "2026-11-04T11:00");
    e.tags = vec!["#uni".into(), " ".into()];
    e.checklist =
        vec![ChecklistItem { done: false, text: "Folien".into() }, ChecklistItem { done: true, text: " ".into() }];
    e.reminders = vec![15];
    let uid = f.cal.save(e).unwrap();
    assert!(f.changes() > 0);

    let saved = f.cal.entry(uid.clone()).unwrap();
    assert_eq!(saved.title, "Seminar");
    assert_eq!(saved.tags, vec!["uni"]);
    assert_eq!(saved.checklist.len(), 1);
    assert_eq!(saved.start.as_deref(), Some("2026-11-04T09:30"));
    assert_eq!(saved.end.as_deref(), Some("2026-11-04T11:00"));
    assert_eq!(saved.calendar, "local");
    assert!(!saved.read_only);
    let ics = std::fs::read_to_string(f.path(".atlas/calendar/events.ics")).unwrap();
    assert!(ics.contains("SUMMARY:Seminar"), "{ics}");

    // All day: dates, the end exclusive.
    let day = f.cal.save(event("Ausflug", "2026-11-06", "2026-11-08")).unwrap();
    let day_entry = f.cal.entry(day.clone()).unwrap();
    assert_eq!(day_entry.end.as_deref(), Some("2026-11-08"));

    let week = f.cal.occurrences("2026-11-02T00:00".into(), "2026-11-09T00:00".into()).unwrap();
    assert_eq!(week.len(), 2);
    assert_eq!(week[0].uid, uid);
    assert_eq!(week[0].checklist_total, 1);
    assert!(week[1].all_day);
    assert_eq!(week[1].start, "2026-11-06");
    assert_eq!(week[1].end, "2026-11-08");

    // Edit keeps the uid; an empty title becomes „Ohne Titel“.
    let mut edit = saved.clone();
    edit.title = String::new();
    edit.start = Some("2026-11-05T14:00".into());
    edit.end = None;
    assert_eq!(f.cal.save(edit).unwrap(), uid);
    let edited = f.cal.entry(uid.clone()).unwrap();
    assert_eq!(edited.title, "Ohne Titel");
    assert_eq!(edited.end.as_deref(), Some("2026-11-05T15:00"));

    f.cal.set_status(uid.clone(), EntryStatus::Done).unwrap();
    f.cal.toggle_check(uid.clone(), 0).unwrap();
    let done = f.cal.entry(uid.clone()).unwrap();
    assert_eq!(done.status, EntryStatus::Done);
    assert!(done.checklist[0].done);
    assert!(matches!(f.cal.toggle_check(uid.clone(), 5), Err(AtlasError::Invalid { .. })));

    f.cal.remove(uid.clone()).unwrap();
    assert!(f.cal.entry(uid.clone()).is_none());
    assert!(matches!(f.cal.remove(uid), Err(AtlasError::NotFound { .. })));
    assert!(matches!(f.cal.occurrences("gestern".into(), "morgen".into()), Err(AtlasError::Invalid { .. })));
}

#[test]
fn tasks_get_scheduled_and_back() {
    let f = fixture();
    let mut t = event("Kapitel 2 schreiben", "2026-11-04T09:00", "2026-11-04T10:00");
    t.kind = EntryKind::Task;
    t.estimate_minutes = Some(90);
    let uid = f.cal.save(t).unwrap();
    let task = f.cal.entry(uid.clone()).unwrap();
    assert_eq!(task.start, None, "tasks have no time");
    assert_eq!(f.cal.tasks().iter().map(|t| t.uid.clone()).collect::<Vec<_>>(), vec![uid.clone()]);

    f.cal.schedule(uid.clone(), Some("2026-11-10T08:00".into()), None).unwrap();
    let block = f.cal.entry(uid.clone()).unwrap();
    assert_eq!(block.kind, EntryKind::Block);
    assert_eq!(block.end.as_deref(), Some("2026-11-10T09:30"), "length from the estimate");
    assert!(f.cal.tasks().is_empty());

    f.cal
        .move_occurrence(
            uid.clone(),
            "2026-11-10T08:00".into(),
            "2026-11-11T13:00".into(),
            "2026-11-11T14:30".into(),
            false,
        )
        .unwrap();
    assert_eq!(f.cal.entry(uid.clone()).unwrap().start.as_deref(), Some("2026-11-11T13:00"));

    f.cal.schedule(uid.clone(), None, None).unwrap();
    let back = f.cal.entry(uid.clone()).unwrap();
    assert_eq!(back.kind, EntryKind::Task);
    assert_eq!(back.estimate_minutes, Some(90));
    assert_eq!(f.cal.tasks().len(), 1);

    // A deadline is a point in time.
    let mut d = event("Abgabe", "2026-11-20T12:00", "2026-11-20T13:00");
    d.kind = EntryKind::Deadline;
    let deadline = f.cal.save(d).unwrap();
    assert_eq!(f.cal.entry(deadline).unwrap().end, None);
}

#[test]
fn series_expand_move_and_keep_their_rule() {
    let f = fixture();
    let mut e = event("Sprechstunde", "2026-11-02T10:00", "2026-11-02T11:00");
    e.recurrence = Some(Recurrence {
        frequency: Frequency::Weekly,
        interval: 1,
        count: Some(4),
        until: None,
        text: String::new(),
        supported: true,
    });
    let uid = f.cal.save(e).unwrap();
    let all = f.cal.occurrences("2026-11-01T00:00".into(), "2026-12-31T00:00".into()).unwrap();
    assert_eq!(all.len(), 4);
    assert!(all.iter().all(|o| o.recurring));
    assert_eq!(all[1].instance, "2026-11-09T10:00");

    // Moving the second occurrence by a day moves the series.
    f.cal
        .move_occurrence(
            uid.clone(),
            all[1].instance.clone(),
            "2026-11-10T10:00".into(),
            "2026-11-10T11:00".into(),
            false,
        )
        .unwrap();
    let saved = f.cal.entry(uid.clone()).unwrap();
    assert_eq!(saved.start.as_deref(), Some("2026-11-03T10:00"));
    let rule = saved.recurrence.clone().unwrap();
    assert_eq!((rule.frequency, rule.count), (Frequency::Weekly, Some(4)));
    assert!(!rule.text.is_empty());

    // A rule Atlas cannot edit stays as long as the dialog does not change it.
    let ics = f.path(".atlas/calendar/events.ics");
    let text = std::fs::read_to_string(&ics).unwrap();
    let unusual = text.replace("RRULE:FREQ=WEEKLY;COUNT=4", "RRULE:FREQ=WEEKLY;COUNT=4;BYSETPOS=1;BYDAY=TU,WE");
    assert_ne!(text, unusual, "{text}");
    std::fs::write(&ics, unusual).unwrap();
    f.cal.reload();
    let mut kept = f.cal.entry(uid.clone()).unwrap();
    assert!(!kept.recurrence.as_ref().unwrap().supported);
    kept.title = "Sprechstunde (neu)".into();
    f.cal.save(kept.clone()).unwrap();
    assert!(std::fs::read_to_string(&ics).unwrap().contains("BYSETPOS=1"));
    kept.recurrence = None;
    f.cal.save(kept).unwrap();
    assert!(f.cal.entry(uid).unwrap().recurrence.is_none());
}

#[test]
fn files_tasks_from_notes_and_search() {
    let f = fixture();
    let plan = f.path("plan.md");
    let task = f.cal.task_for_file(plan.clone()).unwrap();
    let entry = f.cal.entry(task.clone()).unwrap();
    assert_eq!(entry.title, "plan");
    assert_eq!(entry.files, vec!["/plan.md"]);

    let block = f.cal.block_for_file(f.path("kapitel/eins.md"), "2026-11-04T14:00".into(), "".into()).unwrap();
    assert_eq!(f.cal.entry(block.clone()).unwrap().end.as_deref(), Some("2026-11-04T15:00"));
    f.cal.link_file(block.clone(), plan.clone()).unwrap();
    f.cal.link_file(block.clone(), plan.clone()).unwrap();
    assert_eq!(f.cal.entry(block.clone()).unwrap().files, vec!["/kapitel/eins.md", "/plan.md"]);
    f.cal.unlink_file(block.clone(), "/plan.md".into()).unwrap();

    f.cal.files_moved(f.path("kapitel"), f.path("teil"));
    assert_eq!(f.cal.entry(block).unwrap().files, vec!["/teil/eins.md"]);
    let links = std::fs::read_to_string(f.path(".atlas/calendar/links.json")).unwrap();
    assert!(links.contains("/teil/eins.md"), "{links}");

    let items = f.cal.checklist_of(plan.clone());
    assert_eq!(
        items.iter().map(|i| (i.line, i.done, i.text.as_str())).collect::<Vec<_>>(),
        vec![(3, false, "Gliederung"), (4, true, "Quellen")]
    );
    assert!(!items[0].planned);
    assert_eq!(f.cal.add_tasks(plan.clone(), vec![items[0].clone()]).unwrap(), 1);
    assert!(f.cal.checklist_of(plan).iter().any(|i| i.text == "Gliederung" && i.planned));
    let hits = f.cal.search("glied".into(), 5);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, EntryKind::Task);
    assert!(f.cal.entry(hits[0].uid.clone()).unwrap().description.contains("Zeile 3"));
}

#[test]
fn calendars_settings_and_account_offline() {
    let f = fixture();
    let list = f.cal.calendars();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "local");
    assert!(list[0].is_default && list[0].writable);
    assert_eq!(f.cal.default_calendar(), "local");

    f.cal.set_default_calendar("local".into(), true).unwrap();
    assert!(f.cal.calendars()[0].project_default);
    assert!(std::fs::read_to_string(f.path(".atlas/calendar/settings.json")).unwrap().contains("default_calendar"));
    assert!(matches!(f.cal.set_calendar_visible("https://nirgends/".into(), true), Err(AtlasError::NotFound { .. })));

    f.cal.set_view_state(r#"{"view":"week","date":"2026-11-04"}"#.into());
    assert!(f.cal.view_state().contains("week"));

    let state = f.cal.sync_state();
    assert!(!state.connected && !state.syncing && state.status.is_empty());
    assert_eq!(f.cal.account().provider, Provider::None);
    assert!(f.cal.conflicts().is_empty());
    f.cal.sync_now(); // not connected: nothing happens
    assert!(f.cal.due_reminders().is_empty());
    assert!(matches!(f.cal.google_sign_in(String::new(), String::new()), Err(AtlasError::Invalid { .. })));
    f.cal.disconnect().unwrap();
}
