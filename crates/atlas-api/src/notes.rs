// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Notes and their connections: links and backlinks, tags, quotes,
//! deadlines, manual relations, the context graph, link repair after moves.
//! Template: `app/notes.rs`, `app/relations.rs`.

use crate::{Result, todo_task};

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
pub struct Notes {}

#[cfg_attr(feature = "ffi", uniffi::export)]
impl Notes {
    /// Paths of all notes (Markdown files) of the project.
    pub fn list(&self) -> Vec<String> {
        todo_task!("P2.3")
    }

    /// Re-reads a note after it was saved (the editor's text, not the disk).
    pub fn note_changed(&self, path: String, text: Option<String>) {
        let _ = (path, text);
        todo_task!("P2.3")
    }

    pub fn backlinks(&self, path: String) -> Vec<Backlink> {
        let _ = path;
        todo_task!("P2.3")
    }

    pub fn tags(&self) -> Vec<Tag> {
        todo_task!("P2.3")
    }

    pub fn tags_of(&self, path: String) -> Vec<String> {
        let _ = path;
        todo_task!("P2.3")
    }

    pub fn notes_with_tag(&self, tag: String) -> Vec<String> {
        let _ = tag;
        todo_task!("P2.3")
    }

    pub fn deadlines(&self) -> Vec<Deadline> {
        todo_task!("P2.3")
    }

    pub fn quotes(&self) -> Vec<Quote> {
        todo_task!("P2.3")
    }

    pub fn has_checklist(&self, path: String) -> bool {
        let _ = path;
        todo_task!("P2.3")
    }

    /// Line (1-based) of the heading with this slug, 0 if none.
    pub fn heading_line(&self, path: String, slug: String) -> u32 {
        let _ = (path, slug);
        todo_task!("P2.3")
    }

    /// Appends a block (quote, code) to a note; returns the new text.
    pub fn append(&self, path: String, block: String) -> Result<String> {
        let _ = (path, block);
        todo_task!("P2.3")
    }

    /// Files related by hand to `path` (both directions).
    pub fn related(&self, path: String) -> Vec<String> {
        let _ = path;
        todo_task!("P2.3")
    }

    pub fn relate(&self, a: String, b: String) -> Result<bool> {
        let _ = (a, b);
        todo_task!("P2.3")
    }

    pub fn unrelate(&self, a: String, b: String) -> Result<bool> {
        let _ = (a, b);
        todo_task!("P2.3")
    }

    /// Folders shown as nodes of their own in the graph.
    pub fn is_folder_node(&self, path: String) -> bool {
        let _ = path;
        todo_task!("P2.3")
    }

    pub fn set_folder_node(&self, path: String, on: bool) -> Result<bool> {
        let _ = (path, on);
        todo_task!("P2.3")
    }

    /// Neighbourhood of `path` up to `depth` steps (links, quotes, relations).
    pub fn graph(&self, path: String, depth: u8) -> Graph {
        let _ = (path, depth);
        todo_task!("P2.3")
    }

    /// Updates links, relations and quotes after a move or rename; returns
    /// how many links were rewritten. Calendar links: `Calendar::files_moved`.
    pub fn files_moved(&self, from: String, to: String) -> Result<u32> {
        let _ = (from, to);
        todo_task!("P2.3")
    }

    pub fn files_trashed(&self, paths: Vec<String>) {
        let _ = paths;
        todo_task!("P2.3")
    }

    /// Looks for broken links in `path` in the background; result:
    /// `Event::LinkRepairsFound`.
    pub fn find_link_repairs(&self, path: String) {
        let _ = path;
        todo_task!("P2.3")
    }

    pub fn apply_link_repairs(&self, path: String, repairs: Vec<LinkRepair>) -> Result<()> {
        let _ = (path, repairs);
        todo_task!("P2.3")
    }
}
