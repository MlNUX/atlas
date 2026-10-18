// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Symbol overview for the code preview.
//!
//! Atlas is no IDE: symbols come from conservative, line-based patterns for
//! common languages, not from a parser or language server. Only definitions
//! that can be recognised reliably are listed (functions, types, classes,
//! sections); everything else is left out rather than guessed. Comment and
//! string contents are not analysed, so a definition inside a block comment
//! may show up – acceptable for an overview.

use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Symbol {
    pub name: String,
    /// "function", "method", "type", "class", "module", "section", …
    pub kind: &'static str,
    /// 1-based line.
    pub line: usize,
    /// Nesting depth from indentation (0 = top level).
    pub depth: usize,
}

struct Rule {
    re: Regex,
    kind: &'static str,
}

fn rule(re: &str, kind: &'static str) -> Rule {
    Rule { re: Regex::new(re).expect("valid symbol pattern"), kind }
}

type Rules = Vec<Rule>;

fn rules_for(lang: &str) -> Option<&'static Rules> {
    macro_rules! lang {
        ($($re:expr => $kind:expr),* $(,)?) => {{
            static R: OnceLock<Rules> = OnceLock::new();
            Some(R.get_or_init(|| vec![$(rule($re, $kind)),*]))
        }};
    }
    match lang {
        "rust" => lang![
            r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+|async\s+|unsafe\s+|extern\s+\S+\s+)*fn\s+([A-Za-z_][A-Za-z0-9_]*)" => "function",
            r"^\s*(?:pub(?:\([^)]*\))?\s+)?struct\s+([A-Za-z_][A-Za-z0-9_]*)" => "type",
            r"^\s*(?:pub(?:\([^)]*\))?\s+)?enum\s+([A-Za-z_][A-Za-z0-9_]*)" => "type",
            r"^\s*(?:pub(?:\([^)]*\))?\s+)?union\s+([A-Za-z_][A-Za-z0-9_]*)" => "type",
            r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:unsafe\s+)?trait\s+([A-Za-z_][A-Za-z0-9_]*)" => "interface",
            r"^\s*(?:pub(?:\([^)]*\))?\s+)?type\s+([A-Za-z_][A-Za-z0-9_]*)" => "type",
            r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{" => "module",
            r"^\s*(?:unsafe\s+)?impl(?:<[^{]*?>)?\s+([^{]+?)\s*(?:where\b[^{]*)?\{?\s*$" => "impl",
            r"^\s*macro_rules!\s+([A-Za-z_][A-Za-z0-9_]*)" => "macro",
        ],
        "python" => lang![
            r"^\s*(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)" => "function",
            r"^\s*class\s+([A-Za-z_][A-Za-z0-9_]*)" => "class",
        ],
        "javascript" => lang![
            r"^\s*(?:export\s+)?(?:default\s+)?(?:async\s+)?function\s*\*?\s*([A-Za-z_$][A-Za-z0-9_$]*)" => "function",
            r"^\s*(?:export\s+)?(?:default\s+)?(?:abstract\s+)?class\s+([A-Za-z_$][A-Za-z0-9_$]*)" => "class",
            r"^\s*(?:export\s+)?interface\s+([A-Za-z_$][A-Za-z0-9_$]*)" => "interface",
            r"^\s*(?:export\s+)?type\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*(?:<[^=]*>)?\s*=" => "type",
            r"^\s*(?:export\s+)?enum\s+([A-Za-z_$][A-Za-z0-9_$]*)" => "type",
            r"^\s*(?:export\s+)?(?:const|let|var)\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*=\s*(?:async\s+)?(?:function\b|\([^)]*\)\s*(?::[^=]*)?=>|[A-Za-z_$][A-Za-z0-9_$]*\s*=>)" => "function",
        ],
        "go" => lang![
            r"^func\s+\([^)]*\)\s*([A-Za-z_][A-Za-z0-9_]*)" => "method",
            r"^func\s+([A-Za-z_][A-Za-z0-9_]*)" => "function",
            r"^type\s+([A-Za-z_][A-Za-z0-9_]*)" => "type",
        ],
        // Only definitions at column 0 with the opening brace on the same or
        // next line; declarations (ending in `;`) are skipped below.
        "c" => lang![
            r"^(?:struct|union|enum)\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{?\s*$" => "type",
            r"^(?:class|struct|namespace)\s+([A-Za-z_][A-Za-z0-9_:]*)[^;]*$" => "class",
            r"^(?:[A-Za-z_][A-Za-z0-9_:<>,\s\*&]*?[\s\*&])([A-Za-z_~][A-Za-z0-9_:~]*)\s*\([^;]*$" => "function",
            r"^#define\s+([A-Za-z_][A-Za-z0-9_]*)\(" => "macro",
        ],
        "java" => lang![
            r"^\s*(?:(?:public|protected|private|static|final|abstract|sealed)\s+)*(?:class|interface|enum|record)\s+([A-Za-z_][A-Za-z0-9_]*)" => "class",
            r"^\s+(?:(?:public|protected|private|static|final|abstract|synchronized|override|open|suspend)\s+)+(?:fun\s+)?[A-Za-z_<>\[\],\s]*?\b([A-Za-z_][A-Za-z0-9_]*)\s*\([^;]*$" => "method",
            r"^\s*(?:(?:private|public|internal|override|suspend)\s+)*fun\s+(?:<[^>]*>\s*)?([A-Za-z_][A-Za-z0-9_.]*)" => "function",
        ],
        "shell" => lang![
            r"^\s*(?:function\s+)?([A-Za-z_][A-Za-z0-9_:.-]*)\s*\(\)\s*\{?" => "function",
            r"^\s*function\s+([A-Za-z_][A-Za-z0-9_:.-]*)" => "function",
        ],
        "qml" => lang![
            r"^\s*(?:readonly\s+)?property\s+[A-Za-z_<>.]+\s+([A-Za-z_][A-Za-z0-9_]*)" => "property",
            r"^\s*function\s+([A-Za-z_][A-Za-z0-9_]*)" => "function",
            r"^\s*signal\s+([A-Za-z_][A-Za-z0-9_]*)" => "signal",
            r"^\s*id:\s*([A-Za-z_][A-Za-z0-9_]*)" => "object",
        ],
        "lua" => lang![
            r"^\s*(?:local\s+)?function\s+([A-Za-z_][A-Za-z0-9_.:]*)" => "function",
            r"^\s*(?:local\s+)?([A-Za-z_][A-Za-z0-9_.]*)\s*=\s*function\b" => "function",
        ],
        "ruby" => lang![
            r"^\s*def\s+(?:self\.)?([A-Za-z_][A-Za-z0-9_?!=]*)" => "function",
            r"^\s*class\s+([A-Z][A-Za-z0-9_:]*)" => "class",
            r"^\s*module\s+([A-Z][A-Za-z0-9_:]*)" => "module",
        ],
        "toml" => lang![
            r"^\s*\[\[([^\]]+)\]\]" => "section",
            r"^\s*\[([^\]]+)\]" => "section",
        ],
        "ini" => lang![r"^\s*\[([^\]]+)\]" => "section"],
        "nix" => lang![
            r"^\s{0,4}([A-Za-z_][A-Za-z0-9_'-]*(?:\.[A-Za-z_][A-Za-z0-9_'-]*)*)\s*=\s*(?:[A-Za-z_][A-Za-z0-9_]*\s*:|\{[^}]*\}\s*:|rec\s*\{|\{\s*$|let\b|\(?\s*[A-Za-z_][A-Za-z0-9_.]*\s+rec\s*\{)" => "attribute",
        ],
        "make" => lang![r"^([A-Za-z0-9_.%/-]+)\s*:(?:[^=]|$)" => "target"],
        _ => None,
    }
}

/// Language key used by [`symbols`], from the file name.
pub fn language(path: &Path) -> Option<&'static str> {
    let name = path.file_name()?.to_str()?;
    if matches!(name, "Makefile" | "makefile" | "GNUmakefile") {
        return Some("make");
    }
    if matches!(name, ".bashrc" | ".zshrc" | ".profile" | ".bash_profile") {
        return Some("shell");
    }
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "rs" => "rust",
        "py" | "pyi" => "python",
        "js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx" | "mts" | "cts" => "javascript",
        "go" => "go",
        "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" | "ino" => "c",
        "java" | "kt" | "kts" | "scala" => "java",
        "sh" | "bash" | "zsh" | "ksh" => "shell",
        "qml" => "qml",
        "lua" => "lua",
        "rb" | "rake" => "ruby",
        "toml" => "toml",
        "ini" | "cfg" | "conf" | "desktop" | "service" => "ini",
        "nix" => "nix",
        "mk" => "make",
        _ => return None,
    })
}

fn indent_of(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).map(|c| if c == '\t' { 4 } else { 1 }).sum()
}

/// Symbols of `text` (the content of `path`), in file order. Unknown
/// languages give an empty list.
pub fn symbols(path: &Path, text: &str) -> Vec<Symbol> {
    let Some(rules) = language(path).and_then(rules_for) else { return Vec::new() };
    let lang = language(path).unwrap_or_default();
    let mut out = Vec::new();
    // Indentation of enclosing symbols, for the nesting depth.
    let mut stack: Vec<usize> = Vec::new();
    let mut in_block_comment = false;
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        // Skip whole-line comments and C-style block comments.
        if in_block_comment {
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if trimmed.starts_with("/*") && !trimmed.contains("*/") {
            in_block_comment = true;
            continue;
        }
        if trimmed.starts_with("//") || trimmed.starts_with('#') && !matches!(lang, "c" | "rust") || trimmed.is_empty()
        {
            continue;
        }
        if line.len() > 2000 {
            continue; // minified code
        }
        for rule in rules {
            let Some(caps) = rule.re.captures(line) else { continue };
            let Some(name) = caps.get(1) else { continue };
            if lang == "c" && rule.kind == "function" && is_c_keyword(name.as_str()) {
                continue;
            }
            let indent = indent_of(line);
            while stack.last().is_some_and(|&i| i >= indent) {
                stack.pop();
            }
            let kind = if rule.kind == "function" && !stack.is_empty() { "method" } else { rule.kind };
            out.push(Symbol { name: name.as_str().trim().to_owned(), kind, line: i + 1, depth: stack.len() });
            stack.push(indent);
            break;
        }
    }
    out
}

fn is_c_keyword(name: &str) -> bool {
    matches!(name, "if" | "for" | "while" | "switch" | "return" | "sizeof" | "else" | "do" | "case")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(path: &str, text: &str) -> Vec<(String, &'static str, usize, usize)> {
        symbols(Path::new(path), text).into_iter().map(|s| (s.name, s.kind, s.line, s.depth)).collect()
    }

    #[test]
    fn rust() {
        let src = "\
pub struct Parser {
    pos: usize,
}

impl Parser {
    pub fn new() -> Self { todo!() }
    // fn commented() {}
    pub(crate) async fn run(&mut self) {}
}

impl<T: Clone> Iterator for Wrapper<T> {
    fn next(&mut self) -> Option<T> { None }
}

pub(crate) enum Kind { A }
trait Visit {}
mod tests {
    fn helper() {}
}
";
        assert_eq!(
            names("a.rs", src),
            vec![
                ("Parser".into(), "type", 1, 0),
                ("Parser".into(), "impl", 5, 0),
                ("new".into(), "method", 6, 1),
                ("run".into(), "method", 8, 1),
                ("Iterator for Wrapper<T>".into(), "impl", 11, 0),
                ("next".into(), "method", 12, 1),
                ("Kind".into(), "type", 15, 0),
                ("Visit".into(), "interface", 16, 0),
                ("tests".into(), "module", 17, 0),
                ("helper".into(), "method", 18, 1),
            ]
        );
    }

    #[test]
    fn python_js_go_c() {
        assert_eq!(
            names("a.py", "class A:\n    def f(self):\n        pass\n# def no():\ndef g():\n    pass\n"),
            vec![("A".into(), "class", 1, 0), ("f".into(), "method", 2, 1), ("g".into(), "function", 5, 0)]
        );
        assert_eq!(
            names("a.ts", "export function a() {}\nconst b = async (x) => x\nexport class C {}\nconst d = 3\n"),
            vec![("a".into(), "function", 1, 0), ("b".into(), "function", 2, 0), ("C".into(), "class", 3, 0)]
        );
        assert_eq!(
            names("a.go", "func (s *S) Run() {}\nfunc main() {}\ntype S struct{}\n"),
            vec![("Run".into(), "method", 1, 0), ("main".into(), "function", 2, 0), ("S".into(), "type", 3, 0)]
        );
        assert_eq!(
            names(
                "a.c",
                "static int add(int a, int b)\n{\n    if (a) return b;\n}\nint decl(void);\nstruct point {\n/* int fake(void) {\n*/\n"
            ),
            vec![("add".into(), "function", 1, 0), ("point".into(), "type", 6, 0)]
        );
    }

    #[test]
    fn config_files() {
        assert_eq!(
            names("Cargo.toml", "[package]\nname = \"x\"\n[[bin]]\n"),
            vec![("package".into(), "section", 1, 0), ("bin".into(), "section", 3, 0)]
        );
        assert!(names("notes.txt", "fn main() {}").is_empty(), "unknown languages give nothing");
    }
}
