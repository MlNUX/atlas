// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Minimal iCalendar (RFC 5545) reader and writer.
//!
//! Components and properties are kept as they are, so that data written by
//! other clients (Nextcloud, Thunderbird, …) survives a round trip through
//! Atlas: only the properties Atlas manages are changed.

use std::str::FromStr;

use anyhow::{Result, bail};
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// Upper case (`DTSTART`, `X-ATLAS-FILE`).
    pub name: String,
    /// Parameters in order (`TZID`, `VALUE`, …), names upper case.
    pub params: Vec<(String, String)>,
    /// The value as written (escaped for text values).
    pub value: String,
}

impl Property {
    pub fn new(name: &str, value: impl Into<String>) -> Self {
        Self { name: name.to_ascii_uppercase(), params: Vec::new(), value: value.into() }
    }

    /// A text property (value escaped).
    pub fn text(name: &str, value: &str) -> Self {
        Self::new(name, escape(value))
    }

    pub fn with_param(mut self, name: &str, value: &str) -> Self {
        self.params.push((name.to_ascii_uppercase(), value.to_owned()));
        self
    }

    pub fn param(&self, name: &str) -> Option<&str> {
        self.params.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// The value as text (unescaped).
    pub fn text_value(&self) -> String {
        unescape(&self.value)
    }

    /// Comma separated text values (`CATEGORIES`).
    pub fn list(&self) -> Vec<String> {
        split_unescaped(&self.value, ',').into_iter().map(|v| unescape(&v)).filter(|v| !v.is_empty()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Component {
    /// `VCALENDAR`, `VEVENT`, `VTODO`, `VALARM`, `VTIMEZONE`, …
    pub name: String,
    pub props: Vec<Property>,
    pub children: Vec<Component>,
}

impl Component {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_ascii_uppercase(), ..Default::default() }
    }

    pub fn get(&self, name: &str) -> Option<&Property> {
        self.props.iter().find(|p| p.name.eq_ignore_ascii_case(name))
    }

    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Property> + 'a {
        self.props.iter().filter(move |p| p.name.eq_ignore_ascii_case(name))
    }

    pub fn text(&self, name: &str) -> Option<String> {
        self.get(name).map(Property::text_value)
    }

    /// Removes all properties called `name`.
    pub fn remove(&mut self, name: &str) {
        self.props.retain(|p| !p.name.eq_ignore_ascii_case(name));
    }

    /// Replaces all properties called `name` by `prop`.
    pub fn set(&mut self, prop: Property) {
        match self.props.iter().position(|p| p.name.eq_ignore_ascii_case(&prop.name)) {
            Some(i) => {
                let name = prop.name.clone();
                self.props[i] = prop;
                let mut seen = false;
                self.props.retain(|p| {
                    if !p.name.eq_ignore_ascii_case(&name) {
                        return true;
                    }
                    let keep = !seen;
                    seen = true;
                    keep
                });
            }
            None => self.props.push(prop),
        }
    }

    pub fn push(&mut self, prop: Property) {
        self.props.push(prop);
    }
}

// ------------------------------------------------------------------ escaping

pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out
}

pub fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n' | 'N') => out.push('\n'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Splits at `sep` unless escaped with a backslash.
fn split_unescaped(value: &str, sep: char) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut escaped = false;
    for c in value.chars() {
        if escaped {
            parts.last_mut().unwrap().push(c);
            escaped = false;
        } else if c == '\\' {
            parts.last_mut().unwrap().push(c);
            escaped = true;
        } else if c == sep {
            parts.push(String::new());
        } else {
            parts.last_mut().unwrap().push(c);
        }
    }
    parts
}

// ------------------------------------------------------------------- reading

fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if let Some(rest) = line.strip_prefix(' ').or_else(|| line.strip_prefix('\t'))
            && let Some(last) = lines.last_mut()
        {
            last.push_str(rest);
            continue;
        }
        if !line.is_empty() {
            lines.push(line.to_owned());
        }
    }
    lines
}

fn parse_line(line: &str) -> Option<Property> {
    // name *(";" param) ":" value – parameter values may be quoted.
    let mut in_quotes = false;
    let mut colon = None;
    for (i, c) in line.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ':' if !in_quotes => {
                colon = Some(i);
                break;
            }
            _ => {}
        }
    }
    let colon = colon?;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in head.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                current.push(c);
            }
            ';' if !quoted => parts.push(std::mem::take(&mut current)),
            c => current.push(c),
        }
    }
    parts.push(current);
    let name = parts.first()?.trim().to_ascii_uppercase();
    if name.is_empty() {
        return None;
    }
    let params = parts[1..]
        .iter()
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            Some((k.trim().to_ascii_uppercase(), v.trim().trim_matches('"').to_owned()))
        })
        .collect();
    Some(Property { name, params, value: value.to_owned() })
}

/// Parses all top-level components (usually one `VCALENDAR`).
pub fn parse(text: &str) -> Result<Vec<Component>> {
    let mut stack: Vec<Component> = Vec::new();
    let mut out = Vec::new();
    for line in unfold(text) {
        let Some(prop) = parse_line(&line) else { continue };
        match prop.name.as_str() {
            "BEGIN" => stack.push(Component::new(prop.value.trim())),
            "END" => {
                let Some(done) = stack.pop() else { bail!("END:{} ohne BEGIN", prop.value) };
                if !done.name.eq_ignore_ascii_case(prop.value.trim()) {
                    bail!("END:{} passt nicht zu BEGIN:{}", prop.value, done.name);
                }
                match stack.last_mut() {
                    Some(parent) => parent.children.push(done),
                    None => out.push(done),
                }
            }
            _ => {
                // Lines outside of components are ignored.
                if let Some(c) = stack.last_mut() {
                    c.props.push(prop);
                }
            }
        }
    }
    if let Some(open) = stack.last() {
        bail!("BEGIN:{} ohne END", open.name);
    }
    Ok(out)
}

// ------------------------------------------------------------------- writing

fn fold(line: &str, out: &mut String) {
    // At most 75 octets per line, never splitting a UTF-8 character.
    let mut width = 0;
    for c in line.chars() {
        let len = c.len_utf8();
        if width + len > 75 {
            out.push_str("\r\n ");
            width = 1;
        }
        out.push(c);
        width += len;
    }
    out.push_str("\r\n");
}

fn param_value(v: &str) -> String {
    if v.contains([';', ':', ',']) { format!("\"{v}\"") } else { v.to_owned() }
}

fn write_component(c: &Component, out: &mut String) {
    fold(&format!("BEGIN:{}", c.name), out);
    for p in &c.props {
        let mut line = p.name.clone();
        for (k, v) in &p.params {
            line.push(';');
            line.push_str(k);
            line.push('=');
            line.push_str(&param_value(v));
        }
        line.push(':');
        line.push_str(&p.value);
        fold(&line, out);
    }
    for child in &c.children {
        write_component(child, out);
    }
    fold(&format!("END:{}", c.name), out);
}

pub fn serialize(components: &[Component]) -> String {
    let mut out = String::new();
    for c in components {
        write_component(c, &mut out);
    }
    out
}

/// A VCALENDAR wrapping `children`.
pub fn calendar(children: Vec<Component>) -> Component {
    let mut cal = Component::new("VCALENDAR");
    cal.push(Property::new("VERSION", "2.0"));
    cal.push(Property::new("PRODID", "-//Atlas Workspace//Atlas//DE"));
    cal.push(Property::new("CALSCALE", "GREGORIAN"));
    cal.children = children;
    cal
}

// ------------------------------------------------------------- date and time

/// A point in time as written in iCalendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Time {
    /// All-day (`VALUE=DATE`).
    Date(NaiveDate),
    /// Local time without zone ("floating").
    Floating(NaiveDateTime),
    Utc(DateTime<Utc>),
    /// Local time in a named zone.
    Zoned(NaiveDateTime, Tz),
}

impl Time {
    pub fn is_date(&self) -> bool {
        matches!(self, Time::Date(_))
    }

    /// The time in zone `tz` (dates start at midnight).
    pub fn local(&self, tz: Tz) -> NaiveDateTime {
        match *self {
            Time::Date(d) => d.and_time(NaiveTime::MIN),
            Time::Floating(t) => t,
            Time::Utc(t) => t.with_timezone(&tz).naive_local(),
            Time::Zoned(t, zone) => match zone.from_local_datetime(&t).earliest() {
                Some(dt) => dt.with_timezone(&tz).naive_local(),
                None => t,
            },
        }
    }

    pub fn utc(&self, tz: Tz) -> DateTime<Utc> {
        match *self {
            Time::Utc(t) => t,
            Time::Zoned(t, zone) => zone
                .from_local_datetime(&t)
                .earliest()
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|| Utc.from_utc_datetime(&t)),
            other => {
                let local = other.local(tz);
                tz.from_local_datetime(&local)
                    .earliest()
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|| Utc.from_utc_datetime(&local))
            }
        }
    }

    /// The same kind of time, shifted to `local` (in zone `tz`).
    pub fn with_local(&self, local: NaiveDateTime, tz: Tz) -> Time {
        match *self {
            Time::Date(_) => Time::Date(local.date()),
            Time::Floating(_) => Time::Floating(local),
            Time::Utc(_) => tz
                .from_local_datetime(&local)
                .earliest()
                .map(|d| Time::Utc(d.with_timezone(&Utc)))
                .unwrap_or(Time::Floating(local)),
            Time::Zoned(_, zone) => {
                let converted = tz.from_local_datetime(&local).earliest().map(|d| d.with_timezone(&zone).naive_local());
                Time::Zoned(converted.unwrap_or(local), zone)
            }
        }
    }
}

const DATE: &str = "%Y%m%d";
const DATE_TIME: &str = "%Y%m%dT%H%M%S";

pub fn parse_time(prop: &Property) -> Option<Time> {
    let value = prop.value.trim();
    if prop.param("VALUE").is_some_and(|v| v.eq_ignore_ascii_case("DATE")) || value.len() == 8 {
        return NaiveDate::parse_from_str(value, DATE).ok().map(Time::Date);
    }
    if let Some(utc) = value.strip_suffix('Z') {
        return NaiveDateTime::parse_from_str(utc, DATE_TIME).ok().map(|t| Time::Utc(Utc.from_utc_datetime(&t)));
    }
    let local = NaiveDateTime::parse_from_str(value, DATE_TIME).ok()?;
    match prop.param("TZID").map(|z| z.trim_start_matches('/')) {
        Some(zone) => match Tz::from_str(zone) {
            Ok(tz) => Some(Time::Zoned(local, tz)),
            // Unknown zone names (e.g. Windows names): treat as local time.
            Err(_) => Some(Time::Floating(local)),
        },
        None => Some(Time::Floating(local)),
    }
}

pub fn time_property(name: &str, time: &Time) -> Property {
    match time {
        Time::Date(d) => Property::new(name, d.format(DATE).to_string()).with_param("VALUE", "DATE"),
        Time::Floating(t) => Property::new(name, t.format(DATE_TIME).to_string()),
        Time::Utc(t) => Property::new(name, format!("{}Z", t.format(DATE_TIME))),
        Time::Zoned(t, tz) => Property::new(name, t.format(DATE_TIME).to_string()).with_param("TZID", tz.name()),
    }
}

/// `DURATION` / `TRIGGER` values (`-PT15M`, `P1D`, `PT1H30M`) in seconds.
pub fn parse_duration(value: &str) -> Option<i64> {
    let value = value.trim();
    let (sign, rest) = match value.strip_prefix('-') {
        Some(r) => (-1, r),
        None => (1, value.strip_prefix('+').unwrap_or(value)),
    };
    let rest = rest.strip_prefix('P')?;
    let (date, time) = rest.split_once('T').unwrap_or((rest, ""));
    let mut total = 0i64;
    let mut number = String::new();
    for (part, units) in [(date, [('W', 604_800), ('D', 86_400), ('?', 0)]), (time, [('H', 3600), ('M', 60), ('S', 1)])]
    {
        for c in part.chars() {
            if c.is_ascii_digit() {
                number.push(c);
            } else {
                let unit = units.iter().find(|(u, _)| *u == c)?.1;
                total += number.parse::<i64>().ok()? * unit;
                number.clear();
            }
        }
    }
    number.is_empty().then_some(sign * total)
}

pub fn format_duration(seconds: i64) -> String {
    let sign = if seconds < 0 { "-" } else { "" };
    let s = seconds.abs();
    if s == 0 {
        return "PT0S".into();
    }
    let (days, rest) = (s / 86_400, s % 86_400);
    let (h, m, sec) = (rest / 3600, rest % 3600 / 60, rest % 60);
    let mut out = format!("{sign}P");
    if days > 0 {
        out += &format!("{days}D");
    }
    if rest > 0 {
        out.push('T');
        if h > 0 {
            out += &format!("{h}H");
        }
        if m > 0 {
            out += &format!("{m}M");
        }
        if sec > 0 {
            out += &format!("{sec}S");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Nextcloud//DE\r\nBEGIN:VEVENT\r\nUID:abc@example\r\n\
        DTSTART;TZID=Europe/Berlin:20261017T100000\r\nDTEND;TZID=Europe/Berlin:20261017T150000\r\n\
        SUMMARY:Kapitel 3\\, Teil 2\r\nDESCRIPTION:Zeile eins\\nZeile zwei mit einer sehr langen Beschreibung\r\n  die umbrochen wurde\r\n\
        CATEGORIES:Schreiben,Projekt\\,X\r\nX-NEXTCLOUD-FOO;X-PARAM=\"a:b;c\":wert\r\n\
        BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn parses_and_round_trips() {
        let cals = parse(SAMPLE).unwrap();
        assert_eq!(cals.len(), 1);
        let ev = &cals[0].children[0];
        assert_eq!(ev.name, "VEVENT");
        assert_eq!(ev.text("SUMMARY").unwrap(), "Kapitel 3, Teil 2");
        assert_eq!(
            ev.text("DESCRIPTION").unwrap(),
            "Zeile eins\nZeile zwei mit einer sehr langen Beschreibung die umbrochen wurde"
        );
        assert_eq!(ev.get("CATEGORIES").unwrap().list(), ["Schreiben", "Projekt,X"]);
        assert_eq!(ev.get("X-NEXTCLOUD-FOO").unwrap().param("X-PARAM"), Some("a:b;c"));
        assert_eq!(ev.children[0].text("TRIGGER").unwrap(), "-PT15M");

        let start = parse_time(ev.get("DTSTART").unwrap()).unwrap();
        assert_eq!(
            start,
            Time::Zoned(
                NaiveDate::from_ymd_opt(2026, 10, 17).unwrap().and_hms_opt(10, 0, 0).unwrap(),
                chrono_tz::Europe::Berlin
            )
        );
        assert_eq!(start.utc(chrono_tz::UTC).format("%H:%M").to_string(), "08:00", "CEST is UTC+2");
        assert_eq!(start.local(chrono_tz::America::New_York).format("%H:%M").to_string(), "04:00");

        // Writing and reading again gives the same structure; lines are folded.
        let text = serialize(&cals);
        assert!(text.lines().all(|l| l.len() <= 76), "{text}");
        assert_eq!(parse(&text).unwrap(), cals);
    }

    #[test]
    fn times_and_durations() {
        let p = Property::new("DTSTART", "20261020").with_param("VALUE", "DATE");
        assert!(parse_time(&p).unwrap().is_date());
        let p = Property::new("DTSTART", "20261020T081500Z");
        assert!(matches!(parse_time(&p), Some(Time::Utc(_))));
        let p = Property::new("DTSTART", "20261020T081500").with_param("TZID", "W. Europe Standard Time");
        assert!(matches!(parse_time(&p), Some(Time::Floating(_))), "unknown zones are local");
        assert_eq!(parse_duration("-PT15M"), Some(-900));
        assert_eq!(parse_duration("P1DT2H"), Some(93_600));
        assert_eq!(parse_duration("P2W"), Some(1_209_600));
        assert_eq!(parse_duration("PT"), Some(0));
        assert_eq!(parse_duration("15M"), None);
        assert_eq!(format_duration(-900), "-PT15M");
        assert_eq!(format_duration(93_600), "P1DT2H");
        let t = time_property(
            "DTSTART",
            &Time::Zoned(
                NaiveDate::from_ymd_opt(2026, 1, 2).unwrap().and_hms_opt(3, 4, 5).unwrap(),
                chrono_tz::Europe::Berlin,
            ),
        );
        assert_eq!((t.value.as_str(), t.param("TZID")), ("20260102T030405", Some("Europe/Berlin")));
        assert!(parse("BEGIN:VEVENT\nEND:VTODO\n").is_err());
    }

    #[test]
    fn folding_keeps_utf8_characters_whole() {
        let mut c = Component::new("VEVENT");
        c.push(Property::text("SUMMARY", &"Größenänderung ".repeat(20)));
        let text = serialize(&[c.clone()]);
        assert!(text.lines().all(|l| l.len() <= 76));
        assert_eq!(parse(&text).unwrap()[0], c);
    }
}
