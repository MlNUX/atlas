// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Notes and their connections: links and backlinks, tags, quotes,
//! deadlines, manual relations, the context graph, link repair after moves.
//! Template: `app/notes.rs`, `app/relations.rs`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use atlas_core::markdown;
use atlas_core::notes::{LinkAnchor, NoteIndex};
use atlas_core::relations::Relations;

use crate::events::{Event, Level};
use crate::inner::Shared;
use crate::util::{path, string};
use crate::{AtlasError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum LinkKind {
    /// `[[Wiki]]`
    Wiki,
    /// `[text](pfad.md)`
    Markdown,
    /// `![](bild.png)`
    Embed,
    /// A quote from a PDF (`atlas:` link with page).
    Quote,
}

/// A link pointing at a note, from another file.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Backlink {
    pub source: String,
    /// 1-based.
    pub line: u32,
    pub raw: String,
    pub kind: LinkKind,
    /// Heading slug or PDF page the link points to, empty if none.
    pub anchor: String,
    /// The line around the link, for display.
    pub context: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Tag {
    /// Lower case, for matching.
    pub key: String,
    /// As written most often.
    pub display: String,
    pub count: u32,
}

/// A note with a due date (front matter `due:` / `status:` / `estimate:`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Deadline {
    pub path: String,
    pub title: String,
    /// ISO date or date-time as written.
    pub due: String,
    pub status: String,
    pub estimate: String,
}

/// A PDF quote inside a note.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Quote {
    pub note: String,
    pub line: u32,
    pub pdf: String,
    pub page: u32,
    pub annotation_id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct GraphNode {
    pub path: String,
    pub title: String,
    /// Distance from the centre.
    pub depth: u8,
    /// Link target that does not exist.
    pub missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct GraphEdge {
    /// Index into `Graph::nodes`.
    pub from: u32,
    pub to: u32,
    /// Number of links.
    pub count: u32,
    pub quotes: u32,
    /// Also (or only) a manual relation.
    pub related: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// A broken link and the target it most likely meant.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct LinkRepair {
    pub source: String,
    pub line: u32,
    pub raw: String,
    pub broken: String,
    pub suggestion: String,
}

#[cfg_attr(feature = "ffi", derive(uniffi::Object))]
pub struct Notes {
    shared: Arc<Shared>,
}

/// At most this many nodes in the graph around a file, and in the whole
/// project (as on Linux).
const GRAPH_LIMIT: usize = 60;
const PROJECT_GRAPH_LIMIT: usize = 500;

impl Notes {
    pub(crate) fn new(shared: Arc<Shared>) -> Self {
        Self { shared }
    }

    /// Runs `f` on the note index (`None` while it is still being built).
    fn with<T>(&self, f: impl FnOnce(&mut NoteIndex) -> T) -> Option<T> {
        self.shared.notes.lock().ok()?.as_mut().map(f)
    }

    fn relative(&self, p: &Path) -> PathBuf {
        self.shared.relative(p).unwrap_or_else(|| p.to_path_buf())
    }

    /// Files related by hand (none without `.atlas/`).
    fn relations(&self) -> Result<Relations> {
        let project = self.shared.project.as_ref().ok_or_else(|| AtlasError::Unavailable {
            message: "Verknüpfungen brauchen den Ordner .atlas/ (nicht beschreibbar).".into(),
        })?;
        Ok(Relations::new(&project.atlas_dir()))
    }

    fn message(&self, text: String) {
        self.shared.emit(Event::Message { level: Level::Info, text });
    }

    /// The relations as pairs of absolute paths, folder nodes included:
    /// each folder node with the files directly in it (not hidden ones) and
    /// with the closest folder node above it.
    fn relation_pairs(&self) -> Vec<(PathBuf, PathBuf)> {
        let Ok(relations) = self.relations() else { return Vec::new() };
        let root = &self.shared.root;
        let mut pairs: Vec<_> = relations.list().into_iter().map(|r| (root.join(r.a), root.join(r.b))).collect();
        let folders = relations.folders();
        for folder in &folders {
            if let Some(above) = folder.ancestors().skip(1).find(|a| folders.iter().any(|f| f == a)) {
                pairs.push((root.join(above), root.join(folder)));
            }
        }
        let files = self.shared.files.read().unwrap().clone();
        for entry in files.iter().filter(|e| !e.is_dir && !e.relative.split('/').any(|p| p.starts_with('.'))) {
            let parent = Path::new(&entry.relative).parent().unwrap_or(Path::new(""));
            if folders.iter().any(|f| f == parent) {
                pairs.push((root.join(parent), atlas_core::finder::absolute(root, entry)));
            }
        }
        pairs
    }

    /// The files of the project for the whole-project graph (no hidden ones,
    /// no folders): PDFs, then other documents and images, then the rest.
    fn graph_files(&self) -> Vec<PathBuf> {
        let rank = |relative: &str| {
            let ext = relative.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
            match ext.as_str() {
                "pdf" => 0,
                "md" | "txt" | "rst" | "html" | "htm" | "docx" | "odt" | "doc" | "rtf" | "tex" | "epub" => 1,
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "csv" | "xlsx" | "ods" | "pptx" | "odp" => 2,
                _ => 3,
            }
        };
        let files = self.shared.files.read().unwrap().clone();
        let mut list: Vec<_> =
            files.iter().filter(|e| !e.is_dir && !e.relative.split('/').any(|part| part.starts_with('.'))).collect();
        list.sort_by_key(|e| rank(&e.relative));
        list.into_iter().map(|e| atlas_core::finder::absolute(&self.shared.root, e)).collect()
    }
}

fn link_kind(kind: markdown::LinkKind, anchor: &LinkAnchor) -> LinkKind {
    match (kind, anchor) {
        (_, LinkAnchor::Annotation(..)) => LinkKind::Quote,
        (markdown::LinkKind::Wiki, _) => LinkKind::Wiki,
        (markdown::LinkKind::Image, _) => LinkKind::Embed,
        _ => LinkKind::Markdown,
    }
}

/// Heading slug, `L<line>` or the PDF page.
fn anchor_text(anchor: &LinkAnchor) -> String {
    match anchor {
        LinkAnchor::None => String::new(),
        LinkAnchor::Heading(h) => h.clone(),
        LinkAnchor::Lines(l) => format!("L{l}"),
        LinkAnchor::Page(p) | LinkAnchor::Annotation(p, _) => p.to_string(),
    }
}

fn save_error(err: atlas_core::textfile::SaveError) -> AtlasError {
    use atlas_core::textfile::SaveError;
    match err {
        SaveError::ReadOnly(message) => AtlasError::Unavailable { message },
        SaveError::Conflict { .. } => AtlasError::Conflict { message: err.to_string() },
        SaveError::Io(err) => err.into(),
    }
}

fn read_utf8(file: &Path) -> std::io::Result<Option<String>> {
    match std::fs::read(file) {
        Ok(bytes) => Ok(String::from_utf8(bytes).ok()),
        Err(err) => Err(err),
    }
}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Notes {
    /// Paths of all notes (Markdown files) of the project.
    pub fn list(&self) -> Vec<String> {
        let mut out = self.with(|n| n.paths().map(|p| string(p)).collect::<Vec<_>>()).unwrap_or_default();
        out.sort();
        out
    }

    /// Re-reads a note after it was saved (the editor's text, not the disk).
    pub fn note_changed(&self, path: String, text: Option<String>) {
        let file = crate::util::path(&path);
        if !file.starts_with(&self.shared.root) {
            return;
        }
        let changed = self.with(|n| match &text {
            Some(text) => n.update_text(&file, text),
            None => n.update(&file),
        });
        if changed == Some(true) {
            self.shared.bump_notes();
        }
    }

    pub fn backlinks(&self, path: String) -> Vec<Backlink> {
        let target = crate::util::path(&path);
        self.with(|n| {
            n.backlinks(&target)
                .into_iter()
                .map(|b| Backlink {
                    source: string(&b.source),
                    line: b.line as u32,
                    raw: b.raw,
                    kind: link_kind(b.kind, &b.anchor),
                    anchor: anchor_text(&b.anchor),
                    context: b.context,
                })
                .collect()
        })
        .unwrap_or_default()
    }

    pub fn tags(&self) -> Vec<Tag> {
        self.with(|n| {
            n.tags().into_iter().map(|t| Tag { key: t.key, display: t.display, count: t.count as u32 }).collect()
        })
        .unwrap_or_default()
    }

    pub fn tags_of(&self, path: String) -> Vec<String> {
        let file = crate::util::path(&path);
        self.with(|n| n.note(&file).map(|note| note.tags.clone())).flatten().unwrap_or_default()
    }

    pub fn notes_with_tag(&self, tag: String) -> Vec<String> {
        self.with(|n| n.with_tag(&tag).iter().map(|p| string(p)).collect()).unwrap_or_default()
    }

    pub fn deadlines(&self) -> Vec<Deadline> {
        let mut out = self
            .with(|n| {
                n.planned()
                    .into_iter()
                    .filter_map(|(file, note)| {
                        let due = note.planning.due.as_deref()?;
                        // Dates and date-times in common spellings.
                        let due = atlas_core::calendar::parse_local(due)
                            .or_else(|| atlas_core::calendar::parse_local(&due.replace(' ', "T")))?;
                        let iso = if due.format("%H:%M:%S").to_string() == "00:00:00" {
                            due.format("%Y-%m-%d").to_string()
                        } else {
                            due.format("%Y-%m-%dT%H:%M").to_string()
                        };
                        let name = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                        Some(Deadline {
                            path: string(file),
                            title: note.title.clone().unwrap_or(name),
                            due: iso,
                            status: note.planning.status.clone().unwrap_or_default(),
                            estimate: note.planning.estimate.clone().unwrap_or_default(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        out.sort_by(|a, b| (&a.due, &a.path).cmp(&(&b.due, &b.path)));
        out
    }

    pub fn quotes(&self) -> Vec<Quote> {
        let quotes = self.with(|n| n.quotes()).unwrap_or_default();
        // Read each quoting note once for the quoted text.
        let mut texts: HashMap<PathBuf, Option<String>> = HashMap::new();
        quotes
            .into_iter()
            .filter_map(|(pdf, b)| {
                let LinkAnchor::Annotation(page, id) = &b.anchor else { return None };
                let text = texts
                    .entry(b.source.clone())
                    .or_insert_with(|| read_utf8(&b.source).ok().flatten())
                    .as_deref()
                    .and_then(|t| atlas_core::notes::quote_text(t, b.line))
                    .unwrap_or_default();
                Some(Quote {
                    note: string(&b.source),
                    line: b.line as u32,
                    pdf: string(&pdf),
                    page: *page,
                    annotation_id: id.clone(),
                    text,
                })
            })
            .collect()
    }

    /// Whether the note has a checklist (`- [ ]` items) – offers „Als
    /// Aufgaben in den Kalender“.
    pub fn has_checklist(&self, path: String) -> bool {
        read_utf8(&crate::util::path(&path))
            .ok()
            .flatten()
            .is_some_and(|text| !atlas_core::calendar::checklist_items(&text).is_empty())
    }

    /// Line (1-based) of the heading with this slug, 0 if none.
    pub fn heading_line(&self, path: String, slug: String) -> u32 {
        let file = crate::util::path(&path);
        let Ok(Some(text)) = read_utf8(&file) else { return 0 };
        let slug = markdown::slugify(&slug);
        let analysis = markdown::analyze(&text, &file, Some(&self.shared.root));
        analysis.headings.iter().find(|h| h.slug == slug).map_or(0, |h| h.line as u32)
    }

    /// Appends a block (quote, code) to a note (created if missing; not
    /// over a change made meanwhile); returns the new text.
    pub fn append(&self, path: String, block: String) -> Result<String> {
        let file = crate::util::path(&path);
        let text = match read_utf8(&file) {
            Ok(Some(text)) => {
                let hash = atlas_core::textfile::hash_bytes(text.as_bytes());
                let new = markdown::append_block(&text, &block);
                atlas_core::textfile::save(&file, &new, Some(&hash)).map_err(save_error)?;
                new
            }
            Ok(None) => return Err(AtlasError::Invalid { message: "Die Notiz ist kein gültiges UTF-8.".into() }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                atlas_core::textfile::save(&file, &block, None).map_err(save_error)?;
                block
            }
            Err(err) => return Err(err.into()),
        };
        self.note_changed(path, Some(text.clone()));
        Ok(text)
    }

    /// Files related by hand to `path` (both directions), the folder node it
    /// belongs to first.
    pub fn related(&self, path: String) -> Vec<String> {
        let Ok(relations) = self.relations() else { return Vec::new() };
        let relative = self.relative(&crate::util::path(&path));
        let mut others = relations.of(&relative);
        if let Some(parent) = relative.parent()
            && relations.folders().iter().any(|f| f == parent)
        {
            others.insert(0, parent.to_path_buf());
        }
        others.into_iter().map(|rel| string(&self.shared.root.join(rel))).collect()
    }

    /// Relates two files by hand; false if they were related already.
    pub fn relate(&self, a: String, b: String) -> Result<bool> {
        let relations = self.relations()?;
        let (a, b) = (self.relative(&path(&a)), self.relative(&path(&b)));
        let added = relations.add(&a, &b)?;
        let text = if added { "Verknüpft: {a} ↔ {b}" } else { "{a} und {b} sind schon verknüpft" };
        self.message(text.replace("{a}", &a.to_string_lossy()).replace("{b}", &b.to_string_lossy()));
        self.shared.bump_notes();
        Ok(added)
    }

    pub fn unrelate(&self, a: String, b: String) -> Result<bool> {
        let relations = self.relations()?;
        let removed = relations.remove(&self.relative(&path(&a)), &self.relative(&path(&b)))?;
        self.shared.bump_notes();
        Ok(removed)
    }

    /// Folders shown as nodes of their own in the graph.
    pub fn is_folder_node(&self, path: String) -> bool {
        let Ok(relations) = self.relations() else { return false };
        let relative = self.relative(&crate::util::path(&path));
        relations.folders().contains(&relative)
    }

    /// Makes a folder a node of its own (or not); false if nothing changed.
    pub fn set_folder_node(&self, path: String, on: bool) -> Result<bool> {
        let relations = self.relations()?;
        let changed = relations.set_folder(&self.relative(&crate::util::path(&path)), on)?;
        if changed {
            self.shared.bump_notes();
        }
        Ok(changed)
    }

    /// Neighbourhood of `path` up to `depth` steps (links, quotes,
    /// relations; at most 2), or with `depth` 0 the whole project around
    /// it (every note, the files they link, files related by hand and the
    /// other files of the project; nodes not connected have depth 255).
    pub fn graph(&self, path: String, depth: u8) -> Graph {
        let center = crate::util::path(&path);
        let (files, pairs) = if depth == 0 { (self.graph_files(), self.relation_pairs()) } else { Default::default() };
        self.with(|n| {
            let graph = if depth == 0 {
                n.project_graph(&center, &files, &pairs, PROJECT_GRAPH_LIMIT)
            } else {
                n.graph(&center, depth.min(2), GRAPH_LIMIT)
            };
            Graph {
                nodes: graph
                    .nodes
                    .iter()
                    .map(|node| {
                        let name = node.path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                        GraphNode {
                            path: string(&node.path),
                            title: n.note(&node.path).and_then(|note| note.title.clone()).unwrap_or(name),
                            depth: node.depth,
                            missing: node.missing,
                        }
                    })
                    .collect(),
                edges: graph
                    .edges
                    .iter()
                    .map(|e| GraphEdge {
                        from: e.from as u32,
                        to: e.to as u32,
                        count: e.count as u32,
                        quotes: e.quotes as u32,
                        related: e.related,
                    })
                    .collect(),
            }
        })
        .unwrap_or(Graph { nodes: Vec::new(), edges: Vec::new() })
    }

    /// Updates links, relations and quotes after a move or rename; returns
    /// how many notes had links rewritten. Annotations of a moved PDF follow
    /// it. Calendar links: `Calendar::files_moved`.
    pub fn files_moved(&self, from: String, to: String) -> Result<u32> {
        let moves = vec![(path(&from), path(&to))];
        let (old, new) = (self.relative(&moves[0].0), self.relative(&moves[0].1));
        if let Some(project) = &self.shared.project {
            let store = atlas_core::annotations::Store::for_atlas_dir(&project.atlas_dir());
            if let Err(err) = store.moved(&old.to_string_lossy(), &new.to_string_lossy()) {
                log::warn!("cannot update annotation paths: {err:#}");
            }
        }
        if let Ok(relations) = self.relations()
            && let Err(err) = relations.moved(&old, &new)
        {
            log::warn!("cannot update relations: {err:#}");
        }
        let Some(done) = self.with(|n| {
            let plan = n.plan_relink(&moves);
            n.apply(&moves, &plan)
        }) else {
            return Ok(0);
        };
        self.shared.bump_notes();
        if !done.is_empty() {
            self.message(if done.len() == 1 {
                format!("Links in {} aktualisiert", self.relative(&done[0]).display())
            } else {
                format!("Links in {} Notizen aktualisiert", done.len())
            });
        }
        Ok(done.len() as u32)
    }

    /// Forgets relations of files moved to the trash.
    pub fn files_trashed(&self, paths: Vec<String>) {
        let Ok(relations) = self.relations() else { return };
        let mut count = 0;
        for p in paths {
            match relations.removed(&self.relative(&path(&p))) {
                Ok(n) => count += n,
                Err(err) => log::warn!("cannot update relations: {err:#}"),
            }
        }
        if count > 0 {
            self.shared.bump_notes();
        }
    }

    /// Looks for broken links in `path` in the background; result:
    /// `Event::LinkRepairsFound` (also when there are none).
    pub fn find_link_repairs(&self, path: String) {
        let source = crate::util::path(&path);
        let root = self.shared.root.clone();
        // Identities of the broken targets as recorded in the file index
        // (taken at startup, i.e. usually before an external move).
        let mut known = HashMap::new();
        if let (Ok(index), Ok(notes)) = (self.shared.index.lock(), self.shared.notes.lock())
            && let (Some(index), Some(notes)) = (index.as_ref(), notes.as_ref())
        {
            for (_, link) in notes.broken_links(Some(&source)) {
                if let Some(id) = link.target.strip_prefix(&root).ok().and_then(|rel| index.identity(rel)) {
                    known.insert(link.target.clone(), id);
                }
            }
        }
        let shared = self.shared.clone();
        std::thread::spawn(move || {
            let repairs = shared
                .notes
                .lock()
                .ok()
                .and_then(|n| n.as_ref().map(|n| n.repair_suggestions(Some(&source), |p| known.get(p).copied())))
                .unwrap_or_default();
            let repairs = repairs
                .into_iter()
                .map(|r| LinkRepair {
                    source: string(&r.source),
                    line: r.line as u32,
                    raw: r.raw,
                    broken: string(&r.broken),
                    suggestion: string(&r.suggestion),
                })
                .collect();
            shared.emit(Event::LinkRepairsFound { path, repairs });
        });
    }

    /// Applies chosen repairs (from `Event::LinkRepairsFound`) to `path`.
    pub fn apply_link_repairs(&self, path: String, repairs: Vec<LinkRepair>) -> Result<()> {
        let source = crate::util::path(&path);
        let fixes: Vec<(PathBuf, PathBuf)> =
            repairs.iter().map(|r| (crate::util::path(&r.broken), crate::util::path(&r.suggestion))).collect();
        let done = self
            .with(|n| match n.plan_repair(&source, &fixes) {
                Some(rewrite) => !n.apply(&[], &[rewrite]).is_empty(),
                None => false,
            })
            .unwrap_or(false);
        if !done {
            return Err(AtlasError::Conflict {
                message: "Die Notiz hat sich geändert oder die Links sind schon repariert.".into(),
            });
        }
        self.shared.bump_notes();
        self.message(format!("{} Link(s) repariert", fixes.len()));
        Ok(())
    }
}
