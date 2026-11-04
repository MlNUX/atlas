// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Which calendars show, synchronising with the server and its conflicts.

use super::*;

impl qobject::CalendarModel {
    fn change_choice(mut self: Pin<&mut Self>, id: &str, f: impl FnOnce(&mut CalendarChoice, &str)) {
        let mut config = Config::load().calendar;
        let root = self.project_root.to_string();
        let Some(choice) = config.calendars.iter_mut().find(|c| c.url == id) else { return };
        f(choice, &root);
        if let Err(err) = Config::save_calendar(&config) {
            self.as_mut().set_error(format!("Einstellungen können nicht gespeichert werden: {err:#}"));
        }
        self.as_mut().reload();
        self.sync_now();
    }

    pub fn set_calendar_visible(self: Pin<&mut Self>, id: &QString, visible: bool) {
        self.change_choice(&id.to_string(), |c, root| {
            c.hidden = !visible;
            // Shown in this project again: also if it was limited to others.
            if visible && !c.projects.is_empty() && !c.projects.iter().any(|p| p == root) {
                c.projects.push(root.to_owned());
            }
        });
    }

    pub fn set_calendar_project_only(self: Pin<&mut Self>, id: &QString, only: bool) {
        self.change_choice(&id.to_string(), |c, root| {
            c.projects = if only { vec![root.to_owned()] } else { Vec::new() };
        });
    }

    pub fn set_default_calendar(mut self: Pin<&mut Self>, id: &QString, project: bool) {
        let id = id.to_string();
        let result = if project {
            let mut settings = self.project_settings();
            settings["default_calendar"] = json!(id);
            serde_json::to_string_pretty(&settings)
                .map_err(anyhow::Error::from)
                .and_then(|t| atlas_core::paths::write_atomic(&self.store().dir().join(SETTINGS), t.as_bytes()))
        } else {
            let mut config = Config::load().calendar;
            config.default = if id == LOCAL { String::new() } else { id };
            Config::save_calendar(&config)
        };
        if let Err(err) = result {
            self.as_mut().set_error(format!("Standardkalender kann nicht gespeichert werden: {err:#}"));
        }
        self.reload();
    }

    pub fn sync_now(mut self: Pin<&mut Self>) {
        if !self.config.connected() {
            return;
        }
        let Some(password) = password() else {
            self.as_mut().update_sync_state();
            self.fetch_password();
            return;
        };
        if self.syncing {
            self.as_mut().rust_mut().sync_again = true;
            return;
        }
        let calendars = self.synced();
        if calendars.is_empty() {
            return;
        }
        self.as_mut().rust_mut().syncing = true;
        self.as_mut().rust_mut().sync_again = false;
        self.as_mut().update_sync_state();
        let store = self.sync_store();
        let remote = client(&self.config, &password);
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = sync::run(&store, &remote, &calendars);
            let _ = thread.queue(move |mut this| {
                this.as_mut().rust_mut().syncing = false;
                let (ok, message) = match &result {
                    Ok(r) => {
                        let mut parts = Vec::new();
                        if r.uploaded + r.deleted > 0 {
                            parts.push(format!("{} gesendet", r.uploaded + r.deleted));
                        }
                        if r.downloaded + r.removed > 0 {
                            parts.push(format!("{} vom Server", r.downloaded + r.removed));
                        }
                        if r.conflicts > 0 {
                            parts.push(format!("{} Konflikt(e)", r.conflicts));
                        }
                        (
                            true,
                            if parts.is_empty() {
                                "Kalender aktuell".to_owned()
                            } else {
                                format!("Kalender: {}", parts.join(", "))
                            },
                        )
                    }
                    Err(err) => (false, err.to_string()),
                };
                this.as_mut().reload();
                this.as_mut().sync_finished(ok, qs(message));
                if this.sync_again {
                    this.sync_now();
                }
            });
        });
    }

    pub fn create_calendar(self: Pin<&mut Self>, name: &QString) {
        let Some(password) = password() else {
            self.calendar_created(false, qs("App-Passwort fehlt"), QString::default());
            return;
        };
        let name = name.to_string();
        let config = self.config.clone();
        let home = self.sync_store().load().home;
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let remote = client(&config, &password);
            // (Google needs no home: the Calendar API creates calendars.)
            let result = match home {
                Some(h) => Ok(h),
                None if config.is_google() => Ok(String::new()),
                None => remote.discover().and_then(|d| {
                    d.home.ok_or_else(|| {
                        atlas_core::calendar::caldav::DavError::Invalid("kein Kalenderverzeichnis".into())
                    })
                }),
            }
            .and_then(|home| remote.create_calendar(&home, &name, "#2ea043"));
            let _ = thread.queue(move |mut this| match result {
                Ok(cal) => {
                    let mut config = Config::load().calendar;
                    config.calendars.push(CalendarChoice {
                        url: cal.url.clone(),
                        name: cal.name.clone(),
                        ..Default::default()
                    });
                    if let Err(err) = Config::save_calendar(&config) {
                        this.as_mut().set_error(format!("{err:#}"));
                    }
                    this.as_mut().reload();
                    this.as_mut().calendar_created(true, qs(format!("Kalender „{}“ angelegt", cal.name)), qs(&cal.url));
                    this.sync_now();
                }
                Err(err) => this.calendar_created(false, qs(err.to_string()), QString::default()),
            });
        });
    }

    pub fn clear_cache(mut self: Pin<&mut Self>) {
        if let Err(err) = self.sync_store().clear() {
            self.as_mut().set_error(format!("Cache kann nicht gelöscht werden: {err:#}"));
        }
        self.as_mut().reload();
        self.sync_now();
    }

    pub fn conflicts(&self) -> QString {
        let tz = self.tz;
        let describe = |data: &Option<String>| -> Value {
            let Some(data) = data else { return Value::Null };
            let entries = sync::parse_object(data, "");
            let Some(e) = entries.iter().find(|e| e.recurrence_id.is_none()).or(entries.first()) else {
                return Value::Null;
            };
            let when = match e.start {
                Some(s) if s.is_date() => s.local(tz).format("%d.%m.%Y").to_string(),
                Some(s) => s.local(tz).format("%d.%m.%Y %H:%M").to_string(),
                None => String::new(),
            };
            json!({ "title": e.title, "when": when, "description": e.description, "status": e.status.as_str() })
        };
        let state = self.sync_store().load();
        let list: Vec<Value> = state
            .conflicts
            .iter()
            .map(|c| {
                json!({
                    "id": c.id, "title": c.title, "calendar": self.calendar_info(&c.calendar).0,
                    "local": describe(&c.local), "remote": describe(&c.remote), "message": c.message,
                })
            })
            .collect();
        qs(Value::from(list).to_string())
    }

    pub fn resolve_conflict(mut self: Pin<&mut Self>, id: i32, choice: &QString) -> bool {
        let choice = match choice.to_string().as_str() {
            "remote" => Choice::Remote,
            "local" => Choice::Local,
            _ => Choice::Both,
        };
        let state = self.sync_store().load();
        let Some(conflict) = state.conflicts.iter().find(|c| c.id == id as u64).cloned() else { return false };
        match self.sync_store().update(|s| s.resolve(conflict.id, choice)) {
            Ok(copy) => {
                // The copy keeps the links of the local version.
                if let Some(uid) = copy {
                    let links = self.store().links();
                    if let Some(l) = links.get(&conflict.uid) {
                        let mut e = Entry::new(Kind::Event, "");
                        e.uid = uid;
                        e.files = l.files.clone();
                        e.session = l.session.clone();
                        let _ = self.store().save_links([&e]);
                    }
                }
                self.as_mut().reload();
                self.sync_now();
                true
            }
            Err(err) => {
                self.as_mut().set_error(format!("{err:#}"));
                false
            }
        }
    }
}
