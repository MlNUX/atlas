// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Spreadsheets (Excel `.xlsx`/`.xlsm`/`.xlsb`/`.xls`, OpenDocument
//! `.ods`), read only, with calamine. A sheet is read when it is shown:
//! the cells with the text Excel would show (number formats with German
//! separators, dates, percent, currency), the formulas behind them and the
//! styles that make a sheet readable (bold, italic, colours, fill,
//! alignment). The file itself is never written.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use anyhow::{Context, Result};
use calamine::{Data, Reader, SheetVisible, Sheets, open_workbook_auto};

/// File extensions opened as spreadsheets.
pub const EXTENSIONS: &[&str] = &["xlsx", "xlsm", "xlsb", "xls", "ods"];

/// Cells of one sheet read at most; rows below are left out (`truncated`).
pub const MAX_CELLS: usize = 4_000_000;

pub fn is_spreadsheet(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Text tables (`.csv`, `.tsv`, `.tab`): text files, shown as a table on
/// request ("Als Tabelle anzeigen").
pub const DELIMITED: &[&str] = &["csv", "tsv", "tab"];

pub fn is_delimited(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| DELIMITED.contains(&e.to_ascii_lowercase().as_str()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetInfo {
    pub name: String,
    /// Hidden in Excel (shown last, marked).
    pub hidden: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellKind {
    #[default]
    Empty,
    Text,
    Number,
    Date,
    Bool,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Align {
    /// By kind: numbers and dates right, truth values and errors centred,
    /// text left.
    #[default]
    Auto,
    Left,
    Center,
    Right,
}

/// What a cell looks like (colours as `0xRRGGBB`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct CellStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub color: Option<u32>,
    pub fill: Option<u32>,
    pub align: Align,
    pub wrap: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cell {
    /// As Excel shows it.
    pub text: String,
    pub kind: CellKind,
    /// Index into [`SheetData::styles`] (0: no style of its own).
    pub style: u32,
}

#[derive(Debug, Clone, Default)]
pub struct SheetData {
    pub name: String,
    /// Rows and columns from A1 to the last cell with a value or style.
    pub rows: u32,
    pub cols: u32,
    /// Row by row; a row holds cells up to its last one.
    pub cells: Vec<Vec<Cell>>,
    /// Styles used; 0 is the plain one.
    pub styles: Vec<CellStyle>,
    /// Formulas (without "="), by (row, column).
    pub formulas: HashMap<(u32, u32), String>,
    /// More than [`MAX_CELLS`]: the rows below were left out.
    pub truncated: bool,
    /// How a text table was read, e.g. "Trennzeichen „;“ · Windows-1252".
    pub info: String,
}

impl SheetData {
    pub fn cell(&self, row: u32, col: u32) -> Option<&Cell> {
        self.cells.get(row as usize)?.get(col as usize)
    }
    pub fn style(&self, cell: &Cell) -> CellStyle {
        self.styles.get(cell.style as usize).copied().unwrap_or_default()
    }
}

fn open(path: &Path) -> Result<Sheets<std::io::BufReader<std::fs::File>>> {
    open_workbook_auto(path).with_context(|| format!("{} ist keine lesbare Tabelle", path.display()))
}

/// The sheets of the workbook at `path`, in their order.
pub fn sheets(path: &Path) -> Result<Vec<SheetInfo>> {
    if is_delimited(path) {
        return Ok(vec![SheetInfo { name: sheet_name_of(path), hidden: false }]);
    }
    let book = open(path)?;
    Ok(book
        .sheets_metadata()
        .iter()
        .map(|s| SheetInfo { name: s.name.clone(), hidden: s.visible != SheetVisible::Visible })
        .collect())
}

/// Column name of 0-based `col`: A … Z, AA … ZZ, AAA …
pub fn column_name(col: u32) -> String {
    let mut n = col as u64 + 1;
    let mut out = Vec::new();
    while n > 0 {
        let rem = ((n - 1) % 26) as u8;
        out.push((b'A' + rem) as char);
        n = (n - 1) / 26;
    }
    out.iter().rev().collect()
}

/// The address of a cell, e.g. `B5` (0-based row and column).
pub fn cell_name(row: u32, col: u32) -> String {
    format!("{}{}", column_name(col), row + 1)
}

fn rgb(color: &calamine::Color) -> u32 {
    color.to_argb() & 0x00ff_ffff
}

fn cell_style(style: &calamine::Style) -> CellStyle {
    use calamine::{FillPattern, HorizontalAlignment as H};
    let font = style.font.as_ref();
    let fill = style
        .fill
        .as_ref()
        .filter(|f| f.pattern != FillPattern::None && f.is_visible())
        .and_then(|f| f.get_color())
        .filter(|c| c.alpha != 0 || c.red != 0 || c.green != 0 || c.blue != 0)
        .map(|c| rgb(&c));
    let align = style.alignment.as_ref();
    CellStyle {
        bold: font.is_some_and(calamine::Font::is_bold),
        italic: font.is_some_and(calamine::Font::is_italic),
        underline: font.is_some_and(calamine::Font::has_underline),
        strike: font.is_some_and(calamine::Font::has_strikethrough),
        // Plain black is the default text colour: left to the theme.
        color: font.and_then(|f| f.color.as_ref()).map(rgb).filter(|&c| c != 0),
        fill: fill.filter(|&c| c != 0xffffff),
        align: match align.map(|a| a.horizontal) {
            Some(H::Left) => Align::Left,
            Some(H::Center | H::CenterContinuous) => Align::Center,
            Some(H::Right) => Align::Right,
            _ => Align::Auto,
        },
        wrap: align.is_some_and(|a| a.wrap_text),
    }
}

/// Sheet `index` of the workbook at `path`.
pub fn load_sheet(path: &Path, index: usize) -> Result<SheetData> {
    if is_delimited(path) {
        anyhow::ensure!(index == 0, "Dieses Blatt gibt es nicht");
        return load_delimited(path);
    }
    let mut book = open(path)?;
    let name = book.sheet_names().get(index).cloned().context("Dieses Blatt gibt es nicht")?;
    let values = book.worksheet_range(&name).with_context(|| format!("Blatt „{name}“ nicht lesbar"))?;
    // Styles and formulas are extras: a file without them still opens.
    let styles = book.worksheet_style(&name).ok();
    let formulas = book.worksheet_formula(&name).ok();

    let mut rows = values.end().map_or(0, |(r, _)| r + 1);
    let mut cols = values.end().map_or(0, |(_, c)| c + 1);
    if let Some((r, c)) = styles.as_ref().and_then(|s| s.end()) {
        // Styled cells (a filled header without text) count too, within reason.
        rows = rows.max((r + 1).min(rows + 200));
        cols = cols.max((c + 1).min(cols + 50));
    }
    let mut truncated = false;
    if rows as usize * cols.max(1) as usize > MAX_CELLS {
        rows = (MAX_CELLS / cols.max(1) as usize) as u32;
        truncated = true;
    }

    let mut palette: Vec<CellStyle> = vec![CellStyle::default()];
    let mut ids: HashMap<CellStyle, u32> = HashMap::from([(CellStyle::default(), 0)]);
    let mut cells: Vec<Vec<Cell>> = Vec::with_capacity(rows as usize);
    for r in 0..rows {
        let mut row: Vec<Cell> = Vec::new();
        for c in 0..cols {
            let style = styles.as_ref().and_then(|s| s.get((r, c)));
            let format = style.and_then(|s| s.number_format.as_ref()).map(|f| f.format_code.as_str()).unwrap_or("");
            let value = values.get_value((r, c)).unwrap_or(&Data::Empty);
            let (text, kind) = display(value, format);
            let look = style.map(cell_style).unwrap_or_default();
            let id = *ids.entry(look).or_insert_with(|| {
                palette.push(look);
                (palette.len() - 1) as u32
            });
            if kind != CellKind::Empty || id != 0 {
                row.resize(c as usize, Cell::default());
                row.push(Cell { text, kind, style: id });
            }
        }
        cells.push(row);
    }
    let mut formula_map = HashMap::new();
    if let Some(f) = formulas
        && let Some((r0, c0)) = f.start()
    {
        for (r, c, text) in f.cells() {
            if !text.is_empty() {
                formula_map.insert((r0 + r as u32, c0 + c as u32), text.clone());
            }
        }
    }
    Ok(SheetData { name, rows, cols, cells, styles: palette, formulas: formula_map, truncated, info: String::new() })
}

// ---------------------------------------------------------------- text tables

/// Largest text table read.
const MAX_DELIMITED: u64 = 256 * 1024 * 1024;

fn sheet_name_of(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The text of a file: UTF-8 (with or without BOM, also UTF-16 with BOM),
/// else Windows-1252 as German Excel writes CSV.
fn decode(bytes: &[u8]) -> (String, &'static str) {
    if let Some((encoding, _)) = encoding_rs::Encoding::for_bom(bytes) {
        let (text, _, _) = encoding.decode(bytes);
        return (text.into_owned(), encoding.name());
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => (text.to_owned(), "UTF-8"),
        Err(_) => (encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(), "Windows-1252"),
    }
}

/// The separator of a text table: the one that splits the first lines into
/// the same number of fields most often (tab for `.tsv`).
fn detect_delimiter(text: &str, path: &Path) -> char {
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if ext == "tsv" || ext == "tab" {
        return '\t';
    }
    let sample: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(30).collect();
    let mut best = (',', 0usize, 0usize);
    for candidate in [';', ',', '\t', '|'] {
        let counts: Vec<usize> = sample.iter().map(|l| split_line(l, candidate).len()).collect();
        let Some(&first) = counts.first() else { continue };
        if first < 2 {
            continue;
        }
        let same = counts.iter().filter(|&&c| c == first).count();
        if (same, first) > (best.1, best.2) {
            best = (candidate, same, first);
        }
    }
    best.0
}

/// The fields of one line (for the detection; quotes respected).
fn split_line(line: &str, delimiter: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut quoted = false;
    let mut start = 0;
    for (i, ch) in line.char_indices() {
        if ch == '"' {
            quoted = !quoted;
        } else if ch == delimiter && !quoted {
            out.push(&line[start..i]);
            start = i + ch.len_utf8();
        }
    }
    out.push(&line[start..]);
    out
}

/// Records of a text table: fields in double quotes may hold separators,
/// line breaks and doubled quotes ("" for ").
fn parse_records(text: &str, delimiter: char, max_cells: usize) -> (Vec<Vec<String>>, bool) {
    let mut records: Vec<Vec<String>> = Vec::new();
    let mut record: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut cells = 0usize;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else if ch != '\r' {
                // (Line breaks in a field as \n, also from Windows files.)
                field.push(ch);
            }
            continue;
        }
        match ch {
            '"' if field.is_empty() => quoted = true,
            c if c == delimiter => record.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                record.push(std::mem::take(&mut field));
                cells += record.len();
                records.push(std::mem::take(&mut record));
                if cells > max_cells {
                    return (records, true);
                }
            }
            c => field.push(c),
        }
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    (records, false)
}

/// Whether a field is a number ("1234", "-1,5", "1.234,50", "3.14",
/// "1e-3", "25%", "12,50 €"): shown right-aligned, as written.
fn looks_numeric(text: &str) -> bool {
    let t = text.trim().trim_end_matches(['%', '€', '$']).trim().trim_start_matches(['€', '$']).trim();
    let t = t.strip_prefix(['-', '+']).unwrap_or(t);
    !t.is_empty()
        && t.chars().next().is_some_and(|c| c.is_ascii_digit() || c == '.' || c == ',')
        && t.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | 'e' | 'E' | '-' | '+' | '\''))
        && t.chars().any(|c| c.is_ascii_digit())
}

/// A text table as one sheet: fields as written (numbers right-aligned).
fn load_delimited(path: &Path) -> Result<SheetData> {
    let size = std::fs::metadata(path).with_context(|| format!("{} fehlt", path.display()))?.len();
    anyhow::ensure!(size <= MAX_DELIMITED, "{} ist zu groß für die Tabellenansicht", path.display());
    let bytes = std::fs::read(path).with_context(|| format!("{} ist nicht lesbar", path.display()))?;
    let (text, encoding) = decode(&bytes);
    let delimiter = detect_delimiter(&text, path);
    let (records, truncated) = parse_records(&text, delimiter, MAX_CELLS);
    let cols = records.iter().map(Vec::len).max().unwrap_or(0) as u32;
    let cells: Vec<Vec<Cell>> = records
        .into_iter()
        .map(|record| {
            let mut row: Vec<Cell> = record
                .into_iter()
                .map(|text| {
                    let kind = if text.is_empty() {
                        CellKind::Empty
                    } else if looks_numeric(&text) {
                        CellKind::Number
                    } else {
                        CellKind::Text
                    };
                    Cell { text, kind, style: 0 }
                })
                .collect();
            while row.last().is_some_and(|c| c.kind == CellKind::Empty) {
                row.pop();
            }
            row
        })
        .collect();
    let shown = match delimiter {
        '\t' => "Tabulator".to_owned(),
        c => format!("„{c}“"),
    };
    Ok(SheetData {
        name: sheet_name_of(path),
        rows: cells.len() as u32,
        cols,
        cells,
        styles: vec![CellStyle::default()],
        formulas: HashMap::new(),
        truncated,
        info: format!("Trennzeichen {shown} · {encoding}"),
    })
}

// ---------------------------------------------------------------- display

/// The text of `value` with Excel number format `format` ("" or
/// "General": as Excel shows numbers without a format).
pub fn display(value: &Data, format: &str) -> (String, CellKind) {
    match value {
        Data::Empty => (String::new(), CellKind::Empty),
        Data::String(s) => (s.clone(), CellKind::Text),
        Data::Bool(b) => ((if *b { "WAHR" } else { "FALSCH" }).to_owned(), CellKind::Bool),
        Data::Error(e) => (e.to_string(), CellKind::Error),
        Data::Int(i) => (format_number(*i as f64, format), CellKind::Number),
        Data::Float(f) => (format_number(*f, format), CellKind::Number),
        Data::DateTime(d) => {
            if d.is_duration() {
                (format_duration(d.as_f64()), CellKind::Date)
            } else {
                match d.as_datetime() {
                    Some(dt) => (format_date(dt, format), CellKind::Date),
                    None => (general(d.as_f64()), CellKind::Number),
                }
            }
        }
        Data::DateTimeIso(s) => (iso_date(s), CellKind::Date),
        Data::DurationIso(s) => (s.clone(), CellKind::Date),
    }
}

/// `2026-03-15T10:30:00` → `15.03.2026 10:30`.
fn iso_date(s: &str) -> String {
    let (date, time) = s.split_once('T').unwrap_or((s, ""));
    let parts: Vec<&str> = date.split('-').collect();
    let date = if parts.len() == 3 { format!("{}.{}.{}", parts[2], parts[1], parts[0]) } else { date.to_owned() };
    let time = time.split(['.', 'Z', '+']).next().unwrap_or("");
    let time = time.strip_suffix(":00").filter(|t| t.len() == 5).unwrap_or(time);
    if time.is_empty() || time == "00:00" { date } else { format!("{date} {time}") }
}

fn format_duration(days: f64) -> String {
    let total = (days * 86400.0).round() as i64;
    let (sign, total) = if total < 0 { ("-", -total) } else { ("", total) };
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if s == 0 { format!("{sign}{h}:{m:02}") } else { format!("{sign}{h}:{m:02}:{s:02}") }
}

/// The parts of a format section outside quotes and brackets, lowercase.
fn format_letters(section: &str) -> String {
    let mut out = String::new();
    let mut quoted = false;
    let mut bracket = false;
    let mut escape = false;
    for ch in section.chars() {
        if escape {
            escape = false;
            continue;
        }
        match ch {
            '\\' => escape = true,
            '"' => quoted = !quoted,
            '[' if !quoted => bracket = true,
            ']' if !quoted => bracket = false,
            _ if quoted || bracket => {}
            c => out.push(c.to_ascii_lowercase()),
        }
    }
    out
}

fn format_date(dt: chrono::NaiveDateTime, format: &str) -> String {
    use chrono::Timelike;
    let letters = format_letters(format.split(';').next().unwrap_or(""));
    // A time alone is a date on Excel's day 0 (1899-12-30/31).
    let day_zero = dt.date() < chrono::NaiveDate::from_ymd_opt(1900, 1, 1).expect("date");
    let has_date = letters.contains('y') || letters.contains('d') || (letters.is_empty() && !day_zero);
    let has_time = letters.contains('h')
        || letters.contains('s')
        || (letters.is_empty() && (dt.hour(), dt.minute(), dt.second()) != (0, 0, 0));
    let with_seconds = letters.contains('s');
    let date = dt.format("%d.%m.%Y").to_string();
    let time = if with_seconds { dt.format("%H:%M:%S") } else { dt.format("%H:%M") }.to_string();
    match (has_date, has_time) {
        (true, true) => format!("{date} {time}"),
        (false, true) => time,
        _ => date,
    }
}

/// `value` with Excel number format `format`, German separators.
pub fn format_number(value: f64, format: &str) -> String {
    let format = format.trim();
    if format.is_empty() || format.eq_ignore_ascii_case("general") || format == "@" {
        return general(value);
    }
    let sections: Vec<&str> = split_sections(format);
    let (section, value, sign) = match sections.len() {
        n if n >= 2 && value < 0.0 => (sections[1], -value, ""),
        n if n >= 3 && value == 0.0 => (sections[2], value, ""),
        _ => (sections[0], value.abs(), if value < 0.0 { "-" } else { "" }),
    };
    if section.eq_ignore_ascii_case("general") {
        return format!("{sign}{}", general(value));
    }
    // Literal text, currency symbols, the number pattern.
    let mut prefix = String::new();
    let mut pattern = String::new();
    let mut suffix = String::new();
    let mut chars = section.chars().peekable();
    let mut in_pattern = false;
    let mut after = false;
    let push = |c: char, prefix: &mut String, suffix: &mut String, after: bool| {
        if after { suffix.push(c) } else { prefix.push(c) }
    };
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                for c in chars.by_ref() {
                    if c == '"' {
                        break;
                    }
                    push(c, &mut prefix, &mut suffix, after || in_pattern);
                }
            }
            '\\' => {
                if let Some(c) = chars.next() {
                    push(c, &mut prefix, &mut suffix, after || in_pattern);
                }
            }
            '[' => {
                let mut inner = String::new();
                for c in chars.by_ref() {
                    if c == ']' {
                        break;
                    }
                    inner.push(c);
                }
                // [$€-407] is a currency; [Red], [>100] are not shown.
                if let Some(cur) = inner.strip_prefix('$') {
                    let symbol = cur.split('-').next().unwrap_or("");
                    for c in symbol.chars() {
                        push(c, &mut prefix, &mut suffix, after || in_pattern);
                    }
                }
            }
            '_' => {
                chars.next();
                push(' ', &mut prefix, &mut suffix, after || in_pattern);
            }
            '*' => {
                chars.next();
            }
            '0' | '#' | '?' | '.' | ',' | 'E' | 'e' if !after => {
                if (ch == 'E' || ch == 'e') && !in_pattern {
                    push(ch, &mut prefix, &mut suffix, false);
                    continue;
                }
                in_pattern = true;
                pattern.push(ch);
                if ch == 'E' || ch == 'e' {
                    if let Some(&s) = chars.peek()
                        && (s == '+' || s == '-')
                    {
                        pattern.push(s);
                        chars.next();
                    }
                }
            }
            c => {
                if in_pattern {
                    after = true;
                }
                push(c, &mut prefix, &mut suffix, after);
            }
        }
    }
    if pattern.is_empty() {
        // Only text (e.g. "-" for zero).
        return format!("{prefix}{suffix}");
    }
    let percent = section.chars().filter(|&c| c == '%').count();
    let mut value = value * 100f64.powi(percent as i32);
    // Trailing commas scale by thousands ("#,##0,").
    let mut core = pattern.as_str();
    while core.ends_with(',') {
        value /= 1000.0;
        core = &core[..core.len() - 1];
    }
    let number = if let Some(epos) = core.find(['E', 'e']) {
        let mantissa = &core[..epos];
        let decimals =
            mantissa.split('.').nth(1).map_or(0, |d| d.chars().filter(|c| matches!(c, '0' | '#' | '?')).count());
        let exp_digits = core[epos..].chars().filter(|&c| c == '0').count().max(1);
        let s = format!("{value:.decimals$E}");
        let (m, e) = s.split_once('E').unwrap_or((&s, "0"));
        let e: i32 = e.parse().unwrap_or(0);
        let esign = if e < 0 { "-" } else { "+" };
        format!("{}E{esign}{:0exp_digits$}", m.replace('.', ","), e.abs())
    } else {
        let (int_part, dec_part) = core.split_once('.').unwrap_or((core, ""));
        let required = dec_part.chars().filter(|&c| c == '0').count();
        let decimals = dec_part.chars().filter(|c| matches!(c, '0' | '#' | '?')).count();
        let grouping = int_part.contains(',');
        let min_int = int_part.chars().filter(|&c| c == '0').count();
        let rounded = format!("{value:.decimals$}");
        let (int_digits, mut frac) = match rounded.split_once('.') {
            Some((i, f)) => (i.to_owned(), f.to_owned()),
            None => (rounded.clone(), String::new()),
        };
        while frac.len() > required && frac.ends_with('0') {
            frac.pop();
        }
        let mut int_digits = int_digits.trim_start_matches('0').to_owned();
        while int_digits.len() < min_int {
            int_digits.insert(0, '0');
        }
        if grouping {
            int_digits = group(&int_digits);
        }
        if frac.is_empty() { int_digits } else { format!("{int_digits},{frac}") }
    };
    let sign = if number.chars().all(|c| matches!(c, '0' | ',' | '.')) { "" } else { sign };
    format!("{sign}{prefix}{number}{suffix}").trim().to_owned()
}

fn split_sections(format: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut quoted = false;
    let mut start = 0;
    for (i, ch) in format.char_indices() {
        match ch {
            '"' => quoted = !quoted,
            ';' if !quoted => {
                out.push(&format[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&format[start..]);
    out
}

/// Thousands separators: `1234567` → `1.234.567`.
fn group(digits: &str) -> String {
    let bytes: Vec<char> = digits.chars().collect();
    let mut out = String::new();
    for (i, c) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(*c);
    }
    out
}

/// A number as Excel shows it without a format: up to 10 significant
/// digits, decimal comma, scientific for very large or small values.
fn general(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    let abs = value.abs();
    if value.fract() == 0.0 && abs < 1e11 {
        return format!("{value:.0}");
    }
    if !(1e-9..1e11).contains(&abs) {
        let s = format!("{value:.5E}");
        let (m, e) = s.split_once('E').unwrap_or((&s, "0"));
        let m = m.trim_end_matches('0').trim_end_matches('.');
        let e: i32 = e.parse().unwrap_or(0);
        return format!("{}E{}{:02}", m.replace('.', ","), if e < 0 { "-" } else { "+" }, e.abs());
    }
    let digits = (9 - abs.log10().floor() as i32).clamp(0, 15) as usize;
    let s = format!("{value:.digits$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_owned() } else { s };
    s.replace('.', ",")
}

// ---------------------------------------------------------------- search

/// The cells with text of every sheet: (sheet, name, [(row, col, text)]).
pub type SheetTexts = Vec<(usize, String, Vec<(u32, u32, String)>)>;

/// Largest spreadsheet searched.
const MAX_SEARCH_SIZE: u64 = 100 * 1024 * 1024;

struct TextCache {
    entries: HashMap<PathBuf, ((SystemTime, u64), Arc<SheetTexts>, u64)>,
    clock: u64,
}

fn text_cache() -> &'static Mutex<TextCache> {
    static CACHE: OnceLock<Mutex<TextCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(TextCache { entries: HashMap::new(), clock: 0 }))
}

/// The texts of `path` for the search, cached by modification time and size.
pub fn search_texts(path: &Path) -> Option<Arc<SheetTexts>> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > MAX_SEARCH_SIZE {
        return None;
    }
    let stamp = (meta.modified().ok()?, meta.len());
    {
        let mut cache = text_cache().lock().ok()?;
        cache.clock += 1;
        let clock = cache.clock;
        if let Some((s, texts, used)) = cache.entries.get_mut(path)
            && *s == stamp
        {
            *used = clock;
            return Some(texts.clone());
        }
    }
    let count = sheets(path).ok()?.len();
    let mut texts = Vec::new();
    for index in 0..count {
        let Ok(sheet) = load_sheet(path, index) else { continue };
        let mut cells = Vec::new();
        for (r, row) in sheet.cells.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                if !cell.text.is_empty() {
                    cells.push((r as u32, c as u32, cell.text.clone()));
                }
            }
        }
        texts.push((index, sheet.name, cells));
    }
    let texts = Arc::new(texts);
    let mut cache = text_cache().lock().ok()?;
    if cache.entries.len() >= 64
        && let Some(oldest) = cache.entries.iter().min_by_key(|(_, (_, _, used))| *used).map(|(p, _)| p.clone())
    {
        cache.entries.remove(&oldest);
    }
    let clock = cache.clock;
    cache.entries.insert(path.to_path_buf(), (stamp, texts.clone(), clock));
    Some(texts)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    /// A small workbook written by hand: two sheets, shared and inline
    /// strings, number formats, a date, a formula, styles.
    pub(crate) fn workbook(path: &Path) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        let mut add = |name: &str, text: &str| {
            zip.start_file(name, options).unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        };
        let ct = "application/vnd.openxmlformats-officedocument.spreadsheetml";
        add(
            "[Content_Types].xml",
            &format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/xl/workbook.xml" ContentType="{ct}.sheet.main+xml"/>
<Override PartName="/xl/worksheets/sheet1.xml" ContentType="{ct}.worksheet+xml"/>
<Override PartName="/xl/worksheets/sheet2.xml" ContentType="{ct}.worksheet+xml"/>
<Override PartName="/xl/styles.xml" ContentType="{ct}.styles+xml"/>
<Override PartName="/xl/sharedStrings.xml" ContentType="{ct}.sharedStrings+xml"/></Types>"#
            ),
        );
        let rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
        add(
            "_rels/.rels",
            &format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="{rel}/officeDocument" Target="xl/workbook.xml"/></Relationships>"#
            ),
        );
        add(
            "xl/workbook.xml",
            &format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="{rel}">
<sheets><sheet name="Daten" sheetId="1" r:id="rId1"/><sheet name="Zweites" sheetId="2" state="hidden" r:id="rId2"/></sheets></workbook>"#
            ),
        );
        add(
            "xl/_rels/workbook.xml.rels",
            &format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="{rel}/worksheet" Target="worksheets/sheet1.xml"/>
<Relationship Id="rId2" Type="{rel}/worksheet" Target="worksheets/sheet2.xml"/>
<Relationship Id="rId3" Type="{rel}/styles" Target="styles.xml"/>
<Relationship Id="rId4" Type="{rel}/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#
            ),
        );
        add(
            "xl/styles.xml",
            r##"<?xml version="1.0" encoding="UTF-8"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<numFmts count="2"><numFmt numFmtId="164" formatCode="#,##0.00"/><numFmt numFmtId="165" formatCode="#,##0.00\ [$€-407]"/></numFmts>
<fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><color rgb="FFC00000"/><name val="Calibri"/></font></fonts>
<fills count="3"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill>
<fill><patternFill patternType="solid"><fgColor rgb="FFFFFF00"/><bgColor indexed="64"/></patternFill></fill></fills>
<borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders>
<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>
<cellXfs count="6"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>
<xf numFmtId="0" fontId="1" fillId="2" borderId="0" xfId="0" applyFont="1" applyFill="1"/>
<xf numFmtId="164" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>
<xf numFmtId="9" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>
<xf numFmtId="14" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>
<xf numFmtId="165" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/></cellXfs></styleSheet>"##,
        );
        add(
            "xl/sharedStrings.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="3" uniqueCount="3">
<si><t>Name</t></si><si><t>Wert</t></si><si><t>Alpha</t></si></sst>"#,
        );
        add(
            "xl/worksheets/sheet1.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
<row r="1"><c r="A1" t="s" s="1"><v>0</v></c><c r="B1" t="s" s="1"><v>1</v></c></row>
<row r="2"><c r="A2" t="s"><v>2</v></c><c r="B2" s="2"><v>1234.5</v></c><c r="C2" s="3"><v>0.25</v></c><c r="D2" s="4"><v>45000</v></c>
<c r="E2"><f>B2*2</f><v>2469</v></c><c r="F2" t="b"><v>1</v></c><c r="G2" t="e"><v>#DIV/0!</v></c><c r="H2" s="5"><v>-19.9</v></c></row>
</sheetData></worksheet>"#,
        );
        add(
            "xl/worksheets/sheet2.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
<row r="3"><c r="C3" t="inlineStr"><is><t>Gefunden im zweiten Blatt</t></is></c></row></sheetData></worksheet>"#,
        );
        zip.finish().unwrap();
    }

    #[test]
    fn reads_values_formats_formulas_and_styles() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Mappe.xlsx");
        workbook(&path);
        assert!(is_spreadsheet(&path) && is_spreadsheet(Path::new("a.ODS")) && !is_spreadsheet(Path::new("a.csv")));
        assert_eq!(
            sheets(&path).unwrap(),
            vec![SheetInfo { name: "Daten".into(), hidden: false }, SheetInfo { name: "Zweites".into(), hidden: true }]
        );
        let sheet = load_sheet(&path, 0).unwrap();
        assert_eq!((sheet.name.as_str(), sheet.rows, sheet.cols), ("Daten", 2, 8));
        let text = |r, c| sheet.cell(r, c).map(|c| c.text.clone()).unwrap_or_default();
        assert_eq!(
            (1..8).map(|c| text(1, c)).collect::<Vec<_>>(),
            ["1.234,50", "25%", "15.03.2023", "2469", "WAHR", "#DIV/0!", "-19,90 €"]
        );
        assert_eq!(sheet.cell(1, 3).unwrap().kind, CellKind::Date);
        assert_eq!(sheet.cell(1, 1).unwrap().kind, CellKind::Number);
        assert_eq!(sheet.formulas.get(&(1, 4)).map(String::as_str), Some("B2*2"));
        let header = sheet.style(sheet.cell(0, 0).unwrap());
        assert!(header.bold && header.fill == Some(0xffff00) && header.color == Some(0xc00000), "{header:?}");
        assert_eq!(sheet.style(sheet.cell(1, 0).unwrap()), CellStyle::default());
        let second = load_sheet(&path, 1).unwrap();
        assert_eq!(second.cell(2, 2).unwrap().text, "Gefunden im zweiten Blatt");
        assert!(load_sheet(&path, 5).is_err());
        let texts = search_texts(&path).unwrap();
        assert_eq!(texts[1].2, vec![(2, 2, "Gefunden im zweiten Blatt".to_owned())]);
    }

    #[test]
    fn reads_text_tables_with_their_separator_and_encoding() {
        let dir = tempfile::tempdir().unwrap();
        // German Excel: semicolons, decimal commas, Windows-1252, a quoted field with a line break.
        let path = dir.path().join("Noten.csv");
        let mut bytes = b"Name;Note;Bemerkung\r\nM\xfcller;1,7;\"sehr gut;\r\nzweite Zeile\"\r\nSchmidt;2,3;\"sagt \"\"hallo\"\"\"\r\n".to_vec();
        bytes.extend_from_slice(b"Leer;;\r\n");
        std::fs::write(&path, &bytes).unwrap();
        assert!(is_delimited(&path) && !is_spreadsheet(&path));
        assert_eq!(sheets(&path).unwrap(), vec![SheetInfo { name: "Noten".into(), hidden: false }]);
        let sheet = load_sheet(&path, 0).unwrap();
        assert_eq!((sheet.rows, sheet.cols), (4, 3));
        assert_eq!(sheet.info, "Trennzeichen „;“ · Windows-1252");
        let text = |r, c| sheet.cell(r, c).map(|c| c.text.clone()).unwrap_or_default();
        assert_eq!(text(1, 0), "Müller");
        assert_eq!(text(1, 2), "sehr gut;\nzweite Zeile");
        assert_eq!(text(2, 2), "sagt \"hallo\"");
        assert_eq!(sheet.cell(1, 1).unwrap().kind, CellKind::Number);
        assert_eq!(sheet.cell(1, 0).unwrap().kind, CellKind::Text);
        assert_eq!(sheet.cells[3].len(), 1, "empty fields at the end are dropped");
        // Commas and UTF-8 with BOM; tabs in .tsv.
        let path = dir.path().join("daten.csv");
        std::fs::write(&path, "\u{feff}a,b,c\n1,2.5,x\n").unwrap();
        let sheet = load_sheet(&path, 0).unwrap();
        assert_eq!(
            (sheet.cols, sheet.info.as_str(), sheet.cell(1, 1).unwrap().text.as_str()),
            (3, "Trennzeichen „,“ · UTF-8", "2.5")
        );
        assert_eq!(sheet.cell(0, 0).unwrap().text, "a", "no BOM in the first cell");
        let path = dir.path().join("daten.tsv");
        std::fs::write(&path, "a,1\tb\n").unwrap();
        assert_eq!(load_sheet(&path, 0).unwrap().cell(0, 0).unwrap().text, "a,1");
        assert!(load_sheet(&path, 1).is_err());
        assert!(["12", "-1,5", "1.234,50", "3.14", "1e-3", "25%", "12,50 €"].iter().all(|t| looks_numeric(t)));
        assert!(!["", "abc", "12a", "-", "2026-03-01x", "P-001"].iter().any(|t| looks_numeric(t)));
    }

    #[test]
    fn number_formats_like_excel() {
        assert_eq!(format_number(1234567.891, "#,##0.00"), "1.234.567,89");
        assert_eq!(format_number(0.125, "0.0%"), "12,5%");
        assert_eq!(format_number(-5.0, "0.00;[Red]-0.00"), "-5,00");
        assert_eq!(format_number(-5.0, "0.00;(0.00)"), "(5,00)");
        assert_eq!(format_number(0.0, "0.00;-0.00;\"-\""), "-");
        assert_eq!(format_number(1500.0, "\"€\"#,##0"), "€1.500");
        assert_eq!(format_number(12345.0, "0.00E+00"), "1,23E+04");
        assert_eq!(format_number(7.0, "000"), "007");
        assert_eq!(format_number(1234567.0, "#,##0,"), "1.235");
        assert_eq!(format_number(0.5, "#.##"), ",5");
        assert_eq!(format_number(3.0, "0.##"), "3");
        assert_eq!(general(1234.5), "1234,5");
        assert_eq!(general(1.0 / 3.0), "0,3333333333");
        assert_eq!(general(42.0), "42");
        assert_eq!(general(1.5e20), "1,5E+20");
        let at = |y, mo, d, h, mi| chrono::NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, mi, 0).unwrap();
        assert_eq!(format_date(at(2023, 3, 15, 12, 0), "dd/mm/yyyy hh:mm"), "15.03.2023 12:00");
        assert_eq!(format_date(at(1899, 12, 30, 18, 0), "h:mm"), "18:00");
        assert_eq!(format_date(at(2023, 3, 15, 0, 0), ""), "15.03.2023");
        assert_eq!(format_duration(1.5), "36:00");
        assert_eq!(iso_date("2026-03-15T10:30:00"), "15.03.2026 10:30");
        assert_eq!(
            (column_name(0), column_name(25), column_name(26), column_name(701)),
            ("A".into(), "Z".into(), "AA".into(), "ZZ".into())
        );
        assert_eq!(cell_name(4, 1), "B5");
    }
}
