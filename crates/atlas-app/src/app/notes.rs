// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Notes: Markdown rendering, backlinks, quotes, deadlines, tags, link repairs, pasting and importing into notes.

use super::*;

impl qobject::AppController {
    pub(super) fn bump_notes(mut self: Pin<&mut Self>) {
        let revision = self.notes_revision.wrapping_add(1);
        self.as_mut().rust_mut().notes_revision = revision;
        self.notes_revision_changed();
    }

    pub(super) fn with_notes<T>(&self, f: impl FnOnce(&mut NoteIndex) -> T) -> Option<T> {
        self.notes.as_ref().and_then(|n| n.lock().ok()).map(|mut n| f(&mut n))
    }

    pub fn render_markdown(&self, text: &QString, path: &QString) -> QString {
        let path = path_of(path);
        let r = markdown::render(&text.to_string(), &path, self.root.as_deref());
        let headings: Vec<_> = r
            .headings
            .iter()
            .map(|h| serde_json::json!({ "level": h.level, "text": h.text, "slug": h.slug, "line": h.line }))
            .collect();
        let broken = r.links.iter().filter(|l| l.kind != markdown::LinkKind::Definition && l.is_broken()).count();
        qs(serde_json::json!({
            "html": r.html,
            "title": r.title,
            "tags": r.tags,
            "headings": headings,
            "broken": broken,
        })
        .to_string())
    }

    pub fn heading_line(&self, path: &QString, slug: &QString) -> i32 {
        let path = path_of(path);
        let Ok(text) = std::fs::read_to_string(&path) else { return 0 };
        let slug = markdown::slugify(&slug.to_string());
        let analysis = markdown::analyze(&text, &path, self.root.as_deref());
        analysis.headings.iter().find(|h| h.slug == slug).map(|h| h.line as i32).unwrap_or(0)
    }

    pub fn symbols(&self, path: &QString, text: &QString) -> QString {
        let symbols = atlas_core::symbols::symbols(&path_of(path), &text.to_string());
        qs(serde_json::to_string(&symbols).unwrap_or_else(|_| "[]".into()))
    }

    pub fn note_list(&self) -> QStringList {
        let mut out = QStringList::default();
        for path in self.with_notes(|n| n.paths().cloned().collect::<Vec<_>>()).unwrap_or_default() {
            out.append(qs(path.to_string_lossy()));
        }
        out
    }

    pub fn code_block(&self, text: &QString, language: &QString) -> QString {
        qs(markdown::code_block(&text.to_string(), &language.to_string()))
    }

    pub fn append_to_note(self: Pin<&mut Self>, path: &QString, block: &QString) -> QString {
        let file = path_of(path);
        let result = match std::fs::read(&file) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => {
                    let hash = atlas_core::textfile::hash_bytes(text.as_bytes());
                    atlas_core::textfile::save(&file, &markdown::append_block(&text, &block.to_string()), Some(&hash))
                        .map_err(|e| e.to_string())
                }
                Err(_) => Err("Die Notiz ist kein gültiges UTF-8.".to_owned()),
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                atlas_core::textfile::save(&file, &block.to_string(), None).map_err(|e| e.to_string())
            }
            Err(err) => Err(err.to_string()),
        };
        match result {
            Ok(_) => {
                self.note_changed(path);
                QString::default()
            }
            Err(err) => qs(err),
        }
    }

    pub fn backlinks(&self, path: &QString) -> QString {
        use atlas_core::notes::LinkAnchor;
        let path = path_of(path);
        let mut texts = NoteTexts::default();
        let list = self
            .with_notes(|n| {
                n.backlinks(&path)
                    .iter()
                    .map(|b| {
                        let (anchor_kind, anchor) = match &b.anchor {
                            LinkAnchor::None => ("", String::new()),
                            LinkAnchor::Heading(h) => ("heading", format!("#{h}")),
                            LinkAnchor::Lines(l) => ("lines", format!("Z. {l}")),
                            LinkAnchor::Page(p) => ("page", format!("S. {p}")),
                            LinkAnchor::Annotation(p, _) => ("quote", format!("Zitat S. {p}")),
                        };
                        let note = n.note(&b.source);
                        serde_json::json!({
                            "source": b.source.to_string_lossy(),
                            "relative": self.relative(&b.source).to_string_lossy(),
                            "line": b.line,
                            "raw": b.raw,
                            "kind": b.kind.as_str(),
                            "anchor": anchor,
                            "anchorKind": anchor_kind,
                            "context": b.context,
                            "quote": if anchor_kind == "quote" { texts.quote(&b.source, b.line) } else { None },
                            "title": note.and_then(|n| n.title.clone()),
                            "tags": note.map(|n| n.tags.clone()).unwrap_or_default(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn project_quotes(&self) -> QString {
        use atlas_core::notes::LinkAnchor;
        use serde_json::{Value, json};
        crate::annotations_model::flush_io();
        let Some(root) = self.root.clone() else { return qs("[]") };
        // Where notes quote annotations: annotation id → citations. A deleted
        // mark is gone from the list, even if a note still quotes it.
        let mut cited: std::collections::HashMap<String, Vec<Value>> = std::collections::HashMap::new();
        self.with_notes(|n| {
            for (_, q) in n.quotes() {
                let LinkAnchor::Annotation(_, id) = &q.anchor else { continue };
                let note = n.note(&q.source);
                let citation = json!({
                    "source": q.source.to_string_lossy(),
                    "relative": self.relative(&q.source).to_string_lossy(),
                    "line": q.line,
                    "title": note.and_then(|n| n.title.clone()),
                });
                cited.entry(id.clone()).or_default().push(citation);
            }
        });
        let entry = |target: &Path, page: u32, id: &str, quote: String, comment: &str, color: &str| {
            let citations = cited.get(id).cloned().unwrap_or_default();
            let first = citations.first();
            json!({
                "target": target.to_string_lossy(),
                "targetRelative": self.relative(target).to_string_lossy(),
                "page": page,
                "annot": id,
                "quote": quote,
                "comment": comment,
                "color": color,
                "cited": citations,
                // For the filters: the citing note, else the PDF.
                "source": first.and_then(|c| c["source"].as_str()).map(str::to_owned).unwrap_or_else(|| target.to_string_lossy().into_owned()),
                "relative": first.and_then(|c| c["relative"].as_str()).map(str::to_owned).unwrap_or_else(|| self.relative(target).to_string_lossy().into_owned()),
                "line": first.and_then(|c| c["line"].as_u64()).unwrap_or(0),
                "title": first.and_then(|c| c["title"].as_str()),
                "anchor": format!("S. {page}"),
                "anchorKind": "quote",
                "kind": "markup",
                "context": "",
                "tags": [],
            })
        };
        // Marked text of all the project's PDFs.
        let store = atlas_core::annotations::Store::new(self.annotations_dir.to_string());
        let mut list: Vec<(String, u32, usize, Value)> = Vec::new();
        for (_, file) in store.all() {
            let key = &file.document.path;
            let path =
                if Path::new(key).is_absolute() { PathBuf::from(key) } else { root.join(key.trim_start_matches('/')) };
            if !path.starts_with(&root) {
                continue;
            }
            let sort_key = self.relative(&path).to_string_lossy().into_owned();
            // A mark over several pages: one entry, its text joined.
            let mut groups: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for a in file.annotations.iter().filter(|a| a.kind.is_text_markup() && !a.text.trim().is_empty()) {
                if !a.group.is_empty()
                    && let Some(&i) = groups.get(&a.group)
                {
                    let joined = format!("{} {}", list[i].3["quote"].as_str().unwrap_or(""), a.text.trim());
                    list[i].3["quote"] = Value::from(joined);
                    continue;
                }
                if !a.group.is_empty() {
                    groups.insert(a.group.clone(), list.len());
                }
                let order = list.len();
                list.push((
                    sort_key.clone(),
                    a.page,
                    order,
                    entry(&path, a.page, &a.id, a.text.trim().to_owned(), &a.comment, &a.color),
                ));
            }
        }
        list.sort_by(|a, b| (&a.0, a.1, a.2).cmp(&(&b.0, b.1, b.2)));
        qs(Value::from(list.into_iter().map(|e| e.3).collect::<Vec<_>>()).to_string())
    }

    pub fn note_deadlines(&self) -> QString {
        let list = self
            .with_notes(|n| {
                n.planned()
                    .into_iter()
                    .filter_map(|(path, note)| {
                        let due = note.planning.due.as_deref()?;
                        // Dates and date-times in common spellings.
                        let due = atlas_core::calendar::parse_local(due)
                            .or_else(|| atlas_core::calendar::parse_local(&due.replace(' ', "T")))?;
                        let iso = if due.time() == chrono::NaiveTime::MIN {
                            due.format("%Y-%m-%d").to_string()
                        } else {
                            due.format("%Y-%m-%dT%H:%M").to_string()
                        };
                        let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                        Some(serde_json::json!({
                            "path": path.to_string_lossy(),
                            "relative": self.relative(path).to_string_lossy(),
                            "title": note.title.clone().unwrap_or(name),
                            "due": iso,
                            "status": note.planning.status,
                            "estimate": note.planning.estimate,
                        }))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn note_tags(&self) -> QString {
        let tags = self.with_notes(|n| n.tags()).unwrap_or_default();
        let list: Vec<_> =
            tags.iter().map(|t| serde_json::json!({ "key": t.key, "display": t.display, "count": t.count })).collect();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn notes_with_tag(&self, tag: &QString) -> QStringList {
        let mut out = QStringList::default();
        for path in self.with_notes(|n| n.with_tag(&tag.to_string())).unwrap_or_default() {
            out.append(qs(path.to_string_lossy()));
        }
        out
    }

    pub fn note_changed(self: Pin<&mut Self>, path: &QString) {
        let path = path_of(path);
        if self.root.as_ref().is_some_and(|r| path.starts_with(r)) && self.with_notes(|n| n.update(&path)) == Some(true)
        {
            self.bump_notes();
        }
    }

    pub fn files_moved(mut self: Pin<&mut Self>, from: &QString, to: &QString) -> i32 {
        let moves = vec![(path_of(from), path_of(to))];
        // Annotations follow their PDFs (pending writes land first).
        if self.project.is_some() {
            crate::annotations_model::flush_io();
            let store = atlas_core::annotations::Store::new(self.annotations_dir.to_string());
            let (old, new) = (self.relative(&moves[0].0), self.relative(&moves[0].1));
            if let Err(err) = store.moved(&old.to_string_lossy(), &new.to_string_lossy()) {
                log::warn!("cannot update annotation paths: {err:#}");
            }
        }
        // So do files related by hand.
        if let Some(relations) = self.relations()
            && let Err(err) = relations.moved(&self.relative(&moves[0].0), &self.relative(&moves[0].1))
        {
            log::warn!("cannot update relations: {err:#}");
        }
        let Some(done) = self.with_notes(|n| {
            let plan = n.plan_relink(&moves);
            n.apply(&moves, &plan)
        }) else {
            return 0;
        };
        self.as_mut().bump_notes();
        if !done.is_empty() {
            let message = if done.len() == 1 {
                format!("Links in {} aktualisiert", self.relative(&done[0]).display())
            } else {
                format!("Links in {} Notizen aktualisiert", done.len())
            };
            self.as_mut().notes_message(qs(message));
        }
        done.len() as i32
    }

    pub fn find_link_repairs(self: Pin<&mut Self>, path: &QString) {
        let (Some(notes), Some(root)) = (self.notes.clone(), self.root.clone()) else { return };
        let source = path_of(path);
        // Identities of the broken targets as recorded in the file index
        // (taken at startup, i.e. usually before the external move).
        let mut known = std::collections::HashMap::new();
        if let (Some(index), Ok(n)) = (self.index.as_ref(), notes.lock()) {
            for (_, link) in n.broken_links(Some(&source)) {
                if let Some(id) = link.target.strip_prefix(&root).ok().and_then(|rel| index.identity(rel)) {
                    known.insert(link.target.clone(), id);
                }
            }
        }
        let thread = self.qt_thread();
        let path = path.clone();
        std::thread::spawn(move || {
            let repairs = notes.lock().map(|n| n.repair_suggestions(Some(&source), |p| known.get(p).copied()));
            let list: Vec<_> = repairs
                .unwrap_or_default()
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "line": r.line,
                        "raw": r.raw,
                        "broken": r.broken.to_string_lossy(),
                        "suggestion": r.suggestion.to_string_lossy(),
                        "label": r.suggestion.strip_prefix(&root).unwrap_or(&r.suggestion).to_string_lossy(),
                    })
                })
                .collect();
            let json = qs(serde_json::Value::from(list).to_string());
            let _ = thread.queue(move |app| app.link_repairs_found(path, json));
        });
    }

    pub fn apply_link_repairs(mut self: Pin<&mut Self>, path: &QString, repairs: &QString) -> bool {
        let source = path_of(path);
        let Ok(serde_json::Value::Array(items)) = serde_json::from_str(&repairs.to_string()) else { return false };
        let fixes: Vec<(PathBuf, PathBuf)> = items
            .iter()
            .filter_map(|i| Some((PathBuf::from(i["broken"].as_str()?), PathBuf::from(i["suggestion"].as_str()?))))
            .collect();
        let done = self
            .with_notes(|n| match n.plan_repair(&source, &fixes) {
                Some(rewrite) => !n.apply(&[], &[rewrite]).is_empty(),
                None => false,
            })
            .unwrap_or(false);
        if done {
            self.as_mut().bump_notes();
            self.as_mut().notes_message(qs(format!("{} Link(s) repariert", fixes.len())));
        }
        done
    }

    pub fn annotation_search(&self, query: &QString, limit: i32) -> QString {
        crate::annotations_model::flush_io();
        let query = query.to_string();
        let query = query.trim();
        let store = atlas_core::annotations::Store::new(self.annotations_dir.to_string());
        let mut found: Vec<(i32, serde_json::Value)> = Vec::new();
        for (_, file) in store.all() {
            let key = &file.document.path;
            let path = if Path::new(key).is_absolute() {
                PathBuf::from(key)
            } else {
                self.root.as_ref().map(|r| r.join(key)).unwrap_or_else(|| PathBuf::from(key))
            };
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let mut groups = std::collections::HashSet::new();
            for a in &file.annotations {
                // One entry per multi-page markup.
                if !a.group.is_empty() && !groups.insert(a.group.clone()) {
                    continue;
                }
                let haystack = format!("{} {} {} {name}", a.text, a.comment, a.title);
                let score = if query.is_empty() { 0 } else { finder::score(query, &haystack).unwrap_or(-1) };
                if score < 0 {
                    continue;
                }
                found.push((
                    score,
                    serde_json::json!({
                        "path": path.to_string_lossy(),
                        "relative": key,
                        "page": a.page,
                        "id": a.id,
                        "kind": a.kind,
                        "text": a.text,
                        "comment": a.comment,
                        "title": a.title,
                    }),
                ));
            }
        }
        found.sort_by_key(|f| std::cmp::Reverse(f.0));
        found.truncate(limit.max(1) as usize);
        qs(serde_json::Value::from(found.into_iter().map(|f| f.1).collect::<Vec<_>>()).to_string())
    }

    pub fn pdf_area_to_note(&self, pdf: &QString, page: i32, area: &QString, note: &QString) -> QString {
        use atlas_core::assets;
        let (pdf, note) = (path_of(pdf), path_of(note));
        let Ok(area) = serde_json::from_str::<[f64; 4]>(&area.to_string()) else { return QString::default() };
        let page = page.max(1) as u32;
        let dir = assets::assets_dir(self.root.as_deref(), &note);
        if let Err(err) = std::fs::create_dir_all(&dir) {
            log::warn!("cannot create {}: {err}", dir.display());
            return QString::default();
        }
        let stem = pdf.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "pdf".into());
        let name = assets::new_image_name(Path::new(&format!("{stem}-s{page}.md")), "png");
        let target = assets::unique_path(&dir, &name);
        // About 220 dpi: sharp in notes and when printed.
        if let Err(err) = atlas_core::pdf::PdfService::get().save_area_png(&pdf, page as usize - 1, area, 3.0, &target)
        {
            log::warn!("cannot save the picture: {err:#}");
            return QString::default();
        }
        let file_name =
            pdf.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().replace(['[', ']'], "");
        let source = match &self.root {
            Some(root) if pdf.starts_with(root) && note.starts_with(root) => {
                format!("/{}", self.relative(&pdf).to_string_lossy())
            }
            _ => atlas_core::markdown::relative_path(note.parent().unwrap_or(Path::new("/")), &pdf)
                .to_string_lossy()
                .into_owned(),
        };
        let label = format!("{file_name}, S. {page}");
        // The source as caption under the picture (same paragraph).
        qs(format!(
            "![{label}]({})\n*[{label}]({})*",
            assets::link_from(&note, &target),
            atlas_core::annotations::link(&source, page, None)
        ))
    }

    pub fn folder_tree(&self, path: &QString) -> QString {
        qs(atlas_core::text_tree::of_folder(&path_of(path), 3, 300))
    }

    pub fn paste_into_note(&self, note: &QString) -> QString {
        let note = path_of(note);
        let root = self.root.as_deref();
        let files: Vec<PathBuf> =
            crate::qt_helpers::clipboard_files().iter().map(|p| PathBuf::from(p.to_string())).collect();
        if !files.is_empty() {
            return qs(self.import_files(&note, &files));
        }
        let dir = atlas_core::assets::assets_dir(root, &note);
        if std::fs::create_dir_all(&dir).is_err() {
            return QString::default();
        }
        let target = atlas_core::assets::unique_path(&dir, &atlas_core::assets::new_image_name(&note, "png"));
        if crate::qt_helpers::clipboard_save_image(&qs(target.to_string_lossy())) {
            log::info!("picture from the clipboard saved as {}", target.display());
            qs(atlas_core::assets::image_markdown(&note, &target))
        } else {
            QString::default()
        }
    }

    pub fn import_into_note(&self, note: &QString, paths: &QString) -> QString {
        let list: Vec<String> = serde_json::from_str(&paths.to_string()).unwrap_or_default();
        let files: Vec<PathBuf> = list
            .iter()
            .map(|p| {
                PathBuf::from(
                    p.strip_prefix("file://").map(atlas_core::text::percent_decode).unwrap_or_else(|| p.clone()),
                )
            })
            .collect();
        qs(self.import_files(&path_of(note), &files))
    }

    fn import_files(&self, note: &Path, files: &[PathBuf]) -> String {
        let mut out = Vec::new();
        for file in files.iter().filter(|f| f.is_file()) {
            match atlas_core::assets::import(self.root.as_deref(), note, file) {
                Ok(markdown) => out.push(markdown),
                Err(err) => log::warn!("cannot insert {}: {err:#}", file.display()),
            }
        }
        out.join("\n")
    }
}
