// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Synchronisation of the project calendar with CalDAV calendars.
//!
//! Everything lives in `.atlas/calendar/cache/sync.json`:
//!
//! - the **cache** of each calendar (`href → ETag, iCalendar text`) as last
//!   seen on the server, with local changes already applied, so Atlas works
//!   offline;
//! - the **queue** of local changes not yet on the server (one operation
//!   per object; later changes replace earlier ones but keep the ETag they
//!   are based on);
//! - **conflicts**: objects changed both locally and on the server (or a
//!   change the server refused), resolved by the user;
//! - time and error of the last synchronisation.
//!
//! Writes are conditional (`If-Match` with the base ETag, `If-None-Match: *`
//! for new objects whose name comes from the UID), so nothing on the server
//! is overwritten silently and an upload repeated after a lost response
//! never creates a duplicate.
//!
//! The file is only changed in short transactions ([`SyncStore::update`]);
//! network requests run outside of them, so the UI can keep editing while a
//! synchronisation runs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::caldav::{DavError, Remote, RemoteCalendar, Write, object_name, same_href};
use super::ical::{self, Component};
use super::{Entry, LOCAL};

pub const CACHE: &str = "cache";
pub const STATE: &str = "sync.json";
const VERSION: u32 = 1;

/// One object as cached.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cached {
    pub uid: String,
    /// ETag of the server version the data is based on ("" = not on the
    /// server yet).
    pub etag: String,
    pub data: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedCalendar {
    #[serde(flatten)]
    pub info: RemoteCalendar,
    /// `href → object`.
    #[serde(default)]
    pub objects: BTreeMap<String, Cached>,
    /// Not found on the server at the last synchronisation (kept, never
    /// overwritten).
    #[serde(default)]
    pub missing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpKind {
    Put,
    Delete,
}

/// A local change waiting for the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Op {
    pub id: u64,
    pub kind: OpKind,
    pub calendar: String,
    pub href: String,
    pub uid: String,
    /// ETag of the server version this change replaces (`None` = new).
    pub base: Option<String>,
    /// iCalendar text (puts only).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub data: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    pub id: u64,
    pub calendar: String,
    pub href: String,
    pub uid: String,
    pub title: String,
    /// Local version (`None`: deleted in Atlas).
    pub local: Option<String>,
    /// Server version (`None`: deleted on the server).
    pub remote: Option<String>,
    pub remote_etag: Option<String>,
    /// Why the server refused the change (empty for real conflicts).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub message: String,
}

/// How the user resolves a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Nextcloud-Version
    Remote,
    /// Lokale Version
    Local,
    /// Beide behalten (the local version becomes a new entry)
    Both,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub version: u32,
    /// Calendar home of the account (for creating calendars).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<String>,
    #[serde(default)]
    pub calendars: BTreeMap<String, CachedCalendar>,
    #[serde(default)]
    pub queue: Vec<Op>,
    #[serde(default)]
    pub conflicts: Vec<Conflict>,
    /// Unix time of the last complete synchronisation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync: Option<i64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub error: String,
    #[serde(default)]
    pub next_id: u64,
}

/// Serialises transactions of all projects in this process.
static LOCK: Mutex<()> = Mutex::new(());

/// `.atlas/calendar/cache/` of a project.
#[derive(Debug, Clone)]
pub struct SyncStore {
    dir: PathBuf,
}

impl SyncStore {
    pub fn new(calendar_dir: &Path) -> Self {
        Self { dir: calendar_dir.join(CACHE) }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn read(&self) -> State {
        let mut state: State = std::fs::read_to_string(self.dir.join(STATE))
            .ok()
            .and_then(|t| serde_json::from_str(&t).map_err(|e| log::warn!("calendar cache: {e}")).ok())
            .unwrap_or_default();
        state.version = VERSION;
        state
    }

    /// A snapshot of the state.
    pub fn load(&self) -> State {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        self.read()
    }

    /// Runs `f` on the state and writes it back.
    pub fn update<R>(&self, f: impl FnOnce(&mut State) -> R) -> Result<R> {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut state = self.read();
        let before = state.clone();
        let out = f(&mut state);
        if state != before {
            crate::paths::write_atomic(&self.dir.join(STATE), serde_json::to_string(&state)?.as_bytes())?;
        }
        Ok(out)
    }

    /// Removes the cache (pending changes and conflicts included).
    pub fn clear(&self) -> Result<()> {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        match std::fs::remove_file(self.dir.join(STATE)) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err.into()),
            _ => Ok(()),
        }
    }
}

// ------------------------------------------------------------ local changes

/// The iCalendar object for all components of one UID (series with its
/// changed occurrences).
pub fn object_data(entries: &[&Entry]) -> String {
    ical::serialize(&[ical::calendar(entries.iter().map(|e| e.to_component()).collect())])
}

/// Entries of an object (calendar set to `calendar`).
pub fn parse_object(data: &str, calendar: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for cal in ical::parse(data).unwrap_or_default() {
        for e in cal.children.iter().filter_map(Entry::from_component) {
            out.push(Entry { calendar: calendar.to_owned(), ..e });
        }
    }
    out
}

impl State {
    fn next(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Where the object with `uid` lives in `calendar`.
    pub fn href_of(&self, calendar: &str, uid: &str) -> Option<String> {
        self.calendars.get(calendar)?.objects.iter().find(|(_, o)| o.uid == uid).map(|(h, _)| h.clone())
    }

    /// All entries of the cached calendars `calendars`.
    pub fn entries(&self, calendars: &[String]) -> Vec<Entry> {
        let mut out = Vec::new();
        for url in calendars {
            if let Some(c) = self.calendars.get(url) {
                for o in c.objects.values() {
                    out.extend(parse_object(&o.data, url));
                }
            }
        }
        out
    }

    pub fn pending(&self, calendar: &str, uid: &str) -> bool {
        self.queue.iter().any(|op| op.calendar == calendar && op.uid == uid)
    }

    fn queue_op(&mut self, mut op: Op) {
        // The newest change replaces an older one but keeps its base.
        if let Some(i) = self.queue.iter().position(|o| o.href == op.href) {
            let old = self.queue.remove(i);
            op.base = old.base;
            if op.kind == OpKind::Delete && op.base.is_none() {
                return; // never reached the server
            }
        }
        self.queue.push(op);
    }

    /// Records a new or changed object `data` with `uid` in `calendar`.
    pub fn local_put(&mut self, calendar: &str, uid: &str, data: String) {
        let href = self.href_of(calendar, uid).unwrap_or_else(|| {
            let objects = self.calendars.get(calendar).map(|c| &c.objects);
            let mut href = format!("{calendar}{}", object_name(uid));
            let mut n = 2;
            while objects.is_some_and(|o| o.contains_key(&href)) {
                href = format!("{calendar}{}", object_name(&format!("{uid}-{n}")));
                n += 1;
            }
            href
        });
        let id = self.next();
        let cal = self.calendars.entry(calendar.to_owned()).or_insert_with(|| CachedCalendar {
            info: RemoteCalendar { url: calendar.to_owned(), writable: true, ..Default::default() },
            ..Default::default()
        });
        let object =
            cal.objects.entry(href.clone()).or_insert_with(|| Cached { uid: uid.to_owned(), ..Default::default() });
        object.data = data.clone();
        let base = (!object.etag.is_empty()).then(|| object.etag.clone());
        self.queue_op(Op {
            id,
            kind: OpKind::Put,
            calendar: calendar.to_owned(),
            href,
            uid: uid.to_owned(),
            base,
            data,
        });
    }

    /// Records the deletion of the object with `uid`.
    pub fn local_delete(&mut self, calendar: &str, uid: &str) {
        let Some(href) = self.href_of(calendar, uid) else { return };
        let id = self.next();
        let Some(object) = self.calendars.get_mut(calendar).and_then(|c| c.objects.remove(&href)) else { return };
        let base = (!object.etag.is_empty()).then_some(object.etag);
        self.queue_op(Op {
            id,
            kind: OpKind::Delete,
            calendar: calendar.to_owned(),
            href,
            uid: uid.to_owned(),
            base,
            data: String::new(),
        });
    }

    /// Applies the user's choice; returns the UID of a copy made for
    /// [`Choice::Both`].
    pub fn resolve(&mut self, id: u64, choice: Choice) -> Option<String> {
        let i = self.conflicts.iter().position(|c| c.id == id)?;
        let c = self.conflicts.remove(i);
        let cache = |state: &mut State, data: Option<&String>, etag: &Option<String>| {
            let Some(cal) = state.calendars.get_mut(&c.calendar) else { return };
            match data {
                Some(d) => {
                    cal.objects.insert(
                        c.href.clone(),
                        Cached { uid: c.uid.clone(), etag: etag.clone().unwrap_or_default(), data: d.clone() },
                    );
                }
                None => {
                    cal.objects.remove(&c.href);
                }
            }
        };
        let choice = match (choice, &c.local, &c.remote) {
            (Choice::Both, None, _) => Choice::Remote,
            (Choice::Both, _, None) => Choice::Local,
            (choice, _, _) => choice,
        };
        match choice {
            Choice::Remote => {
                cache(self, c.remote.as_ref(), &c.remote_etag);
                None
            }
            Choice::Local => {
                // Replace exactly the server version that was seen.
                cache(self, c.remote.as_ref(), &c.remote_etag);
                match &c.local {
                    Some(data) => {
                        if let Some(o) =
                            self.calendars.get_mut(&c.calendar).and_then(|cal| cal.objects.get_mut(&c.href))
                        {
                            o.data = data.clone();
                        } else if let Some(cal) = self.calendars.get_mut(&c.calendar) {
                            cal.objects.insert(
                                c.href.clone(),
                                Cached { uid: c.uid.clone(), etag: String::new(), data: data.clone() },
                            );
                        }
                        let id = self.next();
                        self.queue.retain(|o| o.href != c.href);
                        self.queue.push(Op {
                            id,
                            kind: OpKind::Put,
                            calendar: c.calendar.clone(),
                            href: c.href.clone(),
                            uid: c.uid.clone(),
                            base: c.remote_etag.clone().filter(|_| c.remote.is_some()),
                            data: data.clone(),
                        });
                    }
                    None => self.local_delete(&c.calendar, &c.uid),
                }
                None
            }
            Choice::Both => {
                cache(self, c.remote.as_ref(), &c.remote_etag);
                let local = c.local.as_deref().unwrap_or_default();
                let uid = format!("{}@atlas", uuid::Uuid::new_v4());
                let mut entries = parse_object(local, &c.calendar);
                for e in &mut entries {
                    e.uid = uid.clone();
                }
                let refs: Vec<&Entry> = entries.iter().collect();
                self.local_put(&c.calendar, &uid, object_data(&refs));
                Some(uid)
            }
        }
    }
}

// ----------------------------------------------------------- synchronising

/// What a run did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub uploaded: usize,
    pub deleted: usize,
    pub downloaded: usize,
    pub removed: usize,
    pub conflicts: usize,
}

/// Content of an object for comparisons: its components without the
/// properties every write changes.
fn essence(data: &str) -> Vec<Component> {
    let mut out = Vec::new();
    for cal in ical::parse(data).unwrap_or_default() {
        for mut c in cal.children.into_iter().filter(|c| c.name == "VEVENT" || c.name == "VTODO") {
            for name in ["DTSTAMP", "LAST-MODIFIED", "CREATED"] {
                c.remove(name);
            }
            c.props.sort_by(|a, b| (&a.name, &a.value).cmp(&(&b.name, &b.value)));
            out.push(c);
        }
    }
    out.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
    out
}

fn title_of(data: &str) -> String {
    parse_object(data, "").into_iter().find(|e| e.recurrence_id.is_none()).map(|e| e.title).unwrap_or_default()
}

fn now_unix() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Synchronises `calendars` (URLs) with the server: refreshes the calendar
/// list, sends the queue, then fetches what changed on the server.
///
/// Fails without losing anything when the server cannot be reached; the
/// queue stays for the next run.
pub fn run(store: &SyncStore, remote: &dyn Remote, calendars: &[String]) -> Result<Report, DavError> {
    let result = run_inner(store, remote, calendars);
    let message = result.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
    let _ = store.update(|s| {
        s.error = message;
        if result.is_ok() {
            s.last_sync = Some(now_unix());
        }
    });
    result
}

fn save(store: &SyncStore, f: impl FnOnce(&mut State)) -> Result<(), DavError> {
    store.update(f).map_err(|e| DavError::Invalid(format!("Kalender-Cache kann nicht geschrieben werden: {e:#}")))
}

fn run_inner(store: &SyncStore, remote: &dyn Remote, calendars: &[String]) -> Result<Report, DavError> {
    let mut report = Report::default();

    // 1. Calendar list: names, colours and rights from the server.
    let found = remote.discover()?;
    save(store, |s| {
        if found.home.is_some() {
            s.home = found.home.clone();
        }
        for url in calendars {
            let info = found.calendars.iter().find(|c| same_href(&c.url, url));
            let cal = s.calendars.entry(url.clone()).or_default();
            match info {
                Some(info) => {
                    cal.info = RemoteCalendar { url: url.clone(), ..info.clone() };
                    cal.missing = false;
                }
                None => {
                    cal.info.url = url.clone();
                    cal.missing = true;
                }
            }
        }
    })?;

    // 2. Local changes, oldest first.
    let mut attempted: Vec<u64> = Vec::new();
    loop {
        let state = store.load();
        let Some(op) = state
            .queue
            .iter()
            .find(|op| {
                !attempted.contains(&op.id)
                    && calendars.contains(&op.calendar)
                    && state.calendars.get(&op.calendar).is_some_and(|c| !c.missing)
            })
            .cloned()
        else {
            break;
        };
        attempted.push(op.id);
        let writable = state.calendars.get(&op.calendar).is_some_and(|c| c.info.writable);
        let outcome = if !writable {
            Err(DavError::Http(403, "Kalender ist schreibgeschützt".into()))
        } else {
            match op.kind {
                OpKind::Put => remote.put(&op.href, &op.data, op.base.as_deref()),
                OpKind::Delete => match &op.base {
                    Some(etag) => remote.delete(&op.href, etag),
                    None => Ok(Write::Done(None)),
                },
            }
        };
        let outcome = match outcome {
            Ok(w) => w,
            Err(e) if e.is_offline() || e == DavError::Auth => return Err(e),
            Err(e) => {
                // Refused (read-only, invalid data …): the user decides.
                let message = e.to_string();
                save(store, |s| {
                    s.queue.retain(|o| o.id != op.id);
                    let id = s.next();
                    let remote_etag =
                        s.calendars.get(&op.calendar).and_then(|c| c.objects.get(&op.href)).map(|o| o.etag.clone());
                    s.conflicts.retain(|c| c.href != op.href);
                    s.conflicts.push(Conflict {
                        id,
                        calendar: op.calendar.clone(),
                        href: op.href.clone(),
                        uid: op.uid.clone(),
                        title: title_of(&op.data),
                        local: (op.kind == OpKind::Put).then(|| op.data.clone()),
                        remote: None,
                        remote_etag: remote_etag.filter(|e| !e.is_empty()),
                        message,
                    });
                })?;
                report.conflicts += 1;
                continue;
            }
        };
        match outcome {
            Write::Done(etag) => {
                // Without an ETag in the answer, ask for it.
                let etag = match (op.kind, etag) {
                    (OpKind::Put, None) => remote
                        .etags(&op.calendar)?
                        .and_then(|m| m.into_iter().find(|(h, _)| same_href(h, &op.href)).map(|(_, e)| e))
                        .unwrap_or_default(),
                    (_, e) => e.unwrap_or_default(),
                };
                save(store, |s| {
                    let newer = s.queue.iter_mut().find(|o| o.href == op.href && o.id != op.id);
                    match (op.kind, newer) {
                        // Changed again meanwhile: that change now builds on ours.
                        (_, Some(n)) => n.base = (op.kind == OpKind::Put && !etag.is_empty()).then(|| etag.clone()),
                        (OpKind::Put, None) => {
                            if let Some(o) = s.calendars.get_mut(&op.calendar).and_then(|c| c.objects.get_mut(&op.href))
                            {
                                o.etag = etag.clone();
                            }
                        }
                        (OpKind::Delete, None) => {}
                    }
                    s.queue.retain(|o| o.id != op.id);
                })?;
                if op.kind == OpKind::Put { report.uploaded += 1 } else { report.deleted += 1 }
            }
            Write::Changed | Write::Gone => {
                let current = if outcome == Write::Gone {
                    Vec::new()
                } else {
                    remote.fetch(&op.calendar, std::slice::from_ref(&op.href))?
                };
                let server = current.into_iter().next();
                // A repeated upload whose first answer was lost: already there.
                let same =
                    op.kind == OpKind::Put && server.as_ref().is_some_and(|o| essence(&o.data) == essence(&op.data));
                save(store, |s| {
                    s.queue.retain(|o| o.id != op.id);
                    if same {
                        if let Some(o) = s.calendars.get_mut(&op.calendar).and_then(|c| c.objects.get_mut(&op.href)) {
                            o.etag = server.as_ref().map(|x| x.etag.clone()).unwrap_or_default();
                        }
                        return;
                    }
                    if op.kind == OpKind::Delete && server.is_none() {
                        return;
                    }
                    let id = s.next();
                    let title = title_of(server.as_ref().map(|o| o.data.as_str()).unwrap_or(&op.data));
                    s.conflicts.retain(|c| c.href != op.href);
                    s.conflicts.push(Conflict {
                        id,
                        calendar: op.calendar.clone(),
                        href: op.href.clone(),
                        uid: op.uid.clone(),
                        title,
                        local: (op.kind == OpKind::Put).then(|| op.data.clone()),
                        remote: server.as_ref().map(|o| o.data.clone()),
                        remote_etag: server.as_ref().map(|o| o.etag.clone()),
                        message: String::new(),
                    });
                })?;
                if same { report.uploaded += 1 } else { report.conflicts += 1 }
            }
        }
    }

    // 3. Changes on the server.
    for url in calendars {
        if store.load().calendars.get(url).is_none_or(|c| c.missing) {
            continue;
        }
        let Some(etags) = remote.etags(url)? else {
            save(store, |s| s.calendars.entry(url.clone()).or_default().missing = true)?;
            continue;
        };
        let state = store.load();
        let cal = state.calendars.get(url).cloned().unwrap_or_default();
        let busy = |href: &str| state.queue.iter().any(|o| o.href == href);
        // The cache's name for a server href (they may differ in escaping).
        let known = |href: &str| cal.objects.keys().find(|k| same_href(k, href)).cloned();
        let changed: Vec<String> = etags
            .iter()
            .filter(|(href, etag)| {
                let key = known(href).unwrap_or_else(|| (*href).clone());
                let conflict = state.conflicts.iter().find(|c| c.href == key);
                match conflict {
                    Some(c) => c.remote_etag.as_deref() != Some(etag.as_str()),
                    None => !busy(&key) && cal.objects.get(&key).is_none_or(|o| o.etag != **etag),
                }
            })
            .map(|(h, _)| h.clone())
            .collect();
        let objects = if changed.is_empty() { Vec::new() } else { remote.fetch(url, &changed)? };
        save(store, |s| {
            let pending: Vec<String> = s.queue.iter().map(|o| o.href.clone()).collect();
            let cal = s.calendars.entry(url.clone()).or_default();
            // Deleted on the server – or kept there under another name
            // (Google renames new objects and sends no ETag, which left an
            // entry without one behind: shown twice).
            let on_server = |h: &str| etags.keys().any(|e| same_href(e, h));
            let gone: Vec<String> = cal
                .objects
                .iter()
                .filter(|(h, _)| !on_server(h) && !pending.contains(h))
                .map(|(h, _)| h.clone())
                .collect();
            for h in &gone {
                if !s.conflicts.iter().any(|c| c.href == *h) {
                    cal.objects.remove(h);
                    report.removed += 1;
                }
            }
            for c in s
                .conflicts
                .iter_mut()
                .filter(|c| c.calendar == *url && !etags.contains_key(&c.href) && c.remote_etag.is_some())
            {
                c.remote = None;
                c.remote_etag = None;
            }
            for o in objects {
                if let Some(c) = s.conflicts.iter_mut().find(|c| c.href == o.href) {
                    c.remote = Some(o.data);
                    c.remote_etag = Some(o.etag);
                    continue;
                }
                if pending.contains(&o.href) {
                    continue;
                }
                let uid = parse_object(&o.data, url).first().map(|e| e.uid.clone()).unwrap_or_default();
                let key = cal.objects.keys().find(|k| same_href(k, &o.href)).cloned().unwrap_or(o.href);
                // One entry per UID: an older copy under another name goes
                // (unless a change of it is still on its way).
                if !uid.is_empty() {
                    cal.objects.retain(|k, c| *k == key || c.uid != uid || pending.contains(k));
                }
                cal.objects.insert(key, Cached { uid, etag: o.etag, data: o.data });
                report.downloaded += 1;
            }
        })?;
    }
    report.conflicts = store.load().conflicts.len();
    Ok(report)
}

/// Entries of the local calendar are never part of the synchronisation.
pub fn is_remote(calendar: &str) -> bool {
    calendar != LOCAL
}

#[cfg(test)]
mod tests {
    use super::super::caldav::{Account, Client, tests::TestServer};
    use super::super::{Kind, ical::Time, parse_local};
    use super::*;
    use std::cell::RefCell;

    fn entry(title: &str) -> Entry {
        let mut e = Entry::new(Kind::Event, title);
        e.start = Some(Time::Floating(parse_local("2026-10-20T10:00").unwrap()));
        e.end = Some(Time::Floating(parse_local("2026-10-20T11:00").unwrap()));
        e
    }

    fn server_titles(server: &TestServer, cal: &str) -> Vec<String> {
        let s = server.state.lock().unwrap();
        let mut t: Vec<String> = s.calendars[cal].1.values().map(|(_, d)| title_of(d)).collect();
        t.sort();
        t
    }

    const CAL: &str = "/remote.php/dav/calendars/minux/personal/";

    #[test]
    fn server_that_renames_new_objects_gives_no_duplicates() {
        // Google keeps a new event under its own name and sends no ETag:
        // the entry must not appear twice.
        let server = TestServer::start();
        server.state.lock().unwrap().own_names = true;
        let url = format!("{}{CAL}", server.base);
        let dir = tempfile::tempdir().unwrap();
        let store = SyncStore::new(dir.path());
        let client = Client::new(
            Account { server: server.base.clone(), user: "minux".into(), dav_url: String::new() },
            "geheim",
            false,
        );
        let e = entry("Sprechstunde");
        store.update(|s| s.local_put(&url, &e.uid, object_data(&[&e]))).unwrap();
        run(&store, &client, std::slice::from_ref(&url)).unwrap();
        run(&store, &client, std::slice::from_ref(&url)).unwrap();
        let state = store.load();
        let copies: Vec<_> = state.calendars[&url].objects.iter().filter(|(_, o)| o.uid == e.uid).collect();
        assert_eq!(copies.len(), 1, "{copies:?}");
        let (href, cached) = copies[0];
        assert!(href.ends_with("%40atlas.ics") && !cached.etag.is_empty(), "{href} {cached:?}");
        assert_eq!(server_titles(&server, CAL), ["Sprechstunde"]);

        // A change goes to the server's name: still one, on both sides.
        let mut changed = e.clone();
        changed.title = "Sprechstunde (verschoben)".into();
        store.update(|s| s.local_put(&url, &e.uid, object_data(&[&changed]))).unwrap();
        run(&store, &client, std::slice::from_ref(&url)).unwrap();
        let state = store.load();
        assert_eq!(state.calendars[&url].objects.values().filter(|o| o.uid == e.uid).count(), 1);
        assert_eq!(server_titles(&server, CAL), ["Sprechstunde (verschoben)"]);
        assert!(state.queue.is_empty() && state.conflicts.is_empty());
    }

    #[test]
    fn local_entry_appears_on_server_and_comes_back() {
        let server = TestServer::start();
        let url = format!("{}{CAL}", server.base);
        let dir = tempfile::tempdir().unwrap();
        let store = SyncStore::new(dir.path());
        let client = Client::new(
            Account { server: server.base.clone(), user: "minux".into(), dav_url: String::new() },
            "geheim",
            false,
        );

        let e = entry("Kapitel 3");
        store.update(|s| s.local_put(&url, &e.uid, object_data(&[&e]))).unwrap();
        let r = run(&store, &client, std::slice::from_ref(&url)).unwrap();
        assert_eq!(r.uploaded, 1);
        assert_eq!(server_titles(&server, CAL), ["Kapitel 3"], "created in the chosen calendar");
        let state = store.load();
        assert!(state.queue.is_empty());
        assert!(state.last_sync.is_some());
        let href = state.href_of(&url, &e.uid).unwrap();
        assert!(!state.calendars[&url].objects[&href].etag.is_empty());

        // Changed on the server by another client: downloaded.
        {
            let mut s = server.state.lock().unwrap();
            let path = &href[server.base.len()..];
            let obj = s.calendars.get_mut(CAL).unwrap().1.get_mut(path).unwrap();
            obj.0 = "\"fremd\"".into();
            obj.1 = obj.1.replace("SUMMARY:Kapitel 3", "SUMMARY:Kapitel 3 (geändert)");
        }
        let r = run(&store, &client, std::slice::from_ref(&url)).unwrap();
        assert_eq!(r.downloaded, 1);
        assert_eq!(store.load().entries(std::slice::from_ref(&url))[0].title, "Kapitel 3 (geändert)");

        // Second run: nothing to do.
        let puts = server.state.lock().unwrap().puts;
        assert_eq!(run(&store, &client, std::slice::from_ref(&url)).unwrap(), Report::default());
        assert_eq!(server.state.lock().unwrap().puts, puts);

        // Deleted in Atlas.
        store.update(|s| s.local_delete(&url, &e.uid)).unwrap();
        assert_eq!(run(&store, &client, std::slice::from_ref(&url)).unwrap().deleted, 1);
        assert!(server_titles(&server, CAL).is_empty());
    }

    #[test]
    fn offline_changes_are_sent_once_after_reconnecting() {
        let server = TestServer::start();
        let url = format!("{}{CAL}", server.base);
        let dir = tempfile::tempdir().unwrap();
        let store = SyncStore::new(dir.path());
        let account = Account { server: server.base.clone(), user: "minux".into(), dav_url: String::new() };
        let online = Client::new(account, "geheim", false);
        run(&store, &online, std::slice::from_ref(&url)).unwrap();

        // Offline: two new entries, one changed twice.
        let offline = Client::new(
            Account { server: "http://127.0.0.1:9".into(), user: "minux".into(), dav_url: String::new() },
            "geheim",
            false,
        );
        let a = entry("Offline A");
        let mut b = entry("Offline B");
        store.update(|s| s.local_put(&url, &a.uid, object_data(&[&a]))).unwrap();
        store.update(|s| s.local_put(&url, &b.uid, object_data(&[&b]))).unwrap();
        b.title = "Offline B, zweite Fassung".into();
        store.update(|s| s.local_put(&url, &b.uid, object_data(&[&b]))).unwrap();
        let err = run(&store, &offline, std::slice::from_ref(&url)).unwrap_err();
        assert!(err.is_offline());
        let state = store.load();
        assert_eq!(state.queue.len(), 2, "one operation per entry");
        assert!(state.error.contains("nicht erreichbar"));
        assert_eq!(state.entries(std::slice::from_ref(&url)).len(), 2, "visible offline");

        // Back online.
        let r = run(&store, &online, std::slice::from_ref(&url)).unwrap();
        assert_eq!(r.uploaded, 2);
        assert_eq!(server_titles(&server, CAL), ["Offline A", "Offline B, zweite Fassung"]);
        assert!(store.load().error.is_empty());

        // A repeated upload after a lost answer (queue as before the run).
        store
            .update(|s| {
                let href = s.href_of(&url, &a.uid).unwrap();
                s.calendars.get_mut(&url).unwrap().objects.get_mut(&href).unwrap().etag.clear();
                s.queue.push(Op {
                    id: 999,
                    kind: OpKind::Put,
                    calendar: url.clone(),
                    href,
                    uid: a.uid.clone(),
                    base: None,
                    data: object_data(&[&a]),
                });
            })
            .unwrap();
        run(&store, &online, std::slice::from_ref(&url)).unwrap();
        assert_eq!(server_titles(&server, CAL).len(), 2, "no duplicates");
        assert!(store.load().conflicts.is_empty());
    }

    #[test]
    fn conflicts_are_shown_and_resolved() {
        let server = TestServer::start();
        let url = format!("{}{CAL}", server.base);
        let dir = tempfile::tempdir().unwrap();
        let store = SyncStore::new(dir.path());
        let client = Client::new(
            Account { server: server.base.clone(), user: "minux".into(), dav_url: String::new() },
            "geheim",
            false,
        );
        let mut e = entry("Besprechung");
        store.update(|s| s.local_put(&url, &e.uid, object_data(&[&e]))).unwrap();
        run(&store, &client, std::slice::from_ref(&url)).unwrap();
        let href = store.load().href_of(&url, &e.uid).unwrap();
        let path = href[server.base.len()..].to_owned();
        let change_on_server = |title: &str, etag: &str| {
            let mut s = server.state.lock().unwrap();
            let obj = s.calendars.get_mut(CAL).unwrap().1.get_mut(&path).unwrap();
            obj.0 = etag.into();
            obj.1 = obj.1.replace("SUMMARY:Besprechung", &format!("SUMMARY:{title}"));
        };

        // Changed on both sides.
        change_on_server("Besprechung (Server)", "\"s1\"");
        e.title = "Besprechung (Atlas)".into();
        store.update(|s| s.local_put(&url, &e.uid, object_data(&[&e]))).unwrap();
        let r = run(&store, &client, std::slice::from_ref(&url)).unwrap();
        assert_eq!(r.conflicts, 1);
        let state = store.load();
        let c = state.conflicts[0].clone();
        assert!(c.local.as_deref().unwrap().contains("Besprechung (Atlas)"));
        assert!(c.remote.as_deref().unwrap().contains("Besprechung (Server)"));
        assert_eq!(server_titles(&server, CAL), ["Besprechung (Server)"], "nothing overwritten");

        // Keep both: the local version becomes a new entry.
        let copy = store.update(|s| s.resolve(c.id, Choice::Both)).unwrap().unwrap();
        assert_ne!(copy, e.uid);
        run(&store, &client, std::slice::from_ref(&url)).unwrap();
        assert_eq!(server_titles(&server, CAL), ["Besprechung (Atlas)", "Besprechung (Server)"]);

        // Local version wins.
        e.title = "Lokal gewinnt".into();
        let mut server_entry = parse_object(&store.load().calendars[&url].objects[&href].data, &url).remove(0);
        server_entry.title = "Lokal gewinnt".into();
        store.update(|s| s.local_put(&url, &e.uid, object_data(&[&server_entry]))).unwrap();
        {
            let mut s = server.state.lock().unwrap();
            let obj = s.calendars.get_mut(CAL).unwrap().1.get_mut(&path).unwrap();
            obj.0 = "\"s2\"".into();
        }
        run(&store, &client, std::slice::from_ref(&url)).unwrap();
        let id = store.load().conflicts[0].id;
        store.update(|s| s.resolve(id, Choice::Local)).unwrap();
        run(&store, &client, std::slice::from_ref(&url)).unwrap();
        assert!(server_titles(&server, CAL).contains(&"Lokal gewinnt".to_owned()));
        assert!(store.load().conflicts.is_empty());

        // Server version wins.
        change_on_server("egal", "\"s3\"");
        e.title = "verworfen".into();
        store.update(|s| s.local_put(&url, &e.uid, object_data(&[&e]))).unwrap();
        {
            let mut s = server.state.lock().unwrap();
            let obj = s.calendars.get_mut(CAL).unwrap().1.get_mut(&path).unwrap();
            obj.0 = "\"s4\"".into();
            obj.1 = obj.1.replace("SUMMARY:Lokal gewinnt", "SUMMARY:Server gewinnt");
        }
        run(&store, &client, std::slice::from_ref(&url)).unwrap();
        let id = store.load().conflicts[0].id;
        store.update(|s| s.resolve(id, Choice::Remote)).unwrap();
        let titles: Vec<String> =
            store.load().entries(std::slice::from_ref(&url)).into_iter().map(|e| e.title).collect();
        assert!(titles.contains(&"Server gewinnt".to_owned()), "{titles:?}");
        assert!(!titles.contains(&"verworfen".to_owned()));
    }

    /// A server whose calendar disappeared and one that is read-only.
    struct Fake {
        calendars: Vec<RemoteCalendar>,
        puts: RefCell<u32>,
    }
    impl Remote for Fake {
        fn discover(&self) -> Result<super::super::caldav::Discovery, DavError> {
            Ok(super::super::caldav::Discovery { home: None, calendars: self.calendars.clone() })
        }
        fn etags(&self, _: &str) -> Result<Option<BTreeMap<String, String>>, DavError> {
            Ok(Some(BTreeMap::new()))
        }
        fn fetch(&self, _: &str, _: &[String]) -> Result<Vec<super::super::caldav::Object>, DavError> {
            Ok(Vec::new())
        }
        fn put(&self, _: &str, _: &str, _: Option<&str>) -> Result<Write, DavError> {
            *self.puts.borrow_mut() += 1;
            Ok(Write::Done(Some("\"x\"".into())))
        }
        fn delete(&self, _: &str, _: &str) -> Result<Write, DavError> {
            Ok(Write::Done(None))
        }
        fn create_calendar(&self, _: &str, _: &str, _: &str) -> Result<RemoteCalendar, DavError> {
            unimplemented!()
        }
    }

    #[test]
    fn missing_and_read_only_calendars_are_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let store = SyncStore::new(dir.path());
        let gone = "https://h/cal/gone/".to_owned();
        let ro = "https://h/cal/ro/".to_owned();
        let e = entry("Termin");
        store
            .update(|s| {
                s.local_put(&gone, &e.uid, object_data(&[&e]));
                s.local_put(&ro, &e.uid, object_data(&[&e]));
            })
            .unwrap();
        let fake = Fake {
            calendars: vec![RemoteCalendar {
                url: ro.clone(),
                name: "Geburtstage".into(),
                writable: false,
                ..Default::default()
            }],
            puts: RefCell::new(0),
        };
        run(&store, &fake, &[gone.clone(), ro.clone()]).unwrap();
        let state = store.load();
        assert!(state.calendars[&gone].missing);
        assert_eq!(state.calendars[&gone].objects.len(), 1, "cache of a vanished calendar kept");
        assert_eq!(state.queue.len(), 1, "change for the vanished calendar waits");
        assert_eq!(*fake.puts.borrow(), 0, "nothing written to the read-only calendar");
        assert_eq!(state.conflicts.len(), 1);
        assert!(state.conflicts[0].message.contains("schreibgeschützt"));
    }
}
