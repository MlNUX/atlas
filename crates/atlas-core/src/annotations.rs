// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! PDF annotations kept next to the project, never inside the PDF.
//!
//! Every annotated PDF gets one JSON file in `.atlas/annotations/`
//! (`<name>-<8 hex>.json`, see docs/data-format.md). The file records the
//! PDF's project path and a fingerprint (BLAKE3 of the content, size, page
//! count and the PDF's permanent ID), so the annotations are found again
//! after the PDF was moved or renamed outside of Atlas. Pages are 1-based
//! as in links (`datei.pdf#page=3`); coordinates are PDF points with the
//! origin at the top left of the unrotated page.
//!
//! The original PDF is only written by an explicit export, which always
//! creates a copy (see [`crate::pdf::PdfService::export_annotated`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const FORMAT_VERSION: u32 = 1;
pub const DIR: &str = "annotations";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Highlight,
    Underline,
    Strikeout,
    /// A comment pinned to a point of the page.
    Note,
    /// Freehand strokes.
    Ink,
    /// A bookmark for the whole page.
    Bookmark,
}

impl Kind {
    pub fn is_text_markup(self) -> bool {
        matches!(self, Kind::Highlight | Kind::Underline | Kind::Strikeout)
    }
}

/// `[x, y, width, height]` in points, origin top left.
pub type Rect = [f64; 4];
/// `[x, y]` in points, origin top left.
pub type Point = [f64; 2];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String,
    pub kind: Kind,
    /// 1-based page.
    pub page: u32,
    /// Text markup: one rectangle per line part; note: the icon.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rects: Vec<Rect>,
    /// Marked text (text markup only).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,
    /// `#rrggbb`.
    #[serde(default = "default_color")]
    pub color: String,
    /// Freehand strokes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ink: Vec<Vec<Point>>,
    /// Stroke width of freehand strokes in points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// Bookmark title.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// Markup spanning several pages: one annotation per page, same group.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    pub created: i64,
    pub modified: i64,
}

fn default_color() -> String {
    "#ffd400".into()
}

/// How a PDF is identified.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    /// BLAKE3 of the whole file, hex.
    pub blake3: String,
    pub size: u64,
    #[serde(default)]
    pub pages: u32,
    /// Permanent ID from the PDF trailer (hex), if it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pdf_id: Option<String>,
}

impl Fingerprint {
    /// Hashes the file; `pages` and `pdf_id` come from the PDF engine.
    pub fn of(path: &Path, pages: u32, pdf_id: Option<String>) -> std::io::Result<Self> {
        let mut hasher = blake3::Hasher::new();
        let size = hasher.update_reader(std::fs::File::open(path)?)?.count();
        Ok(Self { blake3: hasher.finalize().to_hex().to_string(), size, pages, pdf_id })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DocumentRef {
    /// Project-relative path with `/`; absolute for PDFs outside a project.
    pub path: String,
    #[serde(flatten)]
    pub fingerprint: Fingerprint,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AnnotationFile {
    pub version: u32,
    pub document: DocumentRef,
    #[serde(default)]
    pub annotations: Vec<Annotation>,
}

/// How the stored annotations relate to the PDF on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Match {
    /// Same path and content.
    Exact,
    /// Same path, but the PDF changed since: positions may be off.
    Changed,
    /// Found by fingerprint after the PDF was moved outside of Atlas; the
    /// stored path was updated.
    Moved,
    /// No annotations yet (the file is created with the first one).
    New,
}

impl Match {
    pub fn as_str(self) -> &'static str {
        match self {
            Match::Exact => "exact",
            Match::Changed => "changed",
            Match::Moved => "moved",
            Match::New => "new",
        }
    }
}

/// Annotations of one PDF and where they are stored.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub file: PathBuf,
    pub data: AnnotationFile,
    pub matched: Match,
}

/// The `annotations/` folder of a project (or of Atlas' state folder for
/// PDFs outside of projects).
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Short random ID for new annotations.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_owned()
}

fn file_stem_for(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.strip_suffix(".pdf").or_else(|| name.strip_suffix(".PDF")).unwrap_or(name);
    let clean: String =
        stem.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).take(40).collect();
    if clean.trim_matches('-').is_empty() { "dokument".into() } else { clean }
}

impl Store {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// `.atlas/annotations` of a project.
    pub fn for_atlas_dir(atlas_dir: &Path) -> Self {
        Self::new(atlas_dir.join(DIR))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// All annotation files (unreadable ones are skipped and logged).
    pub fn all(&self) -> Vec<(PathBuf, AnnotationFile)> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return Vec::new() };
        let mut out: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .filter_map(|p| match read(&p) {
                Ok(data) => Some((p, data)),
                Err(err) => {
                    log::warn!("ignoring {}: {err:#}", p.display());
                    None
                }
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// Finds the annotations of the PDF at `key` (project-relative path)
    /// with `fingerprint`. `exists` tells whether a stored path still
    /// exists (then a match by content is a copy, not a move).
    pub fn load(&self, key: &str, fingerprint: &Fingerprint, exists: impl Fn(&str) -> bool) -> Result<Loaded> {
        let all = self.all();
        if let Some((file, data)) = all.iter().find(|(_, d)| d.document.path == key) {
            let matched =
                if data.document.fingerprint.blake3 == fingerprint.blake3 { Match::Exact } else { Match::Changed };
            return Ok(Loaded { file: file.clone(), data: data.clone(), matched });
        }
        let moved = all.into_iter().find(|(_, d)| {
            let f = &d.document.fingerprint;
            let same = f.blake3 == fingerprint.blake3
                || (f.pdf_id.is_some() && f.pdf_id == fingerprint.pdf_id && f.pages == fingerprint.pages);
            same && !exists(&d.document.path)
        });
        if let Some((file, mut data)) = moved {
            log::info!("annotations of {} follow the PDF to {key}", data.document.path);
            data.document.path = key.to_owned();
            data.document.fingerprint = fingerprint.clone();
            write(&file, &data)?;
            return Ok(Loaded { file, data, matched: Match::Moved });
        }
        let file = self.dir.join(format!("{}-{}.json", file_stem_for(key), &new_id()[..8]));
        let data = AnnotationFile {
            version: FORMAT_VERSION,
            document: DocumentRef { path: key.to_owned(), fingerprint: fingerprint.clone() },
            annotations: Vec::new(),
        };
        Ok(Loaded { file, data, matched: Match::New })
    }

    /// Updates stored paths after Atlas moved `old` to `new` (files or
    /// folders, project-relative). Returns the number of updated files.
    pub fn moved(&self, old: &str, new: &str) -> Result<usize> {
        let mut count = 0;
        for (file, mut data) in self.all() {
            let path = &data.document.path;
            let rest = if path == old {
                Some(String::new())
            } else {
                path.strip_prefix(old).filter(|r| r.starts_with('/')).map(str::to_owned)
            };
            if let Some(rest) = rest {
                data.document.path = format!("{new}{rest}");
                write(&file, &data)?;
                count += 1;
            }
        }
        Ok(count)
    }
}

pub fn read(file: &Path) -> Result<AnnotationFile> {
    let text = std::fs::read_to_string(file).with_context(|| format!("cannot read {}", file.display()))?;
    let data: AnnotationFile = serde_json::from_str(&text).with_context(|| format!("invalid {}", file.display()))?;
    anyhow::ensure!(data.version <= FORMAT_VERSION, "{} has the newer format {}", file.display(), data.version);
    Ok(data)
}

/// Writes atomically; an empty annotation list removes the file.
pub fn write(file: &Path, data: &AnnotationFile) -> Result<()> {
    if data.annotations.is_empty() {
        match std::fs::remove_file(file) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => return Err(err.into()),
            _ => return Ok(()),
        }
    }
    let mut data = data.clone();
    data.version = FORMAT_VERSION;
    for a in &mut data.annotations {
        round_annotation(a);
    }
    let json = serde_json::to_string_pretty(&data)?;
    crate::paths::write_atomic(file, json.as_bytes())
}

fn round(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round_annotation(a: &mut Annotation) {
    for r in &mut a.rects {
        for v in r.iter_mut() {
            *v = round(*v);
        }
    }
    for stroke in &mut a.ink {
        for p in stroke.iter_mut() {
            p[0] = round(p[0]);
            p[1] = round(p[1]);
        }
    }
}

/// Text of a selection as one paragraph: line breaks become spaces,
/// words hyphenated across lines are joined again.
pub fn clean_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    for (i, line) in lines.iter().enumerate() {
        out.push_str(line);
        if i + 1 == lines.len() {
            break;
        }
        let next_lower = lines[i + 1].chars().next().is_some_and(char::is_lowercase);
        let hyphen = line.ends_with('-') && !line.ends_with(" -") && line.len() > 1;
        if hyphen && next_lower {
            out.pop();
        } else {
            out.push(' ');
        }
    }
    out
}

/// Link to a PDF page and annotation as written into notes:
/// `/docs/paper.pdf#page=8&annot=1a2b3c4d5e6f`.
pub fn link(project_path: &str, page: u32, annot: Option<&str>) -> String {
    let mut path = String::new();
    for c in project_path.chars() {
        match c {
            ' ' => path.push_str("%20"),
            '(' => path.push_str("%28"),
            ')' => path.push_str("%29"),
            '#' => path.push_str("%23"),
            '?' => path.push_str("%3F"),
            '%' => path.push_str("%25"),
            c => path.push(c),
        }
    }
    match annot {
        Some(id) => format!("{path}#page={page}&annot={id}"),
        None => format!("{path}#page={page}"),
    }
}

/// Markdown quote of marked text with source and page:
///
/// ```text
/// > Der markierte Text …
/// >
/// > — [paper.pdf, S. 8](/docs/paper.pdf#page=8&annot=1a2b3c4d5e6f)
/// ```
///
/// `link_path` is root-relative (`/docs/paper.pdf`) or relative to the
/// note; a comment follows the quote as normal text.
pub fn quote_markdown(text: &str, comment: &str, file_name: &str, link_path: &str, page: u32, annot: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            out.push_str(">\n");
        } else {
            out.push_str("> ");
            out.push_str(&escape_quote(line));
            out.push('\n');
        }
    }
    let label = format!("{}, S. {page}", file_name.replace(['[', ']'], ""));
    out.push_str(&format!(">\n> — [{label}]({})\n", link(link_path, page, Some(annot))));
    if !comment.trim().is_empty() {
        out.push('\n');
        out.push_str(comment.trim());
        out.push('\n');
    }
    out
}

/// Keeps quoted PDF text from turning into Markdown structure.
fn escape_quote(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for (i, c) in line.chars().enumerate() {
        if matches!(c, '*' | '_' | '`' | '[' | ']' | '<' | '>' | '\\' | '$')
            || (i == 0 && matches!(c, '#' | '-' | '+' | '|'))
        {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Annotations by page (for the viewer), keeping the file order.
pub fn by_page(annotations: &[Annotation]) -> BTreeMap<u32, Vec<&Annotation>> {
    let mut map: BTreeMap<u32, Vec<&Annotation>> = BTreeMap::new();
    for a in annotations {
        map.entry(a.page).or_default().push(a);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(hash: &str) -> Fingerprint {
        Fingerprint { blake3: hash.into(), size: 10, pages: 3, pdf_id: Some("abc".into()) }
    }

    fn highlight(page: u32) -> Annotation {
        Annotation {
            id: new_id(),
            kind: Kind::Highlight,
            page,
            rects: vec![[10.123456, 20.0, 100.0, 12.0]],
            text: "Wichtiger Satz".into(),
            comment: String::new(),
            color: default_color(),
            ink: Vec::new(),
            width: None,
            title: String::new(),
            group: String::new(),
            created: 1,
            modified: 1,
        }
    }

    #[test]
    fn stores_and_finds_annotations_by_path_and_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("annotations"));
        let none = |_: &str| false;

        let mut loaded = store.load("docs/paper.pdf", &fp("h1"), none).unwrap();
        assert_eq!(loaded.matched, Match::New);
        assert!(!loaded.file.exists(), "nothing is written before the first annotation");
        loaded.data.annotations.push(highlight(2));
        write(&loaded.file, &loaded.data).unwrap();
        assert!(loaded.file.file_name().unwrap().to_string_lossy().starts_with("paper-"));
        let text = std::fs::read_to_string(&loaded.file).unwrap();
        assert!(text.contains("\"kind\": \"highlight\"") && text.contains("10.12"), "{text}");

        let again = store.load("docs/paper.pdf", &fp("h1"), none).unwrap();
        assert_eq!(again.matched, Match::Exact);
        assert_eq!(again.data.annotations.len(), 1);
        assert_eq!(store.load("docs/paper.pdf", &fp("h2"), none).unwrap().matched, Match::Changed);

        // Moved outside of Atlas: found by content and re-pointed.
        let moved = store.load("archiv/paper-final.pdf", &fp("h1"), none).unwrap();
        assert_eq!(moved.matched, Match::Moved);
        assert_eq!(moved.file, loaded.file);
        assert_eq!(read(&loaded.file).unwrap().document.path, "archiv/paper-final.pdf");

        // A copy (the old path still exists) gets its own annotations.
        let copy = store.load("kopie.pdf", &fp("h1"), |_| true).unwrap();
        assert_eq!(copy.matched, Match::New);

        // Moved inside Atlas (a folder rename).
        assert_eq!(store.moved("archiv", "alt/archiv").unwrap(), 1);
        assert_eq!(read(&loaded.file).unwrap().document.path, "alt/archiv/paper-final.pdf");
        assert_eq!(store.moved("alt/arch", "x").unwrap(), 0, "prefix must end at a path separator");

        // Removing the last annotation removes the file.
        let mut data = read(&loaded.file).unwrap();
        data.annotations.clear();
        write(&loaded.file, &data).unwrap();
        assert!(!loaded.file.exists());
    }

    #[test]
    fn fingerprints_hash_the_content() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.pdf");
        std::fs::write(&a, b"%PDF-1.4 x").unwrap();
        let f = Fingerprint::of(&a, 1, None).unwrap();
        assert_eq!(f.size, 10);
        assert_eq!(f.blake3, blake3::hash(b"%PDF-1.4 x").to_hex().to_string());
    }

    #[test]
    fn quotes_and_links() {
        assert_eq!(clean_text("Ein Bei-\nspiel mit\n  Zeilen-\nUmbruch\n"), "Ein Beispiel mit Zeilen- Umbruch");
        assert_eq!(link("/docs/Mein Paper.pdf", 8, Some("ab12")), "/docs/Mein%20Paper.pdf#page=8&annot=ab12");
        let q = quote_markdown("Ein *wichtiger* Satz", "Gut für Kapitel 2", "paper.pdf", "/docs/paper.pdf", 8, "ab12");
        assert_eq!(
            q,
            "> Ein \\*wichtiger\\* Satz\n>\n> — [paper.pdf, S. 8](/docs/paper.pdf#page=8&annot=ab12)\n\nGut für Kapitel 2\n"
        );
        // The link in the quote resolves to the PDF page.
        let target =
            crate::markdown::resolve("/docs/paper.pdf#page=8&annot=ab12", Path::new("/p/n.md"), Some(Path::new("/p")));
        assert!(
            matches!(&target, crate::markdown::Target::File { anchor: crate::markdown::Anchor::Annotation(8, id), .. } if id == "ab12"),
            "{target:?}"
        );
    }
}
