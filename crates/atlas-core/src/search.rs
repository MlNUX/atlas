// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Project search on top of `ripgrep`.
//!
//! `rg --json` runs as a child process; its output is parsed on a background
//! thread and delivered in batches, so the UI never waits for the search.
//! ripgrep's defaults already respect `.gitignore`, `.ignore` and hidden
//! files.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
    /// Glob filters such as `*.rs` or `!target/**`.
    pub globs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub path: PathBuf,
    /// Path relative to the project root.
    pub relative: String,
    pub line: u64,
    pub text: String,
    /// Byte range of the first submatch in `text`.
    pub column: (usize, usize),
}

#[derive(Debug, Clone, Default)]
pub struct Summary {
    pub matches: usize,
    pub truncated: bool,
    pub cancelled: bool,
    pub elapsed: Duration,
    pub error: Option<String>,
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
}

impl SearchHandle {
    pub fn cancel(&self) {
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
    for glob in &query.globs {
        cmd.arg("--glob").arg(glob);
    }
    cmd.arg("--").arg(&query.pattern).arg(root);
    cmd.current_dir(root).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd
}

/// Starts a search. `on_batch` receives matches in file order, `on_done`
/// is called exactly once at the end (also after cancellation). Both run on
/// a background thread.
pub fn start(
    root: &Path,
    query: Query,
    mut on_batch: impl FnMut(Vec<Match>) + Send + 'static,
    on_done: impl FnOnce(Summary) + Send + 'static,
) -> Result<SearchHandle> {
    let started = Instant::now();
    let mut child = build_command(root, &query)
        .spawn()
        .with_context(|| format!("cannot start ripgrep ({})", rg_binary().display()))?;
    let stdout = child.stdout.take().context("ripgrep without stdout")?;
    let stderr = child.stderr.take();
    let child = Arc::new(Mutex::new(Some(child)));
    let handle = SearchHandle { child: child.clone() };
    let root = root.to_path_buf();

    std::thread::Builder::new().name("atlas-search".into()).spawn(move || {
        let mut summary = Summary::default();
        let mut batch = Vec::new();
        let mut last_flush = Instant::now();
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
        if !batch.is_empty() {
            on_batch(batch);
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
        summary.elapsed = started.elapsed();
        on_done(summary);
    })?;

    Ok(handle)
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
    let column = data.submatches.first().map_or((0, 0), |s| (s.start, s.end));
    let text = text.trim_end_matches(['\n', '\r']).to_owned();
    Some(Match { path, relative, line: data.line_number.unwrap_or(0), text, column })
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
        let (matches, summary) = run(root, Query { pattern: "needle".into(), ..Default::default() });
        assert_eq!(summary.matches, 1);
        assert_eq!(matches[0].relative, "b.txt");
        assert_eq!(matches[0].line, 2);
        assert_eq!(matches[0].column, (0, 6));
    }

    #[test]
    fn literal_by_default_regex_on_request() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.txt"), "a.c\nabc\n").unwrap();
        let (m, _) = run(d.path(), Query { pattern: "a.c".into(), ..Default::default() });
        assert_eq!(m.len(), 1);
        let (m, _) = run(d.path(), Query { pattern: "a.c".into(), regex: true, ..Default::default() });
        assert_eq!(m.len(), 2);
    }
}
