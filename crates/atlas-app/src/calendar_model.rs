// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! `CalendarModel`: the project calendar for QML (see
//! `atlas_core::calendar`). Times cross the bridge as local ISO strings
//! (`2026-10-19T14:30`) in the user's time zone; lists and entries as JSON.

use std::path::Path;
use std::pin::Pin;

use atlas_core::calendar::{self, Entry, Kind, Status, Store, ical::Time, recur};
use chrono::{Duration, NaiveDateTime, Utc};
use chrono_tz::Tz;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use serde_json::{Value, json};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        /// `.atlas` of the project ("" = Atlas' state folder).
        #[qproperty(QString, atlas_dir, cxx_name = "atlasDir", READ, WRITE, NOTIFY)]
        #[qproperty(QString, project_root, cxx_name = "projectRoot", READ, WRITE, NOTIFY)]
        /// Incremented after every change (views reload).
        #[qproperty(i32, revision, READ, NOTIFY)]
        /// Last problem reading or writing the calendar ("" if none).
        #[qproperty(QString, error, READ, NOTIFY)]
        /// IANA name of the time zone used for display.
        #[qproperty(QString, zone, READ, CONSTANT)]
        type CalendarModel = super::CalendarModelRust;

        /// Occurrences in `from..to`: JSON list of {uid, kind, title,
        /// status, start, end, allDay, recurring, instance, tags, files,
        /// session, checklist: [done, total]}.
        #[qinvokable]
        fn occurrences(self: &CalendarModel, from: &QString, to: &QString) -> QString;

        /// Unscheduled tasks (JSON list of {uid, title, status, estimate,
        /// tags, files}).
        #[qinvokable]
        fn tasks(self: &CalendarModel) -> QString;

        /// One entry with all fields (JSON, "" if unknown).
        #[qinvokable]
        fn entry(self: &CalendarModel, uid: &QString) -> QString;

        /// Creates (`uid` empty) or updates an entry from JSON; returns the UID.
        #[qinvokable]
        fn save(self: Pin<&mut CalendarModel>, json: &QString) -> QString;

        #[qinvokable]
        fn remove(self: Pin<&mut CalendarModel>, uid: &QString) -> bool;

        /// Moves an entry (a whole series) so it starts at `start` and ends
        /// at `end`; `all_day` switches between timed and all-day. For a series
        /// `instance` is the moved occurrence's original start.
        #[qinvokable]
        #[cxx_name = "moveTo"]
        fn move_to(
            self: Pin<&mut CalendarModel>,
            uid: &QString,
            instance: &QString,
            start: &QString,
            end: &QString,
            all_day: bool,
        ) -> bool;

        /// Plans a task: it becomes a work block from `start` to `end`.
        #[qinvokable]
        fn schedule(self: Pin<&mut CalendarModel>, uid: &QString, start: &QString, end: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "setStatus"]
        fn set_status(self: Pin<&mut CalendarModel>, uid: &QString, status: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "toggleCheck"]
        fn toggle_check(self: Pin<&mut CalendarModel>, uid: &QString, index: i32) -> bool;

        /// Links a project file (absolute path) with an entry.
        #[qinvokable]
        #[cxx_name = "linkFile"]
        fn link_file(self: Pin<&mut CalendarModel>, uid: &QString, path: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "unlinkFile"]
        fn unlink_file(self: Pin<&mut CalendarModel>, uid: &QString, file: &QString) -> bool;

        /// A work block for a file dropped on a free time; returns the UID.
        #[qinvokable]
        #[cxx_name = "blockForFile"]
        fn block_for_file(self: Pin<&mut CalendarModel>, path: &QString, start: &QString, end: &QString) -> QString;

        /// A task (unscheduled) for a file; returns the UID.
        #[qinvokable]
        #[cxx_name = "taskForFile"]
        fn task_for_file(self: Pin<&mut CalendarModel>, path: &QString) -> QString;

        /// Task list items of a Markdown file: JSON list of {line, done,
        /// text, planned (a task with this text and file exists)}.
        #[qinvokable]
        #[cxx_name = "checklistOf"]
        fn checklist_of(self: &CalendarModel, path: &QString) -> QString;

        /// Creates tasks for chosen items (JSON list of {line, text}) of the
        /// Markdown file `path`; returns how many.
        #[qinvokable]
        #[cxx_name = "addTasks"]
        fn add_tasks(self: Pin<&mut CalendarModel>, path: &QString, items: &QString) -> i32;

        /// Entries matching `query` (JSON list of {uid, title, kind, start}).
        #[qinvokable]
        fn search(self: &CalendarModel, query: &QString, limit: i32) -> QString;

        /// Reminders due since the last call (JSON list of {uid, title, start,
        /// minutes}).
        #[qinvokable]
        #[cxx_name = "dueReminders"]
        fn due_reminders(self: Pin<&mut CalendarModel>) -> QString;

        #[qinvokable]
        #[cxx_name = "viewState"]
        fn view_state(self: &CalendarModel) -> QString;

        #[qinvokable]
        #[cxx_name = "setViewState"]
        fn set_view_state(self: &CalendarModel, json: &QString);

        /// Reads the calendar files again.
        #[qinvokable]
        fn reload(self: Pin<&mut CalendarModel>);
    }

    impl cxx_qt::Initialize for CalendarModel {}
}

pub struct CalendarModelRust {
    atlas_dir: QString,
    project_root: QString,
    revision: i32,
    error: QString,
    zone: QString,

    tz: Tz,
    entries: Vec<Entry>,
    last_reminder_check: chrono::DateTime<Utc>,
}

impl Default for CalendarModelRust {
    fn default() -> Self {
        let tz = calendar::local_zone();
        Self {
            atlas_dir: QString::default(),
            project_root: QString::default(),
            revision: 0,
            error: QString::default(),
            zone: QString::from(tz.name()),
            tz,
            entries: Vec::new(),
            last_reminder_check: Utc::now(),
        }
    }
}

fn qs(s: impl AsRef<str>) -> QString {
    QString::from(s.as_ref())
}

const ISO: &str = "%Y-%m-%dT%H:%M";

fn iso(t: NaiveDateTime) -> String {
    t.format(ISO).to_string()
}

impl cxx_qt::Initialize for qobject::CalendarModel {
    fn initialize(mut self: Pin<&mut Self>) {
        self.as_mut().on_atlas_dir_changed(|m| m.reload()).release();
    }
}

impl qobject::CalendarModel {
    fn store(&self) -> Store {
        let atlas = self.atlas_dir.to_string();
        if atlas.is_empty() {
            Store::new(atlas_core::paths::state_root().join(calendar::DIR))
        } else {
            Store::for_atlas_dir(Path::new(&atlas))
        }
    }

    fn set_error(mut self: Pin<&mut Self>, message: String) {
        if self.error.to_string() != message {
            self.as_mut().rust_mut().error = qs(message);
            self.error_changed();
        }
    }

    fn bump(mut self: Pin<&mut Self>) {
        let r = self.revision.wrapping_add(1);
        self.as_mut().rust_mut().revision = r;
        self.revision_changed();
    }

    pub fn reload(mut self: Pin<&mut Self>) {
        match self.store().load() {
            Ok(entries) => {
                self.as_mut().rust_mut().entries = entries;
                self.as_mut().set_error(String::new());
            }
            Err(err) => {
                log::warn!("calendar: {err:#}");
                self.as_mut().rust_mut().entries = Vec::new();
                self.as_mut().set_error(format!("{err:#}"));
            }
        }
        self.bump();
    }

    /// Writes all entries and their links.
    fn persist(mut self: Pin<&mut Self>) -> bool {
        let store = self.store();
        let result = store.save(&self.entries).and_then(|()| store.save_links(self.entries.iter()));
        let ok = match result {
            Ok(()) => {
                self.as_mut().set_error(String::new());
                true
            }
            Err(err) => {
                self.as_mut().set_error(format!("Kalender kann nicht gespeichert werden: {err:#}"));
                false
            }
        };
        self.bump();
        ok
    }

    fn index_of(&self, uid: &str) -> Option<usize> {
        // The master entry (not an override of a series).
        self.entries.iter().position(|e| e.uid == uid && e.recurrence_id.is_none())
    }

    fn relative(&self, path: &str) -> String {
        let root = self.project_root.to_string();
        match Path::new(path).strip_prefix(&root) {
            Ok(rel) if !root.is_empty() => format!("/{}", rel.to_string_lossy()),
            _ => path.to_owned(),
        }
    }

    fn new_time(&self, local: NaiveDateTime, all_day: bool) -> Time {
        if all_day { Time::Date(local.date()) } else { Time::Zoned(local, self.tz) }
    }

    fn entry_json(&self, e: &Entry) -> Value {
        let tz = self.tz;
        let start = e.start.map(|s| s.local(tz));
        let end = e.end.map(|s| s.local(tz));
        let all_day = e.all_day();
        // All-day ends are exclusive in iCalendar; the UI shows the last day.
        let end_date = match (all_day, end, start) {
            (true, Some(end), _) => Some((end - Duration::days(1)).date()),
            (true, None, Some(start)) => Some(start.date()),
            _ => None,
        };
        let rule = e.rrule.as_deref().and_then(recur::Rule::parse);
        json!({
            "uid": e.uid,
            "kind": e.kind.as_str(),
            "title": e.title,
            "description": e.description,
            "start": start.map(iso),
            "end": end.map(iso),
            "endDate": end_date.map(|d| d.format("%Y-%m-%d").to_string()),
            "allDay": all_day,
            "status": e.status.as_str(),
            "tags": e.tags,
            "checklist": e.checklist.iter().map(|(d, t)| json!({ "done": d, "text": t })).collect::<Vec<_>>(),
            "files": e.files,
            "session": e.session,
            "estimate": e.estimate_minutes,
            "reminders": e.reminders,
            "rrule": rule.as_ref().map(|r| json!({
                "freq": r.freq.as_str().to_lowercase(),
                "interval": r.interval,
                "count": r.count,
                "until": r.until.map(|u| u.format("%Y-%m-%d").to_string()),
                "text": r.describe(),
                "supported": r.supported,
            })),
            "calendar": calendar::LOCAL,
        })
    }

    pub fn occurrences(&self, from: &QString, to: &QString) -> QString {
        let (Some(from), Some(to)) = (calendar::parse_local(&from.to_string()), calendar::parse_local(&to.to_string())) else {
            return qs("[]");
        };
        let list: Vec<Value> = calendar::occurrences(&self.entries, self.tz, from, to)
            .into_iter()
            .map(|o| {
                let e = self.entries.iter().find(|e| e.uid == o.uid && e.recurrence_id.is_none());
                let done = e.map(|e| e.checklist.iter().filter(|c| c.0).count()).unwrap_or(0);
                json!({
                    "uid": o.uid,
                    "kind": o.kind.as_str(),
                    "title": o.title,
                    "status": o.status.as_str(),
                    "start": iso(o.start),
                    "end": iso(o.end),
                    "allDay": o.all_day,
                    "recurring": o.recurring,
                    "instance": o.instance.map(iso),
                    "tags": e.map(|e| e.tags.clone()).unwrap_or_default(),
                    "files": e.map(|e| e.files.clone()).unwrap_or_default(),
                    "session": e.map(|e| e.session.clone()).unwrap_or_default(),
                    "checklist": [done, e.map(|e| e.checklist.len()).unwrap_or(0)],
                })
            })
            .collect();
        qs(Value::from(list).to_string())
    }

    pub fn tasks(&self) -> QString {
        let list: Vec<Value> = self
            .entries
            .iter()
            .filter(|e| e.kind == Kind::Task && e.start.is_none())
            .map(|e| {
                json!({
                    "uid": e.uid, "title": e.title, "status": e.status.as_str(), "estimate": e.estimate_minutes,
                    "tags": e.tags, "files": e.files,
                })
            })
            .collect();
        qs(Value::from(list).to_string())
    }

    pub fn entry(&self, uid: &QString) -> QString {
        self.index_of(&uid.to_string()).map(|i| qs(self.entry_json(&self.entries[i]).to_string())).unwrap_or_default()
    }

    pub fn save(mut self: Pin<&mut Self>, json: &QString) -> QString {
        let Ok(v) = serde_json::from_str::<Value>(&json.to_string()) else { return QString::default() };
        let tz = self.tz;
        let uid = v["uid"].as_str().unwrap_or_default().to_owned();
        let kind = v["kind"].as_str().and_then(Kind::parse).unwrap_or(Kind::Event);
        let mut entry = match self.index_of(&uid) {
            Some(i) => self.entries[i].clone(),
            None => Entry::new(kind, ""),
        };
        entry.kind = kind;
        entry.title = v["title"].as_str().unwrap_or_default().trim().to_owned();
        if entry.title.is_empty() {
            entry.title = "Ohne Titel".into();
        }
        entry.description = v["description"].as_str().unwrap_or_default().to_owned();
        if let Some(s) = v["status"].as_str().and_then(Status::parse) {
            entry.status = s;
        }
        let strings = |key: &str| -> Vec<String> {
            v[key].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default()
        };
        entry.tags = strings("tags").into_iter().map(|t| t.trim().trim_start_matches('#').to_owned()).filter(|t| !t.is_empty()).collect();
        entry.files = strings("files");
        entry.session = v["session"].as_str().unwrap_or_default().to_owned();
        entry.checklist = v["checklist"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|c| Some((c["done"].as_bool().unwrap_or(false), c["text"].as_str()?.trim().to_owned())))
                    .filter(|c| !c.1.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        entry.estimate_minutes = v["estimate"].as_u64().map(|m| m as u32).or_else(|| v["estimate"].as_str().and_then(calendar::parse_estimate));
        entry.reminders = v["reminders"].as_array().map(|a| a.iter().filter_map(Value::as_i64).collect()).unwrap_or_default();

        // Times: tasks have none; all-day entries use dates.
        let all_day = v["allDay"].as_bool().unwrap_or(false);
        let start = v["start"].as_str().and_then(calendar::parse_local);
        if kind == Kind::Task {
            entry.start = None;
            entry.end = None;
        } else if let Some(start) = start {
            let end = if all_day {
                v["endDate"].as_str().and_then(calendar::parse_local).map(|d| d + Duration::days(1)).filter(|e| *e > start).unwrap_or(start + Duration::days(1))
            } else {
                v["end"].as_str().and_then(calendar::parse_local).filter(|e| *e >= start).unwrap_or(start + Duration::hours(1))
            };
            // Keep the zone kind of existing entries (UTC from a server stays UTC).
            entry.start = Some(match (entry.start, all_day) {
                (Some(old), false) if !old.is_date() => old.with_local(start, tz),
                _ => self.new_time(start, all_day),
            });
            entry.end = if kind == Kind::Deadline && !all_day {
                None
            } else {
                Some(match (entry.end, all_day) {
                    (Some(old), false) if !old.is_date() => old.with_local(end, tz),
                    _ => self.new_time(end, all_day),
                })
            };
        }
        // Recurrence: only replaced when the dialog changed it, so rules
        // Atlas cannot fully edit stay as they are.
        if v["rruleChanged"].as_bool().unwrap_or(false) {
            entry.rrule = match v["rrule"]["freq"].as_str() {
                Some(freq) if freq != "none" && entry.start.is_some() => {
                    let freq = match freq {
                        "daily" => recur::Freq::Daily,
                        "weekly" => recur::Freq::Weekly,
                        "monthly" => recur::Freq::Monthly,
                        _ => recur::Freq::Yearly,
                    };
                    let mut rule = recur::Rule::new(freq);
                    rule.interval = v["rrule"]["interval"].as_u64().unwrap_or(1).max(1) as u32;
                    rule.count = v["rrule"]["count"].as_u64().filter(|c| *c > 0).map(|c| c as u32);
                    rule.until = v["rrule"]["until"].as_str().and_then(calendar::parse_local).map(|d| d + Duration::hours(23) + Duration::minutes(59));
                    Some(rule.to_value())
                }
                _ => None,
            };
            entry.exdates.clear();
        }
        entry.sequence += 1;
        let uid = entry.uid.clone();
        match self.index_of(&uid) {
            Some(i) => self.as_mut().rust_mut().entries[i] = entry,
            None => self.as_mut().rust_mut().entries.push(entry),
        }
        self.persist();
        qs(uid)
    }

    pub fn remove(mut self: Pin<&mut Self>, uid: &QString) -> bool {
        let uid = uid.to_string();
        let before = self.entries.len();
        self.as_mut().rust_mut().entries.retain(|e| e.uid != uid);
        if self.entries.len() == before {
            return false;
        }
        let _ = self.store().forget_links(&uid);
        self.persist()
    }

    pub fn move_to(
        mut self: Pin<&mut Self>,
        uid: &QString,
        instance: &QString,
        start: &QString,
        end: &QString,
        all_day: bool,
    ) -> bool {
        let tz = self.tz;
        let Some(i) = self.index_of(&uid.to_string()) else { return false };
        let (Some(start), Some(end)) = (calendar::parse_local(&start.to_string()), calendar::parse_local(&end.to_string())) else {
            return false;
        };
        let mut e = self.entries[i].clone();
        let Some(old_start) = e.start else { return false };
        // A series moves as a whole: by the distance the occurrence moved.
        let reference = calendar::parse_local(&instance.to_string()).unwrap_or_else(|| old_start.local(tz));
        let delta = start - reference;
        let length = (end - start).max(Duration::minutes(if all_day { 24 * 60 } else { 15 }));
        let new_start_local = old_start.local(tz) + delta;
        if all_day {
            e.start = Some(Time::Date(new_start_local.date()));
            e.end = Some(Time::Date(new_start_local.date() + Duration::days(length.num_days().max(1))));
        } else {
            e.start = Some(if old_start.is_date() { self.new_time(new_start_local, false) } else { old_start.with_local(new_start_local, tz) });
            e.end = match (e.kind, e.end) {
                (Kind::Deadline, None) => None,
                (_, Some(old)) if !old.is_date() => Some(old.with_local(new_start_local + length, tz)),
                _ => Some(self.new_time(new_start_local + length, false)),
            };
        }
        e.sequence += 1;
        self.as_mut().rust_mut().entries[i] = e;
        self.persist()
    }

    pub fn schedule(mut self: Pin<&mut Self>, uid: &QString, start: &QString, end: &QString) -> bool {
        let Some(i) = self.index_of(&uid.to_string()) else { return false };
        let Some(start) = calendar::parse_local(&start.to_string()) else { return false };
        let mut e = self.entries[i].clone();
        let length = e.estimate_minutes.map(|m| Duration::minutes(m as i64)).unwrap_or(Duration::hours(1));
        let end = calendar::parse_local(&end.to_string()).filter(|e| *e > start).unwrap_or(start + length);
        e.kind = Kind::Block;
        e.start = Some(self.new_time(start, false));
        e.end = Some(self.new_time(end, false));
        e.sequence += 1;
        self.as_mut().rust_mut().entries[i] = e;
        self.persist()
    }

    fn modify(mut self: Pin<&mut Self>, uid: &QString, f: impl FnOnce(&mut Entry) -> bool) -> bool {
        let Some(i) = self.index_of(&uid.to_string()) else { return false };
        let mut e = self.entries[i].clone();
        if !f(&mut e) {
            return false;
        }
        e.sequence += 1;
        self.as_mut().rust_mut().entries[i] = e;
        self.persist()
    }

    pub fn set_status(self: Pin<&mut Self>, uid: &QString, status: &QString) -> bool {
        let Some(status) = Status::parse(&status.to_string()) else { return false };
        self.modify(uid, |e| {
            e.status = status;
            true
        })
    }

    pub fn toggle_check(self: Pin<&mut Self>, uid: &QString, index: i32) -> bool {
        self.modify(uid, |e| match e.checklist.get_mut(index.max(0) as usize) {
            Some(item) => {
                item.0 = !item.0;
                true
            }
            None => false,
        })
    }

    pub fn link_file(self: Pin<&mut Self>, uid: &QString, path: &QString) -> bool {
        let file = self.relative(&path.to_string());
        self.modify(uid, |e| {
            if e.files.contains(&file) {
                return false;
            }
            e.files.push(file);
            true
        })
    }

    pub fn unlink_file(self: Pin<&mut Self>, uid: &QString, file: &QString) -> bool {
        let file = file.to_string();
        self.modify(uid, |e| {
            let before = e.files.len();
            e.files.retain(|f| f != &file);
            e.files.len() != before
        })
    }

    fn title_for(path: &str) -> String {
        let p = Path::new(path);
        p.file_stem().or(p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_owned())
    }

    pub fn block_for_file(mut self: Pin<&mut Self>, path: &QString, start: &QString, end: &QString) -> QString {
        let path = path.to_string();
        let Some(start) = calendar::parse_local(&start.to_string()) else { return QString::default() };
        let end = calendar::parse_local(&end.to_string()).filter(|e| *e > start).unwrap_or(start + Duration::hours(1));
        let mut e = Entry::new(Kind::Block, &Self::title_for(&path));
        e.start = Some(self.new_time(start, false));
        e.end = Some(self.new_time(end, false));
        e.files = vec![self.relative(&path)];
        let uid = e.uid.clone();
        self.as_mut().rust_mut().entries.push(e);
        self.persist();
        qs(uid)
    }

    pub fn task_for_file(mut self: Pin<&mut Self>, path: &QString) -> QString {
        let path = path.to_string();
        let mut e = Entry::new(Kind::Task, &Self::title_for(&path));
        e.files = vec![self.relative(&path)];
        let uid = e.uid.clone();
        self.as_mut().rust_mut().entries.push(e);
        self.persist();
        qs(uid)
    }

    pub fn checklist_of(&self, path: &QString) -> QString {
        let path = path.to_string();
        let file = self.relative(&path);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let list: Vec<Value> = calendar::checklist_items(&text)
            .into_iter()
            .map(|(line, done, item)| {
                let planned = self.entries.iter().any(|e| e.title == item && e.files.iter().any(|f| f == &file));
                json!({ "line": line, "done": done, "text": item, "planned": planned })
            })
            .collect();
        qs(Value::from(list).to_string())
    }

    pub fn add_tasks(mut self: Pin<&mut Self>, path: &QString, items: &QString) -> i32 {
        let file = self.relative(&path.to_string());
        let Ok(Value::Array(items)) = serde_json::from_str(&items.to_string()) else { return 0 };
        let mut count = 0;
        for item in items {
            let Some(text) = item["text"].as_str() else { continue };
            let mut e = Entry::new(Kind::Task, text);
            e.files = vec![file.clone()];
            if let Some(line) = item["line"].as_u64() {
                e.description = format!("Aus {file}, Zeile {line}");
            }
            self.as_mut().rust_mut().entries.push(e);
            count += 1;
        }
        if count > 0 {
            self.persist();
        }
        count
    }

    pub fn search(&self, query: &QString, limit: i32) -> QString {
        let query = query.to_string();
        let tz = self.tz;
        let mut found: Vec<(i32, Value)> = self
            .entries
            .iter()
            .filter(|e| e.recurrence_id.is_none())
            .filter_map(|e| {
                let hay = format!("{} {} {}", e.title, e.tags.join(" "), e.description);
                let score = atlas_core::finder::score(query.trim(), &hay)?;
                Some((
                    score,
                    json!({ "uid": e.uid, "title": e.title, "kind": e.kind.as_str(), "start": e.start.map(|s| iso(s.local(tz))) }),
                ))
            })
            .collect();
        found.sort_by_key(|f| std::cmp::Reverse(f.0));
        found.truncate(limit.max(1) as usize);
        qs(Value::from(found.into_iter().map(|f| f.1).collect::<Vec<_>>()).to_string())
    }

    pub fn due_reminders(mut self: Pin<&mut Self>) -> QString {
        let now = Utc::now();
        let from = self.last_reminder_check;
        self.as_mut().rust_mut().last_reminder_check = now;
        let list: Vec<Value> = calendar::due_reminders(&self.entries, self.tz, from, now)
            .into_iter()
            .map(|(uid, title, start, minutes)| json!({ "uid": uid, "title": title, "start": iso(start), "minutes": minutes }))
            .collect();
        qs(Value::from(list).to_string())
    }

    pub fn view_state(&self) -> QString {
        qs(self.store().state().to_string())
    }

    pub fn set_view_state(&self, json: &QString) {
        if let Ok(v) = serde_json::from_str::<Value>(&json.to_string())
            && let Err(err) = self.store().save_state(&v)
        {
            log::warn!("calendar state: {err:#}");
        }
    }
}
