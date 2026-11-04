// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Syntax highlighting with `syntect` (pure Rust, the Sublime syntaxes it
//! ships): recognizing the language (file extension, info string of a code
//! block) and the spans to colour – Markdown including its fenced code
//! blocks, Rust, Python, MATLAB, LaTeX, shell, C/C++, JSON and more.
//!
//! Spans carry a [`Kind`], not a colour: the interface maps kinds to the
//! colours of its theme (light / dark). Positions are UTF-16 code units, as
//! in QML strings and `NSString`. [`Session`] keeps the parser state per
//! line, so an edit only re-highlights from the changed line until the
//! state matches the previous run again.

use std::path::Path;
use std::sync::LazyLock;

use serde::Serialize;
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};

static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);

/// What a span is, for choosing its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Comment,
    String,
    Escape,
    Number,
    Constant,
    Keyword,
    Operator,
    Function,
    Type,
    Variable,
    Tag,
    Attribute,
    Punctuation,
    /// Markdown heading, LaTeX sectioning.
    Heading,
    Emphasis,
    Strong,
    Link,
    /// Inline code and code blocks (where the language is unknown).
    Code,
    Quote,
    /// List markers.
    List,
    /// Formulas (LaTeX math, Markdown `$…$` where the syntax knows it).
    Math,
}

/// `start..end` (UTF-16) has kind `kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub kind: Kind,
}

/// Scope prefixes and their kind; the first match of the innermost scope
/// that matches anything wins.
const KINDS: &[(&str, Kind)] = &[
    ("comment", Kind::Comment),
    ("constant.character.escape", Kind::Escape),
    ("string.other.math", Kind::Math),
    ("meta.environment.math", Kind::Math),
    ("markup.math", Kind::Math),
    ("string", Kind::String),
    ("constant.numeric", Kind::Number),
    ("constant", Kind::Constant),
    ("keyword.operator", Kind::Operator),
    ("keyword", Kind::Keyword),
    ("storage.type", Kind::Keyword),
    ("storage.modifier", Kind::Keyword),
    ("storage", Kind::Keyword),
    ("entity.name.function", Kind::Function),
    ("support.function", Kind::Function),
    ("variable.function", Kind::Function),
    ("entity.name.section", Kind::Heading),
    ("entity.name.type", Kind::Type),
    ("entity.name.class", Kind::Type),
    ("entity.name.struct", Kind::Type),
    ("entity.name.enum", Kind::Type),
    ("entity.name.trait", Kind::Type),
    ("support.type", Kind::Type),
    ("support.class", Kind::Type),
    ("entity.name.tag", Kind::Tag),
    ("entity.other.attribute-name", Kind::Attribute),
    ("variable.parameter", Kind::Variable),
    ("variable", Kind::Variable),
    ("markup.heading", Kind::Heading),
    ("markup.italic", Kind::Emphasis),
    ("markup.bold", Kind::Strong),
    ("markup.underline.link", Kind::Link),
    ("meta.link", Kind::Link),
    ("markup.raw", Kind::Code),
    ("markup.quote", Kind::Quote),
    ("markup.list.numbered.bullet", Kind::List),
    ("markup.list.unnumbered.bullet", Kind::List),
    ("punctuation.definition.list_item", Kind::List),
    ("punctuation", Kind::Punctuation),
];

fn kind_of(scope: Scope) -> Option<Kind> {
    let name = scope.build_string();
    KINDS.iter().find(|(prefix, _)| name == *prefix || name.starts_with(&format!("{prefix}."))).map(|(_, k)| *k)
}

/// The kind at a point: the innermost scope that means something, except
/// that comments and formulas take everything inside them, and the
/// punctuation of strings, headings, emphasis, code, quotes and links
/// (`"`, `#`, `*`, `` ` ``, `>`, `[`) belongs to them.
fn kind_of_stack(stack: &ScopeStack) -> Option<Kind> {
    let kinds: Vec<Kind> = stack.as_slice().iter().filter_map(|s| kind_of(*s)).collect();
    if let Some(k) = kinds.iter().find(|k| matches!(k, Kind::Comment | Kind::Math)) {
        return Some(*k);
    }
    let inner = *kinds.last()?;
    if inner == Kind::Punctuation
        && let Some(k) = kinds.iter().rev().find(|k| {
            matches!(
                k,
                Kind::String | Kind::Heading | Kind::Emphasis | Kind::Strong | Kind::Link | Kind::Code | Kind::Quote
            )
        })
    {
        return Some(*k);
    }
    Some(inner)
}

/// Short names in code block info strings that are not file extensions.
const ALIASES: &[(&str, &str)] = &[
    ("shell", "sh"),
    ("console", "sh"),
    ("zsh", "sh"),
    ("bash", "sh"),
    ("m", "matlab"),
    ("octave", "matlab"),
    ("tex", "latex"),
    ("python3", "py"),
    ("rust", "rs"),
    ("javascript", "js"),
    ("c++", "cpp"),
    ("markdown", "md"),
    ("yml", "yaml"),
];

fn syntax(language: &str) -> Option<&'static SyntaxReference> {
    let token = language.trim().to_lowercase();
    if token.is_empty() {
        return None;
    }
    let token = ALIASES.iter().find(|(a, _)| *a == token).map_or(token.as_str(), |(_, to)| to);
    // MATLAB has only the extension "matlab": `.m` would otherwise be Objective-C.
    if token == "matlab" {
        return SYNTAXES.find_syntax_by_name("MATLAB");
    }
    SYNTAXES.find_syntax_by_token(token)
}

/// The language of a file by its extension (or name, e.g. `.bashrc`), as
/// the name to pass to [`highlight`]; `None` for plain text.
pub fn language_for_path(path: &Path) -> Option<String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let found = if ext.eq_ignore_ascii_case("m") {
        syntax("matlab")
    } else {
        syntax(ext).or_else(|| SYNTAXES.find_syntax_by_extension(name))
    };
    found.filter(|s| s.name != "Plain Text").map(|s| s.name.clone())
}

/// The language of a fenced code block by its info string (```` ```py ````,
/// ```` ```{r} ````, ```` ```matlab title="x" ````).
pub fn language_for_fence(info: &str) -> Option<String> {
    let word = info.trim().trim_start_matches('{').split(|c: char| c.is_whitespace() || c == '}' || c == ',').next()?;
    syntax(word).filter(|s| s.name != "Plain Text").map(|s| s.name.clone())
}

/// Every known language: (name, file extensions).
pub fn languages() -> Vec<(String, Vec<String>)> {
    let mut out: Vec<_> = SYNTAXES
        .syntaxes()
        .iter()
        .filter(|s| !s.hidden && s.name != "Plain Text")
        .map(|s| (s.name.clone(), s.file_extensions.clone()))
        .collect();
    out.sort();
    out
}

/// Spans of one line (UTF-16, `offset` added); advances the parser.
fn line_spans(line: &str, offset: usize, state: &mut ParseState, stack: &mut ScopeStack) -> Vec<Span> {
    let ops = state.parse_line(line, &SYNTAXES).unwrap_or_default();
    // The newline itself is not coloured (scopes often end after it).
    let content = line.trim_end_matches(['\n', '\r']).len();
    let mut spans: Vec<Span> = Vec::new();
    let mut push = |from_b: usize, to_b: usize, kind: Option<Kind>| {
        let Some(kind) = kind else { return };
        let (from_b, to_b) = (from_b.min(content), to_b.min(content));
        if to_b <= from_b {
            return;
        }
        let start = offset + line[..from_b].encode_utf16().count();
        let end = start + line[from_b..to_b].encode_utf16().count();
        match spans.last_mut() {
            Some(last) if last.kind == kind && last.end == start => last.end = end,
            _ => spans.push(Span { start, end, kind }),
        }
    };
    let mut at = 0;
    for (pos, op) in ops {
        push(at, pos, kind_of_stack(stack));
        at = pos;
        let _ = stack.apply(&op);
    }
    push(at, line.len(), kind_of_stack(stack));
    spans
}

/// A fenced code block in Markdown: its fence and, for a known language,
/// that language's parser (the Markdown syntax of syntect does not
/// highlight code blocks by language).
#[derive(Clone, PartialEq, Eq)]
struct Fence {
    marker: char,
    len: usize,
    inner: Option<(ParseState, ScopeStack)>,
}

/// Parser state after a line.
#[derive(Clone, PartialEq, Eq)]
struct State {
    parse: ParseState,
    stack: ScopeStack,
    fence: Option<Fence>,
}

/// `(marker, length, info)` if `line` is a code fence (```` ``` ```` or `~~~`).
fn fence_of(line: &str) -> Option<(char, usize, &str)> {
    let t = line.trim_start();
    if line.len() - t.len() > 3 {
        return None;
    }
    let marker = t.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let len = t.chars().take_while(|c| *c == marker).count();
    (len >= 3).then(|| (marker, len, t[len..].trim()))
}

/// Highlights one line and advances `state`.
fn step(line: &str, offset: usize, state: &mut State, markdown: bool) -> Vec<Span> {
    let spans = line_spans(line, offset, &mut state.parse, &mut state.stack);
    if !markdown {
        return spans;
    }
    match &mut state.fence {
        Some(fence) => {
            let closing =
                fence_of(line).is_some_and(|(m, l, info)| m == fence.marker && l >= fence.len && info.is_empty());
            if closing {
                state.fence = None;
                return spans;
            }
            match &mut fence.inner {
                Some((parse, stack)) => line_spans(line, offset, parse, stack),
                None => spans,
            }
        }
        None => {
            if let Some((marker, len, info)) = fence_of(line) {
                let inner = language_for_fence(info)
                    .and_then(|name| SYNTAXES.find_syntax_by_name(&name))
                    .map(|s| (ParseState::new(s), ScopeStack::new()));
                state.fence = Some(Fence { marker, len, inner });
            }
            spans
        }
    }
}

/// All spans of `text` in `language` (name, extension or code block info);
/// empty for unknown languages.
pub fn highlight(text: &str, language: &str) -> Vec<Span> {
    let mut session = Session::new(language);
    session.update(text).to_vec()
}

/// One line as last highlighted: its text, the state after it and its spans.
#[derive(Clone)]
struct Line {
    text: String,
    state: State,
    spans: Vec<Span>,
}

/// Highlighting of a document that changes: keeps the parser state after
/// each line, so [`Session::update`] only re-parses from the first changed
/// line until a line ends in the same state as before.
pub struct Session {
    syntax: Option<&'static SyntaxReference>,
    markdown: bool,
    lines: Vec<Line>,
    spans: Vec<Span>,
}

impl Session {
    pub fn new(language: &str) -> Self {
        let syntax = syntax(language);
        let markdown = syntax.is_some_and(|s| s.name == "Markdown" || s.name == "MultiMarkdown");
        Session { syntax, markdown, lines: Vec::new(), spans: Vec::new() }
    }

    /// Whether the language is known (otherwise no spans).
    pub fn known(&self) -> bool {
        self.syntax.is_some()
    }

    /// Highlights `text` and returns all spans; work is done only from the
    /// first changed line on.
    pub fn update(&mut self, text: &str) -> &[Span] {
        let Some(syntax) = self.syntax else { return &[] };
        let new_lines: Vec<&str> = text.split_inclusive('\n').collect();
        // First line that differs.
        let first = self.lines.iter().zip(&new_lines).take_while(|(old, new)| old.text == **new).count();
        let mut out: Vec<Line> = self.lines[..first].to_vec();
        let mut state = match out.last() {
            Some(l) => l.state.clone(),
            None => State { parse: ParseState::new(syntax), stack: ScopeStack::new(), fence: None },
        };
        let mut offset: usize = out.iter().map(|l| l.text.encode_utf16().count()).sum();
        let old = std::mem::take(&mut self.lines);
        // Old lines after the edit, aligned from the end, for reuse once the
        // state matches again.
        let tail = old.len().min(new_lines.len());
        let mut i = first;
        while i < new_lines.len() {
            let line = new_lines[i];
            let from_end = new_lines.len() - i;
            let j = old.len().wrapping_sub(from_end);
            if from_end <= tail && i > first && j > 0 && old[j].text == line && old[j - 1].state == state {
                // Same text, same state before: the rest is unchanged, only
                // moved.
                let before: usize = old[..j].iter().map(|l| l.text.encode_utf16().count()).sum();
                let shift = offset as i64 - before as i64;
                for l in &old[j..] {
                    let mut l = l.clone();
                    for s in &mut l.spans {
                        s.start = (s.start as i64 + shift) as usize;
                        s.end = (s.end as i64 + shift) as usize;
                    }
                    out.push(l);
                }
                break;
            }
            let spans = step(line, offset, &mut state, self.markdown);
            out.push(Line { text: line.to_owned(), state: state.clone(), spans });
            offset += line.encode_utf16().count();
            i += 1;
        }
        self.lines = out;
        self.spans = self.lines.iter().flat_map(|l| l.spans.iter().copied()).collect();
        &self.spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text and kinds of the spans of `text`.
    fn kinds(text: &str, language: &str) -> Vec<(String, Kind)> {
        let units: Vec<u16> = text.encode_utf16().collect();
        highlight(text, language)
            .into_iter()
            .map(|s| (String::from_utf16_lossy(&units[s.start..s.end]), s.kind))
            .collect()
    }

    fn has(spans: &[(String, Kind)], text: &str, kind: Kind) -> bool {
        spans.iter().any(|(t, k)| t.contains(text) && *k == kind)
    }

    #[test]
    fn recognizes_languages() {
        let p = |s: &str| language_for_path(Path::new(s));
        assert_eq!(p("notes/a.md").as_deref(), Some("Markdown"));
        assert_eq!(p("src/main.rs").as_deref(), Some("Rust"));
        assert_eq!(p("x.py").as_deref(), Some("Python"));
        assert_eq!(p("signal.m").as_deref(), Some("MATLAB"));
        assert_eq!(p("thesis.tex").as_deref(), Some("LaTeX"));
        assert_eq!(p("run.sh").as_deref(), Some("Bourne Again Shell (bash)"));
        assert_eq!(p("notes.txt"), None);
        assert_eq!(p("Makefile").as_deref(), Some("Makefile"));
        let f = |s: &str| language_for_fence(s);
        assert_eq!(f("py").as_deref(), Some("Python"));
        assert_eq!(f("python").as_deref(), Some("Python"));
        assert_eq!(f("{r}").as_deref(), Some("R"));
        assert_eq!(f("matlab title=\"x\"").as_deref(), Some("MATLAB"));
        assert_eq!(f("tex").as_deref(), Some("LaTeX"));
        assert_eq!(f("shell").as_deref(), Some("Bourne Again Shell (bash)"));
        assert_eq!(f("mermaid"), None);
        assert_eq!(f(""), None);
        assert!(languages().iter().any(|(n, _)| n == "Rust"));
    }

    #[test]
    fn rust() {
        let s = kinds("// Kommentar\nfn main() {\n    let x = 42; // da\n    println!(\"hi\\n\");\n}\n", "rs");
        assert!(has(&s, "Kommentar", Kind::Comment));
        assert!(has(&s, "fn", Kind::Keyword));
        assert!(has(&s, "main", Kind::Function));
        assert!(has(&s, "let", Kind::Keyword));
        assert!(has(&s, "42", Kind::Number));
        assert!(has(&s, "hi", Kind::String));
        assert!(has(&s, "\\n", Kind::Escape));
    }

    #[test]
    fn python() {
        let s = kinds("def f(a):\n    \"\"\"Doc\"\"\"\n    return a + 1  # eins\n", "py");
        assert!(has(&s, "def", Kind::Keyword));
        assert!(has(&s, "f", Kind::Function));
        assert!(has(&s, "Doc", Kind::Comment) || has(&s, "Doc", Kind::String));
        assert!(has(&s, "return", Kind::Keyword));
        assert!(has(&s, "1", Kind::Number));
        assert!(has(&s, "eins", Kind::Comment));
    }

    #[test]
    fn matlab() {
        let s = kinds("function y = f(x)\n  % Kommentar\n  y = x.^2 + 3;\nend\n", "matlab");
        assert!(has(&s, "function", Kind::Keyword));
        assert!(has(&s, "Kommentar", Kind::Comment));
        assert!(has(&s, "3", Kind::Number));
        assert!(has(&s, "end", Kind::Keyword));
    }

    #[test]
    fn latex() {
        let s = kinds("\\section{Einleitung}\nText mit $E = mc^2$ und \\textbf{fett}. % Notiz\n", "tex");
        assert!(has(&s, "section", Kind::Function) || has(&s, "section", Kind::Keyword), "{s:?}");
        assert!(has(&s, "Einleitung", Kind::Heading), "{s:?}");
        assert!(has(&s, "mc", Kind::Math), "{s:?}");
        assert!(has(&s, "Notiz", Kind::Comment));
    }

    #[test]
    fn markdown_with_code_blocks() {
        let text = "# Titel\n\nText *kursiv* und **fett**, `code` und [Link](a.md).\n\n- Punkt\n\n> Zitat\n\n```rust\nfn main() {}\n```\n\n```python\nimport os\n```\n";
        let s = kinds(text, "md");
        assert!(has(&s, "Titel", Kind::Heading), "{s:?}");
        assert!(has(&s, "kursiv", Kind::Emphasis));
        assert!(has(&s, "fett", Kind::Strong));
        assert!(has(&s, "code", Kind::Code));
        assert!(has(&s, "Link", Kind::Link) || has(&s, "a.md", Kind::Link), "{s:?}");
        assert!(has(&s, "-", Kind::List), "{s:?}");
        assert!(has(&s, "Zitat", Kind::Quote) || s.iter().any(|(t, k)| t.contains('>') && *k == Kind::Quote), "{s:?}");
        // Embedded languages in fenced code blocks.
        assert!(has(&s, "fn", Kind::Keyword), "{s:?}");
        assert!(has(&s, "import", Kind::Keyword), "{s:?}");
    }

    #[test]
    fn positions_are_utf16() {
        let text = "// äöü 🙂\nlet x = 1;\n";
        let spans = highlight(text, "rs");
        let units: Vec<u16> = text.encode_utf16().collect();
        let comment = spans.iter().find(|s| s.kind == Kind::Comment).unwrap();
        assert_eq!(String::from_utf16_lossy(&units[comment.start..comment.end]), "// äöü 🙂");
        let let_kw = spans.iter().find(|s| s.kind == Kind::Keyword).unwrap();
        assert_eq!(String::from_utf16_lossy(&units[let_kw.start..let_kw.end]), "let");
    }

    #[test]
    fn unknown_language_gives_no_spans() {
        assert!(highlight("anything", "no-such-language").is_empty());
        assert!(!Session::new("").known());
    }

    #[test]
    fn session_matches_full_highlight_after_edits() {
        let mut text = String::from("fn a() {}\n/* lang\n */\nfn b() { let s = \"x\"; }\n");
        for _ in 0..20 {
            text.push_str("fn c() { 1 + 2 }\n");
        }
        let mut session = Session::new("rs");
        assert_eq!(session.update(&text), highlight(&text, "rs").as_slice());
        // Typing in the middle, opening a comment that changes the rest,
        // closing it again, deleting lines.
        let edits: [(&str, &str); 4] =
            [("fn b()", "fn bb()"), ("fn bb()", "/* fn bb()"), ("/* fn bb()", "fn bb()"), ("/* lang\n */\n", "")];
        for (from, to) in edits {
            text = text.replacen(from, to, 1);
            assert_eq!(session.update(&text), highlight(&text, "rs").as_slice(), "after {from:?} -> {to:?}");
        }
    }

    #[test]
    fn session_follows_code_fences() {
        let mut text =
            String::from("# Notiz\n\n```rust\nfn a() {}\n```\n\nText *kursiv*\n\n```python\nimport os\n```\n");
        let mut session = Session::new("md");
        assert_eq!(session.update(&text), highlight(&text, "md").as_slice());
        // Closing fence removed: everything after becomes code; back again;
        // another language.
        let edits: [(&str, &str); 3] =
            [("fn a() {}\n```\n", "fn a() {}\n"), ("fn a() {}\n", "fn a() {}\n```\n"), ("```rust", "```python")];
        for (from, to) in edits {
            text = text.replacen(from, to, 1);
            assert_eq!(session.update(&text), highlight(&text, "md").as_slice(), "after {from:?} -> {to:?}");
        }
    }

    #[test]
    fn typing_in_a_long_file_is_quick() {
        let mut text = String::new();
        for i in 0..3000 {
            text.push_str(&format!("fn f{i}(x: u32) -> u32 {{ x + {i} }} // Zeile {i}\n"));
        }
        let mut session = Session::new("rs");
        let start = std::time::Instant::now();
        session.update(&text);
        let full = start.elapsed();
        text = text.replacen("fn f1500(", "fn g1500(", 1);
        let start = std::time::Instant::now();
        session.update(&text);
        let edit = start.elapsed();
        assert!(edit * 5 < full, "edit {edit:?} vs full {full:?}");
    }
}
