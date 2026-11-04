// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Editing help for Markdown notes, independent of the editor widget:
//! continuing and indenting lists, Markdown tables with aligned columns
//! (Tab / Shift+Tab from cell to cell, Enter adds a row, aligning one or all
//! tables), ticking tasks and the completion of wiki links (`[[`) and tags
//! (`#`).
//!
//! Positions are **UTF-16 code units**, as in QML strings and `NSString`;
//! the functions return an [`Edit`] the editor applies (replace a range,
//! then set the selection) or `None` when the key should do what it
//! usually does. Edits are kept small (common prefix and suffix left
//! alone) so that undo stays fine-grained.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

/// Replace `start..end` of the old text by `text`, then select from
/// `anchor` to `cursor` (positions in the new text). All UTF-16.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub anchor: usize,
    pub cursor: usize,
}

impl Edit {
    /// The text after applying the edit to `text`.
    pub fn apply(&self, text: &str) -> String {
        let (s, e) = (byte_at(text, self.start), byte_at(text, self.end));
        format!("{}{}{}", &text[..s], self.text, &text[e..])
    }
}

// ---- UTF-16 positions --------------------------------------------------

/// Byte index of UTF-16 position `pos` (clamped; inside a surrogate pair:
/// the start of the character).
pub fn byte_at(text: &str, pos: usize) -> usize {
    let mut units = 0;
    for (i, c) in text.char_indices() {
        if units + c.len_utf16() > pos {
            return i;
        }
        units += c.len_utf16();
    }
    text.len()
}

/// UTF-16 position of byte index `byte`.
pub fn utf16_at(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].encode_utf16().count()
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// The edit that turns `text` into `text[..s] + new + text[e..]` (bytes),
/// reduced to what really changes; `anchor` / `cursor` are byte positions
/// in the new text.
fn edit(text: &str, s: usize, e: usize, new: &str, anchor: usize, cursor: usize) -> Edit {
    let old = &text[s..e];
    let prefix = old.chars().zip(new.chars()).take_while(|(a, b)| a == b).map(|(a, _)| a.len_utf8()).sum::<usize>();
    let suffix = old[prefix..]
        .chars()
        .rev()
        .zip(new[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum::<usize>();
    let after = format!("{}{}{}", &text[..s], new, &text[e..]);
    Edit {
        start: utf16_at(text, s + prefix),
        end: utf16_at(text, e - suffix),
        text: new[prefix..new.len() - suffix].to_owned(),
        anchor: utf16_at(&after, anchor),
        cursor: utf16_at(&after, cursor),
    }
}

fn line_start(text: &str, b: usize) -> usize {
    text[..b].rfind('\n').map_or(0, |i| i + 1)
}

fn line_end(text: &str, b: usize) -> usize {
    text[b..].find('\n').map_or(text.len(), |i| b + i)
}

// ---- lists --------------------------------------------------------------

static LIST_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*([-*+]|\d+[.)])(\s|$)").expect("regex"));
static LIST_MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)(?:([-*+])|(\d+)([.)])|(>))(\s+)(\[[ xX]\]\s+)?").expect("regex"));
static TASK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s*(?:[-*+]|\d+[.)])\s+\[)([ xX])\]").expect("regex"));

/// Indents (or with `outdent` moves back) the list items on the lines of
/// the selection `anchor..cursor` by two spaces; `None` if the first of
/// these lines is no list item (Tab then works as usual).
pub fn indent_list_items(text: &str, anchor: usize, cursor: usize, outdent: bool) -> Option<Edit> {
    let (a, c) = (byte_at(text, anchor), byte_at(text, cursor));
    let (start, end) = (a.min(c), a.max(c));
    let first = line_start(text, start);
    if !LIST_ITEM.is_match(&text[first..line_end(text, first)]) {
        return None;
    }
    let last_pos = start.max(if end > start { end - 1 } else { end });
    let mut starts = Vec::new();
    let mut p = first;
    while p <= last_pos && p <= text.len() {
        starts.push(p);
        match text[p..].find('\n') {
            Some(i) => p += i + 1,
            None => break,
        }
    }
    let block_end = line_end(text, *starts.last().expect("one line"));
    let (mut shift_a, mut shift_c) = (0i64, 0i64);
    let mut out = String::new();
    for (i, &p) in starts.iter().enumerate() {
        let line = &text[p..line_end(text, p)];
        let mut delta = 0i64;
        if LIST_ITEM.is_match(line) {
            if !outdent {
                out.push_str("  ");
                out.push_str(line);
                delta = 2;
            } else {
                let lead = if line.starts_with('\t') {
                    1
                } else if line.starts_with("  ") {
                    2
                } else {
                    usize::from(line.starts_with(' '))
                };
                out.push_str(&line[lead..]);
                delta = -(lead as i64);
            }
        } else {
            out.push_str(line);
        }
        if i + 1 < starts.len() {
            out.push('\n');
        }
        if c >= p {
            shift_c += delta;
        }
        if a >= p {
            shift_a += delta;
        }
    }
    let moved = |b: usize, shift: i64| (b as i64 + shift).max(first as i64) as usize;
    let (new_a, new_c) =
        if a == c { (moved(c, shift_c), moved(c, shift_c)) } else { (moved(a, shift_a), moved(c, shift_c)) };
    Some(edit(text, first, block_end, &out, new_a, new_c))
}

/// Enter in a Markdown list continues it: a new bullet, a new open
/// checkbox (`- [ ] `), the next number, or `> ` in a quote. Enter on an
/// empty item ends the list (an indented one moves out a level first).
/// `None` if the cursor `pos` is not after such a marker.
pub fn continue_list(text: &str, pos: usize) -> Option<Edit> {
    let b = byte_at(text, pos);
    let (ls, le) = (line_start(text, b), line_end(text, b));
    let line = &text[ls..le];
    let m = LIST_MARKER.captures(line)?;
    let whole = m.get(0).expect("match").end();
    if b < ls + whole {
        return None;
    }
    let indent = m.get(1).map_or("", |x| x.as_str());
    let quote = m.get(5).is_some();
    if line[whole..].trim().is_empty() {
        if b != le {
            return None;
        }
        if !indent.is_empty() && !quote {
            return indent_list_items(text, pos, pos, true);
        }
        return Some(edit(text, ls, le, "", ls, ls));
    }
    let marker = if quote {
        ">".to_owned()
    } else if let Some(n) = m.get(3) {
        format!("{}{}", n.as_str().parse::<u64>().map_or(1, |n| n + 1), &m[4])
    } else {
        m[2].to_owned()
    };
    let insert = format!("\n{indent}{marker}{}{}", &m[6], if m.get(7).is_some() { "[ ] " } else { "" });
    Some(edit(text, b, b, &insert, b + insert.len(), b + insert.len()))
}

/// Ticks (or clears) the checkbox of the task on line `line` (1-based).
pub fn toggle_task(text: &str, line: usize, done: bool) -> Option<Edit> {
    let mut start = 0;
    for _ in 1..line {
        match text[start..].find('\n') {
            Some(i) => start += i + 1,
            None => break,
        }
    }
    let end = line_end(text, start);
    let m = TASK.captures(&text[start..end])?;
    let at = start + m[1].len();
    let mark = if done { "x" } else { " " };
    Some(edit(text, at, at + 1, mark, at + 1, at + 1))
}

// ---- tables -------------------------------------------------------------

/// The delimiter row under the header: `| --- | :-: | --: |`.
static DELIMITER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$").expect("regex"));
static FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*(```|~~~)").expect("regex"));

pub fn is_delimiter(line: &str) -> bool {
    DELIMITER.is_match(line)
}

/// Cells of a table row (trimmed; `\|` stays inside a cell).
pub fn table_cells(line: &str) -> Vec<String> {
    let mut s = line.trim();
    if let Some(rest) = s.strip_prefix('|') {
        s = rest;
    }
    if s.ends_with('|') && !s.ends_with("\\|") {
        s = &s[..s.len() - 1];
    }
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek().is_some() {
            cell.push(c);
            cell.push(chars.next().expect("peeked"));
        } else if c == '|' {
            cells.push(cell.trim().to_owned());
            cell.clear();
        } else {
            cell.push(c);
        }
    }
    cells.push(cell.trim().to_owned());
    cells
}

/// Columns `s` takes in a monospace font (wide CJK and emoji count two,
/// combining marks none).
pub fn text_columns(s: &str) -> usize {
    s.chars()
        .map(|ch| {
            let c = ch as u32;
            if (0x300..=0x36f).contains(&c) || c == 0x200d || (0xfe00..=0xfe0f).contains(&c) {
                return 0;
            }
            let wide = (0x1100..=0x115f).contains(&c)
                || (0x2e80..=0xa4cf).contains(&c)
                || (0xac00..=0xd7a3).contains(&c)
                || (0xf900..=0xfaff).contains(&c)
                || (0xfe30..=0xfe4f).contains(&c)
                || (0xff00..=0xff60).contains(&c)
                || (0xffe0..=0xffe6).contains(&c)
                || (0x1f300..=0x1faff).contains(&c)
                || (0x20000..=0x3fffd).contains(&c);
            if wide { 2 } else { 1 }
        })
        .sum()
}

/// Table rows with aligned columns; `offsets[row][col]` is where a cell's
/// text starts in its line (bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Formatted {
    pub lines: Vec<String>,
    pub offsets: Vec<Vec<usize>>,
}

/// The table rows `lines` (header, delimiter, body) with aligned columns.
/// Alignment from the delimiter row is kept and applied (`:-:` centred,
/// `--:` right).
pub fn format_table(lines: &[String]) -> Formatted {
    let indent: String = lines.first().map_or("", |l| &l[..l.len() - l.trim_start().len()]).to_owned();
    let rows: Vec<Vec<String>> = lines.iter().map(|l| table_cells(l)).collect();
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut align = Vec::new();
    let mut width = Vec::new();
    for c in 0..cols {
        let d = rows.get(1).and_then(|r| r.get(c)).map_or("", String::as_str);
        align.push(if d.len() > 1 && d.starts_with(':') && d.ends_with(':') {
            'c'
        } else if d.ends_with(':') {
            'r'
        } else if d.starts_with(':') {
            'l'
        } else {
            ' '
        });
        let w = rows
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 1)
            .map(|(_, r)| text_columns(r.get(c).map_or("", String::as_str)))
            .fold(3, usize::max);
        width.push(w);
    }
    let mut out = Vec::new();
    let mut offsets = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let mut line = format!("{indent}|");
        let mut offs = Vec::new();
        for c in 0..cols {
            let (w, a) = (width[c], align[c]);
            if i == 1 {
                let cell = match a {
                    'c' => format!(":{}:", "-".repeat(w - 2)),
                    'r' => format!("{}:", "-".repeat(w - 1)),
                    'l' => format!(":{}", "-".repeat(w - 1)),
                    _ => "-".repeat(w),
                };
                line.push(' ');
                offs.push(line.len());
                line.push_str(&cell);
                line.push_str(" |");
            } else {
                let t = r.get(c).map_or("", String::as_str);
                let pad = w - text_columns(t);
                let left = match a {
                    'r' => pad,
                    'c' => pad / 2,
                    _ => 0,
                };
                line.push(' ');
                line.push_str(&" ".repeat(left));
                offs.push(line.len());
                line.push_str(t);
                line.push_str(&" ".repeat(pad - left));
                line.push_str(" |");
            }
        }
        out.push(line);
        offsets.push(offs);
    }
    Formatted { lines: out, offsets }
}

/// A table given as text, aligned (for inserting).
pub fn align_table(markdown: &str) -> String {
    let lines: Vec<String> = markdown.split('\n').map(str::to_owned).collect();
    format_table(&lines).lines.join("\n")
}

/// Inside a fenced code block at byte `b`?
fn in_code_fence(text: &str, b: usize) -> bool {
    FENCE.find_iter(&text[..b]).count() % 2 == 1
}

/// A table in the text (bytes): from the header to the last row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub start: usize,
    pub end: usize,
    pub lines: Vec<String>,
    pub starts: Vec<usize>,
}

/// The table with the line at byte `b`, or `None` (also in code blocks).
pub fn table_at(text: &str, b: usize) -> Option<Table> {
    let is_row = |l: &str| !l.trim().is_empty() && l.contains('|');
    let mut s = line_start(text, b);
    if !is_row(&text[s..line_end(text, s)]) {
        return None;
    }
    // The block of consecutive lines with a pipe around `b`.
    while s > 0 {
        let prev = line_start(text, s - 1);
        if !is_row(&text[prev..s - 1]) {
            break;
        }
        s = prev;
    }
    let (mut lines, mut starts) = (Vec::new(), Vec::new());
    let mut p = s;
    loop {
        let e = line_end(text, p);
        let l = &text[p..e];
        if !is_row(l) {
            break;
        }
        lines.push(l.to_owned());
        starts.push(p);
        if e >= text.len() {
            break;
        }
        p = e + 1;
    }
    // The table starts at the header above the first delimiter row.
    let k = lines.iter().enumerate().position(|(i, l)| i > 0 && is_delimiter(l))?;
    let first = k - 1;
    if b < starts[first] || in_code_fence(text, starts[first]) {
        return None;
    }
    let last = lines.len() - 1;
    Some(Table {
        start: starts[first],
        end: starts[last] + lines[last].len(),
        lines: lines[first..].to_vec(),
        starts: starts[first..].to_vec(),
    })
}

/// Row and cell of byte `b` in table `t`, and the offset into the cell text.
fn table_cell_at(t: &Table, b: usize) -> (usize, usize, usize) {
    let mut row = t.starts.len() - 1;
    while row > 0 && t.starts[row] > b {
        row -= 1;
    }
    let line = &t.lines[row];
    let col = (b - t.starts[row]).min(line.len());
    let lead = line.len() - line.trim_start().len() + usize::from(line.trim_start().starts_with('|'));
    let (mut cell, mut cell_start) = (0, lead);
    let bytes = line.as_bytes();
    let mut i = lead;
    while i < col && i < line.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'|' {
            cell += 1;
            cell_start = i + 1;
        }
        i += 1;
    }
    let before = &line[cell_start.min(col)..col];
    (row, cell, before.trim_start().len())
}

fn columns(lines: &[String]) -> usize {
    lines.iter().map(|l| table_cells(l).len()).max().unwrap_or(0)
}

fn empty_row(cols: usize) -> String {
    format!("|{}", " |".repeat(cols))
}

/// Byte range of the text of cell `cell` in row `row` of the new table
/// starting at byte `start` (trailing spaces left out).
fn cell_range(f: &Formatted, start: usize, row: usize, cell: usize) -> (usize, usize) {
    let row_start: usize = start + f.lines[..row].iter().map(|l| l.len() + 1).sum::<usize>();
    let line = &f.lines[row];
    let begin = f.offsets[row][cell];
    let bytes = line.as_bytes();
    let mut end = begin;
    while end < line.len() && !(bytes[end] == b'|' && bytes[end - 1] != b'\\') {
        end += 1;
    }
    let len = line[begin..end].trim_end().len();
    (row_start + begin, row_start + begin + len)
}

/// Tab / Shift+Tab (`back`) in a table at `pos`: align it and select the
/// next / previous cell (Tab in the last cell adds a row). `None` outside a
/// table.
pub fn table_tab(text: &str, pos: usize, back: bool) -> Option<Edit> {
    let b = byte_at(text, pos);
    let t = table_at(text, b)?;
    let (r, c, _) = table_cell_at(&t, b);
    let mut lines = t.lines.clone();
    let cols = columns(&lines) as i64;
    let mut row = r as i64;
    let mut cell = (c as i64).min(cols - 1) + if back { -1 } else { 1 };
    if cell >= cols {
        row += 1;
        cell = 0;
    }
    if cell < 0 {
        row -= 1;
        cell = cols - 1;
    }
    if row == 1 {
        row += if back { -1 } else { 1 };
    }
    if row < 0 {
        row = 0;
        cell = 0;
    }
    if row as usize >= lines.len() {
        lines.push(empty_row(cols as usize));
    }
    let f = format_table(&lines);
    let (s, e) = cell_range(&f, t.start, row as usize, cell as usize);
    Some(edit(text, t.start, t.end, &f.lines.join("\n"), s, e))
}

/// Enter at the end of a table row at `pos`: align the table and add an
/// empty row below (below the delimiter for the header). In an empty last
/// row: the row goes and the table ends, as with lists. `None` otherwise.
pub fn table_enter(text: &str, pos: usize) -> Option<Edit> {
    let b = byte_at(text, pos);
    let t = table_at(text, b)?;
    let (r, _, _) = table_cell_at(&t, b);
    if b != t.starts[r] + t.lines[r].len() {
        return None;
    }
    let last = t.lines.len() - 1;
    if r == last && r > 1 && table_cells(&t.lines[last]).iter().all(String::is_empty) {
        let f = format_table(&t.lines[..last]);
        let new = format!("{}\n", f.lines.join("\n"));
        let at = t.start + new.len();
        return Some(edit(text, t.start, t.end, &new, at, at));
    }
    let cols = columns(&t.lines);
    let row = 2.max(r + 1);
    let mut lines = t.lines[..row].to_vec();
    lines.push(empty_row(cols));
    lines.extend_from_slice(&t.lines[row..]);
    let f = format_table(&lines);
    let (s, e) = cell_range(&f, t.start, row, 0);
    Some(edit(text, t.start, t.end, &f.lines.join("\n"), s, e))
}

/// Aligns the table at `pos`, keeping the cursor (at `cursor`) in its
/// cell. `None` if there is no table or it is aligned already.
pub fn align_table_at(text: &str, pos: usize, cursor: usize) -> Option<Edit> {
    let b = byte_at(text, pos);
    let t = table_at(text, b)?;
    let f = format_table(&t.lines);
    let new = f.lines.join("\n");
    if new == text[t.start..t.end] {
        return None;
    }
    let cb = byte_at(text, cursor);
    let at = if cb >= t.start && cb <= t.end { Some(table_cell_at(&t, cb)) } else { None };
    let new_cursor = match at {
        Some((row, cell, offset)) => {
            let cell = cell.min(f.offsets[row].len().saturating_sub(1));
            let cell_text = table_cells(&f.lines[row]).get(cell).cloned().unwrap_or_default();
            let row_start: usize = t.start + f.lines[..row].iter().map(|l| l.len() + 1).sum::<usize>();
            let mut off = offset.min(cell_text.len());
            while !cell_text.is_char_boundary(off) {
                off -= 1;
            }
            row_start + f.offsets[row].get(cell).copied().unwrap_or(0) + off
        }
        None if cb > t.end => cb + new.len() - (t.end - t.start),
        None => cb,
    };
    Some(edit(text, t.start, t.end, &new, new_cursor, new_cursor))
}

/// Aligns every table of the note, from the bottom (so the edits can be
/// applied one after the other). The cursor stays in its cell.
pub fn align_tables(text: &str, cursor: usize) -> Vec<Edit> {
    let mut found: Vec<usize> = Vec::new();
    let mut p = 0;
    while p < text.len() {
        let end = line_end(text, p);
        let t = if p > 0 && is_delimiter(&text[p..end]) { table_at(text, p) } else { None };
        if let Some(t) = &t
            && found.last() != Some(&t.start)
        {
            found.push(t.start);
        }
        p = t.map_or(end, |t| t.end) + 1;
    }
    let mut current = text.to_owned();
    let mut cursor = cursor;
    let mut edits = Vec::new();
    for start in found.into_iter().rev() {
        let pos = utf16_at(&current, start);
        if let Some(e) = align_table_at(&current, pos, cursor) {
            current = e.apply(&current);
            cursor = e.cursor;
            edits.push(e);
        }
    }
    edits
}

// ---- completion -----------------------------------------------------------

static TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(^|[\s(])#([^\s\d#.,;:!?()\[\]{}"'`<>|*~=+-][^\s#.,;:!?()\[\]{}"'`<>|*~=+]*)$"#).expect("regex")
});

/// What may be completed at the cursor: a tag after `#` and/or a wiki link
/// after `[[`. `start` is where the typed text begins (UTF-16).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Completion {
    /// Typed tag (lower case), without `#`.
    pub tag: Option<String>,
    pub tag_start: usize,
    /// Typed path after `[[` (a leading `/` removed for the search).
    pub wiki: Option<String>,
    pub wiki_start: usize,
}

/// The completion context at `pos`: looks at most 200 characters back.
pub fn completion_at(text: &str, pos: usize) -> Completion {
    let b = byte_at(text, pos);
    let from = text[..b].char_indices().rev().nth(199).map_or(0, |(i, _)| i);
    let before = &text[from..b];
    let mut out = Completion::default();
    if let Some(m) = TAG.captures(before) {
        let typed = m.get(2).expect("group");
        out.tag = Some(typed.as_str().to_lowercase());
        out.tag_start = pos - utf16_len(typed.as_str());
    }
    if let Some(open) = before.rfind("[[") {
        let query = &before[open + 2..];
        if !query.contains([']', '\n', '|', '#']) {
            out.wiki = Some(query.strip_prefix('/').unwrap_or(query).to_owned());
            out.wiki_start = pos - utf16_len(query);
        }
    }
    out
}

/// A tag of the project (`key` lower case, `display` as written).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, Serialize)]
pub struct Tag {
    pub key: String,
    pub display: String,
    pub count: usize,
}

/// Tags to suggest for `typed`: starting with it (not equal), most used
/// first, at most eight.
pub fn tag_suggestions(tags: &[Tag], typed: &str) -> Vec<Tag> {
    let mut out: Vec<Tag> = tags.iter().filter(|t| t.key.starts_with(typed) && t.key != typed).cloned().collect();
    out.sort_by_key(|t| std::cmp::Reverse(t.count));
    out.truncate(8);
    out
}

/// Replaces the typed tag `start..end` by `tag`.
pub fn accept_tag(text: &str, start: usize, end: usize, tag: &str) -> Edit {
    let (s, e) = (byte_at(text, start), byte_at(text, end));
    edit(text, s, e, tag, s + tag.len(), s + tag.len())
}

/// Replaces the typed path `start..end` by `/relative` and closes the link
/// (`]]`) unless it is closed already; the cursor goes after it.
pub fn accept_wiki(text: &str, start: usize, end: usize, relative: &str) -> Edit {
    let (s, e) = (byte_at(text, start), byte_at(text, end));
    let closing = if text[e..].starts_with("]]") { "" } else { "]]" };
    let new = format!("/{relative}{closing}");
    let cursor = s + 1 + relative.len() + 2;
    edit(text, s, e, &new, cursor, cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Applies `e` and returns (text, selected text, cursor).
    fn run(text: &str, e: Edit) -> (String, String, usize) {
        let after = e.apply(text);
        let (a, c) = (e.anchor.min(e.cursor), e.anchor.max(e.cursor));
        let sel = after[byte_at(&after, a)..byte_at(&after, c)].to_owned();
        (after, sel, e.cursor)
    }

    fn end(s: &str) -> usize {
        utf16_len(s)
    }

    #[test]
    fn utf16_positions() {
        let s = "aä日🙂b";
        assert_eq!(byte_at(s, 0), 0);
        assert_eq!(byte_at(s, 2), 3);
        assert_eq!(byte_at(s, 3), 6);
        assert_eq!(byte_at(s, 5), 10, "after the surrogate pair");
        assert_eq!(utf16_at(s, 10), 5);
        assert_eq!(end(s), 6);
    }

    #[test]
    fn tab_indents_list_items_not_text() {
        let t = "Notiz\n- Punkt";
        let (t, _, c) = run(t, indent_list_items(t, end(t), end(t), false).unwrap());
        assert!(t.ends_with("\n  - Punkt"));
        assert_eq!(c, end(&t));
        let (t, _, _) = run(&t, indent_list_items(&t, end(&t), end(&t), true).unwrap());
        assert!(t.ends_with("\n- Punkt"));
        let t = format!("{t}\nText");
        assert!(indent_list_items(&t, end(&t), end(&t), false).is_none());
    }

    #[test]
    fn indent_several_items_keeps_selection() {
        let t = "- a\n- b\nText\n- c";
        let e = indent_list_items(t, 0, end(t), false).unwrap();
        let (after, sel, c) = run(t, e);
        assert_eq!(after, "  - a\n  - b\nText\n  - c");
        // As before: the anchor at a line start moves past the new spaces.
        assert_eq!(sel, &after[2..]);
        assert_eq!(c, end(&after));
    }

    #[test]
    fn enter_continues_and_ends_lists() {
        let mut t = String::from("- eins");
        t = run(&t, continue_list(&t, end(&t)).unwrap()).0;
        assert_eq!(t, "- eins\n- ");
        t.push_str("zwei");
        t = run(&t, continue_list(&t, end(&t)).unwrap()).0;
        t = run(&t, continue_list(&t, end(&t)).unwrap()).0;
        assert_eq!(t, "- eins\n- zwei\n", "empty item ends the list");

        let t = "- [x] erledigt";
        assert_eq!(run(t, continue_list(t, end(t)).unwrap()).0, "- [x] erledigt\n- [ ] ");
        let t = "9. neun";
        assert_eq!(run(t, continue_list(t, end(t)).unwrap()).0, "9. neun\n10. ");
        let t = "> Zitat";
        assert_eq!(run(t, continue_list(t, end(t)).unwrap()).0, "> Zitat\n> ");
        assert!(continue_list("Text", 4).is_none());
        assert!(continue_list("- a", 1).is_none(), "cursor inside the marker");
        // An empty indented item moves out a level first.
        let t = "- a\n  - ";
        assert_eq!(run(t, continue_list(t, end(t)).unwrap()).0, "- a\n- ");
    }

    #[test]
    fn toggles_task() {
        let t = "# Aufgaben\n\n- [ ] Literatur\n- [x] Gliederung";
        assert_eq!(run(t, toggle_task(t, 3, true).unwrap()).0, "# Aufgaben\n\n- [x] Literatur\n- [x] Gliederung");
        assert_eq!(run(t, toggle_task(t, 4, false).unwrap()).0, "# Aufgaben\n\n- [ ] Literatur\n- [ ] Gliederung");
        assert!(toggle_task(t, 1, true).is_none());
    }

    #[test]
    fn formats_tables() {
        assert_eq!(
            align_table("| Spalte 1 | Spalte 2 |\n|---|---|\n| | |\n| | |"),
            "| Spalte 1 | Spalte 2 |\n| -------- | -------- |\n|          |          |\n|          |          |"
        );
        assert_eq!(table_cells(r"| a \| b | c |"), vec![r"a \| b", "c"]);
        assert_eq!(text_columns("日本x"), 5);
        assert_eq!(text_columns("e\u{301}"), 1);
        assert_eq!(align_table("| 日本 | x |\n|-|-|\n| a | b |"), "| 日本 | x   |\n| ---- | --- |\n| a    | b   |");
    }

    const ALIGNED: &str = "|      a      |   b |\n| :---------: | --: |\n| langer Text |   1 |";

    #[test]
    fn tab_moves_through_cells_and_adds_a_row() {
        let t = "Vorher\n\n| a | b |\n|:-:|--:|\n| langer Text | 1 |";
        let at = utf16_at(t, t.find("| a").unwrap()) + 2;
        let (t, sel, c) = run(t, table_tab(t, at, false).unwrap());
        assert!(t.ends_with(ALIGNED), "{t}");
        assert_eq!(sel, "b");
        let (t, sel, c) = run(&t, table_tab(&t, c, false).unwrap());
        assert_eq!(sel, "langer Text", "skips the delimiter row");
        let (t, sel, c) = run(&t, table_tab(&t, c, true).unwrap());
        assert_eq!(sel, "b", "Shift+Tab goes back");
        let (t, _, c) = run(&t, table_tab(&t, c, false).unwrap());
        let (t, _, c) = run(&t, table_tab(&t, c, false).unwrap());
        let (t, _, _) = run(&t, table_tab(&t, c, false).unwrap());
        assert!(t.ends_with(&format!("{ALIGNED}\n|             |     |")), "{t}");
        assert!(table_tab("kein | Tisch", 2, false).is_none());
    }

    #[test]
    fn enter_adds_a_row_and_ends_the_table() {
        let t = format!("{ALIGNED}\n| x | |");
        let (t, sel, _) = run(&t, table_enter(&t, end(&t)).unwrap());
        assert!(t.ends_with("|      x      |     |\n|             |     |"), "{t}");
        assert_eq!(sel, "");
        let (t, _, c) = run(&t, table_enter(&t, end(&t)).unwrap());
        assert!(t.ends_with("|      x      |     |\n"), "{t}");
        assert_eq!(c, end(&t));
        // Not at the end of a row.
        assert!(table_enter(ALIGNED, 3).is_none());
    }

    #[test]
    fn aligns_all_tables_but_not_in_code() {
        let t = "Text\n| c | d |\n|---|---|\n| e | ffff |\n\n```\n| x | y |\n|---|---|\n```\n\n| 1 | 2 |\n|-|-|";
        let edits = align_tables(t, 0);
        assert_eq!(edits.len(), 2);
        let mut after = t.to_owned();
        for e in &edits {
            after = e.apply(&after);
        }
        assert!(after.contains("| c   | d    |\n| --- | ---- |\n| e   | ffff |"), "{after}");
        assert!(after.contains("```\n| x | y |\n|---|---|\n```"));
        assert!(after.ends_with("| 1   | 2   |\n| --- | --- |"));
        assert!(align_tables(&after, 0).is_empty(), "nothing left to do");
    }

    #[test]
    fn align_keeps_cursor_in_its_cell() {
        let t = "| a | b |\n|---|---|\n| langer | x |";
        let pos = utf16_at(t, t.find("x").unwrap()) + 1;
        let (after, _, c) = run(t, align_table_at(t, 0, pos).unwrap());
        assert_eq!(&after[..byte_at(&after, c)].lines().last().unwrap().trim_end(), &"| langer | x");
    }

    #[test]
    fn edits_are_small() {
        let t = "| a | b |\n|---|---|\n| c | d |";
        let e = align_table_at(t, 0, 0).unwrap();
        // Common start and end are left alone.
        assert_eq!(e.start, 4, "{e:?}");
        assert_eq!(e.end, utf16_len(t) - 2, "{e:?}");
        assert_eq!(e.apply(t), "| a   | b   |\n| --- | --- |\n| c   | d   |");
    }

    #[test]
    fn completion_contexts() {
        let t = "Neu: [[plan";
        let c = completion_at(t, end(t));
        assert_eq!(c.wiki.as_deref(), Some("plan"));
        assert_eq!(c.wiki_start, 7);
        let (after, _, cur) = run(t, accept_wiki(t, c.wiki_start, end(t), "notes/plan-neu.md"));
        assert_eq!(after, "Neu: [[/notes/plan-neu.md]]");
        assert_eq!(cur, end(&after));
        // Closed already: no second `]]`.
        let t = "[[/no]]";
        assert_eq!(run(t, accept_wiki(t, 2, 5, "notes/x.md")).0, "[[/notes/x.md]]");
        assert_eq!(completion_at("[[a|b", 5).wiki, None);
        assert_eq!(completion_at("[[/notes/p", 10).wiki.as_deref(), Some("notes/p"));

        let t = "Lernen für #Wi";
        let c = completion_at(t, end(t));
        assert_eq!(c.tag.as_deref(), Some("wi"));
        let (after, _, _) = run(t, accept_tag(t, c.tag_start, end(t), "wiki"));
        assert_eq!(after, "Lernen für #wiki");
        assert_eq!(completion_at("Preis #5", 8).tag, None, "numbers are no tags");
        assert_eq!(completion_at("a#b", 3).tag, None, "only after a space");
    }

    #[test]
    fn suggests_tags_by_use() {
        let tag = |k: &str, n| Tag { key: k.into(), display: k.into(), count: n };
        let tags = [tag("wiki", 2), tag("wi", 9), tag("wissen", 5), tag("rust", 7)];
        let s: Vec<_> = tag_suggestions(&tags, "wi").into_iter().map(|t| t.key).collect();
        assert_eq!(s, vec!["wissen", "wiki"]);
    }
}
