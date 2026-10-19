// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Quick open (Ctrl+P): the project's files and folders and fuzzy matching.
//!
//! The list respects `.gitignore` and leaves out typical build and
//! dependency folders even in projects without ignore files.

use std::path::{Path, PathBuf};

/// Folders that hold generated files or dependencies. Excluded from quick
/// open and (unless asked for) from the project search.
pub const BUILD_DIRS: &[&str] = &[
    ".git",
    ".atlas",
    "target",
    "node_modules",
    "build",
    "dist",
    "result",
    ".direnv",
    "__pycache__",
    ".venv",
    "venv",
    ".mypy_cache",
    ".pytest_cache",
    ".cache",
    ".gradle",
    ".next",
    ".nuxt",
    "zig-cache",
    ".zig-cache",
    "_build",
];

/// ripgrep globs excluding [`BUILD_DIRS`].
pub fn build_dir_globs() -> Vec<String> {
    BUILD_DIRS.iter().map(|d| format!("!**/{d}/**")).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Relative to the project root, with `/`.
    pub relative: String,
    pub is_dir: bool,
}

/// Upper bound for the quick open list (very large trees are cut off).
pub const MAX_ENTRIES: usize = 200_000;

/// All files and folders of the project (hidden ones included, ignored and
/// build folders excluded), sorted.
pub fn list(root: &Path) -> Vec<Entry> {
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .parents(true)
        .require_git(false)
        .filter_entry(|e| e.depth() == 0 || !(e.file_type().is_some_and(|t| t.is_dir()) && is_build_dir(e.file_name())))
        .build();
    let mut out = Vec::new();
    for entry in walker.flatten() {
        if entry.depth() == 0 {
            continue;
        }
        let Ok(relative) = entry.path().strip_prefix(root) else { continue };
        out.push(Entry {
            relative: relative.to_string_lossy().into_owned(),
            is_dir: entry.file_type().is_some_and(|t| t.is_dir()),
        });
        if out.len() >= MAX_ENTRIES {
            break;
        }
    }
    out.sort_by(|a, b| a.relative.cmp(&b.relative));
    out
}

fn is_build_dir(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|n| BUILD_DIRS.contains(&n))
}

/// Fuzzy score of `query` against `candidate` (higher is better), `None` if
/// the query's characters do not appear in order. Case-insensitive; spaces
/// in the query separate terms that must all match.
///
/// Bonuses: matches at word starts (after `/`, `_`, `-`, `.`, space or a
/// lower→upper case change), consecutive matches and matches in the last
/// path segment (the file name). Shorter candidates win ties.
pub fn score(query: &str, candidate: &str) -> Option<i32> {
    score_terms(&terms(query), candidate)
}

/// Lowercased query terms.
fn terms(query: &str) -> Vec<String> {
    query.split_whitespace().map(str::to_lowercase).collect()
}

fn score_terms(terms: &[String], candidate: &str) -> Option<i32> {
    let mut total = 0;
    for term in terms {
        total += if candidate.is_ascii() && term.is_ascii() {
            score_ascii(term.as_bytes(), candidate.as_bytes())?
        } else {
            score_term(term, candidate)?
        };
    }
    Some(total - candidate.chars().count() as i32 / 4)
}

/// Allocation-free variant of [`score_term`] for ASCII (the common case for
/// paths); `needle` is lowercase. Same scoring.
fn score_ascii(needle: &[u8], cand: &[u8]) -> Option<i32> {
    // Quick reject: the needle must be a subsequence at all.
    let mut it = cand.iter().map(u8::to_ascii_lowercase);
    if !needle.iter().all(|&c| it.any(|b| b == c)) {
        return None;
    }
    let name_start = cand.iter().rposition(|&b| b == b'/').map_or(0, |i| i + 1);
    let attempt = |start: usize| -> Option<i32> {
        let mut score = 0;
        let mut pos = start;
        let mut last: Option<usize> = None;
        for &c in needle {
            let found = (pos..cand.len()).find(|&i| cand[i].to_ascii_lowercase() == c)?;
            let boundary = found == 0
                || matches!(cand[found - 1], b'/' | b'_' | b'-' | b'.' | b' ')
                || (cand[found - 1].is_ascii_lowercase() && cand[found].is_ascii_uppercase());
            score += 1;
            if boundary {
                score += 8;
            }
            if last.is_some_and(|l| l + 1 == found) {
                score += 5;
            }
            if found >= name_start {
                score += 3;
            }
            last = Some(found);
            pos = found + 1;
        }
        Some(score)
    };
    let in_name = attempt(name_start).map(|s| s + 10);
    let anywhere = attempt(0);
    in_name.into_iter().chain(anywhere).max()
}

fn score_term(term: &str, candidate: &str) -> Option<i32> {
    let chars: Vec<char> = candidate.chars().collect();
    let lower: Vec<char> = candidate.chars().flat_map(char::to_lowercase).collect();
    if lower.len() != chars.len() {
        // Rare case-mapping changes length; fall back to a plain search.
        return candidate.to_lowercase().contains(&term.to_lowercase()).then_some(1);
    }
    let name_start = candidate.rfind('/').map(|i| candidate[..=i].chars().count()).unwrap_or(0);
    let needle: Vec<char> = term.chars().flat_map(char::to_lowercase).collect();

    // Prefer a match inside the file name: try to place the whole term there
    // first, then fall back to the full path.
    let attempt = |start: usize| -> Option<i32> {
        let mut score = 0;
        let mut pos = start;
        let mut last: Option<usize> = None;
        for &c in &needle {
            let found = (pos..lower.len()).find(|&i| lower[i] == c)?;
            let boundary = found == 0
                || matches!(chars[found - 1], '/' | '_' | '-' | '.' | ' ')
                || (chars[found - 1].is_lowercase() && chars[found].is_uppercase());
            score += 1;
            if boundary {
                score += 8;
            }
            if last.is_some_and(|l| l + 1 == found) {
                score += 5;
            }
            if found >= name_start {
                score += 3;
            }
            last = Some(found);
            pos = found + 1;
        }
        Some(score)
    };
    let in_name = attempt(name_start).map(|s| s + 10);
    let anywhere = attempt(0);
    in_name.into_iter().chain(anywhere).max()
}

/// Splits `src/main.rs:42` or `src/main.rs:42:7` into path and line.
pub fn split_location(query: &str) -> (&str, u32) {
    let mut parts = query.rsplitn(3, ':');
    let last = parts.next().unwrap_or_default();
    let middle = parts.next();
    let first = parts.next();
    match (first, middle.map(str::parse::<u32>), last.parse::<u32>()) {
        (Some(path), Some(Ok(line)), Ok(_column)) => (path, line),
        (None, Some(_), Ok(line)) => (&query[..query.len() - last.len() - 1], line),
        (Some(_), Some(Err(_)), Ok(line)) => (&query[..query.len() - last.len() - 1], line),
        _ => (query, 0),
    }
}

/// Best matches for `query`, highest score first (at most `limit`).
pub fn best<'a>(entries: &'a [Entry], query: &str, limit: usize) -> Vec<(&'a Entry, i32)> {
    let terms = terms(query);
    let mut scored: Vec<(&Entry, i32)> = entries
        .iter()
        .filter_map(|e| score_terms(&terms, &e.relative).map(|s| (e, s - if e.is_dir { 2 } else { 0 })))
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.relative.cmp(&b.0.relative)));
    scored.truncate(limit);
    scored
}

/// Absolute path of a relative entry.
pub fn absolute(root: &Path, entry: &Entry) -> PathBuf {
    root.join(&entry.relative)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_ranking() {
        assert!(score("prs", "src/parser.rs").is_some());
        assert!(score("xyz", "src/parser.rs").is_none());
        // File name beats a match spread over the path.
        assert!(score("main", "src/main.rs") > score("main", "maintenance/notes/x.md"));
        // Word starts beat inner letters.
        assert!(score("fr", "src/file_reader.rs") > score("fr", "src/offrange.rs"));
        // All terms must match.
        assert!(score("src main", "src/main.rs").is_some());
        assert!(score("src zzz", "src/main.rs").is_none());
        assert!(score("Main", "src/main.rs").is_some(), "case-insensitive");
    }

    #[test]
    fn lists_without_build_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for p in ["src/main.rs", "target/debug/x", "node_modules/a/index.js", "ignored/x", ".hidden/conf", "docs/a.md"]
        {
            let path = root.join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }
        std::fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
        let names: Vec<String> = list(root).into_iter().map(|e| e.relative).collect();
        assert_eq!(names, [".gitignore", ".hidden", ".hidden/conf", "docs", "docs/a.md", "src", "src/main.rs"]);
        let entries = list(root);
        assert_eq!(best(&entries, "main", 5)[0].0.relative, "src/main.rs");
    }

    #[test]
    fn locations() {
        assert_eq!(split_location("src/main.rs:42"), ("src/main.rs", 42));
        assert_eq!(split_location("src/main.rs:42:7"), ("src/main.rs", 42));
        assert_eq!(split_location("src/main.rs"), ("src/main.rs", 0));
        assert_eq!(split_location("a:b.rs"), ("a:b.rs", 0));
    }
}
