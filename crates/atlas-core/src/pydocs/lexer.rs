// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! A small Python tokenizer: enough to find definitions, imports and
//! docstrings without a full parser, so new syntax (PEP 695 generics,
//! `match`, `type` aliases) never breaks a file. Comments are dropped,
//! newlines inside brackets and after `\` join lines into one logical line.

/// One token of a logical line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    Name(String),
    /// A string literal as written (prefix and quotes included).
    Str(String),
    Num(String),
    /// Operator or punctuation: `(`, `)`, `->`, `**`, `:=`, `.` …
    Op(String),
}

impl Tok {
    pub fn is_op(&self, op: &str) -> bool {
        matches!(self, Tok::Op(o) if o == op)
    }
    pub fn is_name(&self, name: &str) -> bool {
        matches!(self, Tok::Name(n) if n == name)
    }
    pub fn name(&self) -> Option<&str> {
        match self {
            Tok::Name(n) => Some(n),
            _ => None,
        }
    }
    /// The token as source text.
    pub fn text(&self) -> &str {
        match self {
            Tok::Name(s) | Tok::Str(s) | Tok::Num(s) | Tok::Op(s) => s,
        }
    }
}

/// A logical line: its indentation (columns, tabs to 8) and tokens.
#[derive(Debug, Clone)]
pub struct Line {
    pub indent: usize,
    pub toks: Vec<Tok>,
}

const OPS3: &[&str] = &["**=", "//=", ">>=", "<<=", "...", "!=="];
const OPS2: &[&str] =
    &["->", "**", "//", "==", "!=", "<=", ">=", "<<", ">>", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "@=", ":="];

fn is_string_prefix(p: &str) -> bool {
    p.len() <= 2 && p.chars().all(|c| matches!(c.to_ascii_lowercase(), 'r' | 'b' | 'u' | 'f' | 't'))
}

/// Splits `src` into logical lines.
pub fn lines(src: &str) -> Vec<Line> {
    let s = src.as_bytes();
    let mut out = Vec::new();
    let mut toks: Vec<Tok> = Vec::new();
    let mut depth = 0usize;
    let mut indent = 0usize;
    let mut at_line_start = true;
    let mut i = 0;
    let flush = |toks: &mut Vec<Tok>, out: &mut Vec<Line>, indent: usize| {
        if !toks.is_empty() {
            out.push(Line { indent, toks: std::mem::take(toks) });
        }
    };
    while i < s.len() {
        let c = s[i];
        if at_line_start {
            // Indentation of a new logical line (blank lines and comments
            // do not count).
            let mut col = 0;
            let mut j = i;
            while j < s.len() && (s[j] == b' ' || s[j] == b'\t' || s[j] == b'\x0c') {
                col = if s[j] == b'\t' { (col / 8 + 1) * 8 } else { col + 1 };
                j += 1;
            }
            if j < s.len() && (s[j] == b'\n' || s[j] == b'\r' || s[j] == b'#') {
                // Blank or comment line.
                while j < s.len() && s[j] != b'\n' {
                    j += 1;
                }
                i = j + 1;
                continue;
            }
            indent = col;
            at_line_start = false;
            i = j;
            continue;
        }
        match c {
            b'\n' => {
                i += 1;
                if depth == 0 {
                    flush(&mut toks, &mut out, indent);
                    at_line_start = true;
                }
            }
            b' ' | b'\t' | b'\r' | b'\x0c' => i += 1,
            b'#' => {
                while i < s.len() && s[i] != b'\n' {
                    i += 1;
                }
            }
            b'\\' if s.get(i + 1) == Some(&b'\n') => i += 2,
            b'\\' if s.get(i + 1) == Some(&b'\r') => i += 3,
            b'"' | b'\'' => {
                let end = string_end(s, i);
                toks.push(Tok::Str(src[i..end].to_owned()));
                i = end;
            }
            _ if c == b'_' || c.is_ascii_alphabetic() || c >= 0x80 => {
                let start = i;
                while i < s.len() && (s[i] == b'_' || s[i].is_ascii_alphanumeric() || s[i] >= 0x80) {
                    i += 1;
                }
                // Keep char boundaries intact for non-ASCII names.
                while !src.is_char_boundary(i) {
                    i += 1;
                }
                let word = &src[start..i];
                if i < s.len() && (s[i] == b'"' || s[i] == b'\'') && is_string_prefix(word) {
                    let end = string_end(s, i);
                    toks.push(Tok::Str(src[start..end].to_owned()));
                    i = end;
                } else {
                    toks.push(Tok::Name(word.to_owned()));
                }
            }
            _ if c.is_ascii_digit() || (c == b'.' && s.get(i + 1).is_some_and(u8::is_ascii_digit)) => {
                let start = i;
                while i < s.len() && (s[i].is_ascii_alphanumeric() || s[i] == b'.' || s[i] == b'_') {
                    // Exponent signs: 1e-5
                    if (s[i] == b'e' || s[i] == b'E') && matches!(s.get(i + 1), Some(b'-') | Some(b'+')) {
                        i += 1;
                    }
                    i += 1;
                }
                toks.push(Tok::Num(src[start..i].to_owned()));
            }
            _ => {
                let rest = &src[i..];
                let op = OPS3
                    .iter()
                    .chain(OPS2)
                    .find(|o| rest.starts_with(**o))
                    .map(|o| (*o).to_owned())
                    .unwrap_or_else(|| rest.chars().next().map(String::from).unwrap_or_default());
                match op.as_str() {
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" => depth = depth.saturating_sub(1),
                    _ => {}
                }
                i += op.len().max(1);
                toks.push(Tok::Op(op));
            }
        }
    }
    flush(&mut toks, &mut out, indent);
    out
}

/// End (exclusive) of the string literal whose quote starts at `i`.
fn string_end(s: &[u8], i: usize) -> usize {
    let q = s[i];
    let triple = s.get(i + 1) == Some(&q) && s.get(i + 2) == Some(&q);
    let mut j = if triple { i + 3 } else { i + 1 };
    while j < s.len() {
        match s[j] {
            b'\\' => j += 2,
            b'\n' if !triple => return j,
            c if c == q => {
                if !triple {
                    return j + 1;
                }
                if s.get(j + 1) == Some(&q) && s.get(j + 2) == Some(&q) {
                    return j + 3;
                }
                j += 1;
            }
            _ => j += 1,
        }
    }
    s.len()
}

/// The value of a string literal token (escapes decoded unless raw).
pub fn string_value(lit: &str) -> String {
    let quote = lit.find(['"', '\'']).unwrap_or(0);
    let prefix = lit[..quote].to_ascii_lowercase();
    let body = &lit[quote..];
    let q = &body[..1];
    let triple = body.len() >= 6 && body[1..].starts_with(&q.repeat(2));
    let n = if triple { 3 } else { 1 };
    let inner = if body.len() >= 2 * n && body.ends_with(&q.repeat(n)) { &body[n..body.len() - n] } else { &body[n..] };
    if prefix.contains('r') {
        return inner.to_owned();
    }
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some('\'') => out.push('\''),
            Some('"') => out.push('"'),
            Some('\n') => {}
            Some('x') => {
                let hex: String = (0..2).filter_map(|_| chars.next()).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(ch) => out.push(ch),
                    None => out.push_str(&format!("\\x{hex}")),
                }
            }
            Some(u @ ('u' | 'U')) => {
                let len = if u == 'u' { 4 } else { 8 };
                let hex: String = (0..len).filter_map(|_| chars.next()).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(ch) => out.push(ch),
                    None => out.push_str(&format!("\\{u}{hex}")),
                }
            }
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// The text of adjacent string literals (implicit concatenation), if the
/// tokens are only strings.
pub fn strings_value(toks: &[Tok]) -> Option<String> {
    if toks.is_empty() || !toks.iter().all(|t| matches!(t, Tok::Str(_))) {
        return None;
    }
    Some(toks.iter().map(|t| string_value(t.text())).collect())
}

/// Index of the bracket closing the one at `open` (same depth), or the end.
pub fn closing(toks: &[Tok], open: usize) -> usize {
    let mut depth = 0i32;
    for (i, t) in toks.iter().enumerate().skip(open) {
        if let Tok::Op(o) = t {
            match o.as_str() {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => {
                    depth -= 1;
                    if depth == 0 {
                        return i;
                    }
                }
                _ => {}
            }
        }
    }
    toks.len()
}

/// Splits `toks` at commas of depth 0.
pub fn split_commas(toks: &[Tok]) -> Vec<&[Tok]> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, t) in toks.iter().enumerate() {
        if let Tok::Op(o) = t {
            match o.as_str() {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth -= 1,
                "," if depth == 0 => {
                    out.push(&toks[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
        }
    }
    if start < toks.len() {
        out.push(&toks[start..]);
    }
    out
}

/// Source-like text of tokens, spaced the way Python is usually written
/// (`f(a, *b, c=1)`, `dict[str, int] | None`, `x -> y`).
pub fn join(toks: &[Tok]) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    let mut prev: Option<&Tok> = None;
    let mut prev_unary = false;
    for t in toks {
        let op = match t {
            Tok::Op(o) => Some(o.as_str()),
            _ => None,
        };
        let prev_op = match prev {
            Some(Tok::Op(o)) => Some(o.as_str()),
            _ => None,
        };
        // An operator right after another one (or at the start) is unary.
        let unary = matches!(op, Some("*" | "**" | "-" | "+" | "~"))
            && (prev.is_none() || prev_op.is_some_and(|p| !matches!(p, ")" | "]" | "}")));
        let space = match (prev, op) {
            (None, _) => false,
            (_, Some(")" | "]" | "}" | "," | "." | ":")) => false,
            // Calls and subscripts.
            (Some(Tok::Name(_) | Tok::Str(_)), Some("(" | "[")) => false,
            (Some(Tok::Op(p)), Some("(" | "[")) if matches!(p.as_str(), ")" | "]") => false,
            // Keyword arguments inside brackets.
            (_, Some("=")) if depth > 0 => false,
            _ if prev_op == Some("=") && depth > 0 => false,
            _ if matches!(prev_op, Some("(" | "[" | "{" | "." | "~")) => false,
            _ if prev_unary => false,
            _ => true,
        };
        if space {
            out.push(' ');
        }
        out.push_str(t.text());
        match op {
            Some("(" | "[" | "{") => depth += 1,
            Some(")" | "]" | "}") => depth -= 1,
            _ => {}
        }
        prev = Some(t);
        prev_unary = unary;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_lines_join_brackets_and_drop_comments() {
        let src = "import a  # comment\n\ndef f(x,\n      y=1):  # c\n    \"\"\"Doc\n    more\"\"\"\n    return x\n";
        let l = lines(src);
        assert_eq!(l.len(), 4);
        assert_eq!(l[0].toks, [Tok::Name("import".into()), Tok::Name("a".into())]);
        assert_eq!(l[1].indent, 0);
        assert!(l[1].toks.iter().any(|t| t.is_name("y")));
        assert_eq!(l[2].indent, 4);
        assert_eq!(strings_value(&l[2].toks).unwrap(), "Doc\n    more");
    }

    #[test]
    fn strings_with_prefixes_and_escapes() {
        assert_eq!(string_value(r#"r"a\nb""#), "a\\nb");
        assert_eq!(string_value(r#""a\nb\"c""#), "a\nb\"c");
        assert_eq!(string_value("'''x'y'''"), "x'y");
        let l = lines("x = rb'\\x00' f\"{a}\" u'ü'\n");
        assert_eq!(l[0].toks.iter().filter(|t| matches!(t, Tok::Str(_))).count(), 3);
        // A quote inside a triple string does not end it.
        let l = lines("s = \"\"\"a \"quoted\" b\"\"\"\ny = 1\n");
        assert_eq!(l.len(), 2);
    }

    #[test]
    fn joins_tokens_like_source() {
        let l = lines("def f(a, b: int = 1, *args, c: dict[str, int] | None = None, **kw) -> list[int]: ...\n");
        let open = l[0].toks.iter().position(|t| t.is_op("(")).unwrap();
        let close = closing(&l[0].toks, open);
        let parts: Vec<String> = split_commas(&l[0].toks[open + 1..close]).into_iter().map(join).collect();
        assert_eq!(parts, ["a", "b: int = 1", "*args", "c: dict[str, int] | None = None", "**kw"]);
        assert_eq!(join(&lines("x = f(a, -1, key=g(2)[0], *rest)\n")[0].toks), "x = f(a, -1, key=g(2)[0], *rest)");
    }
}
