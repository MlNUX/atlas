// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Application controller: project selection, the project's `.atlas/`
//! folder (session, metadata, index) and small file helpers for QML.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use atlas_core::config::{Config, EditorConfig, EditorLaunch, editor_launch};
use atlas_core::markdown;
use atlas_core::notes::NoteIndex;
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
        // Line per entry of `openFiles` ("0" = none).
        #[qproperty(QStringList, open_lines, cxx_name = "openLines", READ, CONSTANT)]
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
        #[qproperty(QStringList, session_ids, cxx_name = "sessionIds", READ, NOTIFY = sessions_changed)]
        #[qproperty(QStringList, session_names, cxx_name = "sessionNames", READ, NOTIFY = sessions_changed)]
        #[qproperty(QString, active_session, cxx_name = "activeSession", READ, NOTIFY = sessions_changed)]
        #[qproperty(QString, active_session_name, cxx_name = "activeSessionName", READ, NOTIFY = sessions_changed)]
        // Where recovery copies of edited files go.
        #[qproperty(QString, recovery_dir, cxx_name = "recoveryDir", READ, NOTIFY = project_root_changed)]
        // Incremented whenever the note index (links, tags) changed.
        #[qproperty(i32, notes_revision, cxx_name = "notesRevision", READ, NOTIFY)]
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

        /// Queues the active session for `.atlas/sessions/<device>/<id>.json` and
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

        #[qsignal]
        #[cxx_name = "sessionsChanged"]
        fn sessions_changed(self: Pin<&mut AppController>);

        /// A session operation failed (shown to the user).
        #[qsignal]
        #[cxx_name = "sessionError"]
        fn session_error(self: Pin<&mut AppController>, message: QString);

        /// Makes `id` the active session. Save the current one first.
        #[qinvokable]
        #[cxx_name = "switchSession"]
        fn switch_session(self: Pin<&mut AppController>, id: &QString) -> bool;

        /// Creates a session – empty, or with `layout_json` (duplicate) – and
        /// returns its ID ("" on error).
        #[qinvokable]
        #[cxx_name = "createSession"]
        fn create_session(
            self: Pin<&mut AppController>,
            name: &QString,
            layout_json: &QString,
            expanded: &QStringList,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "renameSession"]
        fn rename_session(self: Pin<&mut AppController>, id: &QString, name: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "deleteSession"]
        fn delete_session(self: Pin<&mut AppController>, id: &QString) -> bool;

        /// Current absolute path of a session document (relative or
        /// absolute input), following external moves; "" if it is gone.
        #[qinvokable]
        #[cxx_name = "resolveDocument"]
        fn resolve_document(self: &AppController, path: &QString) -> QString;

        /// Remembered viewer state of a document (JSON, "" if none).
        #[qinvokable]
        #[cxx_name = "documentState"]
        fn document_state(self: &AppController, path: &QString) -> QString;

        /// Remembers the viewer state of a document (written in the background).
        #[qinvokable]
        #[cxx_name = "saveDocumentState"]
        fn save_document_state(self: &AppController, path: &QString, state_json: &QString);

        /// Asks all editors to save now (before quitting or switching).
        #[qsignal]
        #[cxx_name = "saveAllRequested"]
        fn save_all_requested(self: Pin<&mut AppController>);

        #[qinvokable]
        #[cxx_name = "requestSaveAll"]
        fn request_save_all(self: Pin<&mut AppController>);

        /// Renders Markdown of `path` for the preview: JSON with `html`,
        /// `title`, `tags`, `headings` and the number of `broken` links.
        #[qinvokable]
        #[cxx_name = "renderMarkdown"]
        fn render_markdown(self: &AppController, text: &QString, path: &QString) -> QString;

        /// Line of the heading with anchor `slug` in a Markdown file (0 if
        /// there is none).
        #[qinvokable]
        #[cxx_name = "headingLine"]
        fn heading_line(self: &AppController, path: &QString, slug: &QString) -> i32;

        /// Symbol overview of source text (JSON list of {name, kind, line,
        /// depth}; empty for unsupported languages).
        #[qinvokable]
        fn symbols(self: &AppController, path: &QString, text: &QString) -> QString;

        /// Configured external editor command ("" = `$VISUAL`/`$EDITOR` or
        /// the system default).
        #[qinvokable]
        #[cxx_name = "editorCommand"]
        fn editor_command(self: &AppController) -> QString;

        #[qinvokable]
        #[cxx_name = "editorInTerminal"]
        fn editor_in_terminal(self: &AppController) -> bool;

        /// Saves the editor command in `config.toml`; returns an error text
        /// ("" on success).
        #[qinvokable]
        #[cxx_name = "setEditorCommand"]
        fn set_editor_command(self: &AppController, command: &QString, terminal: bool) -> QString;

        /// Opens `path` at `line` in the external editor. JSON result:
        /// {mode: "detached"|"terminal"|"system"|"error", command, error};
        /// for "terminal" QML runs `command` in an Atlas terminal.
        #[qinvokable]
        #[cxx_name = "openInEditor"]
        fn open_in_editor(self: &AppController, path: &QString, line: i32) -> QString;

        /// All Markdown files of the project (absolute paths, sorted).
        #[qinvokable]
        #[cxx_name = "noteList"]
        fn note_list(self: &AppController) -> QStringList;

        /// Wraps `text` in a fenced code block (`language` may be "").
        #[qinvokable]
        #[cxx_name = "codeBlock"]
        fn code_block(self: &AppController, text: &QString, language: &QString) -> QString;

        /// Appends `block` to the note at `path` (created if missing) with an
        /// atomic save; returns an error text ("" on success).
        #[qinvokable]
        #[cxx_name = "appendToNote"]
        fn append_to_note(self: Pin<&mut AppController>, path: &QString, block: &QString) -> QString;

        /// Notes linking to `path` (JSON list of {source, line, raw}).
        #[qinvokable]
        fn backlinks(self: &AppController, path: &QString) -> QString;

        /// All tags of the project (JSON list of {key, display, count}).
        #[qinvokable]
        #[cxx_name = "noteTags"]
        fn note_tags(self: &AppController) -> QString;

        /// Notes with `tag` or one of its sub tags (case-insensitive).
        #[qinvokable]
        #[cxx_name = "notesWithTag"]
        fn notes_with_tag(self: &AppController, tag: &QString) -> QStringList;

        /// Re-reads one note into the index (after saving or reloading).
        #[qinvokable]
        #[cxx_name = "noteChanged"]
        fn note_changed(self: Pin<&mut AppController>, path: &QString);

        /// Atlas moved/renamed `from` to `to`: update links in all notes.
        /// Returns the number of rewritten files.
        #[qinvokable]
        #[cxx_name = "filesMoved"]
        fn files_moved(self: Pin<&mut AppController>, from: &QString, to: &QString) -> i32;

        /// Looks for repairs of broken links in `path` in the background;
        /// answers with `linkRepairsFound`.
        #[qinvokable]
        #[cxx_name = "findLinkRepairs"]
        fn find_link_repairs(self: Pin<&mut AppController>, path: &QString);

        /// JSON list of {line, raw, broken, suggestion, label}.
        #[qsignal]
        #[cxx_name = "linkRepairsFound"]
        fn link_repairs_found(self: Pin<&mut AppController>, path: QString, repairs: QString);

        /// Applies confirmed repairs (JSON list of {broken, suggestion}).
        #[qinvokable]
        #[cxx_name = "applyLinkRepairs"]
        fn apply_link_repairs(self: Pin<&mut AppController>, path: &QString, repairs: &QString) -> bool;

        /// Status message about note links (shown in the status bar).
        #[qsignal]
        #[cxx_name = "notesMessage"]
        fn notes_message(self: Pin<&mut AppController>, message: QString);

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
    open_lines: QStringList,
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
    session_ids: QStringList,
    session_names: QStringList,
    active_session: QString,
    active_session_name: QString,
    recovery_dir: QString,
    notes_revision: i32,

    /// Root of the open project (also set when nothing is persisted).
    root: Option<PathBuf>,
    /// `.atlas/` access; `None` with `--no-session` or if it cannot be written.
    project: Option<Arc<Project>>,
    /// Read connection for lookups; writes go through `writer`.
    metadata: Option<Metadata>,
    writer: Option<SessionWriter>,
    index: Option<FileIndex>,
    /// Links and tags of the project's Markdown files.
    notes: Option<Arc<Mutex<NoteIndex>>>,
    /// Incremented per opened project; drops index results of older ones.
    generation: u64,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        let cli = Cli::get();
        let mut open_files = QStringList::default();
        let mut open_lines = QStringList::default();
        for line in &cli.open_lines {
            open_lines.append(qs(line.to_string()));
        }
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
            open_lines,
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
            session_ids: QStringList::default(),
            session_names: QStringList::default(),
            active_session: QString::default(),
            active_session_name: QString::default(),
            recovery_dir: QString::default(),
            notes_revision: 0,
            root: None,
            project: None,
            metadata: None,
            writer: None,
            index: None,
            notes: None,
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

        let root_for_index = root.clone();
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
            this.notes = None;
            this.recovery_dir = qs(project
                .as_ref()
                .map(|p| p.atlas_dir().join("recovery"))
                .unwrap_or_else(|| atlas_core::paths::state_root().join("recovery"))
                .to_string_lossy());
            this.index_ready = project.is_none();
            this.index_status = qs(if project.is_some() { "Index wird aufgebaut …" } else { "" });
            this.generation
        };
        self.as_mut().index_ready_changed();
        self.as_mut().index_status_changed();

        // The indexes are rebuildable data: rescan in the background and swap
        // them in, repairing a missing or corrupted file index on the way. The
        // note index (links, tags, backlinks) is kept in memory and also works
        // without `.atlas/`.
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = project.as_ref().map(|p| FileIndex::ensure(p.root(), &p.index_file()));
            let notes = NoteIndex::build(&root_for_index);
            let _ = thread.queue(move |mut app| {
                if app.generation != generation {
                    return;
                }
                log::info!("note index: {} Markdown files", notes.len());
                app.as_mut().rust_mut().notes = Some(Arc::new(Mutex::new(notes)));
                app.as_mut().bump_notes();
                let Some(result) = result else { return };
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

        if self.persist_session {
            let mut app = AppState::load();
            app.last_project = Some(root);
            if let Err(err) = app.save() {
                log::warn!("cannot save app state: {err:#}");
            }
        }
        self.as_mut().refresh_sessions();
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
        if is_terminal_path(&path_of(path)) {
            return true;
        }
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
        self.project.as_ref().map(|p| p.load_named(&self.active_session.to_string()))
    }

    /// Re-reads the session list from `.atlas/` and notifies QML.
    fn refresh_sessions(mut self: Pin<&mut Self>) {
        let (mut ids, mut names) = (QStringList::default(), QStringList::default());
        let (mut active, mut active_name) = (String::new(), String::new());
        if let Some(project) = self.project.clone() {
            active = project.active_session();
            for s in project.list_sessions() {
                if s.id == active {
                    active_name = s.name.clone();
                }
                ids.append(qs(&s.id));
                names.append(qs(&s.name));
            }
        }
        {
            let mut this = self.as_mut().rust_mut();
            this.session_ids = ids;
            this.session_names = names;
            this.active_session = qs(active);
            this.active_session_name = qs(active_name);
        }
        self.sessions_changed();
    }

    fn session_failed(self: Pin<&mut Self>, err: anyhow::Error) {
        self.session_error(qs(format!("{err:#}")));
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

    fn build_session(&self, layout_json: &QString, expanded: &QStringList, documents: &[PathBuf]) -> ProjectSession {
        ProjectSession {
            expanded: expanded.iter().map(|d| self.relative(&path_of(d))).collect(),
            documents: documents.iter().map(|d| self.relative(d)).collect(),
            layout: serde_json::from_str(&layout_json.to_string()).unwrap_or(serde_json::Value::Null),
            ..Default::default()
        }
    }

    pub fn save_session(
        self: Pin<&mut Self>,
        layout_json: &QString,
        expanded: &QStringList,
        documents: &QStringList,
    ) -> bool {
        let Some(writer) = self.writer.as_ref() else { return false };
        let documents: Vec<PathBuf> = documents.iter().filter_map(|d| self.absolute(&path_of(d))).collect();
        let session = self.build_session(layout_json, expanded, &documents);
        writer.save(self.active_session.to_string(), session, documents);
        true
    }

    pub fn flush_session(&self) {
        crate::text_document::flush_io();
        if let Some(writer) = self.writer.as_ref() {
            writer.flush();
        }
    }

    pub fn switch_session(mut self: Pin<&mut Self>, id: &QString) -> bool {
        let Some(project) = self.project.clone() else { return false };
        // Pending writes for the old session must land before switching.
        self.flush_session();
        match project.set_active_session(&id.to_string()) {
            Ok(()) => {
                self.refresh_sessions();
                true
            }
            Err(err) => {
                self.as_mut().session_failed(err);
                false
            }
        }
    }

    pub fn create_session(
        mut self: Pin<&mut Self>,
        name: &QString,
        layout_json: &QString,
        expanded: &QStringList,
    ) -> QString {
        let Some(project) = self.project.clone() else { return QString::default() };
        let from = (!layout_json.is_empty()).then(|| self.build_session(layout_json, expanded, &[]));
        match project.create_session(&name.to_string(), from.as_ref()) {
            Ok(id) => {
                self.refresh_sessions();
                qs(id)
            }
            Err(err) => {
                self.as_mut().session_failed(err);
                QString::default()
            }
        }
    }

    pub fn rename_session(mut self: Pin<&mut Self>, id: &QString, name: &QString) -> bool {
        let Some(project) = self.project.clone() else { return false };
        self.flush_session();
        match project.rename_session(&id.to_string(), &name.to_string()) {
            Ok(()) => {
                self.refresh_sessions();
                true
            }
            Err(err) => {
                self.as_mut().session_failed(err);
                false
            }
        }
    }

    pub fn delete_session(mut self: Pin<&mut Self>, id: &QString) -> bool {
        let Some(project) = self.project.clone() else { return false };
        match project.delete_session(&id.to_string()) {
            Ok(()) => {
                self.refresh_sessions();
                true
            }
            Err(err) => {
                self.as_mut().session_failed(err);
                false
            }
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

    pub fn document_state(&self, path: &QString) -> QString {
        let relative = self.relative(&path_of(path));
        self.metadata.as_ref().and_then(|m| m.document_state(&relative).ok().flatten()).map(qs).unwrap_or_default()
    }

    pub fn save_document_state(&self, path: &QString, state_json: &QString) {
        // Only files inside the project are remembered.
        let path = path_of(path);
        if let (Some(writer), Some(root)) = (self.writer.as_ref(), self.root.as_ref())
            && let Ok(relative) = path.strip_prefix(root)
        {
            writer.save_document_state(relative.to_path_buf(), state_json.to_string());
        }
    }

    pub fn request_save_all(self: Pin<&mut Self>) {
        self.save_all_requested();
    }

    fn bump_notes(mut self: Pin<&mut Self>) {
        let revision = self.notes_revision.wrapping_add(1);
        self.as_mut().rust_mut().notes_revision = revision;
        self.notes_revision_changed();
    }

    fn with_notes<T>(&self, f: impl FnOnce(&mut NoteIndex) -> T) -> Option<T> {
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
        let broken = r.links.iter().filter(|l| l.is_broken()).count();
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

    pub fn editor_command(&self) -> QString {
        qs(Config::load().editor.command)
    }

    pub fn editor_in_terminal(&self) -> bool {
        Config::load().editor.terminal.unwrap_or(false)
    }

    pub fn set_editor_command(&self, command: &QString, terminal: bool) -> QString {
        let command = command.to_string();
        if let Err(err) = atlas_core::config::split_words(&command) {
            return qs(format!("{err:#}"));
        }
        let editor = EditorConfig { command, terminal: Some(terminal) };
        match Config::save_editor(&editor) {
            Ok(()) => QString::default(),
            Err(err) => qs(format!("{err:#}")),
        }
    }

    pub fn open_in_editor(&self, path: &QString, line: i32) -> QString {
        let file = path_of(path);
        let result = match editor_launch(&Config::load().editor, &file, line.max(0) as u32, 0) {
            Ok(EditorLaunch::System) => serde_json::json!({ "mode": "system" }),
            Ok(EditorLaunch::Terminal(command)) => serde_json::json!({ "mode": "terminal", "command": command }),
            Ok(EditorLaunch::Detached(command)) => match spawn_detached(&command, file.parent()) {
                Ok(()) => serde_json::json!({ "mode": "detached", "command": command }),
                Err(err) => serde_json::json!({ "mode": "error", "command": command, "error": format!("{err:#}") }),
            },
            Err(err) => serde_json::json!({ "mode": "error", "error": format!("{err:#}") }),
        };
        qs(result.to_string())
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
        let path = path_of(path);
        let links = self.with_notes(|n| n.backlinks(&path)).unwrap_or_default();
        let list: Vec<_> = links
            .iter()
            .map(|b| {
                serde_json::json!({
                    "source": b.source.to_string_lossy(),
                    "relative": self.relative(&b.source).to_string_lossy(),
                    "line": b.line,
                    "raw": b.raw,
                })
            })
            .collect();
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
        if self.root.as_ref().is_some_and(|r| path.starts_with(r)) && self.with_notes(|n| n.update(&path)).is_some() {
            self.bump_notes();
        }
    }

    pub fn files_moved(mut self: Pin<&mut Self>, from: &QString, to: &QString) -> i32 {
        let moves = vec![(path_of(from), path_of(to))];
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

/// Starts a program in its own session, detached from Atlas (it keeps
/// running when Atlas quits and never shares the terminal).
fn spawn_detached(command: &[String], dir: Option<&Path>) -> anyhow::Result<()> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    let (program, args) = command.split_first().ok_or_else(|| anyhow::anyhow!("leerer Befehl"))?;
    let mut cmd = Command::new(program);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    if let Some(dir) = dir {
        cmd.current_dir(dir);
    }
    // SAFETY: setsid is async-signal-safe.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let mut child = cmd.spawn().map_err(|err| anyhow::anyhow!("„{program}“ kann nicht gestartet werden: {err}"))?;
    // Reap the child in the background so it does not linger as a zombie.
    std::thread::spawn(move || child.wait());
    Ok(())
}

fn is_terminal_path(path: &Path) -> bool {
    path.as_os_str().as_encoded_bytes().starts_with(b"terminal:")
}

/// Coarse file type used to pick a viewer.
pub fn file_kind(path: &Path) -> &'static str {
    // Terminal tabs: `terminal:<start directory>#<id>`.
    if is_terminal_path(path) {
        return "terminal";
    }
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
        "md" | "markdown" | "mdown" | "mkd" => "markdown",
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
