// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Project search on top of `ripgrep`, plus the text of PDFs.
//!
//! `rg --json` runs as a child process; its output is parsed on a background
//! thread and delivered in batches, so the UI never waits for the search.
//! ripgrep's defaults already respect `.gitignore`, `.ignore` and hidden
//! files. PDFs (which ripgrep cannot read) are searched afterwards on the
//! same thread, page by page, with the same rules.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use serde::Deserialize;

/// Upper bound of reported matches; the search stops afterwards.
pub const MAX_MATCHES: usize = 10_000;
const BATCH_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Default)]
pub struct Query {
    pub pattern: String,
    /// Treat `pattern` as a regular expression (otherwise literal).
    pub regex: bool,
    /// `None` = smart case.
    pub case_sensitive: Option<bool>,
    /// Only whole words (`rg --word-regexp`).
    pub whole_word: bool,
    /// Glob filters such as `*.rs` or `!target/**`.
    pub globs: Vec<String>,
    /// Search text files (ripgrep).
    pub text: bool,
    /// Search the text of PDFs.
    pub pdf: bool,
}

impl Query {
    /// A search of text files and PDFs for `pattern`.
    pub fn new(pattern: impl Into<String>) -> Self {
        Self { pattern: pattern.into(), text: true, pdf: true, ..Default::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub path: PathBuf,
    /// Path relative to the project root.
    pub relative: String,
    /// Line in a text file; page (1-based) in a PDF.
    pub line: u64,
    pub text: String,
    /// Byte range of the first submatch in `text`.
    pub column: (usize, usize),
    /// Byte ranges of all submatches in `text`.
    pub ranges: Vec<(usize, usize)>,
    /// Found in the text of a PDF (`line` is the page).
    pub pdf: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Summary {
    pub matches: usize,
    pub truncated: bool,
    pub cancelled: bool,
    pub elapsed: Duration,
    pub error: Option<String>,
}

/// Document types of the search filter and their file extensions (PDFs
/// are searched by their text, not by ripgrep).
pub const KINDS: &[(&str, &[&str])] = &[
    ("markdown", &["md", "markdown", "mdx", "qmd", "rmd"]),
    ("pdf", &["pdf"]),
    ("latex", &["tex", "bib", "sty", "cls", "bst", "ltx"]),
    (
        "code",
        &[
            "rs", "py", "js", "mjs", "ts", "tsx", "jsx", "qml", "c", "h", "cc", "cpp", "cxx", "hpp", "java", "kt",
            "go", "sh", "bash", "zsh", "fish", "nix", "toml", "json", "yaml", "yml", "xml", "html", "css", "scss",
            "lua", "rb", "php", "sql", "swift", "cs", "r", "jl", "hs", "ml", "ex", "exs", "dart", "vue", "svelte", "m",
            "ipynb", "cmake", "mk", "gradle",
        ],
    ),
    ("text", &["txt", "csv", "tsv", "log", "org", "rst", "adoc", "ini", "cfg", "conf", "srt"]),
];

/// The document type of `path` (a name from [`KINDS`]), `"other"` for
/// other files.
pub fn kind_of(path: &Path) -> &'static str {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else { return "other" };
    let ext = ext.to_ascii_lowercase();
    KINDS.iter().find(|(_, exts)| exts.contains(&ext.as_str())).map_or("other", |(name, _)| name)
}

/// Applies a type filter to `query`: only files of `kinds` (names from
/// [`KINDS`]); no kinds means all files.
pub fn restrict_to_kinds(query: &mut Query, kinds: &[&str]) {
    let selected: Vec<&(&str, &[&str])> = KINDS.iter().filter(|(name, _)| kinds.contains(name)).collect();
    if selected.is_empty() {
        query.text = true;
        query.pdf = true;
        return;
    }
    query.pdf = selected.iter().any(|(name, _)| *name == "pdf");
    query.text = selected.iter().any(|(name, _)| *name != "pdf");
    let globs = selected
        .iter()
        .filter(|(name, _)| *name != "pdf")
        .flat_map(|(_, exts)| exts.iter())
        .map(|ext| format!("*.{ext}"))
        // Without these, PDFs would no longer pass the globs.
        .chain(query.pdf.then(|| "*.pdf".to_owned()));
    query.globs.extend(globs);
}

/// Path of the ripgrep binary: `ATLAS_RG_BIN` at runtime, then the path
/// baked in by the Nix build, then `rg` from `PATH`.
pub fn rg_binary() -> PathBuf {
    std::env::var_os("ATLAS_RG_BIN")
        .map(PathBuf::from)
        .or_else(|| option_env!("ATLAS_RG_BIN").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("rg"))
}

/// A running search. Dropping the handle cancels it.
pub struct SearchHandle {
    child: Arc<Mutex<Option<Child>>>,
    cancelled: Arc<AtomicBool>,
}

impl SearchHandle {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        if let Some(mut child) = self.child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for SearchHandle {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn build_command(root: &Path, query: &Query) -> Command {
    let mut cmd = Command::new(rg_binary());
    cmd.arg("--json").arg("--max-columns=400").arg("--max-columns-preview").arg("--max-filesize=10M");
    // Stream results: ripgrep block-buffers its output when stdout is a pipe.
    cmd.arg("--line-buffered");
    match query.case_sensitive {
        None => cmd.arg("--smart-case"),
        Some(true) => cmd.arg("--case-sensitive"),
        Some(false) => cmd.arg("--ignore-case"),
    };
    if !query.regex {
        cmd.arg("--fixed-strings");
    }
    if query.whole_word {
        cmd.arg("--word-regexp");
    }
    for glob in &query.globs {
        cmd.arg("--glob").arg(glob);
    }
    cmd.arg("--").arg(&query.pattern).arg(root);
    cmd.current_dir(root).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd
}

/// Starts a search. `on_batch` receives matches in file order (the
/// matches of a file always together), `on_done` is called exactly once at
/// the end (also after cancellation). Both run on a background thread.
pub fn start(
    root: &Path,
    query: Query,
    mut on_batch: impl FnMut(Vec<Match>) + Send + 'static,
    on_done: impl FnOnce(Summary) + Send + 'static,
) -> Result<SearchHandle> {
    let started = Instant::now();
    // Checked before ripgrep starts, so a bad pattern is reported at once.
    let pdf_regex = if query.pdf { Some(pdf_regex(&query)?) } else { None };
    let (child, stdout, stderr) = if query.text {
        let mut child = build_command(root, &query)
            .spawn()
            .with_context(|| format!("cannot start ripgrep ({})", rg_binary().display()))?;
        let stdout = child.stdout.take().context("ripgrep without stdout")?;
        let stderr = child.stderr.take();
        (Some(child), Some(stdout), stderr)
    } else {
        (None, None, None)
    };
    let child = Arc::new(Mutex::new(child));
    let cancelled = Arc::new(AtomicBool::new(false));
    let handle = SearchHandle { child: child.clone(), cancelled: cancelled.clone() };
    let root = root.to_path_buf();

    std::thread::Builder::new().name("atlas-search".into()).spawn(move || {
        let mut summary = Summary::default();
        let mut batch = Vec::new();
        let mut last_flush = Instant::now();
        if let Some(stdout) = stdout {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Some(m) = parse_line(&line, &root) {
                    batch.push(m);
                    summary.matches += 1;
                    if summary.matches >= MAX_MATCHES {
                        summary.truncated = true;
                        break;
                    }
                }
                if !batch.is_empty() && last_flush.elapsed() >= BATCH_INTERVAL {
                    on_batch(std::mem::take(&mut batch));
                    last_flush = Instant::now();
                }
            }
            match child.lock().unwrap().take() {
                Some(mut c) => {
                    if summary.truncated {
                        let _ = c.kill();
                    }
                    let status = c.wait();
                    // Exit code 1 means "no match"; 2 means an error occurred
                    // (possibly only for some files).
                    if let Ok(status) = status
                        && status.code() == Some(2)
                    {
                        let mut msg = String::new();
                        if let Some(mut err) = stderr {
                            use std::io::Read;
                            let _ = err.read_to_string(&mut msg);
                        }
                        summary.error = Some(msg.lines().next().unwrap_or("ripgrep error").to_owned());
                    }
                }
                None => summary.cancelled = true,
            }
        }
        if let Some(regex) = pdf_regex
            && !summary.cancelled
            && !summary.truncated
        {
            for path in pdf_files(&root, &query.globs) {
                if cancelled.load(Ordering::Relaxed) {
                    summary.cancelled = true;
                    break;
                }
                let found = search_pdf(&path, &root, &regex);
                summary.matches += found.len();
                batch.extend(found);
                if summary.matches >= MAX_MATCHES {
                    batch.truncate(batch.len().saturating_sub(summary.matches - MAX_MATCHES));
                    summary.matches = MAX_MATCHES;
                    summary.truncated = true;
                    break;
                }
                if !batch.is_empty() && last_flush.elapsed() >= BATCH_INTERVAL {
                    on_batch(std::mem::take(&mut batch));
                    last_flush = Instant::now();
                }
            }
        }
        if cancelled.load(Ordering::Relaxed) {
            summary.cancelled = true;
        }
        if !batch.is_empty() && !summary.cancelled {
            on_batch(batch);
        }
        summary.elapsed = started.elapsed();
        on_done(summary);
    })?;

    Ok(handle)
}

/// The pattern of `query` for the text of PDFs, with ripgrep's rules for
/// case, literal text and whole words.
fn pdf_regex(query: &Query) -> Result<regex::Regex> {
    let pattern = if query.regex { query.pattern.clone() } else { regex::escape(&query.pattern) };
    let pattern = if query.whole_word { format!(r"\b(?:{pattern})\b") } else { pattern };
    let insensitive = match query.case_sensitive {
        Some(sensitive) => !sensitive,
        None => !query.pattern.chars().any(char::is_uppercase),
    };
    regex::RegexBuilder::new(&pattern)
        .case_insensitive(insensitive)
        .size_limit(1 << 22)
        .build()
        .with_context(|| format!("ungültiger Suchausdruck: {}", query.pattern))
}

/// The PDFs below `root`, with the ignore rules ripgrep uses and the
/// `globs` of the query, sorted by path.
fn pdf_files(root: &Path, globs: &[String]) -> Vec<PathBuf> {
    let mut overrides = ignore::overrides::OverrideBuilder::new(root);
    for glob in globs {
        let _ = overrides.add(glob);
    }
    let mut walker = ignore::WalkBuilder::new(root);
    if let Ok(overrides) = overrides.build() {
        walker.overrides(overrides);
    }
    let mut files: Vec<PathBuf> = walker
        .build()
        .flatten()
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .map(ignore::DirEntry::into_path)
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")))
        .collect();
    files.sort();
    files
}

/// Largest PDF whose text is searched.
const MAX_PDF_SIZE: u64 = 200 * 1024 * 1024;
/// Upper bound of the cached PDF text (bytes).
const PDF_CACHE_BYTES: usize = 64 * 1024 * 1024;

type PdfCache = HashMap<PathBuf, (SystemTime, u64, Arc<Vec<String>>)>;

/// The text of each page of `path`, cached by modification time and size.
fn pdf_text(path: &Path) -> Option<Arc<Vec<String>>> {
    static CACHE: OnceLock<Mutex<PdfCache>> = OnceLock::new();
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > MAX_PDF_SIZE {
        return None;
    }
    let stamp = (meta.modified().ok()?, meta.len());
    let cache = CACHE.get_or_init(Mutex::default);
    if let Some((time, len, pages)) = cache.lock().ok()?.get(path)
        && (*time, *len) == stamp
    {
        return Some(pages.clone());
    }
    let pages = Arc::new(crate::pdf::page_texts(path).ok()?);
    let mut cache = cache.lock().ok()?;
    let size: usize = cache.values().map(|(_, _, p)| p.iter().map(String::len).sum::<usize>()).sum();
    if size > PDF_CACHE_BYTES {
        cache.clear();
    }
    cache.insert(path.to_path_buf(), (stamp.0, stamp.1, pages.clone()));
    Some(pages)
}

/// The text of page `page` (1-based) of a PDF, from the search cache.
pub fn pdf_page_text(path: &Path, page: usize) -> Option<String> {
    pdf_text(path)?.get(page.checked_sub(1)?).cloned()
}

/// Longest line of a PDF match shown (the text around the match).
const PDF_LINE_CHARS: usize = 400;

fn search_pdf(path: &Path, root: &Path, regex: &regex::Regex) -> Vec<Match> {
    let Some(pages) = pdf_text(path) else { return Vec::new() };
    let relative = path.strip_prefix(root).unwrap_or(path).to_string_lossy().into_owned();
    let mut out = Vec::new();
    for (page, text) in pages.iter().enumerate() {
        for line in text.lines() {
            let ranges: Vec<(usize, usize)> =
                regex.find_iter(line).filter(|m| !m.is_empty()).map(|m| (m.start(), m.end())).collect();
            let Some(&first) = ranges.first() else { continue };
            let (text, ranges) = clip_line(line, first.0, &ranges);
            out.push(Match {
                path: path.to_path_buf(),
                relative: relative.clone(),
                line: page as u64 + 1,
                text,
                column: ranges[0],
                ranges,
                pdf: true,
            });
            if out.len() >= MAX_MATCHES {
                return out;
            }
        }
    }
    out
}

/// Cuts very long lines (whole paragraphs in some PDFs) to the part around
/// the first match; ranges move along.
fn clip_line(line: &str, first: usize, ranges: &[(usize, usize)]) -> (String, Vec<(usize, usize)>) {
    let line = line.trim_end();
    if line.len() <= PDF_LINE_CHARS {
        return (line.to_owned(), ranges.to_vec());
    }
    let mut from = first.saturating_sub(PDF_LINE_CHARS / 4);
    while !line.is_char_boundary(from) {
        from -= 1;
    }
    let mut to = (from + PDF_LINE_CHARS).min(line.len());
    while !line.is_char_boundary(to) {
        to -= 1;
    }
    let ranges = ranges.iter().filter(|(s, e)| *s >= from && *e <= to).map(|(s, e)| (s - from, e - from)).collect();
    (line[from..to].to_owned(), ranges)
}

#[derive(Deserialize)]
struct RgMessage {
    #[serde(rename = "type")]
    kind: String,
    data: Option<RgMatch>,
}

#[derive(Deserialize)]
struct RgMatch {
    path: Option<RgText>,
    lines: Option<RgText>,
    line_number: Option<u64>,
    #[serde(default)]
    submatches: Vec<RgSubmatch>,
}

#[derive(Deserialize)]
struct RgText {
    text: Option<String>,
}

#[derive(Deserialize)]
struct RgSubmatch {
    start: usize,
    end: usize,
}

fn parse_line(line: &str, root: &Path) -> Option<Match> {
    if !line.starts_with("{\"type\":\"match\"") {
        return None;
    }
    let msg: RgMessage = serde_json::from_str(line).ok()?;
    if msg.kind != "match" {
        return None;
    }
    let data = msg.data?;
    // Non UTF-8 paths/lines are reported as base64 `bytes`; skipped here.
    let path = PathBuf::from(data.path?.text?);
    let text = data.lines?.text?;
    let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().into_owned();
    let ranges: Vec<(usize, usize)> = data.submatches.iter().map(|s| (s.start, s.end)).collect();
    let column = ranges.first().copied().unwrap_or((0, 0));
    let text = text.trim_end_matches(['\n', '\r']).to_owned();
    Some(Match { path, relative, line: data.line_number.unwrap_or(0), text, column, ranges, pdf: false })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    fn run(root: &Path, query: Query) -> (Vec<Match>, Summary) {
        let (tx, rx) = unbounded();
        let (done_tx, done_rx) = unbounded();
        let _h = start(root, query, move |b| tx.send(b).unwrap(), move |s| done_tx.send(s).unwrap())
            .expect("ripgrep available");
        let summary = done_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let mut all: Vec<Match> = rx.try_iter().flatten().collect();
        all.sort_by(|a, b| a.relative.cmp(&b.relative));
        (all, summary)
    }

    #[test]
    fn respects_gitignore() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        // ripgrep only honours .gitignore inside a git repository.
        std::fs::create_dir(root.join(".git")).unwrap();
        std::fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
        std::fs::create_dir(root.join("ignored")).unwrap();
        std::fs::write(root.join("ignored/a.txt"), "needle\n").unwrap();
        std::fs::write(root.join("b.txt"), "hay\nneedle here\n").unwrap();
        let (matches, summary) = run(root, Query::new("needle"));
        assert_eq!(summary.matches, 1);
        assert_eq!(matches[0].relative, "b.txt");
        assert_eq!(matches[0].line, 2);
        assert_eq!(matches[0].column, (0, 6));
    }

    #[test]
    fn literal_by_default_regex_on_request() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.txt"), "a.c\nabc\n").unwrap();
        let (m, _) = run(d.path(), Query::new("a.c"));
        assert_eq!(m.len(), 1);
        let (m, _) = run(d.path(), Query { regex: true, ..Query::new("a.c") });
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn whole_words_case_and_all_ranges() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.txt"), "Wort wort Wortschatz\n").unwrap();
        let (m, _) = run(d.path(), Query { case_sensitive: Some(false), ..Query::new("wort") });
        assert_eq!(m[0].ranges, vec![(0, 4), (5, 9), (10, 14)]);
        let (m, _) = run(d.path(), Query { case_sensitive: Some(false), whole_word: true, ..Query::new("wort") });
        assert_eq!(m[0].ranges, vec![(0, 4), (5, 9)]);
        let (m, _) = run(d.path(), Query { case_sensitive: Some(true), whole_word: true, ..Query::new("wort") });
        assert_eq!(m[0].ranges, vec![(5, 9)]);
    }

    fn pdf_with(path: &Path, pages: &[&str]) {
        let surface = cairo::PdfSurface::new(595.0, 842.0, path).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        cr.set_font_size(20.0);
        for text in pages {
            cr.move_to(100.0, 200.0);
            cr.show_text(text).unwrap();
            cr.show_page().unwrap();
        }
        surface.finish();
    }

    #[test]
    fn finds_text_in_pdfs() {
        let d = tempfile::tempdir().unwrap();
        pdf_with(&d.path().join("skript.pdf"), &["Einleitung", "Die Fourier Transformation"]);
        std::fs::write(d.path().join("notiz.md"), "fourier\n").unwrap();
        let (m, summary) = run(d.path(), Query { case_sensitive: Some(false), ..Query::new("fourier") });
        assert_eq!(summary.matches, 2);
        let pdf = m.iter().find(|m| m.pdf).expect("PDF match");
        assert_eq!(pdf.relative, "skript.pdf");
        assert_eq!(pdf.line, 2, "page");
        assert_eq!(&pdf.text[pdf.column.0..pdf.column.1], "Fourier");
        // Only PDFs, only text files.
        let (m, _) = run(d.path(), Query { text: false, ..Query::new("Fourier") });
        assert!(m.len() == 1 && m[0].pdf);
        let (m, _) = run(d.path(), Query { pdf: false, case_sensitive: Some(false), ..Query::new("fourier") });
        assert!(m.len() == 1 && !m[0].pdf);
        // Globs apply to PDFs as well.
        let (m, _) = run(d.path(), Query { globs: vec!["*.md".into()], ..Query::new("Fourier") });
        assert!(m.iter().all(|m| !m.pdf));
    }

    #[test]
    fn bad_pattern_is_an_error() {
        let d = tempfile::tempdir().unwrap();
        assert!(start(d.path(), Query { regex: true, ..Query::new("(") }, |_| {}, |_| {}).is_err());
    }

    #[test]
    fn long_lines_are_clipped_around_the_match() {
        let line = format!("{}needle{}", "a".repeat(1000), "b".repeat(1000));
        let (text, ranges) = clip_line(&line, 1000, &[(1000, 1006)]);
        assert!(text.len() <= PDF_LINE_CHARS);
        assert_eq!(&text[ranges[0].0..ranges[0].1], "needle");
    }

    #[test]
    fn kinds_become_globs() {
        let mut q = Query::new("x");
        restrict_to_kinds(&mut q, &[]);
        assert!(q.text && q.pdf && q.globs.is_empty());
        let mut q = Query::new("x");
        restrict_to_kinds(&mut q, &["pdf"]);
        assert!(!q.text && q.pdf);
        let mut q = Query::new("x");
        restrict_to_kinds(&mut q, &["markdown", "latex"]);
        assert!(q.text && !q.pdf);
        assert!(q.globs.contains(&"*.md".to_owned()) && q.globs.contains(&"*.tex".to_owned()));
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.md"), "x\n").unwrap();
        std::fs::write(d.path().join("b.rs"), "x\n").unwrap();
        let (m, _) = run(d.path(), q);
        assert_eq!(m.iter().map(|m| m.relative.as_str()).collect::<Vec<_>>(), ["a.md"]);
    }

    #[test]
    fn kind_of_files() {
        assert_eq!(kind_of(Path::new("a/B.MD")), "markdown");
        assert_eq!(kind_of(Path::new("skript.pdf")), "pdf");
        assert_eq!(kind_of(Path::new("main.rs")), "code");
        assert_eq!(kind_of(Path::new("arbeit.tex")), "latex");
        assert_eq!(kind_of(Path::new("Makefile")), "other");
    }
}
