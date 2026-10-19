// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Project calendar: appointments, work blocks, deadlines and unscheduled
//! tasks, linked with project files and named sessions (see CALENDAR.md).
//!
//! Local entries live in `.atlas/calendar/events.ics` (iCalendar); Atlas
//! specific links are stored as `X-ATLAS-*` properties and, keyed by the
//! entry's UID, in `.atlas/calendar/links.json`, so they survive servers
//! that drop unknown properties. `state.json` holds only view state.
//!
//! Unknown properties and components of an entry are kept unchanged.

pub mod ical;
pub mod recur;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

use ical::{Component, Property, Time};

pub const DIR: &str = "calendar";
pub const EVENTS: &str = "events.ics";
pub const LINKS: &str = "links.json";
pub const STATE: &str = "state.json";
/// Calendar ID of entries kept only in the project.
pub const LOCAL: &str = "local";

/// The user's time zone (falls back to UTC).
pub fn local_zone() -> Tz {
    if let Ok(name) = std::env::var("ATLAS_TZ")
        && let Ok(tz) = name.parse()
    {
        return tz;
    }
    iana_time_zone::get_timezone().ok().and_then(|n| n.parse().ok()).unwrap_or(chrono_tz::UTC)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Termin
    Event,
    /// Arbeitsblock
    Block,
    /// Frist
    Deadline,
    /// Aufgabe ohne Zeit (noch nicht eingeplant)
    Task,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Event => "event",
            Kind::Block => "block",
            Kind::Deadline => "deadline",
            Kind::Task => "task",
        }
    }
    pub fn parse(s: &str) -> Option<Kind> {
        Some(match s.to_ascii_lowercase().as_str() {
            "event" => Kind::Event,
            "block" => Kind::Block,
            "deadline" => Kind::Deadline,
            "task" => Kind::Task,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Planned,
    Started,
    Done,
    Postponed,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Planned => "planned",
            Status::Started => "started",
            Status::Done => "done",
            Status::Postponed => "postponed",
        }
    }
    pub fn parse(s: &str) -> Option<Status> {
        Some(match s.to_ascii_lowercase().as_str() {
            "planned" | "geplant" => Status::Planned,
            "started" | "begonnen" | "in-process" => Status::Started,
            "done" | "erledigt" | "completed" => Status::Done,
            "postponed" | "verschoben" => Status::Postponed,
            _ => return None,
        })
    }
}

/// One VEVENT or VTODO.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub uid: String,
    pub kind: Kind,
    pub title: String,
    pub description: String,
    pub start: Option<Time>,
    /// Exclusive end (for all-day entries the day after).
    pub end: Option<Time>,
    pub status: Status,
    pub tags: Vec<String>,
    pub checklist: Vec<(bool, String)>,
    /// Project-relative files (`/docs/a.md`).
    pub files: Vec<String>,
    /// Name of the linked session ("" = none).
    pub session: String,
    pub estimate_minutes: Option<u32>,
    /// Reminders in minutes before the start.
    pub reminders: Vec<i64>,
    /// RRULE value as written.
    pub rrule: Option<String>,
    pub exdates: Vec<Time>,
    /// Set for a changed occurrence of a series.
    pub recurrence_id: Option<Time>,
    pub sequence: u32,
    /// The component as read (unknown properties and children are kept).
    pub raw: Component,
}

fn now_stamp() -> String {
    Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
}

impl Entry {
    pub fn new(kind: Kind, title: &str) -> Self {
        let mut raw = Component::new(if kind == Kind::Task { "VTODO" } else { "VEVENT" });
        raw.push(Property::new("CREATED", now_stamp()));
        Entry {
            uid: format!("{}@atlas", uuid::Uuid::new_v4()),
            kind,
            title: title.to_owned(),
            description: String::new(),
            start: None,
            end: None,
            status: Status::Planned,
            tags: Vec::new(),
            checklist: Vec::new(),
            files: Vec::new(),
            session: String::new(),
            estimate_minutes: None,
            reminders: Vec::new(),
            rrule: None,
            exdates: Vec::new(),
            recurrence_id: None,
            sequence: 0,
            raw,
        }
    }

    pub fn all_day(&self) -> bool {
        self.start.is_some_and(|s| s.is_date())
    }

    /// Reads a VEVENT or VTODO; `None` for other components.
    pub fn from_component(c: &Component) -> Option<Entry> {
        let is_todo = match c.name.as_str() {
            "VEVENT" => false,
            "VTODO" => true,
            _ => return None,
        };
        let uid = c.text("UID")?;
        let time = |name: &str| c.get(name).and_then(ical::parse_time);
        let start = time("DTSTART");
        let mut end = if is_todo { time("DUE") } else { time("DTEND") };
        if end.is_none()
            && let (Some(s), Some(d)) = (start, c.get("DURATION").and_then(|p| ical::parse_duration(&p.value)))
        {
            end = Some(match s {
                Time::Date(day) => Time::Date(day + Duration::seconds(d)),
                Time::Floating(t) => Time::Floating(t + Duration::seconds(d)),
                Time::Utc(t) => Time::Utc(t + Duration::seconds(d)),
                Time::Zoned(t, z) => Time::Zoned(t + Duration::seconds(d), z),
            });
        }
        let kind = c.text("X-ATLAS-KIND").and_then(|k| Kind::parse(&k)).unwrap_or(if is_todo {
            if start.is_none() && end.is_some() { Kind::Deadline } else { Kind::Task }
        } else {
            Kind::Event
        });
        // A deadline stored as VTODO has its time in DUE.
        let (start, end) = if is_todo && kind == Kind::Deadline && start.is_none() { (end, None) } else { (start, end) };
        let status = c.text("X-ATLAS-STATUS").and_then(|s| Status::parse(&s)).unwrap_or_else(|| {
            match c.text("STATUS").unwrap_or_default().to_ascii_uppercase().as_str() {
                "COMPLETED" => Status::Done,
                "IN-PROCESS" => Status::Started,
                _ => Status::Planned,
            }
        });
        let reminders = c
            .children
            .iter()
            .filter(|a| a.name == "VALARM")
            .filter(|a| a.get("TRIGGER").is_some_and(|t| t.param("RELATED").is_none_or(|r| r.eq_ignore_ascii_case("START"))))
            .filter_map(|a| a.get("TRIGGER").and_then(|t| ical::parse_duration(&t.value)))
            .map(|secs| -secs / 60)
            .collect();
        let checklist = c
            .all("X-ATLAS-CHECK")
            .map(|p| {
                let text = p.text_value();
                match text.split_once(';') {
                    Some((state, item)) => (state.trim() == "x", item.to_owned()),
                    None => (false, text),
                }
            })
            .collect();
        Some(Entry {
            uid,
            kind,
            title: c.text("SUMMARY").unwrap_or_default(),
            description: c.text("DESCRIPTION").unwrap_or_default(),
            start,
            end,
            status,
            tags: c.all("CATEGORIES").flat_map(|p| p.list()).collect(),
            checklist,
            files: c.all("X-ATLAS-FILE").map(|p| p.text_value()).collect(),
            session: c.text("X-ATLAS-SESSION").unwrap_or_default(),
            estimate_minutes: c.text("X-ATLAS-ESTIMATE").and_then(|e| e.trim().parse().ok()),
            reminders,
            rrule: c.get("RRULE").map(|p| p.value.clone()),
            exdates: c
                .all("EXDATE")
                .flat_map(|p| {
                    p.value
                        .split(',')
                        .filter_map(|v| ical::parse_time(&Property { value: v.to_owned(), ..p.clone() }))
                        .collect::<Vec<_>>()
                })
                .collect(),
            recurrence_id: time("RECURRENCE-ID"),
            sequence: c.text("SEQUENCE").and_then(|s| s.parse().ok()).unwrap_or(0),
            raw: c.clone(),
        })
    }

    /// Writes the entry back into its component, changing only what Atlas
    /// manages.
    pub fn to_component(&self) -> Component {
        let todo = self.kind == Kind::Task || (self.kind == Kind::Deadline && self.raw.name == "VTODO");
        let mut c = self.raw.clone();
        c.name = if todo { "VTODO".into() } else { "VEVENT".into() };
        for name in [
            "UID", "DTSTAMP", "DTSTART", "DTEND", "DUE", "DURATION", "SUMMARY", "DESCRIPTION", "CATEGORIES", "STATUS",
            "RRULE", "EXDATE", "RECURRENCE-ID", "SEQUENCE", "LAST-MODIFIED", "COMPLETED", "X-ATLAS-KIND",
            "X-ATLAS-STATUS", "X-ATLAS-FILE", "X-ATLAS-SESSION", "X-ATLAS-CHECK", "X-ATLAS-ESTIMATE",
        ] {
            c.remove(name);
        }
        c.push(Property::text("UID", &self.uid));
        c.push(Property::new("DTSTAMP", now_stamp()));
        if let Some(r) = &self.recurrence_id {
            c.push(ical::time_property("RECURRENCE-ID", r));
        }
        if todo && self.kind == Kind::Deadline {
            if let Some(s) = &self.start {
                c.push(ical::time_property("DUE", s));
            }
        } else {
            if let Some(s) = &self.start {
                c.push(ical::time_property("DTSTART", s));
            }
            if let Some(e) = &self.end {
                c.push(ical::time_property(if todo { "DUE" } else { "DTEND" }, e));
            }
        }
        c.push(Property::text("SUMMARY", &self.title));
        if !self.description.is_empty() {
            c.push(Property::text("DESCRIPTION", &self.description));
        }
        if !self.tags.is_empty() {
            c.push(Property::new("CATEGORIES", self.tags.iter().map(|t| ical::escape(t)).collect::<Vec<_>>().join(",")));
        }
        let status = match (todo, self.status) {
            (true, Status::Done) => "COMPLETED",
            (true, Status::Started) => "IN-PROCESS",
            (true, _) => "NEEDS-ACTION",
            (false, _) => "CONFIRMED",
        };
        c.push(Property::new("STATUS", status));
        if todo && self.status == Status::Done {
            c.push(Property::new("COMPLETED", now_stamp()));
        }
        if let Some(rule) = &self.rrule {
            c.push(Property::new("RRULE", rule.clone()));
        }
        for ex in &self.exdates {
            c.push(ical::time_property("EXDATE", ex));
        }
        c.push(Property::new("SEQUENCE", self.sequence.to_string()));
        c.push(Property::new("LAST-MODIFIED", now_stamp()));
        c.push(Property::text("X-ATLAS-KIND", self.kind.as_str()));
        c.push(Property::text("X-ATLAS-STATUS", self.status.as_str()));
        for f in &self.files {
            c.push(Property::text("X-ATLAS-FILE", f));
        }
        if !self.session.is_empty() {
            c.push(Property::text("X-ATLAS-SESSION", &self.session));
        }
        for (done, text) in &self.checklist {
            c.push(Property::text("X-ATLAS-CHECK", &format!("{};{text}", if *done { "x" } else { " " })));
        }
        if let Some(e) = self.estimate_minutes {
            c.push(Property::new("X-ATLAS-ESTIMATE", e.to_string()));
        }
        // Simple reminders are Atlas' own; other alarms stay as they were.
        c.children.retain(|a| {
            !(a.name == "VALARM"
                && a.text("ACTION").is_some_and(|x| x.eq_ignore_ascii_case("DISPLAY"))
                && a.get("TRIGGER").is_some_and(|t| t.param("VALUE").is_none() && t.param("RELATED").is_none()))
        });
        for minutes in &self.reminders {
            let mut alarm = Component::new("VALARM");
            alarm.push(Property::new("ACTION", "DISPLAY"));
            alarm.push(Property::text("DESCRIPTION", &self.title));
            alarm.push(Property::new("TRIGGER", ical::format_duration(-minutes * 60)));
            c.children.push(alarm);
        }
        c
    }
}

// ------------------------------------------------------------------ storage

/// Atlas links of an entry, kept separately by UID.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Links {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub session: String,
}

/// `.atlas/calendar/` of a project.
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn for_atlas_dir(atlas_dir: &Path) -> Self {
        Self::new(atlas_dir.join(DIR))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Reads an iCalendar file (missing file: no entries).
    pub fn read_ics(file: &Path) -> Result<Vec<Entry>> {
        let text = match std::fs::read_to_string(file) {
            Ok(t) => t,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err).with_context(|| format!("{} kann nicht gelesen werden", file.display())),
        };
        let mut out = Vec::new();
        for cal in ical::parse(&text).with_context(|| format!("{} ist kein gültiges iCalendar", file.display()))? {
            out.extend(cal.children.iter().filter_map(Entry::from_component));
        }
        Ok(out)
    }

    pub fn write_ics(file: &Path, entries: &[Entry]) -> Result<()> {
        let cal = ical::calendar(entries.iter().map(Entry::to_component).collect());
        crate::paths::write_atomic(file, ical::serialize(&[cal]).as_bytes())
    }

    /// Local entries, with links from `links.json` where the file has none.
    pub fn load(&self) -> Result<Vec<Entry>> {
        let mut entries = Self::read_ics(&self.dir.join(EVENTS))?;
        let links = self.links();
        for e in &mut entries {
            if let Some(l) = links.get(&e.uid) {
                if e.files.is_empty() {
                    e.files = l.files.clone();
                }
                if e.session.is_empty() {
                    e.session = l.session.clone();
                }
            }
        }
        Ok(entries)
    }

    pub fn save(&self, entries: &[Entry]) -> Result<()> {
        Self::write_ics(&self.dir.join(EVENTS), entries)
    }

    pub fn links(&self) -> BTreeMap<String, Links> {
        std::fs::read_to_string(self.dir.join(LINKS)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    /// Records the links of `entries` (others stay, e.g. remote calendars').
    pub fn save_links<'a>(&self, entries: impl IntoIterator<Item = &'a Entry>) -> Result<()> {
        let mut links = self.links();
        for e in entries {
            if e.files.is_empty() && e.session.is_empty() {
                links.remove(&e.uid);
            } else {
                links.insert(e.uid.clone(), Links { files: e.files.clone(), session: e.session.clone() });
            }
        }
        crate::paths::write_atomic(&self.dir.join(LINKS), serde_json::to_string_pretty(&links)?.as_bytes())
    }

    pub fn forget_links(&self, uid: &str) -> Result<()> {
        let mut links = self.links();
        if links.remove(uid).is_some() {
            crate::paths::write_atomic(&self.dir.join(LINKS), serde_json::to_string_pretty(&links)?.as_bytes())?;
        }
        Ok(())
    }

    pub fn state(&self) -> serde_json::Value {
        std::fs::read_to_string(self.dir.join(STATE)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save_state(&self, state: &serde_json::Value) -> Result<()> {
        crate::paths::write_atomic(&self.dir.join(STATE), serde_json::to_string_pretty(state)?.as_bytes())
    }
}

// --------------------------------------------------------------- occurrences

/// One appearance of an entry in a time range (local time of the user).
#[derive(Debug, Clone, PartialEq)]
pub struct Occurrence {
    pub uid: String,
    pub kind: Kind,
    pub title: String,
    pub status: Status,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub all_day: bool,
    pub recurring: bool,
    /// Original start of this occurrence (series only).
    pub instance: Option<NaiveDateTime>,
}

fn same_kind_at(template: &Time, local: NaiveDateTime) -> Time {
    match *template {
        Time::Date(_) => Time::Date(local.date()),
        Time::Floating(_) => Time::Floating(local),
        Time::Zoned(_, z) => Time::Zoned(local, z),
        // UTC series repeat in UTC.
        Time::Utc(_) => Time::Utc(Utc.from_utc_datetime(&local)),
    }
}

/// The entry's start in its own clock (what a series repeats in).
fn own_local(t: &Time) -> NaiveDateTime {
    match *t {
        Time::Date(d) => d.and_time(NaiveTime::MIN),
        Time::Floating(t) | Time::Zoned(t, _) => t,
        Time::Utc(t) => t.naive_utc(),
    }
}

fn span(e: &Entry, start: &Time, tz: Tz) -> Duration {
    let s = start.local(tz);
    match &e.end {
        Some(end) => (end.local(tz) - s).max(Duration::zero()),
        None if start.is_date() => Duration::days(1),
        None => Duration::zero(),
    }
}

/// All occurrences overlapping `from..to` (user's local time `tz`), sorted.
pub fn occurrences(entries: &[Entry], tz: Tz, from: NaiveDateTime, to: NaiveDateTime) -> Vec<Occurrence> {
    let mut out = Vec::new();
    let overrides: BTreeMap<(&str, NaiveDateTime), &Entry> = entries
        .iter()
        .filter_map(|e| e.recurrence_id.as_ref().map(|r| ((e.uid.as_str(), own_local(r)), e)))
        .collect();
    let push = |e: &Entry, start: &Time, recurring: bool, instance: Option<NaiveDateTime>, out: &mut Vec<Occurrence>| {
        let s = start.local(tz);
        let mut end = s + span(e, &e.start.unwrap_or(*start), tz);
        // Point-like entries (deadlines) still need to show up.
        if end <= s {
            end = s + Duration::minutes(1);
        }
        if end > from && s < to {
            out.push(Occurrence {
                uid: e.uid.clone(),
                kind: e.kind,
                title: e.title.clone(),
                status: e.status,
                start: s,
                end,
                all_day: start.is_date(),
                recurring,
                instance,
            });
        }
    };
    for e in entries {
        let Some(start) = e.start else { continue };
        if e.recurrence_id.is_some() {
            push(e, &start, true, e.recurrence_id.as_ref().map(own_local), &mut out);
            continue;
        }
        let Some(rule) = e.rrule.as_deref().and_then(recur::Rule::parse) else {
            push(e, &start, false, None, &mut out);
            continue;
        };
        // Expand in the entry's own clock, a day wider for zone offsets.
        let base = own_local(&start);
        let duration = span(e, &start, tz);
        let excluded: Vec<NaiveDateTime> = e.exdates.iter().map(own_local).collect();
        for t in recur::occurrences(&rule, base, from - Duration::days(1) - duration, to + Duration::days(1)) {
            if excluded.contains(&t) || overrides.contains_key(&(e.uid.as_str(), t)) {
                continue;
            }
            push(e, &same_kind_at(&start, t), true, Some(t), &mut out);
        }
    }
    out.sort_by(|a, b| (a.start, !a.all_day, &a.title).cmp(&(b.start, !b.all_day, &b.title)));
    out
}

/// Reminders that fall into `from..to` (UTC): (uid, title, start, minutes).
pub fn due_reminders(entries: &[Entry], tz: Tz, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<(String, String, NaiveDateTime, i64)> {
    let mut out = Vec::new();
    let window_from = from.with_timezone(&tz).naive_local() - Duration::days(1);
    let window_to = to.with_timezone(&tz).naive_local() + Duration::days(8);
    for occ in occurrences(entries, tz, window_from, window_to) {
        let Some(entry) = entries.iter().find(|e| e.uid == occ.uid) else { continue };
        if occ.status == Status::Done {
            continue;
        }
        for minutes in &entry.reminders {
            let at = tz.from_local_datetime(&(occ.start - Duration::minutes(*minutes))).earliest().map(|d| d.with_timezone(&Utc));
            if at.is_some_and(|at| at >= from && at < to) {
                out.push((occ.uid.clone(), occ.title.clone(), occ.start, *minutes));
            }
        }
    }
    out
}

// ---------------------------------------------------------------- Markdown

/// Unchecked or checked task list items of a Markdown file: (line, done,
/// text); fenced code is skipped.
pub fn checklist_items(text: &str) -> Vec<(usize, bool, String)> {
    let mut out = Vec::new();
    let mut fence: Option<String> = None;
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker: String = trimmed.chars().take_while(|c| *c == '`' || *c == '~').collect();
            match &fence {
                Some(open) if marker.starts_with(open.as_str()) => fence = None,
                None => fence = Some(marker),
                _ => {}
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")).or_else(|| trimmed.strip_prefix("+ ")) else {
            continue;
        };
        let (done, item) = if let Some(r) = rest.strip_prefix("[ ] ") {
            (false, r)
        } else if let Some(r) = rest.strip_prefix("[x] ").or_else(|| rest.strip_prefix("[X] ")) {
            (true, r)
        } else {
            continue;
        };
        if !item.trim().is_empty() {
            out.push((i + 1, done, item.trim().to_owned()));
        }
    }
    out
}

/// Parses `2h`, `90m`, `1h30m`, `1.5h` or plain minutes.
pub fn parse_estimate(text: &str) -> Option<u32> {
    let t = text.trim().to_ascii_lowercase().replace(' ', "");
    if let Ok(m) = t.parse::<u32>() {
        return Some(m);
    }
    let mut total = 0.0f64;
    let mut number = String::new();
    for c in t.chars() {
        match c {
            '0'..='9' | '.' | ',' => number.push(if c == ',' { '.' } else { c }),
            'h' | 'm' | 'd' => {
                let n: f64 = number.parse().ok()?;
                total += n * match c {
                    'd' => 8.0 * 60.0,
                    'h' => 60.0,
                    _ => 1.0,
                };
                number.clear();
            }
            _ => return None,
        }
    }
    (number.is_empty() && total > 0.0).then(|| total.round() as u32)
}

/// Parses a date or date-time given by the UI (`2026-10-19`,
/// `2026-10-19T14:30`).
pub fn parse_local(text: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M")
        .or_else(|_| NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S"))
        .ok()
        .or_else(|| NaiveDate::parse_from_str(text, "%Y-%m-%d").ok().map(|d| d.and_time(NaiveTime::MIN)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> NaiveDateTime {
        parse_local(s).unwrap()
    }

    #[test]
    fn entries_round_trip_and_keep_unknown_data() {
        let text = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:018f-example@atlas\r\nDTSTART:20261017T100000\r\nDTEND:20261017T150000\r\n\
                    SUMMARY:Kapitel 3 fertigstellen\r\nX-ATLAS-FILE:/docs/kapitel-3.md\r\nX-ATLAS-SESSION:Dokumentation\r\n\
                    X-OTHER-CLIENT:bleibt\r\nATTENDEE;CN=Jemand:mailto:a@b.c\r\n\
                    BEGIN:VALARM\r\nACTION:AUDIO\r\nTRIGGER;VALUE=DATE-TIME:20261017T090000Z\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let cal = ical::parse(text).unwrap();
        let mut e = Entry::from_component(&cal[0].children[0]).unwrap();
        assert_eq!(e.kind, Kind::Event);
        assert_eq!(e.files, ["/docs/kapitel-3.md"]);
        assert_eq!(e.session, "Dokumentation");
        assert_eq!(e.start, Some(Time::Floating(dt("2026-10-17T10:00"))));
        e.kind = Kind::Block;
        e.status = Status::Started;
        e.tags = vec!["schreiben".into(), "a,b".into()];
        e.checklist = vec![(true, "Gliederung".into()), (false, "Text; Teil 1".into())];
        e.reminders = vec![15];
        e.estimate_minutes = Some(120);
        let c = e.to_component();
        assert_eq!(c.text("X-OTHER-CLIENT").as_deref(), Some("bleibt"), "unknown properties stay");
        assert!(c.get("ATTENDEE").is_some());
        assert_eq!(c.children.iter().filter(|a| a.name == "VALARM").count(), 2, "foreign alarm kept, own one added");
        let back = Entry::from_component(&c).unwrap();
        assert_eq!(back.kind, Kind::Block);
        assert_eq!(back.status, Status::Started);
        assert_eq!(back.tags, ["schreiben", "a,b"]);
        assert_eq!(back.checklist, e.checklist);
        assert_eq!(back.reminders, [15]);
        assert_eq!(back.estimate_minutes, Some(120));

        // Tasks are VTODOs; deadlines as VEVENT keep their kind.
        let mut task = Entry::new(Kind::Task, "Literatur sichten");
        task.estimate_minutes = parse_estimate("1h30m");
        assert_eq!(task.to_component().name, "VTODO");
        assert_eq!(Entry::from_component(&task.to_component()).unwrap().kind, Kind::Task);
        // A VTODO with only DUE from another client is a deadline.
        let mut todo = Component::new("VTODO");
        todo.push(Property::new("UID", "x"));
        todo.push(Property::new("DUE", "20261020").with_param("VALUE", "DATE"));
        let d = Entry::from_component(&todo).unwrap();
        assert_eq!((d.kind, d.start.is_some()), (Kind::Deadline, true));
        assert!(d.to_component().get("DUE").is_some());
    }

    #[test]
    fn store_keeps_links_separately() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("calendar"));
        let mut e = Entry::new(Kind::Block, "Schreiben");
        e.start = Some(Time::Floating(dt("2026-10-19T09:00")));
        e.end = Some(Time::Floating(dt("2026-10-19T11:00")));
        e.files = vec!["/docs/a.md".into()];
        e.session = "Dokumentation".into();
        store.save(std::slice::from_ref(&e)).unwrap();
        store.save_links([&e]).unwrap();
        // A server that drops X-ATLAS-* properties: links come from links.json.
        let text = std::fs::read_to_string(dir.path().join("calendar/events.ics")).unwrap();
        let stripped: String = text.lines().filter(|l| !l.starts_with("X-ATLAS-FILE") && !l.starts_with("X-ATLAS-SESSION")).map(|l| format!("{l}\n")).collect();
        std::fs::write(dir.path().join("calendar/events.ics"), stripped).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded[0].files, ["/docs/a.md"]);
        assert_eq!(loaded[0].session, "Dokumentation");
        store.forget_links(&e.uid).unwrap();
        assert!(store.links().is_empty());
    }

    #[test]
    fn occurrences_of_single_all_day_and_recurring_entries() {
        let tz = chrono_tz::Europe::Berlin;
        let mut single = Entry::new(Kind::Event, "Besprechung");
        single.start = Some(Time::Utc(Utc.from_utc_datetime(&dt("2026-10-20T08:00"))));
        single.end = Some(Time::Utc(Utc.from_utc_datetime(&dt("2026-10-20T09:00"))));
        let mut allday = Entry::new(Kind::Deadline, "Abgabe");
        allday.start = Some(Time::Date(NaiveDate::from_ymd_opt(2026, 10, 23).unwrap()));
        let mut series = Entry::new(Kind::Block, "Schreiben");
        series.start = Some(Time::Zoned(dt("2026-10-19T09:00"), tz));
        series.end = Some(Time::Zoned(dt("2026-10-19T11:00"), tz));
        series.rrule = Some("FREQ=DAILY;COUNT=10".into());
        series.exdates = vec![Time::Zoned(dt("2026-10-21T09:00"), tz)];
        // The occurrence on the 22nd moved to the afternoon.
        let mut moved = series.clone();
        moved.rrule = None;
        moved.exdates.clear();
        moved.recurrence_id = Some(Time::Zoned(dt("2026-10-22T09:00"), tz));
        moved.start = Some(Time::Zoned(dt("2026-10-22T14:00"), tz));
        moved.end = Some(Time::Zoned(dt("2026-10-22T16:00"), tz));
        let task = Entry::new(Kind::Task, "Ohne Zeit");
        let entries = vec![single, allday, series, moved, task];

        let week = occurrences(&entries, tz, dt("2026-10-19T00:00"), dt("2026-10-26T00:00"));
        let summary: Vec<String> = week.iter().map(|o| format!("{} {}", o.start.format("%a %H:%M"), o.title)).collect();
        assert_eq!(
            summary,
            [
                "Mon 09:00 Schreiben",
                "Tue 09:00 Schreiben",
                "Tue 10:00 Besprechung",
                "Thu 14:00 Schreiben",
                "Fri 00:00 Abgabe",
                "Fri 09:00 Schreiben",
                "Sat 09:00 Schreiben",
                "Sun 09:00 Schreiben",
            ],
            "UTC converted to Berlin (UTC+2), EXDATE skipped, override moved"
        );
        assert!(week.iter().find(|o| o.title == "Abgabe").unwrap().all_day);
        // Across the switch to winter time (25 Oct) the series stays at 09:00.
        let later = occurrences(&entries, tz, dt("2026-10-26T00:00"), dt("2026-10-29T00:00"));
        assert!(later.iter().all(|o| o.start.format("%H:%M").to_string() == "09:00"), "{later:?}");
    }

    #[test]
    fn reminders_while_atlas_runs() {
        let tz = chrono_tz::Europe::Berlin;
        let mut e = Entry::new(Kind::Event, "Termin");
        e.start = Some(Time::Zoned(dt("2026-10-20T10:00"), tz));
        e.reminders = vec![15];
        let at = |s: &str| tz.from_local_datetime(&dt(s)).unwrap().with_timezone(&Utc);
        assert_eq!(due_reminders(std::slice::from_ref(&e), tz, at("2026-10-20T09:44"), at("2026-10-20T09:46")).len(), 1);
        assert!(due_reminders(std::slice::from_ref(&e), tz, at("2026-10-20T09:46"), at("2026-10-20T09:50")).is_empty());
    }

    #[test]
    fn markdown_checklists_and_estimates() {
        let md = "# Plan\n\n- [ ] Literatur sichten\n- [x] Gliederung\n  * [ ] Unterpunkt\n```\n- [ ] kein Eintrag\n```\n- normal\n";
        assert_eq!(
            checklist_items(md),
            vec![(3, false, "Literatur sichten".into()), (4, true, "Gliederung".into()), (5, false, "Unterpunkt".into())]
        );
        assert_eq!(parse_estimate("2h"), Some(120));
        assert_eq!(parse_estimate("1,5h"), Some(90));
        assert_eq!(parse_estimate("45"), Some(45));
        assert_eq!(parse_estimate("viel"), None);
    }
}
