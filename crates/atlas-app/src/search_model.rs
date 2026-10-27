// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Project search results (ripgrep and the text of PDFs) as a
//! `QAbstractListModel`.

use std::path::{Path, PathBuf};
use std::pin::Pin;

use atlas_core::search::{self, Match, Query, SearchHandle};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant, QVector};
use serde_json::{Value, json};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!(<QtCore/QAbstractListModel>);
        type QAbstractListModel;

        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qvector.h");
        type QVector_i32 = cxx_qt_lib::QVector<i32>;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(bool, running, READ, NOTIFY)]
        #[qproperty(i32, count, READ, NOTIFY)]
        #[qproperty(i32, files, READ, NOTIFY)]
        #[qproperty(i32, elapsed_ms, cxx_name = "elapsedMs", READ, NOTIFY)]
        #[qproperty(bool, truncated, READ, NOTIFY)]
        #[qproperty(QString, kind_counts, cxx_name = "kindCounts", READ, NOTIFY)]
        #[qproperty(QString, error, READ, NOTIFY)]
        type SearchModel = super::SearchModelRust;

        /// Starts a search below `root`; a running search is cancelled.
        /// `options` is JSON: `regex`, `caseSensitive`, `wholeWord`,
        /// `globs` (comma separated, e.g. `*.rs, !docs/**`), `buildDirs`
        /// (search build and dependency folders too) and `kinds` (document
        /// types: markdown, pdf, latex, code, text; none means all).
        #[qinvokable]
        fn search(self: Pin<&mut SearchModel>, root: &QString, pattern: &QString, options: &QString);

        #[qinvokable]
        fn cancel(self: Pin<&mut SearchModel>);

        #[qinvokable]
        fn clear(self: Pin<&mut SearchModel>);

        /// Lines around `line` of a text file, or the text of page `line`
        /// of a PDF, as JSON: `[{"line": n, "text": "…"}, …]`.
        /// Match `row` as JSON (`path`, `relative`, `line`, `text`, `pdf`,
        /// `ranges`: UTF-16 offsets `[[start, end], …]`); `{}` without it.
        #[qinvokable]
        fn entry(self: &SearchModel, row: i32) -> QString;

        #[qinvokable]
        fn context(self: &SearchModel, path: &QString, line: i32, radius: i32, pdf: bool) -> QString;
    }

    unsafe extern "RustQt" {
        #[qsignal]
        #[inherit]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut SearchModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QVector_i32,
        );
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        fn begin_insert_rows(self: Pin<&mut SearchModel>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        #[cxx_name = "endInsertRows"]
        fn end_insert_rows(self: Pin<&mut SearchModel>);
        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut SearchModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut SearchModel>);
        #[inherit]
        fn index(self: &SearchModel, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;

        #[cxx_override]
        fn data(self: &SearchModel, index: &QModelIndex, role: i32) -> QVariant;
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &SearchModel) -> QHash_i32_QByteArray;
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &SearchModel, parent: &QModelIndex) -> i32;
    }

    impl cxx_qt::Threading for SearchModel {}
}

const ROLE_PATH: i32 = 0x0101;
const ROLE_RELATIVE: i32 = 0x0102;
const ROLE_LINE: i32 = 0x0103;
const ROLE_TEXT: i32 = 0x0104;
const ROLE_MATCH_START: i32 = 0x0105;
const ROLE_MATCH_END: i32 = 0x0106;
const ROLE_FIRST_IN_FILE: i32 = 0x0107;
const ROLE_RANGES: i32 = 0x0108;
const ROLE_PDF: i32 = 0x0109;
const ROLE_FILE_HITS: i32 = 0x010a;
const ROLE_INDEX_IN_FILE: i32 = 0x010b;

#[derive(Default)]
pub struct SearchModelRust {
    running: bool,
    count: i32,
    files: i32,
    elapsed_ms: i32,
    truncated: bool,
    kind_counts: QString,
    error: QString,

    matches: Vec<Match>,
    /// Row of the first match of each file, in order.
    file_starts: Vec<usize>,
    kinds: std::collections::BTreeMap<&'static str, usize>,
    handle: Option<SearchHandle>,
    generation: u64,
}

#[derive(Default)]
struct Options {
    regex: bool,
    case_sensitive: bool,
    whole_word: bool,
    globs: String,
    build_dirs: bool,
    kinds: Vec<String>,
}

impl Options {
    fn parse(json: &str) -> Self {
        let v: Value = serde_json::from_str(json).unwrap_or_default();
        let flag = |key: &str| v.get(key).and_then(Value::as_bool).unwrap_or(false);
        Self {
            regex: flag("regex"),
            case_sensitive: flag("caseSensitive"),
            whole_word: flag("wholeWord"),
            globs: v.get("globs").and_then(Value::as_str).unwrap_or_default().to_owned(),
            build_dirs: flag("buildDirs"),
            kinds: v
                .get("kinds")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                .unwrap_or_default(),
        }
    }

    fn query(&self, pattern: String) -> Query {
        let mut query = Query {
            regex: self.regex,
            case_sensitive: Some(self.case_sensitive),
            whole_word: self.whole_word,
            globs: (!self.build_dirs)
                .then(atlas_core::finder::build_dir_globs)
                .unwrap_or_default()
                .into_iter()
                .chain(self.globs.split(',').map(str::trim).filter(|g| !g.is_empty()).map(str::to_owned))
                .collect(),
            ..Query::new(pattern)
        };
        let kinds: Vec<&str> = self.kinds.iter().map(String::as_str).collect();
        search::restrict_to_kinds(&mut query, &kinds);
        query
    }
}

impl SearchModelRust {
    /// Index of the file of `row` in `file_starts`.
    fn file_of(&self, row: usize) -> usize {
        self.file_starts.partition_point(|&start| start <= row).saturating_sub(1)
    }
    fn index_in_file(&self, row: usize) -> usize {
        row - self.file_starts.get(self.file_of(row)).copied().unwrap_or(0)
    }
    fn file_hits(&self, row: usize) -> usize {
        let file = self.file_of(row);
        let end = self.file_starts.get(file + 1).copied().unwrap_or(self.matches.len());
        end - self.file_starts.get(file).copied().unwrap_or(0)
    }
}

impl qobject::SearchModel {
    pub fn search(mut self: Pin<&mut Self>, root: &QString, pattern: &QString, options: &QString) {
        self.as_mut().clear();
        let pattern = pattern.to_string();
        if pattern.is_empty() || root.is_empty() {
            return;
        }
        let options = Options::parse(&options.to_string());
        let query = options.query(pattern);
        let generation = self.generation;
        let batch_thread = self.qt_thread();
        let done_thread = self.qt_thread();
        let root = PathBuf::from(root.to_string());
        let handle = search::start(
            &root,
            query,
            move |batch| {
                let _ = batch_thread.queue(move |model| model.append(generation, batch));
            },
            move |summary| {
                let _ = done_thread.queue(move |mut model| {
                    if model.generation != generation {
                        return;
                    }
                    {
                        let mut this = model.as_mut().rust_mut();
                        this.running = false;
                        this.elapsed_ms = summary.elapsed.as_millis() as i32;
                        this.truncated = summary.truncated;
                        this.error = QString::from(summary.error.unwrap_or_default().as_str());
                        this.handle = None;
                    }
                    model.as_mut().running_changed();
                    model.as_mut().elapsed_ms_changed();
                    model.as_mut().truncated_changed();
                    model.as_mut().error_changed();
                });
            },
        );
        match handle {
            Ok(handle) => {
                self.as_mut().rust_mut().handle = Some(handle);
                self.as_mut().rust_mut().running = true;
                self.as_mut().running_changed();
            }
            Err(err) => {
                self.as_mut().rust_mut().error = QString::from(format!("{err:#}").as_str());
                self.as_mut().error_changed();
            }
        }
    }

    pub fn cancel(mut self: Pin<&mut Self>) {
        if let Some(handle) = self.as_mut().rust_mut().handle.take() {
            handle.cancel();
        }
        if self.running {
            self.as_mut().rust_mut().running = false;
            self.as_mut().running_changed();
        }
    }

    pub fn clear(mut self: Pin<&mut Self>) {
        self.as_mut().cancel();
        self.as_mut().begin_reset_model();
        {
            let mut this = self.as_mut().rust_mut();
            this.generation += 1;
            this.matches.clear();
            this.file_starts.clear();
            this.kinds.clear();
            this.kind_counts = QString::from("{}");
            this.count = 0;
            this.files = 0;
            this.elapsed_ms = 0;
            this.truncated = false;
            this.error = QString::default();
        }
        self.as_mut().end_reset_model();
        self.as_mut().kind_counts_changed();
        self.as_mut().count_changed();
        self.as_mut().files_changed();
        self.as_mut().elapsed_ms_changed();
        self.as_mut().truncated_changed();
        self.as_mut().error_changed();
    }

    fn append(mut self: Pin<&mut Self>, generation: u64, batch: Vec<Match>) {
        if generation != self.generation || batch.is_empty() {
            return;
        }
        let first = self.matches.len() as i32;
        let last = first + batch.len() as i32 - 1;
        // The last file goes on: its header shows a new number of hits.
        let grown = self.matches.last().filter(|m| m.path == batch[0].path).and_then(|_| self.file_starts.last().copied());
        self.as_mut().begin_insert_rows(&QModelIndex::default(), first, last);
        {
            let mut this = self.as_mut().rust_mut();
            for m in batch {
                if this.matches.last().is_none_or(|prev| prev.path != m.path) {
                    let row = this.matches.len();
                    this.file_starts.push(row);
                }
                *this.kinds.entry(search::kind_of(&m.path)).or_default() += 1;
                this.matches.push(m);
            }
            let counts = json!(this.kinds).to_string();
            this.kind_counts = QString::from(counts.as_str());
            this.count = this.matches.len() as i32;
            this.files = this.file_starts.len() as i32;
        }
        self.as_mut().end_insert_rows();
        if let Some(row) = grown {
            let index = self.index(row as i32, 0, &QModelIndex::default());
            self.as_mut().data_changed(&index, &index, &QVector::<i32>::default());
        }
        self.as_mut().kind_counts_changed();
        self.as_mut().count_changed();
        self.as_mut().files_changed();
    }

    pub fn entry(&self, row: i32) -> QString {
        let Some(m) = usize::try_from(row).ok().and_then(|r| self.matches.get(r)) else { return QString::from("{}") };
        let ranges: Vec<Value> = m.ranges.iter().map(|&(s, e)| json!([utf16_len(&m.text, s), utf16_len(&m.text, e)])).collect();
        let entry = json!({
            "path": m.path.to_string_lossy(),
            "relative": m.relative,
            "line": m.line,
            "text": m.text,
            "pdf": m.pdf,
            "ranges": ranges,
            "index": self.index_in_file(row as usize),
            "fileHits": self.file_hits(row as usize),
        });
        QString::from(entry.to_string().as_str())
    }

    pub fn context(&self, path: &QString, line: i32, radius: i32, pdf: bool) -> QString {
        let path = PathBuf::from(path.to_string());
        let lines = if pdf { pdf_context(&path, line) } else { text_context(&path, line, radius) };
        let lines: Vec<Value> = lines.into_iter().map(|(line, text)| json!({ "line": line, "text": text })).collect();
        QString::from(Value::Array(lines).to_string().as_str())
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let row = index.row().max(0) as usize;
        let Some(m) = self.matches.get(row) else { return QVariant::default() };
        // Byte offsets → UTF-16 offsets for QML string handling.
        let utf16 = |byte: usize| utf16_len(&m.text, byte) as i32;
        match role {
            ROLE_PATH => QVariant::from(&QString::from(m.path.to_string_lossy().as_ref())),
            ROLE_RELATIVE => QVariant::from(&QString::from(m.relative.as_str())),
            ROLE_LINE => QVariant::from(&(m.line as i32)),
            ROLE_TEXT => QVariant::from(&QString::from(m.text.as_str())),
            ROLE_MATCH_START => QVariant::from(&utf16(m.column.0)),
            ROLE_MATCH_END => QVariant::from(&utf16(m.column.1)),
            ROLE_FIRST_IN_FILE => {
                let first = row == 0 || self.matches[row - 1].path != m.path;
                QVariant::from(&first)
            }
            ROLE_RANGES => {
                let ranges: Vec<String> =
                    m.ranges.iter().map(|&(s, e)| format!("{},{}", utf16(s), utf16(e))).collect();
                QVariant::from(&QString::from(ranges.join(",").as_str()))
            }
            ROLE_PDF => QVariant::from(&m.pdf),
            ROLE_FILE_HITS => QVariant::from(&(self.file_hits(row) as i32)),
            ROLE_INDEX_IN_FILE => QVariant::from(&(self.index_in_file(row) as i32)),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        for (role, name) in [
            (ROLE_PATH, "path"),
            (ROLE_RELATIVE, "relativePath"),
            (ROLE_LINE, "line"),
            (ROLE_TEXT, "text"),
            (ROLE_MATCH_START, "matchStart"),
            (ROLE_MATCH_END, "matchEnd"),
            (ROLE_FIRST_IN_FILE, "firstInFile"),
            (ROLE_RANGES, "matchRanges"),
            (ROLE_PDF, "isPdf"),
            (ROLE_FILE_HITS, "fileHits"),
            (ROLE_INDEX_IN_FILE, "indexInFile"),
        ] {
            roles.insert(role, QByteArray::from(name));
        }
        roles
    }

    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.matches.len() as i32
    }
}

/// UTF-16 length of the first `byte` bytes of `text`.
fn utf16_len(text: &str, byte: usize) -> usize {
    text.get(..byte.min(text.len())).map_or(0, |s| s.encode_utf16().count())
}

/// Longest line in the preview (minified files have huge ones).
const CONTEXT_LINE_CHARS: usize = 600;

fn text_context(path: &Path, line: i32, radius: i32) -> Vec<(usize, String)> {
    use std::io::{BufRead, BufReader};
    let Ok(file) = std::fs::File::open(path) else { return Vec::new() };
    let line = line.max(1) as usize;
    let first = line.saturating_sub(radius.max(0) as usize).max(1);
    let last = line + radius.max(0) as usize;
    BufReader::new(file)
        .split(b'\n')
        .map_while(Result::ok)
        .enumerate()
        .skip(first - 1)
        .take(last + 1 - first)
        .map(|(i, bytes)| {
            let text: String = String::from_utf8_lossy(&bytes).trim_end_matches('\r').chars().take(CONTEXT_LINE_CHARS).collect();
            (i + 1, text)
        })
        .collect()
}

fn pdf_context(path: &Path, page: i32) -> Vec<(usize, String)> {
    let Some(text) = search::pdf_page_text(path, page.max(1) as usize) else { return Vec::new() };
    text.lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty())
        .enumerate()
        .map(|(i, l)| (i + 1, l.chars().take(CONTEXT_LINE_CHARS).collect()))
        .collect()
}
