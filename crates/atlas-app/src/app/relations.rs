// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Files linked by hand, folder nodes and the context graph.

use super::*;

impl qobject::AppController {
    /// Files related by hand in this project (none without `.atlas/`).
    pub(super) fn relations(&self) -> Option<Relations> {
        self.project.as_ref().map(|p| Relations::new(&p.atlas_dir()))
    }

    /// The relations as pairs of absolute paths, folder nodes included:
    /// each folder node with the files directly in it (not hidden ones) and
    /// with the closest folder node above it.
    fn relation_pairs(&self) -> Vec<(PathBuf, PathBuf)> {
        let (Some(root), Some(relations)) = (self.root.as_ref(), self.relations()) else { return Vec::new() };
        let mut pairs: Vec<_> = relations.list().into_iter().map(|r| (root.join(r.a), root.join(r.b))).collect();
        let folders = relations.folders();
        for folder in &folders {
            if let Some(above) = folder.ancestors().skip(1).find(|a| folders.iter().any(|f| f == a)) {
                pairs.push((root.join(above), root.join(folder)));
            }
        }
        if let Some(files) = self.files.as_ref() {
            for entry in files.iter().filter(|e| !e.is_dir && !e.relative.split('/').any(|p| p.starts_with('.'))) {
                let parent = Path::new(&entry.relative).parent().unwrap_or(Path::new(""));
                if folders.iter().any(|f| f == parent) {
                    pairs.push((root.join(parent), finder::absolute(root, entry)));
                }
            }
        }
        pairs
    }

    pub fn is_folder_node(&self, path: &QString) -> bool {
        let Some(relations) = self.relations() else { return false };
        let folder = self.relative(&path_of(path));
        relations.folders().contains(&folder)
    }

    pub fn set_folder_node(mut self: Pin<&mut Self>, path: &QString, on: bool) -> bool {
        let Some(relations) = self.relations() else { return false };
        let folder = self.relative(&path_of(path));
        match relations.set_folder(&folder, on) {
            Ok(changed) => {
                let name = folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let message = if on {
                    let members = self.relation_pairs().iter().filter(|(a, _)| *a == path_of(path)).count();
                    format!("Ordner-Knoten „{name}“: {members} Einträge verknüpft")
                } else {
                    format!("Ordner-Knoten „{name}“ aufgelöst")
                };
                self.as_mut().bump_notes();
                self.notes_message(qs(message));
                changed
            }
            Err(err) => {
                self.notes_message(qs(format!("{err:#}")));
                false
            }
        }
    }

    pub fn request_relate(self: Pin<&mut Self>, path: &QString) {
        self.relate_requested(path.clone());
    }

    pub fn related_files(&self, path: &QString) -> QString {
        let (Some(root), Some(relations)) = (self.root.as_ref(), self.relations()) else { return qs("[]") };
        let relative = self.relative(&path_of(path));
        let mut others = relations.of(&relative);
        // The folder node the file belongs to.
        if let Some(parent) = relative.parent()
            && relations.folders().iter().any(|f| f == parent)
        {
            others.insert(0, parent.to_path_buf());
        }
        let list: Vec<_> = others
            .into_iter()
            .map(|rel| {
                let path = root.join(&rel);
                serde_json::json!({
                    "path": path.to_string_lossy(), "relative": rel.to_string_lossy(),
                    "kind": file_kind(&path), "missing": path.symlink_metadata().is_err(),
                })
            })
            .collect();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn relate_files(mut self: Pin<&mut Self>, a: &QString, b: &QString) -> bool {
        let Some(relations) = self.relations() else { return false };
        let (a, b) = (self.relative(&path_of(a)), self.relative(&path_of(b)));
        match relations.add(&a, &b) {
            Ok(added) => {
                let text = if added { "Verknüpft: {a} ↔ {b}" } else { "{a} und {b} sind schon verknüpft" };
                let text = text.replace("{a}", &a.to_string_lossy()).replace("{b}", &b.to_string_lossy());
                self.as_mut().bump_notes();
                self.notes_message(qs(text));
                added
            }
            Err(err) => {
                self.notes_message(qs(format!("{err:#}")));
                false
            }
        }
    }

    pub fn unrelate_files(mut self: Pin<&mut Self>, a: &QString, b: &QString) -> bool {
        let Some(relations) = self.relations() else { return false };
        let (a, b) = (self.relative(&path_of(a)), self.relative(&path_of(b)));
        match relations.remove(&a, &b) {
            Ok(removed) => {
                self.as_mut().bump_notes();
                removed
            }
            Err(err) => {
                self.notes_message(qs(format!("{err:#}")));
                false
            }
        }
    }

    pub fn files_trashed(mut self: Pin<&mut Self>, paths: &QStringList) {
        let Some(relations) = self.relations() else { return };
        let mut count = 0;
        for path in paths.iter() {
            match relations.removed(&self.relative(&path_of(path))) {
                Ok(n) => count += n,
                Err(err) => log::warn!("cannot update relations: {err:#}"),
            }
        }
        if count > 0 {
            self.as_mut().bump_notes();
        }
    }

    /// The files of the project for the graph (without hidden ones and
    /// folders): PDFs, then other documents and images, then the rest.
    fn graph_files(&self) -> Vec<PathBuf> {
        let (Some(root), Some(files)) = (self.root.as_ref(), self.files.as_ref()) else { return Vec::new() };
        let rank = |relative: &str| {
            let ext = relative.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
            match ext.as_str() {
                "pdf" => 0,
                "md" | "txt" | "rst" | "html" | "htm" | "docx" | "odt" | "doc" | "rtf" | "tex" | "epub" => 1,
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "csv" | "xlsx" | "ods" | "pptx" | "odp" => 2,
                _ => 3,
            }
        };
        let mut list: Vec<&finder::Entry> =
            files.iter().filter(|e| !e.is_dir && !e.relative.split('/').any(|part| part.starts_with('.'))).collect();
        list.sort_by_key(|e| rank(&e.relative));
        list.into_iter().map(|e| finder::absolute(root, e)).collect()
    }

    pub fn context_graph(&self, path: &QString, depth: i32) -> QString {
        const LIMIT: usize = 60;
        const PROJECT_LIMIT: usize = 500;
        let center = path_of(path);
        let Some(json) = self.with_notes(|n| {
            let graph = if depth <= 0 {
                n.project_graph(&center, &self.graph_files(), &self.relation_pairs(), PROJECT_LIMIT)
            } else {
                n.graph(&center, depth.min(2) as u8, LIMIT)
            };
            let nodes: Vec<_> = graph
                .nodes
                .iter()
                .map(|node| {
                    let title = n.note(&node.path).and_then(|note| note.title.clone());
                    let name = node.path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                    let kind = if node.missing { "missing" } else { file_kind(&node.path) };
                    serde_json::json!({
                        "path": node.path.to_string_lossy(),
                        "relative": self.relative(&node.path).to_string_lossy(),
                        "label": title.unwrap_or(name),
                        "kind": kind,
                        "depth": node.depth,
                        "missing": node.missing,
                    })
                })
                .collect();
            let edges: Vec<_> = graph
                .edges
                .iter()
                .map(|e| {
                    serde_json::json!({ "from": e.from, "to": e.to, "count": e.count, "quotes": e.quotes, "related": e.related })
                })
                .collect();
            serde_json::json!({ "nodes": nodes, "edges": edges, "truncated": graph.truncated })
        }) else {
            return qs(r#"{"nodes":[],"edges":[],"truncated":0}"#);
        };
        qs(json.to_string())
    }
}
