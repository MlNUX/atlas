// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Application controller: project selection, the project's `.atlas/`
//! folder (session, metadata, index) and small file helpers for QML.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use atlas_core::project::Project;
use atlas_core::project::index::{FileIndex, Previous, Report};
use atlas_core::project::metadata::Metadata;
use atlas_core::project::writer::SessionWriter;
use atlas_core::session::{AppState, ProjectSession};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};

use crate::cli::Cli;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, project_root, cxx_name = "projectRoot", READ, NOTIFY = project_root_changed)]
        #[qproperty(QString, project_name, cxx_name = "projectName", READ, NOTIFY = project_root_changed)]
        // `.atlas/` of the open project, or "" if nothing is persisted.
        #[qproperty(QString, atlas_dir, cxx_name = "atlasDir", READ, NOTIFY = project_root_changed)]
        // Why project data cannot be stored (read-only folder, …).
        #[qproperty(QString, storage_error, cxx_name = "storageError", READ, NOTIFY = project_root_changed)]
        #[qproperty(bool, index_ready, cxx_name = "indexReady", READ, NOTIFY)]
        #[qproperty(QString, index_status, cxx_name = "indexStatus", READ, NOTIFY)]
        #[qproperty(QStringList, open_files, cxx_name = "openFiles", READ, CONSTANT)]
        #[qproperty(QStringList, place_names, cxx_name = "placeNames", READ, CONSTANT)]
        #[qproperty(QStringList, place_paths, cxx_name = "placePaths", READ, CONSTANT)]
        #[qproperty(QStringList, place_icons, cxx_name = "placeIcons", READ, CONSTANT)]
        #[qproperty(QString, screenshot_path, cxx_name = "screenshotPath", READ, CONSTANT)]
        #[qproperty(i32, quit_after_ms, cxx_name = "quitAfterMs", READ, CONSTANT)]
        #[qproperty(i32, screenshot_delay_ms, cxx_name = "screenshotDelayMs", READ, CONSTANT)]
        #[qproperty(QString, initial_search, cxx_name = "initialSearch", READ, CONSTANT)]
        #[qproperty(bool, restore_session, cxx_name = "restoreSession", READ, CONSTANT)]
        #[qproperty(bool, persist_session, cxx_name = "persistSession", READ, CONSTANT)]
        #[qproperty(QString, version, READ, CONSTANT)]
        type AppController = super::AppControllerRust;

        #[qsignal]
        #[cxx_name = "projectRootChanged"]
        fn project_root_changed(self: Pin<&mut AppController>);

        /// Opens `path` as the project folder and remembers it.
        #[qinvokable]
        #[cxx_name = "openProject"]
        fn open_project(self: Pin<&mut AppController>, path: &QString) -> bool;

        /// Converts a `file://` URL (as returned by dialogs) to a local path.
        #[qinvokable]
        #[cxx_name = "urlToPath"]
        fn url_to_path(self: &AppController, url: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "fileKind"]
        fn file_kind(self: &AppController, path: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "fileName"]
        fn file_name(self: &AppController, path: &QString) -> QString;

        #[qinvokable]
        fn exists(self: &AppController, path: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "relativePath"]
        fn relative_path(self: &AppController, path: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "absolutePath"]
        fn absolute_path(self: &AppController, path: &QString) -> QString;

        /// Text of a file (at most 8 MiB, lossy UTF-8).
        #[qinvokable]
        #[cxx_name = "readTextFile"]
        fn read_text_file(self: &AppController, path: &QString) -> QString;

        /// Layout JSON of this device's session ("" if none).
        #[qinvokable]
        #[cxx_name = "loadLayout"]
        fn load_layout(self: &AppController) -> QString;

        /// Expanded tree directories of the session, as absolute paths.
        #[qinvokable]
        #[cxx_name = "loadExpanded"]
        fn load_expanded(self: &AppController) -> QStringList;

        /// Queues the session for `.atlas/sessions/<device>/active.json` and
        /// the identities of `documents` for `.atlas/metadata.sqlite`; the
        /// writing happens on a background thread. Paths may be absolute;
        /// they are stored relative to the project.
        #[qinvokable]
        #[cxx_name = "saveSession"]
        fn save_session(
            self: Pin<&mut AppController>,
            layout_json: &QString,
            expanded: &QStringList,
            documents: &QStringList,
        ) -> bool;

        /// Blocks until queued project data is written (used on quit).
        #[qinvokable]
        #[cxx_name = "flushSession"]
        fn flush_session(self: &AppController);

        /// Current absolute path of a session document (relative or
        /// absolute input), following external moves; "" if it is gone.
        #[qinvokable]
        #[cxx_name = "resolveDocument"]
        fn resolve_document(self: &AppController, path: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "copyToClipboard"]
        fn copy_to_clipboard(self: &AppController, text: &QString);

        /// Saves a PNG of `window` (a QQuickWindow), used by `--screenshot`.
        #[qinvokable]
        #[cxx_name = "saveWindowImage"]
        unsafe fn save_window_image(self: &AppController, window: *mut QObject, path: &QString) -> bool;
    }

    impl cxx_qt::Initialize for AppController {}
    impl cxx_qt::Threading for AppController {}
}

pub struct AppControllerRust {
    project_root: QString,
    project_name: QString,
    atlas_dir: QString,
    storage_error: QString,
    index_ready: bool,
    index_status: QString,
    open_files: QStringList,
    place_names: QStringList,
    place_paths: QStringList,
    place_icons: QStringList,
    screenshot_path: QString,
    quit_after_ms: i32,
    screenshot_delay_ms: i32,
    initial_search: QString,
    restore_session: bool,
    persist_session: bool,
    version: QString,

    /// Root of the open project (also set when nothing is persisted).
    root: Option<PathBuf>,
    /// `.atlas/` access; `None` with `--no-session` or if it cannot be written.
    project: Option<Arc<Project>>,
    /// Read connection for lookups; writes go through `writer`.
    metadata: Option<Metadata>,
    writer: Option<SessionWriter>,
    index: Option<FileIndex>,
    /// Incremented per opened project; drops index results of older ones.
    generation: u64,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        let cli = Cli::get();
        let mut open_files = QStringList::default();
        for f in &cli.open {
            open_files.append(qs(f.to_string_lossy()));
        }
        let mut place_names = QStringList::default();
        let mut place_paths = QStringList::default();
        let mut place_icons = QStringList::default();
        for place in atlas_core::listing::places() {
            place_names.append(qs(place.name));
            place_paths.append(qs(place.path.to_string_lossy()));
            place_icons.append(qs(place.icon));
        }
        Self {
            project_root: QString::default(),
            project_name: QString::default(),
            atlas_dir: QString::default(),
            storage_error: QString::default(),
            index_ready: false,
            index_status: QString::default(),
            open_files,
            place_names,
            place_paths,
            place_icons,
            screenshot_path: qs(cli.screenshot.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()),
            quit_after_ms: cli.quit_after_ms.unwrap_or(0) as i32,
            screenshot_delay_ms: cli.screenshot_delay_ms.unwrap_or(2500) as i32,
            initial_search: qs(cli.search.as_deref().unwrap_or_default()),
            // `--open` starts with just the given files, but the result is
            // remembered as the new session.
            restore_session: cli.open.is_empty() && !cli.no_session,
            persist_session: !cli.no_session,
            version: qs(env!("CARGO_PKG_VERSION")),
            root: None,
            project: None,
            metadata: None,
            writer: None,
            index: None,
            generation: 0,
        }
    }
}

impl cxx_qt::Initialize for qobject::AppController {
    fn initialize(self: Pin<&mut Self>) {
        let cli = Cli::get();
        let initial = cli
            .project
            .clone()
            .or_else(|| if cli.no_session { None } else { AppState::load().last_project })
            .filter(|p| p.is_dir());
        if let Some(path) = initial {
            self.open_project(&qs(path.to_string_lossy()));
        }
    }
}

fn qs(s: impl AsRef<str>) -> QString {
    QString::from(s.as_ref())
}

fn path_of(s: &QString) -> PathBuf {
    PathBuf::from(s.to_string())
}

fn describe(report: &Report) -> String {
    let previous = match &report.previous {
        Previous::Valid => "aktualisiert".to_owned(),
        Previous::Missing => "neu angelegt".to_owned(),
        Previous::Outdated(v) => format!("neu aufgebaut (Format {v})"),
        Previous::Corrupt(_) => "neu aufgebaut (war beschädigt)".to_owned(),
    };
    format!("Index: {} Einträge, {previous}, {} ms", report.files, report.elapsed.as_millis())
}

impl qobject::AppController {
    pub fn open_project(mut self: Pin<&mut Self>, path: &QString) -> bool {
        let Ok(root) = path_of(path).canonicalize() else {
            log::warn!("cannot open project {path}");
            return false;
        };
        if !root.is_dir() {
            return false;
        }

        let (project, storage_error) = if self.persist_session {
            match Project::open(&root) {
                Ok(p) => (Some(Arc::new(p)), String::new()),
                Err(err) => {
                    log::warn!("project data will not be saved: {err:#}");
                    (None, format!("{err:#}"))
                }
            }
        } else {
            (None, String::new())
        };
        let metadata = project.as_ref().and_then(|p| match p.metadata() {
            Ok(m) => Some(m),
            Err(err) => {
                log::warn!("cannot open metadata: {err:#}");
                None
            }
        });

        let generation = {
            let mut this = self.as_mut().rust_mut();
            this.generation += 1;
            this.project_name = qs(root.file_name().map(|n| n.to_string_lossy()).unwrap_or_default());
            this.project_root = qs(root.to_string_lossy());
            this.atlas_dir =
                qs(project.as_ref().map(|p| p.atlas_dir().to_string_lossy().into_owned()).unwrap_or_default());
            this.storage_error = qs(storage_error);
            this.root = Some(root.clone());
            this.project = project.clone();
            this.metadata = metadata;
            // Dropping the previous writer finishes its pending writes.
            this.writer = project.clone().map(SessionWriter::spawn);
            this.index = None;
            this.index_ready = project.is_none();
            this.index_status = qs(if project.is_some() { "Index wird aufgebaut …" } else { "" });
            this.generation
        };
        self.as_mut().index_ready_changed();
        self.as_mut().index_status_changed();

        // The index is rebuildable data: rescan in the background and swap
        // it in atomically, repairing a missing or corrupted index on the way.
        if let Some(project) = project {
            let thread = self.qt_thread();
            std::thread::spawn(move || {
                let result = FileIndex::ensure(project.root(), &project.index_file());
                let _ = thread.queue(move |mut app| {
                    if app.generation != generation {
                        return;
                    }
                    let status = match result {
                        Ok((index, report)) => {
                            app.as_mut().rust_mut().index = Some(index);
                            describe(&report)
                        }
                        Err(err) => format!("Index nicht verfügbar: {err:#}"),
                    };
                    app.as_mut().rust_mut().index_ready = true;
                    app.as_mut().rust_mut().index_status = qs(status);
                    app.as_mut().index_ready_changed();
                    app.index_status_changed();
                });
            });
        }

        if self.persist_session {
            let mut app = AppState::load();
            app.last_project = Some(root);
            if let Err(err) = app.save() {
                log::warn!("cannot save app state: {err:#}");
            }
        }
        self.project_root_changed();
        true
    }

    pub fn url_to_path(&self, url: &QString) -> QString {
        let url = url.to_string();
        let Some(rest) = url.strip_prefix("file://") else { return qs(url) };
        qs(percent_decode(rest))
    }

    pub fn file_kind(&self, path: &QString) -> QString {
        qs(file_kind(&path_of(path)))
    }

    pub fn file_name(&self, path: &QString) -> QString {
        qs(path_of(path).file_name().map(|n| n.to_string_lossy()).unwrap_or_default())
    }

    pub fn exists(&self, path: &QString) -> bool {
        self.absolute(&path_of(path)).is_some_and(|p| p.exists())
    }

    fn absolute(&self, path: &Path) -> Option<PathBuf> {
        if path.is_absolute() {
            return Some(path.to_path_buf());
        }
        self.root.as_ref().map(|root| root.join(path))
    }

    fn relative(&self, path: &Path) -> PathBuf {
        match &self.root {
            Some(root) => path.strip_prefix(root).unwrap_or(path).to_path_buf(),
            None => path.to_path_buf(),
        }
    }

    pub fn relative_path(&self, path: &QString) -> QString {
        qs(self.relative(&path_of(path)).to_string_lossy())
    }

    pub fn absolute_path(&self, path: &QString) -> QString {
        self.absolute(&path_of(path)).map(|p| qs(p.to_string_lossy())).unwrap_or_else(|| path.clone())
    }

    pub fn read_text_file(&self, path: &QString) -> QString {
        const LIMIT: u64 = 8 * 1024 * 1024;
        let Ok(file) = std::fs::File::open(path_of(path)) else { return QString::default() };
        let mut data = Vec::new();
        use std::io::Read;
        let _ = file.take(LIMIT).read_to_end(&mut data);
        qs(String::from_utf8_lossy(&data))
    }

    fn session(&self) -> Option<ProjectSession> {
        self.project.as_ref().map(|p| p.load_session())
    }

    pub fn load_layout(&self) -> QString {
        match self.session() {
            Some(s) if !s.layout.is_null() => qs(s.layout.to_string()),
            _ => QString::default(),
        }
    }

    pub fn load_expanded(&self) -> QStringList {
        let mut list = QStringList::default();
        if let (Some(project), Some(session)) = (self.project.as_ref(), self.session()) {
            for dir in session.expanded.iter().filter_map(|d| project.absolute(d)) {
                list.append(qs(dir.to_string_lossy()));
            }
        }
        list
    }

    pub fn save_session(
        self: Pin<&mut Self>,
        layout_json: &QString,
        expanded: &QStringList,
        documents: &QStringList,
    ) -> bool {
        let Some(writer) = self.writer.as_ref() else { return false };
        let documents: Vec<PathBuf> = documents.iter().filter_map(|d| self.absolute(&path_of(d))).collect();
        let session = ProjectSession {
            expanded: expanded.iter().map(|d| self.relative(&path_of(d))).collect(),
            documents: documents.iter().map(|d| self.relative(d)).collect(),
            layout: serde_json::from_str(&layout_json.to_string()).unwrap_or(serde_json::Value::Null),
            ..Default::default()
        };
        writer.save(session, documents);
        true
    }

    pub fn flush_session(&self) {
        if let Some(writer) = self.writer.as_ref() {
            writer.flush();
        }
    }

    pub fn resolve_document(&self, path: &QString) -> QString {
        let path = path_of(path);
        let Some(project) = self.project.as_ref() else {
            return self.absolute(&path).filter(|p| p.exists()).map(|p| qs(p.to_string_lossy())).unwrap_or_default();
        };
        let relative = self.relative(&path);
        project
            .locate(self.metadata.as_ref(), self.index.as_ref(), &relative)
            .map(|p| qs(p.to_string_lossy()))
            .unwrap_or_default()
    }

    pub fn copy_to_clipboard(&self, text: &QString) {
        crate::qt_helpers::clipboard_set_text(text);
    }

    /// # Safety
    /// `window` must be null or point to a live QObject (QML passes the
    /// window object).
    pub unsafe fn save_window_image(&self, window: *mut cxx_qt::QObject, path: &QString) -> bool {
        unsafe { crate::qt_helpers::save_window_image(window, path) }
    }
}

/// Coarse file type used to pick a viewer.
pub fn file_kind(path: &Path) -> &'static str {
    if atlas_core::listing::trash_files_dir().is_some_and(|t| t == path) {
        return "trash";
    }
    if path.is_dir() {
        return "dir";
    }
    if path.symlink_metadata().is_err() {
        return "missing";
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "pdf" => "pdf",
        "md" | "markdown" | "mdown" => "markdown",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" => "image",
        _ => {
            // Treat as text unless the first KiB contains NUL bytes.
            let mut buf = [0u8; 1024];
            let n = std::fs::File::open(path).and_then(|mut f| std::io::Read::read(&mut f, &mut buf)).unwrap_or(0);
            if buf[..n].contains(&0) { "binary" } else { "text" }
        }
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok());
        if bytes[i] == b'%'
            && let Some(Ok(v)) = hex.map(|h| u8::from_str_radix(h, 16))
        {
            out.push(v);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::percent_decode;

    #[test]
    fn decodes_file_urls() {
        assert_eq!(percent_decode("/home/a%20b/%C3%A4.pdf"), "/home/a b/ä.pdf");
        assert_eq!(percent_decode("/x%2"), "/x%2");
        assert_eq!(percent_decode("/x%aä"), "/x%aä");
    }
}
