// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Recurrence rules (RRULE) for showing repeating entries.
//!
//! Supported: FREQ=DAILY/WEEKLY/MONTHLY/YEARLY with INTERVAL, COUNT, UNTIL,
//! BYDAY (weekdays; with ordinals like `2MO`/`-1FR` in monthly and yearly
//! rules), BYMONTHDAY and BYMONTH. Rules with other parts are marked as not
//! fully supported; Atlas then shows the first occurrence only and leaves
//! the rule untouched.

use chrono::{Datelike, Duration, Months, NaiveDate, NaiveDateTime, Weekday};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl Freq {
    pub fn as_str(self) -> &'static str {
        match self {
            Freq::Daily => "DAILY",
            Freq::Weekly => "WEEKLY",
            Freq::Monthly => "MONTHLY",
            Freq::Yearly => "YEARLY",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub freq: Freq,
    pub interval: u32,
    pub count: Option<u32>,
    /// Last possible start (inclusive), in the entry's local time.
    pub until: Option<NaiveDateTime>,
    /// Weekdays with optional ordinal (`2MO` → (Some(2), Mon)).
    pub by_day: Vec<(Option<i32>, Weekday)>,
    pub by_month_day: Vec<i32>,
    pub by_month: Vec<u32>,
    /// False if the rule uses parts Atlas does not evaluate.
    pub supported: bool,
}

fn weekday(code: &str) -> Option<Weekday> {
    Some(match code {
        "MO" => Weekday::Mon,
        "TU" => Weekday::Tue,
        "WE" => Weekday::Wed,
        "TH" => Weekday::Thu,
        "FR" => Weekday::Fri,
        "SA" => Weekday::Sat,
        "SU" => Weekday::Sun,
        _ => return None,
    })
}

fn weekday_code(day: Weekday) -> &'static str {
    match day {
        Weekday::Mon => "MO",
        Weekday::Tue => "TU",
        Weekday::Wed => "WE",
        Weekday::Thu => "TH",
        Weekday::Fri => "FR",
        Weekday::Sat => "SA",
        Weekday::Sun => "SU",
    }
}

impl Rule {
    pub fn new(freq: Freq) -> Self {
        Self {
            freq,
            interval: 1,
            count: None,
            until: None,
            by_day: Vec::new(),
            by_month_day: Vec::new(),
            by_month: Vec::new(),
            supported: true,
        }
    }

    /// Parses an RRULE value; `None` if it is not a rule at all.
    pub fn parse(value: &str) -> Option<Rule> {
        let mut rule: Option<Rule> = None;
        let mut rest = Vec::new();
        for part in value.split(';').filter(|p| !p.is_empty()) {
            let (key, val) = part.split_once('=')?;
            if key.eq_ignore_ascii_case("FREQ") {
                let freq = match val.to_ascii_uppercase().as_str() {
                    "DAILY" => Freq::Daily,
                    "WEEKLY" => Freq::Weekly,
                    "MONTHLY" => Freq::Monthly,
                    "YEARLY" => Freq::Yearly,
                    _ => {
                        let mut r = Rule::new(Freq::Daily);
                        r.supported = false;
                        rule = Some(r);
                        continue;
                    }
                };
                rule = Some(Rule::new(freq));
            } else {
                rest.push((key.to_ascii_uppercase(), val.to_owned()));
            }
        }
        let mut rule = rule?;
        for (key, val) in rest {
            match key.as_str() {
                "INTERVAL" => rule.interval = val.parse().ok().filter(|i| *i > 0).unwrap_or(1),
                "COUNT" => rule.count = val.parse().ok(),
                "UNTIL" => {
                    let v = val.trim_end_matches('Z');
                    rule.until = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok().or_else(|| {
                        NaiveDate::parse_from_str(v, "%Y%m%d").ok().and_then(|d| d.and_hms_opt(23, 59, 59))
                    });
                }
                "BYDAY" => {
                    for item in val.split(',') {
                        let item = item.trim().to_ascii_uppercase();
                        let (num, code) = item.split_at(item.len().saturating_sub(2));
                        match (weekday(code), if num.is_empty() { None } else { num.parse().ok() }) {
                            (Some(day), ordinal) if num.is_empty() || ordinal.is_some() => {
                                rule.by_day.push((ordinal, day))
                            }
                            _ => rule.supported = false,
                        }
                    }
                }
                "BYMONTHDAY" => {
                    for d in val.split(',') {
                        match d.trim().parse::<i32>() {
                            Ok(d) if d != 0 && d.abs() <= 31 => rule.by_month_day.push(d),
                            _ => rule.supported = false,
                        }
                    }
                }
                "BYMONTH" => {
                    for m in val.split(',') {
                        match m.trim().parse::<u32>() {
                            Ok(m) if (1..=12).contains(&m) => rule.by_month.push(m),
                            _ => rule.supported = false,
                        }
                    }
                }
                "WKST" => {}
                _ => rule.supported = false,
            }
        }
        if rule.freq == Freq::Weekly && rule.by_day.iter().any(|(n, _)| n.is_some()) {
            rule.supported = false;
        }
        Some(rule)
    }

    /// The rule as RRULE value (for rules Atlas creates or edits).
    pub fn to_value(&self) -> String {
        let mut out = format!("FREQ={}", self.freq.as_str());
        if self.interval > 1 {
            out += &format!(";INTERVAL={}", self.interval);
        }
        if let Some(c) = self.count {
            out += &format!(";COUNT={c}");
        } else if let Some(u) = self.until {
            out += &format!(";UNTIL={}", u.format("%Y%m%dT%H%M%S"));
        }
        if !self.by_day.is_empty() {
            let days: Vec<String> = self
                .by_day
                .iter()
                .map(|(n, d)| format!("{}{}", n.map(|n| n.to_string()).unwrap_or_default(), weekday_code(*d)))
                .collect();
            out += &format!(";BYDAY={}", days.join(","));
        }
        if !self.by_month_day.is_empty() {
            out +=
                &format!(";BYMONTHDAY={}", self.by_month_day.iter().map(i32::to_string).collect::<Vec<_>>().join(","));
        }
        if !self.by_month.is_empty() {
            out += &format!(";BYMONTH={}", self.by_month.iter().map(u32::to_string).collect::<Vec<_>>().join(","));
        }
        out
    }

    /// Short German description for the user.
    pub fn describe(&self) -> String {
        let every = |one: &str, many: &str| {
            if self.interval == 1 { one.to_owned() } else { format!("alle {} {many}", self.interval) }
        };
        let mut out = match self.freq {
            Freq::Daily => every("täglich", "Tage"),
            Freq::Weekly => every("wöchentlich", "Wochen"),
            Freq::Monthly => every("monatlich", "Monate"),
            Freq::Yearly => every("jährlich", "Jahre"),
        };
        if let Some(c) = self.count {
            out += &format!(", {c}-mal");
        } else if let Some(u) = self.until {
            out += &format!(", bis {}", u.format("%d.%m.%Y"));
        }
        if !self.supported {
            out += " (vereinfacht angezeigt)";
        }
        out
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or_default();
    let next = first.checked_add_months(Months::new(1)).unwrap_or(first);
    (next - first).num_days() as u32
}

/// Days of a month matching a monthly/yearly BYDAY/BYMONTHDAY selection.
fn month_days(rule: &Rule, year: i32, month: u32, default_day: u32) -> Vec<NaiveDate> {
    let len = days_in_month(year, month);
    let mut days: Vec<NaiveDate> = Vec::new();
    for d in &rule.by_month_day {
        let day = if *d > 0 { *d as u32 } else { (len as i32 + d + 1).max(0) as u32 };
        if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
            days.push(date);
        }
    }
    for (ordinal, wd) in &rule.by_day {
        let matching: Vec<NaiveDate> =
            (1..=len).filter_map(|d| NaiveDate::from_ymd_opt(year, month, d)).filter(|d| d.weekday() == *wd).collect();
        match ordinal {
            None => days.extend(matching),
            Some(n) if *n > 0 => days.extend(matching.get(*n as usize - 1)),
            Some(n) => days.extend(matching.len().checked_sub(n.unsigned_abs() as usize).and_then(|i| matching.get(i))),
        }
    }
    if rule.by_day.is_empty() && rule.by_month_day.is_empty() {
        days.extend(NaiveDate::from_ymd_opt(year, month, default_day));
    }
    days.sort();
    days.dedup();
    days
}

/// Starts of all occurrences of a series beginning at `start` (local time)
/// that begin in `from..to`. Unsupported rules give the first occurrence.
pub fn occurrences(rule: &Rule, start: NaiveDateTime, from: NaiveDateTime, to: NaiveDateTime) -> Vec<NaiveDateTime> {
    const MAX_STEPS: u32 = 20_000;
    let mut out = Vec::new();
    if !rule.supported {
        if start >= from && start < to {
            out.push(start);
        }
        return out;
    }
    let time = start.time();
    let mut emitted = 0u32;
    // Emits candidates in order; returns false when the series is over.
    let mut emit = |t: NaiveDateTime, out: &mut Vec<NaiveDateTime>| -> bool {
        if t < start {
            return true;
        }
        if rule.until.is_some_and(|u| t > u) || rule.count.is_some_and(|c| emitted >= c) || t >= to {
            return false;
        }
        emitted += 1;
        if t >= from {
            out.push(t);
        }
        true
    };
    let interval = rule.interval.max(1);
    for step in 0..MAX_STEPS {
        let n = step * interval;
        let candidates: Vec<NaiveDateTime> = match rule.freq {
            Freq::Daily => vec![start + Duration::days(n as i64)],
            Freq::Weekly => {
                if rule.by_day.is_empty() {
                    vec![start + Duration::weeks(n as i64)]
                } else {
                    let monday = start.date() - Duration::days(start.weekday().num_days_from_monday() as i64)
                        + Duration::weeks(n as i64);
                    let mut days: Vec<NaiveDate> = rule
                        .by_day
                        .iter()
                        .map(|(_, d)| monday + Duration::days(d.num_days_from_monday() as i64))
                        .collect();
                    days.sort();
                    days.into_iter().map(|d| d.and_time(time)).collect()
                }
            }
            Freq::Monthly => {
                let Some(month_start) = start.date().with_day(1).and_then(|d| d.checked_add_months(Months::new(n)))
                else {
                    break;
                };
                if !rule.by_month.is_empty() && !rule.by_month.contains(&month_start.month()) {
                    continue;
                }
                month_days(rule, month_start.year(), month_start.month(), start.day())
                    .into_iter()
                    .map(|d| d.and_time(time))
                    .collect()
            }
            Freq::Yearly => {
                let year = start.year() + n as i32;
                let months = if rule.by_month.is_empty() { vec![start.month()] } else { rule.by_month.clone() };
                let mut days = Vec::new();
                for m in months {
                    if rule.by_day.is_empty() && rule.by_month_day.is_empty() {
                        days.extend(NaiveDate::from_ymd_opt(year, m, start.day()));
                    } else {
                        days.extend(month_days(rule, year, m, start.day()));
                    }
                }
                days.sort();
                days.into_iter().map(|d| d.and_time(time)).collect()
            }
        };
        for t in candidates {
            if !emit(t, &mut out) {
                return out;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    fn days(v: &[NaiveDateTime]) -> Vec<String> {
        v.iter().map(|t| t.format("%Y-%m-%d").to_string()).collect()
    }

    #[test]
    fn weekly_with_days_count_and_until() {
        let r = Rule::parse("FREQ=WEEKLY;BYDAY=MO,WE;COUNT=5").unwrap();
        assert!(r.supported);
        let start = dt("2026-10-19 09:00"); // Monday
        let all = occurrences(&r, start, dt("2026-01-01 00:00"), dt("2027-01-01 00:00"));
        assert_eq!(days(&all), ["2026-10-19", "2026-10-21", "2026-10-26", "2026-10-28", "2026-11-02"]);
        assert!(all.iter().all(|t| t.format("%H:%M").to_string() == "09:00"));
        // Only the visible range, but COUNT still counts from the start.
        let week = occurrences(&r, start, dt("2026-10-26 00:00"), dt("2026-11-02 00:00"));
        assert_eq!(days(&week), ["2026-10-26", "2026-10-28"]);

        let r = Rule::parse("FREQ=WEEKLY;INTERVAL=2;UNTIL=20261116T235959Z").unwrap();
        let all = occurrences(&r, start, dt("2026-01-01 00:00"), dt("2027-01-01 00:00"));
        assert_eq!(days(&all), ["2026-10-19", "2026-11-02", "2026-11-16"]);
        assert_eq!(r.to_value(), "FREQ=WEEKLY;INTERVAL=2;UNTIL=20261116T235959");
    }

    #[test]
    fn monthly_and_yearly() {
        let all = |rule: &str, start: &str, n: usize| {
            let r = Rule::parse(rule).unwrap();
            days(
                &occurrences(&r, dt(start), dt("2000-01-01 00:00"), dt("2100-01-01 00:00"))
                    .into_iter()
                    .take(n)
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            all("FREQ=MONTHLY", "2026-01-31 10:00", 3),
            ["2026-01-31", "2026-03-31", "2026-05-31"],
            "months without a 31st are skipped"
        );
        assert_eq!(all("FREQ=MONTHLY;BYDAY=-1FR", "2026-10-30 10:00", 3), ["2026-10-30", "2026-11-27", "2026-12-25"]);
        assert_eq!(all("FREQ=MONTHLY;BYDAY=2MO", "2026-10-12 10:00", 2), ["2026-10-12", "2026-11-09"]);
        assert_eq!(
            all("FREQ=MONTHLY;BYMONTHDAY=1,-1", "2026-10-01 08:00", 4),
            ["2026-10-01", "2026-10-31", "2026-11-01", "2026-11-30"]
        );
        assert_eq!(all("FREQ=YEARLY", "2024-02-29 08:00", 2), ["2024-02-29", "2028-02-29"]);
        assert_eq!(all("FREQ=YEARLY;BYMONTH=5;BYDAY=2SU", "2026-05-10 08:00", 2), ["2026-05-10", "2027-05-09"]);
        assert_eq!(
            all("FREQ=DAILY;INTERVAL=3;COUNT=3", "2026-10-30 08:00", 9),
            ["2026-10-30", "2026-11-02", "2026-11-05"]
        );
    }

    #[test]
    fn unsupported_rules_show_the_first_occurrence() {
        let r = Rule::parse("FREQ=MONTHLY;BYSETPOS=-1;BYDAY=MO,TU,WE,TH,FR").unwrap();
        assert!(!r.supported);
        assert!(r.describe().contains("vereinfacht"));
        let all = occurrences(&r, dt("2026-10-30 08:00"), dt("2026-01-01 00:00"), dt("2027-01-01 00:00"));
        assert_eq!(all.len(), 1);
        assert!(Rule::parse("INTERVAL=2").is_none());
        assert_eq!(Rule::parse("FREQ=WEEKLY;INTERVAL=2;COUNT=4").unwrap().describe(), "alle 2 Wochen, 4-mal");
    }
}
