// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Changing entries: new ones, edits, moving and scheduling, checklists and linked files.

use super::*;

impl qobject::CalendarModel {
    pub fn save(mut self: Pin<&mut Self>, json: &QString) -> QString {
        let Ok(v) = serde_json::from_str::<Value>(&json.to_string()) else { return QString::default() };
        let tz = self.tz;
        let uid = v["uid"].as_str().unwrap_or_default().to_owned();
        let kind = v["kind"].as_str().and_then(Kind::parse).unwrap_or(Kind::Event);
        let existing = self.index_of(&uid);
        let mut entry = match existing {
            Some(i) => self.entries[i].clone(),
            None => {
                let mut e = Entry::new(kind, "");
                e.calendar = self.default_calendar().to_string();
                e
            }
        };
        let old_calendar = entry.calendar.clone();
        if let Some(cal) = v["calendar"].as_str().filter(|c| !c.is_empty()) {
            entry.calendar = cal.to_owned();
        }
        if !self.writable(&old_calendar) || !self.writable(&entry.calendar) {
            self.as_mut().set_error("Der Kalender ist schreibgeschützt.".into());
            return QString::default();
        }
        entry.kind = kind;
        // Google's calendars hold events only: tasks stay in the project.
        if kind == Kind::Task && self.config.is_google() && entry.calendar != LOCAL {
            entry.calendar = LOCAL.to_owned();
        }
        entry.title = v["title"].as_str().unwrap_or_default().trim().to_owned();
        if entry.title.is_empty() {
            entry.title = "Ohne Titel".into();
        }
        entry.description = v["description"].as_str().unwrap_or_default().to_owned();
        if let Some(s) = v["status"].as_str().and_then(Status::parse) {
            entry.status = s;
        }
        let strings = |key: &str| -> Vec<String> {
            v[key]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
                .unwrap_or_default()
        };
        if v.get("tags").is_some() {
            entry.tags = strings("tags")
                .into_iter()
                .map(|t| t.trim().trim_start_matches('#').to_owned())
                .filter(|t| !t.is_empty())
                .collect();
        }
        if v.get("files").is_some() {
            entry.files = strings("files");
        }
        if v.get("session").is_some() {
            entry.session = v["session"].as_str().unwrap_or_default().to_owned();
        }
        if v.get("checklist").is_some() {
            entry.checklist = v["checklist"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|c| {
                            Some((c["done"].as_bool().unwrap_or(false), c["text"].as_str()?.trim().to_owned()))
                        })
                        .filter(|c| !c.1.is_empty())
                        .collect()
                })
                .unwrap_or_default();
        }
        if v.get("estimate").is_some() {
            entry.estimate_minutes = v["estimate"]
                .as_u64()
                .map(|m| m as u32)
                .or_else(|| v["estimate"].as_str().and_then(calendar::parse_estimate));
        }
        if v.get("reminders").is_some() {
            entry.reminders =
                v["reminders"].as_array().map(|a| a.iter().filter_map(Value::as_i64).collect()).unwrap_or_default();
        }

        // Times: tasks have none; all-day entries use dates.
        let all_day = v["allDay"].as_bool().unwrap_or(false);
        let start = v["start"].as_str().and_then(calendar::parse_local);
        if kind == Kind::Task {
            entry.start = None;
            entry.end = None;
        } else if let Some(start) = start {
            let end = if all_day {
                v["endDate"]
                    .as_str()
                    .and_then(calendar::parse_local)
                    .map(|d| d + Duration::days(1))
                    .filter(|e| *e > start)
                    .unwrap_or(start + Duration::days(1))
            } else {
                v["end"]
                    .as_str()
                    .and_then(calendar::parse_local)
                    .filter(|e| *e >= start)
                    .unwrap_or(start + Duration::hours(1))
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
                    rule.until = v["rrule"]["until"]
                        .as_str()
                        .and_then(calendar::parse_local)
                        .map(|d| d + Duration::hours(23) + Duration::minutes(59));
                    Some(rule.to_value())
                }
                _ => None,
            };
            entry.exdates.clear();
            // Changed occurrences of the old rule no longer fit.
            let uid = entry.uid.clone();
            self.as_mut().rust_mut().entries.retain(|e| !(e.uid == uid && e.recurrence_id.is_some()));
        }
        entry.sequence += 1;
        let uid = entry.uid.clone();
        let calendar = entry.calendar.clone();
        // Overrides of a series move with it to another calendar.
        for e in self.as_mut().rust_mut().entries.iter_mut().filter(|e| e.uid == uid && e.recurrence_id.is_some()) {
            e.calendar = calendar.clone();
        }
        match self.index_of(&uid) {
            Some(i) => self.as_mut().rust_mut().entries[i] = entry,
            None => self.as_mut().rust_mut().entries.push(entry),
        }
        if old_calendar != calendar && existing.is_some() {
            if old_calendar == LOCAL {
                self.as_mut().commit(LOCAL);
            } else {
                // Removed from the old Nextcloud calendar.
                let _ = self.sync_store().update(|s| s.local_delete(&old_calendar, &uid));
            }
        }
        self.store_entry(&calendar, &uid);
        qs(uid)
    }

    pub fn remove(mut self: Pin<&mut Self>, uid: &QString) -> bool {
        let uid = uid.to_string();
        let Some(i) = self.index_of(&uid) else { return false };
        let calendar = self.entries[i].calendar.clone();
        if !self.writable(&calendar) {
            self.as_mut().set_error("Der Kalender ist schreibgeschützt.".into());
            return false;
        }
        self.as_mut().rust_mut().entries.retain(|e| e.uid != uid);
        let _ = self.store().forget_links(&uid);
        self.store_entry(&calendar, &uid)
    }

    /// Changes the entry `uid` with `f` (false: nothing changed) and stores it.
    fn modify(mut self: Pin<&mut Self>, uid: &str, f: impl FnOnce(&mut Entry) -> bool) -> bool {
        let Some(i) = self.index_of(uid) else { return false };
        let calendar = self.entries[i].calendar.clone();
        if !self.writable(&calendar) {
            self.as_mut().set_error("Der Kalender ist schreibgeschützt.".into());
            return false;
        }
        let mut e = self.entries[i].clone();
        if !f(&mut e) {
            return false;
        }
        e.sequence += 1;
        self.as_mut().rust_mut().entries[i] = e;
        self.store_entry(&calendar, uid)
    }

    pub fn move_to(
        self: Pin<&mut Self>,
        uid: &QString,
        instance: &QString,
        start: &QString,
        end: &QString,
        all_day: bool,
    ) -> bool {
        let tz = self.tz;
        let (Some(start), Some(end)) =
            (calendar::parse_local(&start.to_string()), calendar::parse_local(&end.to_string()))
        else {
            return false;
        };
        let instance = calendar::parse_local(&instance.to_string());
        let zone = self.tz;
        self.modify(&uid.to_string(), |e| {
            let Some(old_start) = e.start else { return false };
            // A series moves as a whole: by the distance the occurrence moved.
            let reference = instance.unwrap_or_else(|| old_start.local(tz));
            let delta = start - reference;
            let length = (end - start).max(Duration::minutes(if all_day { 24 * 60 } else { 15 }));
            let new_start_local = old_start.local(tz) + delta;
            let new_time = |local: NaiveDateTime| Time::Zoned(local, zone);
            if all_day {
                e.start = Some(Time::Date(new_start_local.date()));
                e.end = Some(Time::Date(new_start_local.date() + Duration::days(length.num_days().max(1))));
            } else {
                e.start = Some(if old_start.is_date() {
                    new_time(new_start_local)
                } else {
                    old_start.with_local(new_start_local, tz)
                });
                e.end = match (e.kind, e.end) {
                    (Kind::Deadline, None) if !old_start.is_date() => None,
                    (_, Some(old)) if !old.is_date() => Some(old.with_local(new_start_local + length, tz)),
                    _ => Some(new_time(new_start_local + length)),
                };
            }
            true
        })
    }

    pub fn schedule(self: Pin<&mut Self>, uid: &QString, start: &QString, end: &QString) -> bool {
        let Some(start) = calendar::parse_local(&start.to_string()) else { return false };
        let end = calendar::parse_local(&end.to_string());
        let zone = self.tz;
        self.modify(&uid.to_string(), |e| {
            let length = e.estimate_minutes.map(|m| Duration::minutes(m as i64)).unwrap_or(Duration::hours(1));
            let end = end.filter(|e| *e > start).unwrap_or(start + length);
            e.kind = Kind::Block;
            e.start = Some(Time::Zoned(start, zone));
            e.end = Some(Time::Zoned(end, zone));
            true
        })
    }

    pub fn unschedule(self: Pin<&mut Self>, uid: &QString) -> bool {
        self.modify(&uid.to_string(), |e| {
            if e.start.is_none() || e.rrule.is_some() {
                return false;
            }
            if e.estimate_minutes.is_none()
                && let (Some(s), Some(end)) = (e.start, e.end)
                && !s.is_date()
            {
                let tz = calendar::local_zone();
                e.estimate_minutes = Some((end.local(tz) - s.local(tz)).num_minutes().max(0) as u32);
            }
            e.kind = Kind::Task;
            e.start = None;
            e.end = None;
            true
        })
    }

    pub fn set_status(self: Pin<&mut Self>, uid: &QString, status: &QString) -> bool {
        let Some(status) = Status::parse(&status.to_string()) else { return false };
        self.modify(&uid.to_string(), |e| {
            e.status = status;
            true
        })
    }

    pub fn toggle_check(self: Pin<&mut Self>, uid: &QString, index: i32) -> bool {
        self.modify(&uid.to_string(), |e| match e.checklist.get_mut(index.max(0) as usize) {
            Some(item) => {
                item.0 = !item.0;
                true
            }
            None => false,
        })
    }

    pub fn link_file(self: Pin<&mut Self>, uid: &QString, path: &QString) -> bool {
        let file = self.relative(&path.to_string());
        self.modify(&uid.to_string(), |e| {
            if e.files.contains(&file) {
                return false;
            }
            e.files.push(file);
            true
        })
    }

    pub fn unlink_file(self: Pin<&mut Self>, uid: &QString, file: &QString) -> bool {
        let file = file.to_string();
        self.modify(&uid.to_string(), |e| {
            let before = e.files.len();
            e.files.retain(|f| f != &file);
            e.files.len() != before
        })
    }

    pub fn files_moved(mut self: Pin<&mut Self>, from: &QString, to: &QString) {
        let (from, to) = (self.relative(&from.to_string()), self.relative(&to.to_string()));
        if !from.starts_with('/') || !to.starts_with('/') {
            return;
        }
        let moved = |f: &str| -> Option<String> {
            if f == from {
                Some(to.clone())
            } else {
                f.strip_prefix(&format!("{from}/")).map(|rest| format!("{to}/{rest}"))
            }
        };
        let touched: Vec<(String, String)> = self
            .entries
            .iter()
            .filter(|e| e.files.iter().any(|f| moved(f).is_some()))
            .map(|e| (e.uid.clone(), e.calendar.clone()))
            .collect();
        if touched.is_empty() {
            return;
        }
        for e in self.as_mut().rust_mut().entries.iter_mut() {
            for f in e.files.iter_mut() {
                if let Some(n) = moved(f) {
                    *f = n;
                }
            }
        }
        // Local entries are rewritten; links are recorded for all.
        let local = touched.iter().any(|t| t.1 == LOCAL);
        self.as_mut().commit(if local { LOCAL } else { "" });
        for (uid, cal) in touched.iter().filter(|t| t.1 != LOCAL) {
            self.as_mut().push_remote(cal, uid);
        }
    }

    fn title_for(path: &str) -> String {
        let p = Path::new(path);
        p.file_stem().or(p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_owned())
    }

    /// Adds a new entry in the default calendar.
    fn add(mut self: Pin<&mut Self>, mut e: Entry) -> QString {
        e.calendar = self.default_calendar().to_string();
        let (uid, calendar) = (e.uid.clone(), e.calendar.clone());
        self.as_mut().rust_mut().entries.push(e);
        self.store_entry(&calendar, &uid);
        qs(uid)
    }

    pub fn block_for_file(self: Pin<&mut Self>, path: &QString, start: &QString, end: &QString) -> QString {
        let path = path.to_string();
        let Some(start) = calendar::parse_local(&start.to_string()) else { return QString::default() };
        let end = calendar::parse_local(&end.to_string()).filter(|e| *e > start).unwrap_or(start + Duration::hours(1));
        let mut e = Entry::new(Kind::Block, &Self::title_for(&path));
        e.start = Some(self.new_time(start, false));
        e.end = Some(self.new_time(end, false));
        e.files = vec![self.relative(&path)];
        self.add(e)
    }

    pub fn task_for_file(self: Pin<&mut Self>, path: &QString) -> QString {
        let path = path.to_string();
        let mut e = Entry::new(Kind::Task, &Self::title_for(&path));
        e.files = vec![self.relative(&path)];
        self.add(e)
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
            self.as_mut().add(e);
            count += 1;
        }
        count
    }
}
