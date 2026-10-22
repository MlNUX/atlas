// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! `PdfAnnotations`: the annotations of one PDF for the viewer.
//!
//! Loading hashes the PDF and looks the annotations up in the store on a
//! background thread (see `atlas_core::annotations`). Every change is written
//! atomically, in order, on one I/O thread; [`flush_io`] waits for it on quit.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Mutex, OnceLock};

use atlas_core::annotations::{self, Annotation, AnnotationFile, Fingerprint, Kind, Match, Store};
use atlas_core::pdf::PdfService;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, source, READ, WRITE, NOTIFY)]
        /// Project root ("" outside of projects); stored paths are relative to it.
        #[qproperty(QString, project_root, cxx_name = "projectRoot", READ, WRITE, NOTIFY)]
        /// Folder of the annotation files (`.atlas/annotations`).
        #[qproperty(QString, store_dir, cxx_name = "storeDir", READ, WRITE, NOTIFY)]
        /// "", "loading", "exact", "changed", "moved", "new" or "error".
        #[qproperty(QString, status, READ, NOTIFY)]
        #[qproperty(QString, error, READ, NOTIFY)]
        /// All annotations as a JSON list (format: docs/data-format.md).
        #[qproperty(QString, annotations_json, cxx_name = "annotationsJson", READ, NOTIFY)]
        #[qproperty(i32, count, READ, NOTIFY)]
        type PdfAnnotations = super::PdfAnnotationsRust;

        /// Adds an annotation (JSON with kind, page, rects, text, comment,
        /// color, ink, width, title, group); returns its ID ("" on error).
        #[qinvokable]
        fn add(self: Pin<&mut PdfAnnotations>, json: &QString) -> QString;

        /// Changes fields of an annotation (JSON object with the fields to set).
        #[qinvokable]
        fn update(self: Pin<&mut PdfAnnotations>, id: &QString, patch: &QString) -> bool;

        /// Removes an annotation and the other parts of its group.
        #[qinvokable]
        fn remove(self: Pin<&mut PdfAnnotations>, id: &QString) -> bool;

        /// One annotation as JSON ("" if unknown).
        #[qinvokable]
        fn annotation(self: &PdfAnnotations, id: &QString) -> QString;

        /// The annotation and the other parts of its group as a JSON list
        /// (for undo; "[]" if unknown).
        #[qinvokable]
        fn snapshot(self: &PdfAnnotations, id: &QString) -> QString;

        /// Puts annotations from `snapshot` back exactly as they were (same
        /// IDs, replacing those still there); for undo and redo.
        #[qinvokable]
        fn restore(self: Pin<&mut PdfAnnotations>, snapshot: &QString) -> bool;

        /// Markdown quote of a text markup with source link, for the note at
        /// `note_path` (the link is root-relative or relative to the note).
        #[qinvokable]
        #[cxx_name = "quoteMarkdown"]
        fn quote_markdown(self: &PdfAnnotations, id: &QString, note_path: &QString) -> QString;

        /// Root-relative link to the annotation (`/a.pdf#page=3&annot=…`).
        #[qinvokable]
        #[cxx_name = "linkFor"]
        fn link_for(self: &PdfAnnotations, id: &QString) -> QString;

        /// Proposed file for the export ("<name> (annotiert).pdf").
        #[qinvokable]
        #[cxx_name = "defaultExportPath"]
        fn default_export_path(self: &PdfAnnotations) -> QString;

        /// Writes a copy of the PDF with embedded annotations to `dst`; the
        /// result arrives via `exportFinished`.
        #[qinvokable]
        #[cxx_name = "exportCopy"]
        fn export_copy(self: Pin<&mut PdfAnnotations>, dst: &QString);

        #[qsignal]
        #[cxx_name = "exportFinished"]
        fn export_finished(self: Pin<&mut PdfAnnotations>, ok: bool, message: QString);

        /// The user saw that the PDF changed: store its new fingerprint.
        #[qinvokable]
        #[cxx_name = "acknowledgeChange"]
        fn acknowledge_change(self: Pin<&mut PdfAnnotations>);

        /// Reads the annotations again (after the PDF changed on disk).
        #[qinvokable]
        fn reload(self: Pin<&mut PdfAnnotations>);
    }

    impl cxx_qt::Threading for PdfAnnotations {}
    impl cxx_qt::Initialize for PdfAnnotations {}
}

#[derive(Default)]
pub struct PdfAnnotationsRust {
    source: QString,
    project_root: QString,
    store_dir: QString,
    status: QString,
    error: QString,
    annotations_json: QString,
    count: i32,

    /// Where the annotations are stored and what they are.
    file: Option<PathBuf>,
    data: AnnotationFile,
    /// Fingerprint of the PDF as it is now (stored with the next write).
    current: Fingerprint,
    generation: u64,
}

fn qs(s: impl AsRef<str>) -> QString {
    QString::from(s.as_ref())
}

// ------------------------------------------------------------------- I/O

enum Io {
    Write(PathBuf, AnnotationFile),
    Flush(Sender<()>),
}

fn io() -> &'static Mutex<Sender<Io>> {
    static IO: OnceLock<Mutex<Sender<Io>>> = OnceLock::new();
    IO.get_or_init(|| {
        let (tx, rx) = channel::<Io>();
        std::thread::Builder::new()
            .name("atlas-annotations".into())
            .spawn(move || {
                for job in rx {
                    match job {
                        Io::Write(file, data) => {
                            if let Err(err) = annotations::write(&file, &data) {
                                log::error!("cannot save annotations {}: {err:#}", file.display());
                            }
                        }
                        Io::Flush(done) => {
                            let _ = done.send(());
                        }
                    }
                }
            })
            .expect("spawn annotation writer");
        Mutex::new(tx)
    })
}

/// Waits until all queued annotation writes are done.
pub fn flush_io() {
    let (tx, rx) = channel();
    let sent = io().lock().is_ok_and(|io| io.send(Io::Flush(tx)).is_ok());
    if sent {
        let _ = rx.recv();
    }
}

/// Key of a PDF in the store: project-relative, or absolute outside.
fn store_key(root: &str, path: &Path) -> String {
    if !root.is_empty()
        && let Ok(rel) = path.strip_prefix(root)
    {
        return rel.to_string_lossy().into_owned();
    }
    path.to_string_lossy().into_owned()
}

fn fingerprint(path: &Path) -> anyhow::Result<Fingerprint> {
    let service = PdfService::get();
    let pages = service.open_blocking(path)?.page_sizes.len() as u32;
    let (tx, rx) = channel();
    service.document_id(path.to_path_buf(), move |r| {
        let _ = tx.send(r);
    });
    let pdf_id = rx.recv()?.unwrap_or(None);
    Ok(Fingerprint::of(path, pages, pdf_id)?)
}

impl cxx_qt::Initialize for qobject::PdfAnnotations {
    fn initialize(mut self: Pin<&mut Self>) {
        self.as_mut().on_source_changed(|a| a.reload()).release();
        self.as_mut().on_store_dir_changed(|a| a.reload()).release();
        self.as_mut().on_project_root_changed(|a| a.reload()).release();
    }
}

impl qobject::PdfAnnotations {
    fn set_status(mut self: Pin<&mut Self>, status: &str, error: &str) {
        self.as_mut().rust_mut().status = qs(status);
        self.as_mut().rust_mut().error = qs(error);
        self.as_mut().status_changed();
        self.error_changed();
    }

    fn publish(mut self: Pin<&mut Self>) {
        let json = serde_json::to_string(&self.data.annotations).unwrap_or_else(|_| "[]".into());
        let count = self.data.annotations.len() as i32;
        self.as_mut().rust_mut().annotations_json = qs(json);
        self.as_mut().rust_mut().count = count;
        self.as_mut().annotations_json_changed();
        self.count_changed();
    }

    pub fn reload(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().generation += 1;
        let generation = self.generation;
        {
            let mut this = self.as_mut().rust_mut();
            this.file = None;
            this.data = AnnotationFile::default();
        }
        self.as_mut().publish();
        let source = PathBuf::from(self.source.to_string());
        let store_dir = self.store_dir.to_string();
        if source.as_os_str().is_empty() || store_dir.is_empty() {
            self.set_status("", "");
            return;
        }
        self.as_mut().set_status("loading", "");
        let root = self.project_root.to_string();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = fingerprint(&source).and_then(|fp| {
                let key = store_key(&root, &source);
                let exists = |p: &str| {
                    let p = Path::new(p);
                    if p.is_absolute() { p.exists() } else { Path::new(&root).join(p).exists() }
                };
                let loaded = Store::new(&store_dir).load(&key, &fp, exists)?;
                Ok((fp, loaded))
            });
            let _ = thread.queue(move |mut this| {
                if this.generation != generation {
                    return;
                }
                match result {
                    Ok((fp, loaded)) => {
                        {
                            let mut r = this.as_mut().rust_mut();
                            r.file = Some(loaded.file);
                            r.data = loaded.data;
                            r.current = fp;
                        }
                        this.as_mut().publish();
                        this.set_status(loaded.matched.as_str(), "");
                    }
                    Err(err) => this.set_status("error", &format!("{err:#}")),
                }
            });
        });
    }

    /// Queues the current state for writing.
    fn save(mut self: Pin<&mut Self>) {
        let Some(file) = self.file.clone() else { return };
        let current = self.current.clone();
        self.as_mut().rust_mut().data.document.fingerprint = current;
        let data = self.data.clone();
        if let Ok(io) = io().lock() {
            let _ = io.send(Io::Write(file, data));
        }
        if self.status.to_string() == Match::Changed.as_str() {
            // The stored fingerprint now matches the PDF again.
            self.as_mut().set_status(Match::Exact.as_str(), "");
        }
        self.publish();
    }

    pub fn acknowledge_change(self: Pin<&mut Self>) {
        self.save();
    }

    pub fn add(mut self: Pin<&mut Self>, json: &QString) -> QString {
        if self.file.is_none() {
            return QString::default();
        }
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&json.to_string()) else {
            return QString::default();
        };
        let id = annotations::new_id();
        let now = annotations::now();
        if let Some(obj) = value.as_object_mut() {
            obj.insert("id".into(), id.clone().into());
            obj.insert("created".into(), now.into());
            obj.insert("modified".into(), now.into());
        }
        let mut annotation: Annotation = match serde_json::from_value(value) {
            Ok(a) => a,
            Err(err) => {
                log::warn!("invalid annotation from QML: {err}");
                return QString::default();
            }
        };
        if annotation.kind.is_text_markup() {
            annotation.text = annotations::clean_text(&annotation.text);
        }
        self.as_mut().rust_mut().data.annotations.push(annotation);
        self.save();
        qs(id)
    }

    pub fn update(mut self: Pin<&mut Self>, id: &QString, patch: &QString) -> bool {
        let id = id.to_string();
        let Ok(serde_json::Value::Object(patch)) = serde_json::from_str(&patch.to_string()) else { return false };
        let Some(index) = self.data.annotations.iter().position(|a| a.id == id) else { return false };
        let Ok(mut value) = serde_json::to_value(&self.data.annotations[index]) else { return false };
        for (key, v) in patch {
            if !matches!(key.as_str(), "id" | "created" | "group") {
                value[key] = v;
            }
        }
        value["modified"] = annotations::now().into();
        let Ok(updated) = serde_json::from_value::<Annotation>(value) else { return false };
        // Color and comment apply to all parts of a multi-page markup.
        let group = updated.group.clone();
        {
            let mut this = self.as_mut().rust_mut();
            for a in this.data.annotations.iter_mut().filter(|a| !group.is_empty() && a.group == group && a.id != id) {
                a.color = updated.color.clone();
                a.comment = updated.comment.clone();
                a.kind = updated.kind;
            }
            this.data.annotations[index] = updated;
        }
        self.save();
        true
    }

    pub fn remove(mut self: Pin<&mut Self>, id: &QString) -> bool {
        let id = id.to_string();
        let Some(group) = self.data.annotations.iter().find(|a| a.id == id).map(|a| a.group.clone()) else {
            return false;
        };
        self.as_mut().rust_mut().data.annotations.retain(|a| a.id != id && (group.is_empty() || a.group != group));
        self.save();
        true
    }

    pub fn snapshot(&self, id: &QString) -> QString {
        qs(serde_json::to_string(&self.parts(&id.to_string())).unwrap_or_else(|_| "[]".into()))
    }

    pub fn restore(mut self: Pin<&mut Self>, snapshot: &QString) -> bool {
        let Ok(list) = serde_json::from_str::<Vec<Annotation>>(&snapshot.to_string()) else { return false };
        if list.is_empty() || self.file.is_none() {
            return false;
        }
        {
            let mut this = self.as_mut().rust_mut();
            for a in list {
                match this.data.annotations.iter().position(|b| b.id == a.id) {
                    Some(i) => this.data.annotations[i] = a,
                    None => this.data.annotations.push(a),
                }
            }
        }
        self.save();
        true
    }

    fn find(&self, id: &str) -> Option<&Annotation> {
        self.data.annotations.iter().find(|a| a.id == id)
    }

    /// The annotation and the other parts of its group, in page order.
    fn parts(&self, id: &str) -> Vec<&Annotation> {
        let Some(a) = self.find(id) else { return Vec::new() };
        if a.group.is_empty() {
            return vec![a];
        }
        let mut parts: Vec<_> = self.data.annotations.iter().filter(|b| b.group == a.group).collect();
        parts.sort_by_key(|b| b.page);
        parts
    }

    pub fn annotation(&self, id: &QString) -> QString {
        self.find(&id.to_string()).and_then(|a| serde_json::to_string(a).ok()).map(qs).unwrap_or_default()
    }

    pub fn quote_markdown(&self, id: &QString, note_path: &QString) -> QString {
        let parts = self.parts(&id.to_string());
        let Some(first) = parts.first() else { return QString::default() };
        let pdf = PathBuf::from(self.source.to_string());
        let root = self.project_root.to_string();
        let note = PathBuf::from(note_path.to_string());
        let in_project = |p: &Path| !root.is_empty() && p.starts_with(&root);
        let link_path = if in_project(&pdf) && (note.as_os_str().is_empty() || in_project(&note)) {
            format!("/{}", store_key(&root, &pdf))
        } else {
            let dir = note.parent().unwrap_or(Path::new("/"));
            atlas_core::markdown::relative_path(dir, &pdf).to_string_lossy().into_owned()
        };
        let text = parts.iter().map(|p| p.text.as_str()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
        let text = if text.is_empty() { first.comment.clone() } else { text };
        let comment = if text == first.comment { String::new() } else { first.comment.clone() };
        let name = pdf.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        qs(annotations::quote_markdown(&text, &comment, &name, &link_path, first.page, &first.id))
    }

    pub fn link_for(&self, id: &QString) -> QString {
        let Some(a) = self.parts(&id.to_string()).first().copied() else { return QString::default() };
        let pdf = PathBuf::from(self.source.to_string());
        let root = self.project_root.to_string();
        let path = if !root.is_empty() && pdf.starts_with(&root) {
            format!("/{}", store_key(&root, &pdf))
        } else {
            pdf.to_string_lossy().into_owned()
        };
        qs(annotations::link(&path, a.page, Some(&a.id)))
    }

    pub fn default_export_path(&self) -> QString {
        let pdf = PathBuf::from(self.source.to_string());
        let stem = pdf.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "dokument".into());
        let dir = pdf.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut candidate = dir.join(format!("{stem} (annotiert).pdf"));
        let mut n = 2;
        while candidate.exists() {
            candidate = dir.join(format!("{stem} (annotiert {n}).pdf"));
            n += 1;
        }
        qs(candidate.to_string_lossy())
    }

    pub fn export_copy(self: Pin<&mut Self>, dst: &QString) {
        let src = PathBuf::from(self.source.to_string());
        let dst = PathBuf::from(dst.to_string());
        let list = self.data.annotations.clone();
        let skipped = list.iter().filter(|a| a.kind == Kind::Bookmark).count();
        let thread = self.qt_thread();
        let name = dst.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        PdfService::get().export_annotated(src, dst, list, move |result| {
            let (ok, message) = match result {
                Ok(n) if skipped > 0 => (
                    true,
                    format!("{n} Annotation(en) nach „{name}“ exportiert ({skipped} Lesezeichen nicht übertragbar)"),
                ),
                Ok(n) => (true, format!("{n} Annotation(en) nach „{name}“ exportiert")),
                Err(err) => (false, format!("Export fehlgeschlagen: {err:#}")),
            };
            let _ = thread.queue(move |this| this.export_finished(ok, qs(message)));
        });
    }
}
