// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Application controller: project selection, the project's `.atlas/`
//! folder (session, metadata, index) and small file helpers for QML.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};

use atlas_core::config::{Config, EditorConfig, EditorLaunch, editor_launch};
use atlas_core::finder;
use atlas_core::markdown;
use atlas_core::notes::NoteIndex;
use atlas_core::project::Project;
use atlas_core::project::index::{FileIndex, Previous, Report};
use atlas_core::project::metadata::Metadata;
use atlas_core::project::writer::SessionWriter;
use atlas_core::relations::Relations;
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
        /// Places: Atlas' fixed ones, or with `fileManagerPlaces` exactly
        /// Dolphin's (without KDE: the fixed ones plus the GTK bookmarks).
        #[qproperty(QStringList, place_names, cxx_name = "placeNames", READ, NOTIFY = places_changed)]
        #[qproperty(QStringList, place_paths, cxx_name = "placePaths", READ, NOTIFY = places_changed)]
        #[qproperty(QStringList, place_icons, cxx_name = "placeIcons", READ, NOTIFY = places_changed)]
        /// Image formats Qt can read here (file suffixes, lower case).
        #[qproperty(QStringList, image_formats, cxx_name = "imageFormats", READ, CONSTANT)]
        /// How many of the places are Atlas' fixed ones (0: Dolphin's).
        #[qproperty(i32, fixed_place_count, cxx_name = "fixedPlaceCount", READ, NOTIFY = places_changed)]
        /// Setting `[places] file_manager` (saved when changed).
        #[qproperty(bool, file_manager_places, cxx_name = "fileManagerPlaces", READ, WRITE = set_file_manager_places, NOTIFY = places_changed)]
        /// PDFs in dark mode (inverted brightness); `[pdf] dark` in config.toml.
        #[qproperty(bool, pdf_dark, cxx_name = "pdfDark", READ, WRITE = set_pdf_dark, NOTIFY)]
        /// Modules of the context bar (`[context]` in config.toml) as JSON:
        /// {calendar, tasks, pomodoro, pomodoro_work, pomodoro_break,
        /// pomodoro_long_break}; change with `setContextModule`.
        #[qproperty(QString, context_modules, cxx_name = "contextModules", READ, NOTIFY)]
        /// Numbers in file names by value (`[files] natural_sort`).
        #[qproperty(bool, natural_sort, cxx_name = "naturalSort", READ, WRITE = set_natural_sort, NOTIFY)]
        /// Spellcheck in notes (`[editor] spellcheck`).
        #[qproperty(bool, spellcheck, READ, WRITE = set_spellcheck, NOTIFY)]
        /// Momentum scrolling after touchpad swipes (`[files] touchpad_fling`).
        #[qproperty(bool, touchpad_fling, cxx_name = "touchpadFling", READ, WRITE = set_touchpad_fling, NOTIFY)]
        /// Last chosen order of folder views as JSON {key, ascending}
        /// (`[files]`), the start for new views.
        #[qproperty(QString, folder_sort, cxx_name = "folderSort", READ, NOTIFY)]
        /// `[ilias]` as JSON {base_url, download_dir, show_in_places}.
        #[qproperty(QString, ilias_config, cxx_name = "iliasConfig", READ, NOTIFY)]
        #[qproperty(QString, screenshot_path, cxx_name = "screenshotPath", READ, CONSTANT)]
        #[qproperty(i32, quit_after_ms, cxx_name = "quitAfterMs", READ, CONSTANT)]
        #[qproperty(bool, self_test, cxx_name = "selfTest", READ, CONSTANT)]
        #[qproperty(i32, screenshot_delay_ms, cxx_name = "screenshotDelayMs", READ, CONSTANT)]
        #[qproperty(QString, screenshot_dialog, cxx_name = "screenshotDialog", READ, CONSTANT)]
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
        /// Incremented when the project's PDF annotations change on disk.
        #[qproperty(i32, annotations_revision, cxx_name = "annotationsRevision", READ, NOTIFY)]
        // Folder of the PDF annotation files (`.atlas/annotations`, or Atlas'
        // state folder without a project).
        #[qproperty(QString, annotations_dir, cxx_name = "annotationsDir", READ, NOTIFY = project_root_changed)]
        // Project extensions waiting for trust (names).
        #[qproperty(QStringList, untrusted_extensions, cxx_name = "untrustedExtensions", READ, NOTIFY)]
        type AppController = super::AppControllerRust;

        /// Fingers came to rest on the touchpad (Wayland hold gesture):
        /// momentum scrolling stops.
        #[qsignal]
        #[cxx_name = "touchpadHeld"]
        fn touchpad_held(self: Pin<&mut AppController>);

        #[qsignal]
        #[cxx_name = "projectRootChanged"]
        fn project_root_changed(self: Pin<&mut AppController>);

        #[qsignal]
        #[cxx_name = "placesChanged"]
        fn places_changed(self: Pin<&mut AppController>);

        #[cxx_name = "setFileManagerPlaces"]
        fn set_file_manager_places(self: Pin<&mut AppController>, on: bool);
        fn set_pdf_dark(self: Pin<&mut AppController>, on: bool);
        fn set_natural_sort(self: Pin<&mut AppController>, on: bool);
        fn set_touchpad_fling(self: Pin<&mut AppController>, on: bool);
        fn set_spellcheck(self: Pin<&mut AppController>, on: bool);

        /// Misspelled words of the Markdown `text`: JSON `[[start, length]]`
        /// in characters (UTF-16); "null" if no dictionary was found.
        #[qinvokable]
        #[cxx_name = "spellCheck"]
        fn spell_check(self: &AppController, text: &QString) -> QString;

        /// Up to five suggestions for `word`.
        #[qinvokable]
        #[cxx_name = "spellSuggest"]
        fn spell_suggest(self: &AppController, word: &QString) -> QStringList;

        /// Adds `word` to the personal dictionary (config folder).
        #[qinvokable]
        #[cxx_name = "spellAdd"]
        fn spell_add(self: &AppController, word: &QString);

        /// The dictionaries in use ("de_DE, en_US"; "" if none was found).
        #[qinvokable]
        #[cxx_name = "spellLanguages"]
        fn spell_languages(self: &AppController) -> QString;

        /// Changes `[ilias]`: `base_url`, `download_dir` or
        /// `show_in_places` ("true"/"false").
        #[qinvokable]
        #[cxx_name = "setIliasConfig"]
        fn set_ilias_config(self: Pin<&mut AppController>, key: &QString, value: &QString);

        /// ILIAS state file "index" or "downloads" ("" if there is none).
        #[qinvokable]
        #[cxx_name = "iliasLoad"]
        fn ilias_load(self: &AppController, name: &QString) -> QString;

        /// The web profile (QML WebEngineProfile) whose login session
        /// reading and downloading use.
        #[qinvokable]
        #[cxx_name = "iliasUseProfile"]
        unsafe fn ilias_use_profile(self: &AppController, profile: *mut QObject) -> bool;

        /// Reads the dashboard's courses with everything in them, in the
        /// background; then `iliasScanned` (the tree is in "index").
        #[qinvokable]
        #[cxx_name = "iliasScan"]
        fn ilias_scan(self: &AppController);

        /// Reading goes on: what is read now.
        #[qsignal]
        #[cxx_name = "iliasProgress"]
        fn ilias_progress(self: Pin<&mut AppController>, text: QString);

        /// Reading ended: `status` "ok" (with the number of courses),
        /// "login" (log in again) or "error" (with the message).
        #[qsignal]
        #[cxx_name = "iliasScanned"]
        fn ilias_scanned(self: Pin<&mut AppController>, status: QString, message: QString);

        /// Downloads file `url` (ILIAS address; `download`: the address
        /// sending the file) straight into folder `dir`, named as the server
        /// says or else `title` (numbered if another file has that name; a
        /// new version replaces its earlier download there); remembers it
        /// with `info` in "downloads". Then `iliasDownloaded`.
        #[qinvokable]
        #[cxx_name = "iliasDownload"]
        fn ilias_download(
            self: &AppController,
            url: &QString,
            download: &QString,
            title: &QString,
            info: &QString,
            dir: &QString,
        );

        #[qsignal]
        #[cxx_name = "iliasDownloadProgress"]
        fn ilias_download_progress(self: Pin<&mut AppController>, url: QString, received: f64, total: f64);

        /// A download ended: `status` "ok" (with `path`), "login" or
        /// "error" (with the message).
        #[qsignal]
        #[cxx_name = "iliasDownloaded"]
        fn ilias_downloaded(
            self: Pin<&mut AppController>,
            url: QString,
            path: QString,
            status: QString,
            message: QString,
        );

        /// Adds a line to the ILIAS log (`debug.log` in the ILIAS state
        /// folder; e.g. what the login window saw).
        #[qinvokable]
        #[cxx_name = "iliasTrace"]
        fn ilias_trace(self: &AppController, line: &QString);

        /// The ILIAS log file.
        #[qinvokable]
        #[cxx_name = "iliasDebugLog"]
        fn ilias_debug_log(self: &AppController) -> QString;

        /// Remembers the university login for ILIAS: the user name in
        /// `[ilias]`, the password only encrypted with systemd-creds
        /// (otherwise nothing). Then `iliasLoginStored`.
        #[qinvokable]
        #[cxx_name = "iliasRememberLogin"]
        fn ilias_remember_login(self: &AppController, user: &QString, password: &QString);

        #[qsignal]
        #[cxx_name = "iliasLoginStored"]
        fn ilias_login_stored(self: Pin<&mut AppController>, ok: bool, message: QString);

        /// Forgets the remembered login (password and allowed login page).
        #[qinvokable]
        #[cxx_name = "iliasForgetLogin"]
        fn ilias_forget_login(self: Pin<&mut AppController>);

        /// Decrypts the remembered login, in the background; then
        /// `iliasLoginReady` (empty password if none or on failure).
        #[qinvokable]
        #[cxx_name = "iliasFetchLogin"]
        fn ilias_fetch_login(self: &AppController);

        #[qsignal]
        #[cxx_name = "iliasLoginReady"]
        fn ilias_login_ready(self: Pin<&mut AppController>, user: QString, password: QString, error: QString);

        /// Recognizes the formula in `area` (JSON `[x0, y0, x1, y1]`,
        /// points) of `page` (0-based) of PDF `path` as LaTeX (built-in
        /// pix2tex model, downloaded on first use), in the background; then
        /// `latexRecognized` with `request`.
        #[qinvokable]
        #[cxx_name = "latexFromPdf"]
        fn latex_from_pdf(self: &AppController, request: i32, path: &QString, page: i32, area: &QString);

        /// What the recognition of `request` is doing (the first time:
        /// downloading the model).
        #[qsignal]
        #[cxx_name = "latexProgress"]
        fn latex_progress(self: Pin<&mut AppController>, request: i32, text: QString);

        /// `latex` (tidied) or `error` for `request`.
        #[qsignal]
        #[cxx_name = "latexRecognized"]
        fn latex_recognized(self: Pin<&mut AppController>, request: i32, latex: QString, error: QString);

        /// Whether the formula model is loaded already (later recognitions
        /// are quick).
        #[qinvokable]
        #[cxx_name = "latexOcrWarm"]
        fn latex_ocr_warm(self: &AppController) -> bool;

        /// Atlas' state folder (`$XDG_STATE_HOME/atlas`) and config folder.
        #[qinvokable]
        #[cxx_name = "stateDir"]
        fn state_dir(self: &AppController) -> QString;
        #[qinvokable]
        #[cxx_name = "configDir"]
        fn config_dir(self: &AppController) -> QString;

        /// Re-reads `[ilias]` into `iliasConfig`.
        #[qinvokable]
        #[cxx_name = "reloadIliasConfig"]
        fn reload_ilias_config(self: Pin<&mut AppController>);

        /// Signs out: forgets the web session (all its cookies).
        #[qinvokable]
        #[cxx_name = "iliasLogout"]
        fn ilias_logout(self: &AppController);

        /// Remembers the order chosen in a folder view for new views.
        #[qinvokable]
        #[cxx_name = "rememberFolderSort"]
        fn remember_folder_sort(self: Pin<&mut AppController>, key: &QString, ascending: bool);

        /// Switches a module of the context bar on or off (`calendar`,
        /// `tasks`, `pomodoro`) or sets a pomodoro length in minutes.
        #[qinvokable]
        #[cxx_name = "setContextModule"]
        fn set_context_module(self: Pin<&mut AppController>, key: &QString, value: i32);

        /// Reads the places again (also done when the file managers' files
        /// change).
        #[qinvokable]
        #[cxx_name = "refreshPlaces"]
        fn refresh_places(self: Pin<&mut AppController>);

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

        /// The Markdown file has a checklist (`- [ ] …`, outside code blocks).
        #[qinvokable]
        #[cxx_name = "hasChecklist"]
        fn has_checklist(self: &AppController, path: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "fileName"]
        fn file_name(self: &AppController, path: &QString) -> QString;

        /// Space of the file system holding `path`: JSON {free, total} in
        /// bytes ("" if unknown).
        #[qinvokable]
        #[cxx_name = "diskUsage"]
        fn disk_usage(self: &AppController, path: &QString) -> QString;

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

        /// Pages of an `.atlasnote` from Atlas Tablet: JSON {pages: [{height}], error}.
        #[qinvokable]
        #[cxx_name = "atlasNoteInfo"]
        fn atlas_note_info(self: &AppController, path: &QString) -> QString;

        /// Page `index` of an `.atlasnote` rendered `width` pixels wide, as a
        /// `file://` URL into the cache. Never blocks: "" while the page is
        /// rendered on a background thread (ask again a moment later).
        #[qinvokable]
        #[cxx_name = "atlasNotePage"]
        fn atlas_note_page(self: &AppController, path: &QString, index: i32, width: i32) -> QString;

        /// Any rendering of page `index` that exists already, at another size
        /// ("" if none): shown scaled until `atlasNotePage` has the right one.
        #[qinvokable]
        #[cxx_name = "atlasNotePreview"]
        fn atlas_note_preview(self: &AppController, path: &QString, index: i32) -> QString;

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

        /// The sessions for the session list: JSON list of {id, name, tabs,
        /// splits, kind, saved (Unix seconds of the last save)}.
        #[qinvokable]
        #[cxx_name = "sessionSummaries"]
        fn session_summaries(self: &AppController) -> QString;
        /// The sessions of all opened project folders (one each, this
        /// project first), each with `root`.
        #[qinvokable]
        #[cxx_name = "allSessions"]
        fn all_sessions(self: &AppController) -> QString;

        /// Names the session of the project in `root` (one session per
        /// project folder).
        #[qinvokable]
        #[cxx_name = "renameProjectSession"]
        fn rename_project_session(self: Pin<&mut AppController>, root: &QString, name: &QString) -> bool;
        /// Removes the session of another project from the list (its tabs
        /// and layout; the folder stays).
        #[qinvokable]
        #[cxx_name = "forgetProject"]
        fn forget_project(self: Pin<&mut AppController>, root: &QString) -> bool;

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

        /// Quick open: best files, folders and notes for `query` (fuzzy;
        /// `path:line` jumps to a line). JSON list of {path, relative, kind,
        /// title, line}.
        #[qinvokable]
        #[cxx_name = "quickOpen"]
        fn quick_open(self: &AppController, query: &QString, limit: i32) -> QString;

        /// Re-reads the project's file list for quick open in the background.
        #[qinvokable]
        #[cxx_name = "refreshFileList"]
        fn refresh_file_list(self: Pin<&mut AppController>);

        /// Fuzzy score of `query` in `text` (-1 = no match), as quick open.
        #[qinvokable]
        #[cxx_name = "fuzzyScore"]
        fn fuzzy_score(self: &AppController, query: &QString, text: &QString) -> i32;

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

        /// Notes linking to `path`: JSON list of {source, relative, line, raw,
        /// kind ("link", "wiki", "image", "reference"), anchor (display text),
        /// anchorKind ("", "heading", "lines", "page", "quote"), context,
        /// title, tags} of the source note.
        #[qinvokable]
        fn backlinks(self: &AppController, path: &QString) -> QString;

        /// All quotes of the project: the marked text of all its PDFs, by PDF
        /// and page. JSON list of {target, targetRelative, page, annot, quote,
        /// comment, color, cited: [{source, relative, line, title}], source,
        /// relative, line, title (first citing note, else the PDF), …}.
        #[qinvokable]
        #[cxx_name = "projectQuotes"]
        fn project_quotes(self: &AppController) -> QString;

        /// Notes with `due:` in the front matter (shown read-only in the
        /// calendar): JSON list of {path, relative, title, due, status,
        /// estimate}; `due` as `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM`.
        #[qinvokable]
        #[cxx_name = "noteDeadlines"]
        fn note_deadlines(self: &AppController) -> QString;

        /// Context graph around `path` (`depth` 1 or 2), or of the whole
        /// project with `path` first (`depth` 0; unlinked parts have depth
        /// 255): JSON
        /// {nodes: [{path, relative, label, kind, depth, missing}],
        /// edges: [{from, to, count, quotes}], truncated}.
        /// Files related to `path` by hand: JSON list of {path, relative,
        /// kind, missing}.
        #[qinvokable]
        #[cxx_name = "relatedFiles"]
        fn related_files(self: &AppController, path: &QString) -> QString;
        /// Relates two files ("Verknüpfen mit …"); false if they already
        /// were (a message tells) or on errors.
        #[qinvokable]
        #[cxx_name = "relateFiles"]
        fn relate_files(self: Pin<&mut AppController>, a: &QString, b: &QString) -> bool;
        #[qinvokable]
        #[cxx_name = "unrelateFiles"]
        fn unrelate_files(self: Pin<&mut AppController>, a: &QString, b: &QString) -> bool;
        /// `path` is a folder node (its files are linked with it).
        #[qinvokable]
        #[cxx_name = "isFolderNode"]
        fn is_folder_node(self: &AppController, path: &QString) -> bool;
        /// Makes a folder a folder node ("Als Ordner-Knoten verknüpfen") or
        /// dissolves it.
        #[qinvokable]
        #[cxx_name = "setFolderNode"]
        fn set_folder_node(self: Pin<&mut AppController>, path: &QString, on: bool) -> bool;
        /// Files moved to the trash in Atlas: their relations go.
        #[qinvokable]
        #[cxx_name = "filesTrashed"]
        fn files_trashed(self: Pin<&mut AppController>, paths: &QStringList);

        #[qinvokable]
        #[cxx_name = "contextGraph"]
        fn context_graph(self: &AppController, path: &QString, depth: i32) -> QString;

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

        /// "Verknüpfen mit …" for `path`, from anywhere (Main shows the
        /// dialog).
        #[qsignal]
        #[cxx_name = "relateRequested"]
        fn relate_requested(self: Pin<&mut AppController>, path: QString);
        #[qinvokable]
        #[cxx_name = "requestRelate"]
        fn request_relate(self: Pin<&mut AppController>, path: &QString);

        /// PDF annotations of the project matching `query` (fuzzy over text,
        /// comment, bookmark title and file name; all if empty). JSON list
        /// of {path, relative, page, id, kind, text, comment, title}.
        #[qinvokable]
        #[cxx_name = "annotationSearch"]
        fn annotation_search(self: &AppController, query: &QString, limit: i32) -> QString;

        /// User defined actions for `paths` (files, a folder, or none for
        /// project actions): JSON list of {id, name, description, source}.
        #[qinvokable]
        #[cxx_name = "actionsFor"]
        fn actions_for(self: &AppController, paths: &QStringList) -> QString;

        /// Problems in the action files (JSON list of {file, message}).
        #[qinvokable]
        #[cxx_name = "actionErrors"]
        fn action_errors(self: &AppController) -> QString;

        /// The command of action `id` for `paths` at `line`: JSON {argv,
        /// display, cwd, confirm, destructive, placement ("panel"/"tab"),
        /// name} or {error}.
        /// `path` is a script Atlas can run („Ausführen“): `.sh`/`.bash`, or
        /// executable with a `#!` line.
        #[qinvokable]
        #[cxx_name = "isRunnable"]
        fn is_runnable(self: &AppController, path: &QString) -> bool;

        /// The command running the script at `path` in the terminal: JSON
        /// {name, argv, display, cwd} or {error}.
        #[qinvokable]
        #[cxx_name = "prepareScript"]
        fn prepare_script(self: &AppController, path: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "prepareAction"]
        fn prepare_action(self: &AppController, id: &QString, paths: &QStringList, line: i32) -> QString;

        /// Path of the project (`global` false) or user action file; created
        /// from a commented template if missing. "" on error.
        #[qinvokable]
        #[cxx_name = "actionsFile"]
        fn actions_file(self: &AppController, global: bool) -> QString;

        /// All extensions: JSON list of {name, title, version, description,
        /// dir, manifest, location, status, errors, actions, processors,
        /// exporters}.
        #[qinvokable]
        fn extensions(self: &AppController) -> QString;

        /// Trusts a project extension in its current form.
        #[qinvokable]
        #[cxx_name = "trustExtension"]
        fn trust_extension(self: Pin<&mut AppController>, name: &QString) -> bool;

        /// Switches an extension on or off (`config.toml`); error text or "".
        #[qinvokable]
        #[cxx_name = "setExtensionEnabled"]
        fn set_extension_enabled(self: Pin<&mut AppController>, name: &QString, enabled: bool) -> QString;

        /// Creates a new user extension called `title` from the template;
        /// manifest path ("" on errors).
        #[qinvokable]
        #[cxx_name = "newExtension"]
        fn new_extension(self: &AppController, title: &QString) -> QString;

        /// Re-reads the extensions (updates `untrustedExtensions`).
        #[qinvokable]
        #[cxx_name = "refreshExtensions"]
        fn refresh_extensions(self: Pin<&mut AppController>);

        /// Processors of active extensions for `path`: JSON list of {id,
        /// name, extension, output, auto}.
        #[qinvokable]
        #[cxx_name = "processorsFor"]
        fn processors_for(self: &AppController, path: &QString) -> QString;

        /// Converts `path` with processor `id` in the background; the result
        /// arrives via `fileProcessed`.
        #[qinvokable]
        #[cxx_name = "processFile"]
        fn process_file(self: &AppController, id: &QString, path: &QString);

        /// `kind`: "markdown", "text" or "pdf" (`content` is the PDF path),
        /// or "" with `error`.
        #[qsignal]
        #[cxx_name = "fileProcessed"]
        fn file_processed(
            self: Pin<&mut AppController>,
            path: QString,
            id: QString,
            kind: QString,
            content: QString,
            error: QString,
        );

        /// Exporters of active extensions for one file: JSON list of {id,
        /// name, extension, target, output (proposed result)}.
        #[qinvokable]
        #[cxx_name = "exportersFor"]
        fn exporters_for(self: &AppController, paths: &QStringList) -> QString;

        /// Exports `path` to `output` in the background (`fileExported`).
        #[qinvokable]
        #[cxx_name = "exportFile"]
        fn export_file(self: &AppController, id: &QString, path: &QString, output: &QString);

        #[qsignal]
        #[cxx_name = "fileExported"]
        fn file_exported(self: Pin<&mut AppController>, output: QString, ok: bool, message: QString, open: bool);

        #[qinvokable]
        #[cxx_name = "copyToClipboard"]
        fn copy_to_clipboard(self: &AppController, text: &QString);

        /// Saves `area` (JSON `[x0, y0, x1, y1]`, points) of page `page`
        /// (from 1) of `pdf` as picture into the `assets/` folder for `note`
        /// and returns the Markdown for the note: the picture and a link
        /// to the page as its source ("" on failure).
        #[qinvokable]
        #[cxx_name = "pdfAreaToNote"]
        fn pdf_area_to_note(self: &AppController, pdf: &QString, page: i32, area: &QString, note: &QString) -> QString;

        /// The tree of folder `path` as text (`├──`), 3 levels deep.
        #[qinvokable]
        #[cxx_name = "folderTree"]
        fn folder_tree(self: &AppController, path: &QString) -> QString;

        /// Pasting into the note `note`: files on the clipboard are copied
        /// into the project's `assets/` folder, a picture (screenshot) is
        /// saved there as PNG; returns the Markdown for them ("" if the
        /// clipboard holds neither: then text is pasted as usual).
        #[qinvokable]
        #[cxx_name = "pasteIntoNote"]
        fn paste_into_note(self: &AppController, note: &QString) -> QString;

        /// Files dropped on the note `note` (JSON list of paths or file
        /// URLs): copied into `assets/` like pasted ones; the Markdown.
        #[qinvokable]
        #[cxx_name = "importIntoNote"]
        fn import_into_note(self: &AppController, note: &QString, paths: &QString) -> QString;

        /// Saves a PNG of `window` (a QQuickWindow), used by `--screenshot`.
        #[qinvokable]
        #[cxx_name = "saveWindowImage"]
        unsafe fn save_window_image(self: &AppController, window: *mut QObject, path: &QString) -> bool;

        /// Sends a wheel event at window coordinates; for the smoke test.
        #[qinvokable]
        #[cxx_name = "sendWheel"]
        unsafe fn send_wheel(self: &AppController, window: *mut QObject, x: f64, y: f64, delta: i32, flags: i32);

        /// Sends a key press and release (Qt key code, modifier flags) to
        /// the window's focus item; for the smoke test.
        #[qinvokable]
        #[cxx_name = "sendKey"]
        unsafe fn send_key(self: &AppController, window: *mut QObject, key: i32, modifiers: i32);

        /// Makes `owner` responsible for deleting `object` (QObject parent),
        /// whatever its visual parent.
        #[qinvokable]
        #[cxx_name = "setOwner"]
        unsafe fn set_owner(self: &AppController, object: *mut QObject, owner: *mut QObject);

        /// Sends a mouse event (0 press, 1 move, 2 release; +10 with Ctrl) at window
        /// coordinates after `delay_ms`; for the smoke test.
        #[qinvokable]
        #[cxx_name = "postMouse"]
        unsafe fn post_mouse(self: &AppController, window: *mut QObject, kind: i32, x: f64, y: f64, delay_ms: i32);
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
    image_formats: QStringList,
    fixed_place_count: i32,
    file_manager_places: bool,
    pdf_dark: bool,
    context_modules: QString,
    natural_sort: bool,
    touchpad_fling: bool,
    spellcheck: bool,
    folder_sort: QString,
    ilias_config: QString,
    /// Watches the file managers' places files.
    places_watcher: Option<atlas_core::watcher::DirWatcher>,
    screenshot_path: QString,
    quit_after_ms: i32,
    self_test: bool,
    screenshot_delay_ms: i32,
    screenshot_dialog: QString,
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
    annotations_revision: i32,
    /// Watches the annotation store (`annotationsRevision`).
    annotations_watcher: Option<atlas_core::watcher::DirWatcher>,
    annotations_dir: QString,
    untrusted_extensions: QStringList,

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
    /// Files and folders for quick open (rebuilt in the background).
    files: Option<Arc<Vec<finder::Entry>>>,
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
        Self {
            project_root: QString::default(),
            project_name: QString::default(),
            atlas_dir: QString::default(),
            storage_error: QString::default(),
            index_ready: false,
            index_status: QString::default(),
            open_files,
            open_lines,
            place_names: QStringList::default(),
            place_paths: QStringList::default(),
            place_icons: QStringList::default(),
            image_formats: crate::qt_helpers::image_formats(),
            fixed_place_count: 0,
            file_manager_places: Config::load().places.file_manager,
            pdf_dark: Config::load().pdf.dark,
            context_modules: context_json(&Config::load().context),
            natural_sort: Config::load().files.natural_sort,
            touchpad_fling: Config::load().files.touchpad_fling,
            spellcheck: Config::load().editor.spellcheck.unwrap_or(true),
            folder_sort: folder_sort_json(&Config::load().files),
            ilias_config: ilias_json(&Config::load().ilias),
            places_watcher: None,
            screenshot_path: qs(cli.screenshot.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()),
            quit_after_ms: cli.quit_after_ms.unwrap_or(0) as i32,
            self_test: cli.self_test,
            screenshot_delay_ms: cli.screenshot_delay_ms.unwrap_or(2500) as i32,
            screenshot_dialog: qs(cli.screenshot_dialog.clone().unwrap_or_default()),
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
            annotations_revision: 0,
            annotations_watcher: None,
            annotations_dir: qs(atlas_core::paths::state_root().join(atlas_core::annotations::DIR).to_string_lossy()),
            untrusted_extensions: QStringList::default(),
            root: None,
            project: None,
            metadata: None,
            writer: None,
            index: None,
            notes: None,
            files: None,
            generation: 0,
        }
    }
}

/// The application object's thread, for the touchpad hold callback.
static HOLD_THREAD: std::sync::OnceLock<cxx_qt::CxxQtThread<qobject::AppController>> = std::sync::OnceLock::new();

/// Called by the Wayland hold gesture (touchpad_hold.cpp).
fn touchpad_held() {
    if let Some(thread) = HOLD_THREAD.get() {
        let _ = thread.queue(|app| app.touchpad_held());
    }
}

impl cxx_qt::Initialize for qobject::AppController {
    fn initialize(mut self: Pin<&mut Self>) {
        if HOLD_THREAD.set(self.qt_thread()).is_ok() {
            crate::qt_helpers::watch_touchpad_hold(touchpad_held);
        }
        self.as_mut().refresh_places();
        self.as_mut().watch_places();
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

/// Note texts read once per call, for the quoted text of quote links.
#[derive(Default)]
struct NoteTexts(std::collections::HashMap<PathBuf, Option<String>>);

impl NoteTexts {
    fn quote(&mut self, note: &Path, line: usize) -> Option<String> {
        let text = self.0.entry(note.to_path_buf()).or_insert_with(|| std::fs::read_to_string(note).ok());
        atlas_core::notes::quote_text(text.as_deref()?, line)
    }
}

impl qobject::AppController {
    pub fn refresh_places(mut self: Pin<&mut Self>) {
        let (places, fixed) =
            atlas_core::places::sidebar_places(atlas_core::listing::places(), self.file_manager_places);
        let (mut names, mut paths, mut icons) =
            (QStringList::default(), QStringList::default(), QStringList::default());
        for place in &places {
            names.append(qs(&place.name));
            paths.append(qs(place.path.to_string_lossy()));
            icons.append(qs(&place.icon));
        }
        if paths == self.place_paths
            && names == self.place_names
            && icons == self.place_icons
            && fixed as i32 == self.fixed_place_count
        {
            return;
        }
        let mut r = self.as_mut().rust_mut();
        r.place_names = names;
        r.place_paths = paths;
        r.place_icons = icons;
        r.fixed_place_count = fixed as i32;
        self.places_changed();
    }

    pub fn set_file_manager_places(mut self: Pin<&mut Self>, on: bool) {
        if self.file_manager_places == on {
            return;
        }
        let mut config = Config::load().places;
        config.file_manager = on;
        if let Err(err) = Config::save_places(&config) {
            log::warn!("cannot save [places]: {err:#}");
        }
        self.as_mut().rust_mut().file_manager_places = on;
        self.as_mut().places_changed();
        self.refresh_places();
    }

    pub fn set_pdf_dark(mut self: Pin<&mut Self>, on: bool) {
        if self.pdf_dark == on {
            return;
        }
        let mut config = Config::load().pdf;
        config.dark = on;
        if let Err(err) = Config::save_pdf(&config) {
            log::warn!("cannot save [pdf]: {err:#}");
        }
        self.as_mut().rust_mut().pdf_dark = on;
        self.as_mut().pdf_dark_changed();
    }

    pub fn set_natural_sort(mut self: Pin<&mut Self>, on: bool) {
        if self.natural_sort == on {
            return;
        }
        let mut config = Config::load().files;
        config.natural_sort = on;
        if let Err(err) = Config::save_files(&config) {
            log::warn!("cannot save [files]: {err:#}");
        }
        atlas_core::listing::set_natural_sort(on);
        self.as_mut().rust_mut().natural_sort = on;
        // Views sort again when they see the change.
        self.as_mut().natural_sort_changed();
    }

    pub fn set_touchpad_fling(mut self: Pin<&mut Self>, on: bool) {
        if self.touchpad_fling == on {
            return;
        }
        let mut config = Config::load().files;
        config.touchpad_fling = on;
        if let Err(err) = Config::save_files(&config) {
            log::warn!("cannot save [files]: {err:#}");
        }
        self.as_mut().rust_mut().touchpad_fling = on;
        self.as_mut().touchpad_fling_changed();
    }

    pub fn set_spellcheck(mut self: Pin<&mut Self>, on: bool) {
        if self.spellcheck == on {
            return;
        }
        let mut config = Config::load().editor;
        config.spellcheck = if on { None } else { Some(false) };
        if let Err(err) = Config::save_editor(&config) {
            log::warn!("cannot save [editor]: {err:#}");
        }
        self.as_mut().rust_mut().spellcheck = on;
        self.as_mut().spellcheck_changed();
    }

    pub fn spell_check(&self, text: &QString) -> QString {
        let text = text.to_string();
        with_speller(|checker| {
            let bad = checker.misspelled(&text);
            let list: Vec<[usize; 2]> =
                atlas_core::spell::utf16_ranges(&text, &bad).into_iter().map(|(s, l)| [s, l]).collect();
            serde_json::to_string(&list).unwrap_or_else(|_| "[]".into())
        })
        .map_or_else(|| qs("null"), qs)
    }

    pub fn spell_suggest(&self, word: &QString) -> QStringList {
        let mut out = QStringList::default();
        for s in with_speller(|c| c.suggest(&word.to_string(), 5)).unwrap_or_default() {
            out.append(qs(s));
        }
        out
    }

    pub fn spell_add(&self, word: &QString) {
        if let Some(Err(err)) = with_speller(|c| c.add(&word.to_string())) {
            log::warn!("spellcheck: {err:#}");
        }
    }

    pub fn spell_languages(&self) -> QString {
        qs(with_speller(|c| c.languages().join(", ")).unwrap_or_default())
    }

    pub fn set_ilias_config(mut self: Pin<&mut Self>, key: &QString, value: &QString) {
        let mut config = Config::load().ilias;
        let value = value.to_string();
        match key.to_string().as_str() {
            "base_url" => {
                let url = value.trim();
                config.base_url = if url.ends_with('/') { url.to_owned() } else { format!("{url}/") };
            }
            "download_dir" => config.download_dir = value,
            "show_in_places" => config.show_in_places = value == "true",
            "login_host" => config.login_host = value.trim().to_lowercase(),
            other => {
                log::warn!("unknown ILIAS setting {other}");
                return;
            }
        }
        if let Err(err) = Config::save_ilias(&config) {
            log::warn!("cannot save [ilias]: {err:#}");
        }
        self.as_mut().rust_mut().ilias_config = ilias_json(&config);
        self.as_mut().ilias_config_changed();
    }

    pub fn ilias_load(&self, name: &QString) -> QString {
        qs(atlas_core::ilias::load(&name.to_string()))
    }

    pub unsafe fn ilias_use_profile(&self, profile: *mut cxx_qt::QObject) -> bool {
        // SAFETY: `profile` comes from QML and outlives this call.
        unsafe { crate::qt_helpers::watch_web_cookies(profile) }
    }

    pub fn ilias_scan(&self) {
        use atlas_core::ilias::{self, IliasError};
        let base = Config::load().ilias.base_url;
        let live = crate::qt_helpers::web_cookie_header(&qs(&base)).to_string();
        let profile = PathBuf::from(crate::qt_helpers::web_profile_path().to_string());
        let agent = crate::qt_helpers::web_user_agent().to_string();
        ilias::trace_start("Einlesen");
        ilias::trace(&format!("Web-Profil (seit dem Start gemeldet): {}", crate::qt_helpers::web_cookie_summary()));
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let progress = |p: ilias::Progress| {
                let text = if p.pages == 0 {
                    format!("Lese {} …", p.text)
                } else {
                    format!("Lese … {} Seiten ({})", p.pages, p.text)
                };
                let _ = thread.queue(move |app| app.ilias_progress(qs(text)));
            };
            let cookies = ilias::Url::parse(&base).map(|u| ilias::session_cookies(&profile, &u, &live)).unwrap_or(live);
            let result = ilias::Client::new(&base, &cookies, &agent)
                .and_then(|client| ilias::scan(&client, &progress))
                .and_then(|courses| {
                    let scanned = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    let index = serde_json::json!({ "base_url": base, "scanned": scanned, "courses": courses });
                    ilias::save("index", &index.to_string()).map_err(|e| IliasError::Other(format!("{e:#}")))?;
                    Ok(courses.len())
                });
            let (status, message) = match result {
                Ok(n) => ("ok", n.to_string()),
                Err(IliasError::Login) => ("login", String::new()),
                Err(IliasError::Other(e)) => ("error", e),
            };
            let _ = thread.queue(move |app| app.ilias_scanned(qs(status), qs(message)));
        });
    }

    pub fn ilias_download(&self, url: &QString, download: &QString, title: &QString, info: &QString, dir: &QString) {
        use atlas_core::ilias::{self, IliasError};
        let config = Config::load().ilias;
        let address = if download.is_empty() { url.to_string() } else { download.to_string() };
        let live = crate::qt_helpers::web_cookie_header(&qs(&address)).to_string();
        let profile = PathBuf::from(crate::qt_helpers::web_profile_path().to_string());
        let agent = crate::qt_helpers::web_user_agent().to_string();
        let (url, title, info, dir) = (url.to_string(), title.to_string(), info.to_string(), dir.to_string());
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let last = std::sync::Mutex::new(std::time::Instant::now());
            let progress = |received: u64, total: Option<u64>| {
                let mut last = last.lock().unwrap();
                if last.elapsed() < std::time::Duration::from_millis(100) {
                    return;
                }
                *last = std::time::Instant::now();
                let url = qs(&url);
                let total = total.unwrap_or(0) as f64;
                let _ = thread.queue(move |app| app.ilias_download_progress(url, received as f64, total));
            };
            let result = (|| {
                let other = |e: anyhow::Error| IliasError::Other(format!("{e:#}"));
                let dir = ilias::target_dir(Path::new(&dir), &[]).map_err(other)?;
                let cookies =
                    ilias::Url::parse(&address).map(|u| ilias::session_cookies(&profile, &u, &live)).unwrap_or(live);
                let client = ilias::Client::new(&config.base_url, &cookies, &agent)?;
                let previous = ilias::previous_download(&url);
                let path = ilias::download(&client, &address, &dir, &title, previous.as_deref(), &progress)?;
                ilias::record_download(&url, &path, &info).map_err(other)?;
                Ok(path)
            })();
            let (path, status, message) = match result {
                Ok(path) => (path.to_string_lossy().into_owned(), "ok", String::new()),
                Err(IliasError::Login) => (String::new(), "login", String::new()),
                Err(IliasError::Other(e)) => (String::new(), "error", e),
            };
            let _ = thread.queue(move |app| app.ilias_downloaded(qs(url), qs(path), qs(status), qs(message)));
        });
    }

    pub fn ilias_trace(&self, line: &QString) {
        atlas_core::ilias::trace(&line.to_string());
    }

    pub fn ilias_debug_log(&self) -> QString {
        let path = atlas_core::ilias::debug_log();
        if !path.exists() {
            atlas_core::ilias::trace("(noch nichts protokolliert)");
        }
        qs(path.to_string_lossy())
    }

    pub fn ilias_remember_login(&self, user: &QString, password: &QString) {
        let (user, password) = (user.to_string().trim().to_owned(), password.to_string());
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<()> {
                if user.is_empty() {
                    anyhow::bail!("Benutzername fehlt");
                }
                ilias_secret().store_encrypted(&password)?;
                let mut config = Config::load().ilias;
                config.user = user;
                Config::save_ilias(&config)
            })();
            let (ok, message) = match result {
                Ok(()) => (true, "Anmeldung verschlüsselt gespeichert (systemd-creds)".to_owned()),
                Err(err) => (false, format!("{err:#}")),
            };
            atlas_core::ilias::trace(&format!("Anmeldedaten merken: {message}"));
            let _ = thread.queue(move |mut app| {
                app.as_mut().reload_ilias_config();
                app.ilias_login_stored(ok, qs(message));
            });
        });
    }

    pub fn ilias_forget_login(mut self: Pin<&mut Self>) {
        if let Err(err) = ilias_secret().forget() {
            log::warn!("cannot forget the ILIAS login: {err:#}");
        }
        let mut config = Config::load().ilias;
        config.login_host.clear();
        if let Err(err) = Config::save_ilias(&config) {
            log::warn!("cannot save [ilias]: {err:#}");
        }
        atlas_core::ilias::trace("Anmeldedaten vergessen");
        self.as_mut().reload_ilias_config();
    }

    pub fn ilias_fetch_login(&self) {
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let user = Config::load().ilias.user;
            let (password, error) = match ilias_secret().load() {
                Ok(Some((p, atlas_core::secret::Kind::Encrypted))) => (p, String::new()),
                Ok(Some(_)) => (String::new(), "nicht verschlüsselt gespeichert".to_owned()),
                Ok(None) => (String::new(), String::new()),
                Err(err) => (String::new(), format!("{err:#}")),
            };
            if !error.is_empty() {
                atlas_core::ilias::trace(&format!("Anmeldedaten nicht lesbar: {error}"));
            }
            let _ = thread.queue(move |app| app.ilias_login_ready(qs(user), qs(password), qs(error)));
        });
    }

    pub fn latex_from_pdf(&self, request: i32, path: &QString, page: i32, area: &QString) {
        let thread = self.qt_thread();
        let Ok([x0, y0, x1, y1]) = serde_json::from_str::<[f64; 4]>(&area.to_string()) else {
            log::warn!("latexFromPdf: bad area {area}");
            return;
        };
        atlas_core::pdf::PdfService::get().formula_clip(
            path_of(path),
            page.max(0) as usize,
            [x0, y0, x1, y1],
            move |result| {
                // The recognition takes a while: not on the PDF thread.
                std::thread::spawn(move || {
                    let outcome = result.and_then(|(w, h, pixels)| {
                        ensure_latex_models(&|text| {
                            let _ = thread.queue(move |app| app.latex_progress(request, qs(text)));
                        })?;
                        atlas_core::latex_ocr::recognize_gray(w, h, pixels).map(|t| atlas_core::pdf::formulas::tidy(&t))
                    });
                    let (latex, error) = match outcome {
                        Ok(l) => (l, String::new()),
                        Err(e) => (String::new(), format!("{e:#}")),
                    };
                    let _ = thread.queue(move |app| app.latex_recognized(request, qs(latex), qs(error)));
                });
            },
        );
    }

    pub fn latex_ocr_warm(&self) -> bool {
        atlas_core::latex_ocr::is_running()
    }

    pub fn state_dir(&self) -> QString {
        qs(atlas_core::paths::state_root().to_string_lossy())
    }

    pub fn config_dir(&self) -> QString {
        qs(atlas_core::config::config_dir().to_string_lossy())
    }

    pub fn reload_ilias_config(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().ilias_config = ilias_json(&Config::load().ilias);
        self.as_mut().ilias_config_changed();
    }

    pub fn ilias_logout(&self) {
        atlas_core::ilias::trace("Abmelden: alle Cookies des Web-Profils gelöscht");
        crate::qt_helpers::clear_web_cookies();
    }

    pub fn remember_folder_sort(mut self: Pin<&mut Self>, key: &QString, ascending: bool) {
        let mut config = Config::load().files;
        let key = key.to_string();
        if config.sort_key == key && config.sort_ascending == ascending {
            return;
        }
        config.sort_key = key;
        config.sort_ascending = ascending;
        if let Err(err) = Config::save_files(&config) {
            log::warn!("cannot save [files]: {err:#}");
        }
        self.as_mut().rust_mut().folder_sort = folder_sort_json(&config);
        self.as_mut().folder_sort_changed();
    }

    pub fn set_context_module(mut self: Pin<&mut Self>, key: &QString, value: i32) {
        let mut config = Config::load().context;
        let minutes = value.clamp(1, 180) as u32;
        match key.to_string().as_str() {
            "calendar" => config.calendar = value != 0,
            "tasks" => config.tasks = value != 0,
            "pomodoro" => config.pomodoro = value != 0,
            "pomodoro_work" => config.pomodoro_work = minutes,
            "pomodoro_break" => config.pomodoro_break = minutes,
            "pomodoro_long_break" => config.pomodoro_long_break = minutes,
            other => {
                log::warn!("unknown context module {other}");
                return;
            }
        }
        if let Err(err) = Config::save_context(&config) {
            log::warn!("cannot save [context]: {err:#}");
        }
        self.as_mut().rust_mut().context_modules = context_json(&config);
        self.as_mut().context_modules_changed();
    }

    /// Dolphin or a GTK file manager changed its places: read them again.
    fn watch_places(mut self: Pin<&mut Self>) {
        let sources = atlas_core::places::sources();
        let dirs: Vec<PathBuf> =
            sources.iter().filter_map(|f| f.parent().map(Path::to_path_buf)).filter(|d| d.is_dir()).collect();
        let thread = self.qt_thread();
        let watched = sources.clone();
        match atlas_core::watcher::DirWatcher::new(move |changed| {
            if changed.iter().any(|p| watched.contains(p)) {
                let _ = thread.queue(|this| {
                    if this.file_manager_places {
                        this.refresh_places();
                    }
                });
            }
        }) {
            Ok(watcher) => {
                watcher.sync(&dirs);
                self.as_mut().rust_mut().places_watcher = Some(watcher);
            }
            Err(err) => log::warn!("places watcher unavailable: {err:#}"),
        }
    }
}

fn path_of(s: &QString) -> PathBuf {
    PathBuf::from(s.to_string())
}

/// The session of `project` (one per project folder) for the session
/// list.
fn session_entry(project: &Project) -> serde_json::Value {
    let id = project.active_session();
    let summary = project.load_named(&id).summary();
    let saved = std::fs::metadata(project.session_path(&id))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());
    serde_json::json!({
        "id": id, "name": project.session_name(), "tabs": summary.tabs, "splits": summary.splits,
        "kind": summary.kind, "saved": saved, "root": project.root().to_string_lossy(),
    })
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
            this.files = None;
            this.recovery_dir = qs(project
                .as_ref()
                .map(|p| p.atlas_dir().join("recovery"))
                .unwrap_or_else(|| atlas_core::paths::state_root().join("recovery"))
                .to_string_lossy());
            this.annotations_dir = qs(project
                .as_ref()
                .map(|p| p.atlas_dir())
                .unwrap_or_else(atlas_core::paths::state_root)
                .join(atlas_core::annotations::DIR)
                .to_string_lossy());
            this.annotations_watcher = None;
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
            let files = Arc::new(finder::list(&root_for_index));
            let _ = thread.queue(move |mut app| {
                if app.generation != generation {
                    return;
                }
                log::info!("note index: {} Markdown files", notes.len());
                app.as_mut().rust_mut().notes = Some(Arc::new(Mutex::new(notes)));
                app.as_mut().rust_mut().files = Some(files);
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
            app.remember(&root);
            if let Err(err) = app.save() {
                log::warn!("cannot save app state: {err:#}");
            }
        }
        self.as_mut().refresh_sessions();
        self.as_mut().refresh_extensions();
        self.as_mut().watch_annotations();
        self.project_root_changed();
        true
    }

    /// Bumps `annotationsRevision` when the annotation store changes (marks
    /// are saved shortly after they are made, also from other windows).
    fn watch_annotations(mut self: Pin<&mut Self>) {
        let dir = PathBuf::from(self.annotations_dir.to_string());
        let _ = std::fs::create_dir_all(&dir);
        let thread = self.qt_thread();
        match atlas_core::watcher::DirWatcher::new(move |_| {
            let _ = thread.queue(|mut this| {
                let revision = this.annotations_revision.wrapping_add(1);
                this.as_mut().rust_mut().annotations_revision = revision;
                this.annotations_revision_changed();
            });
        }) {
            Ok(watcher) => {
                watcher.sync(&[dir]);
                self.as_mut().rust_mut().annotations_watcher = Some(watcher);
            }
            Err(err) => log::warn!("annotations watcher unavailable: {err:#}"),
        }
    }

    pub fn url_to_path(&self, url: &QString) -> QString {
        let url = url.to_string();
        let Some(rest) = url.strip_prefix("file://") else { return qs(url) };
        qs(percent_decode(rest))
    }

    pub fn has_checklist(&self, path: &QString) -> bool {
        let path = path_of(path);
        file_kind(&path) == "markdown"
            && std::fs::read_to_string(&path).is_ok_and(|text| !atlas_core::calendar::checklist_items(&text).is_empty())
    }

    pub fn file_kind(&self, path: &QString) -> QString {
        qs(file_kind(&path_of(path)))
    }

    pub fn disk_usage(&self, path: &QString) -> QString {
        use std::os::unix::ffi::OsStrExt;
        let Ok(c) = std::ffi::CString::new(path_of(path).as_os_str().as_bytes()) else { return QString::default() };
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        // SAFETY: `c` is a valid C string and `st` a writable statvfs.
        if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
            return QString::default();
        }
        let block = st.f_frsize as u64;
        qs(serde_json::json!({ "free": st.f_bavail as u64 * block, "total": st.f_blocks as u64 * block }).to_string())
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

    pub fn atlas_note_info(&self, path: &QString) -> QString {
        let info = match atlas_core::atlasnote::NoteFile::open_cached(&path_of(path)) {
            Ok(note) => serde_json::json!({
                "pages": note.notebook.pages.iter().map(|p| serde_json::json!({ "height": p.height })).collect::<Vec<_>>(),
                "error": "",
            }),
            Err(err) => serde_json::json!({ "pages": [], "error": format!("{err:#}") }),
        };
        qs(info.to_string())
    }

    pub fn atlas_note_page(&self, path: &QString, index: i32, width: i32) -> QString {
        let Ok(note) = atlas_core::atlasnote::NoteFile::open_cached(&path_of(path)) else { return QString::default() };
        let (index, width) = (index.max(0) as usize, width.max(1) as u32);
        match note.rendered(index, width) {
            Some(file) => qs(format!("file://{}", file.display())),
            None => {
                note.render_in_background(index, width);
                QString::default()
            }
        }
    }

    pub fn atlas_note_preview(&self, path: &QString, index: i32) -> QString {
        atlas_core::atlasnote::NoteFile::open_cached(&path_of(path))
            .ok()
            .and_then(|note| note.any_rendered(index.max(0) as usize))
            .map_or_else(QString::default, |file| qs(format!("file://{}", file.display())))
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
            active_name = project.session_name();
            for s in project.list_sessions() {
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
        crate::annotations_model::flush_io();
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

    pub fn session_summaries(&self) -> QString {
        let list: Vec<_> = self.project.as_deref().map(session_entry).into_iter().collect();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn all_sessions(&self) -> QString {
        let mut list: Vec<_> = self.project.as_deref().map(session_entry).into_iter().collect();
        if self.persist_session {
            for root in AppState::load().projects {
                if Some(&root) == self.root.as_ref() || !root.is_dir() {
                    continue;
                }
                if let Some(project) = Project::open_existing(&root) {
                    list.push(session_entry(&project));
                }
            }
        }
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn rename_project_session(mut self: Pin<&mut Self>, root: &QString, name: &QString) -> bool {
        let root = path_of(root);
        let result = if Some(&root) == self.root.as_ref() {
            let Some(project) = self.project.clone() else { return false };
            self.flush_session();
            project.rename(&name.to_string())
        } else {
            match Project::open_existing(&root) {
                Some(project) => project.rename(&name.to_string()),
                None => Err(anyhow::anyhow!("{} hat keine Session.", root.display())),
            }
        };
        match result {
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

    pub fn forget_project(mut self: Pin<&mut Self>, root: &QString) -> bool {
        let root = path_of(root);
        if Some(&root) == self.root.as_ref() {
            return false;
        }
        if let Some(project) = Project::open_existing(&root)
            && let Err(err) = project.clear_sessions()
        {
            self.as_mut().session_failed(err);
            return false;
        }
        let mut state = AppState::load();
        state.forget(&root);
        if let Err(err) = state.save() {
            self.as_mut().session_failed(err);
            return false;
        }
        self.sessions_changed();
        true
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
        let editor = EditorConfig { command, terminal: Some(terminal), ..Config::load().editor };
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

    pub fn quick_open(&self, query: &QString, limit: i32) -> QString {
        let (Some(root), Some(files)) = (self.root.as_ref(), self.files.as_ref()) else { return qs("[]") };
        let query = query.to_string();
        let (query, line) = finder::split_location(query.trim());
        let notes = self.notes.as_ref().and_then(|n| n.lock().ok());
        let mut results: Vec<(i32, serde_json::Value)> = finder::best(files, query, limit.max(1) as usize * 2)
            .into_iter()
            .map(|(entry, score)| {
                let path = finder::absolute(root, entry);
                let title = notes.as_ref().and_then(|n| n.note(&path)).and_then(|n| n.title.clone());
                let kind = if entry.is_dir {
                    "dir"
                } else if atlas_core::notes::is_markdown(&path) {
                    "note"
                } else {
                    "file"
                };
                let value = serde_json::json!({
                    "path": path.to_string_lossy(),
                    "relative": entry.relative,
                    "kind": kind,
                    "title": title,
                    "line": line,
                });
                (score, value)
            })
            .collect();
        // Notes are also found by their title.
        if let Some(notes) = notes.as_ref() {
            for path in notes.paths() {
                let Some(title) = notes.note(path).and_then(|n| n.title.clone()) else { continue };
                let Some(score) = finder::score(query, &title) else { continue };
                let path_str = path.to_string_lossy().into_owned();
                if results.iter().any(|(_, v)| v["path"] == path_str.as_str()) {
                    continue;
                }
                results.push((
                    score,
                    serde_json::json!({
                        "path": path_str,
                        "relative": path.strip_prefix(root).unwrap_or(path).to_string_lossy(),
                        "kind": "note",
                        "title": title,
                        "line": line,
                    }),
                ));
            }
        }
        results.sort_by_key(|r| std::cmp::Reverse(r.0));
        results.truncate(limit.max(1) as usize);
        qs(serde_json::Value::from(results.into_iter().map(|(_, v)| v).collect::<Vec<_>>()).to_string())
    }

    pub fn refresh_file_list(self: Pin<&mut Self>) {
        let Some(root) = self.root.clone() else { return };
        let generation = self.generation;
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let files = Arc::new(finder::list(&root));
            let _ = thread.queue(move |mut app| {
                if app.generation == generation {
                    // The graph shows all files (and folder nodes their
                    // current members): redraw on changes.
                    let changed = app.files.as_deref() != Some(&*files);
                    app.as_mut().rust_mut().files = Some(files);
                    if changed {
                        app.as_mut().bump_notes();
                    }
                }
            });
        });
    }

    pub fn fuzzy_score(&self, query: &QString, text: &QString) -> i32 {
        finder::score(&query.to_string(), &text.to_string()).unwrap_or(-1)
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

    /// Files related by hand in this project (none without `.atlas/`).
    fn relations(&self) -> Option<Relations> {
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

    fn load_actions(&self) -> (Vec<atlas_core::actions::Action>, Vec<atlas_core::actions::LoadError>) {
        let atlas_dir = self.project.as_ref().map(|p| p.atlas_dir());
        let (mut actions, errors) = atlas_core::actions::load(atlas_dir.as_deref(), &atlas_core::config::config_dir());
        // Actions of active extensions follow the user's own.
        for ext in self.active_extensions() {
            actions.extend(ext.actions);
        }
        (actions, errors)
    }

    /// All extensions with their status.
    fn extension_list(&self) -> Vec<(atlas_core::extensions::Extension, atlas_core::extensions::Status)> {
        use atlas_core::extensions;
        let project_root = self.project.as_ref().map(|p| extensions::project_root(&p.atlas_dir()));
        let found =
            extensions::discover(&extensions::user_root(&atlas_core::config::config_dir()), project_root.as_deref());
        let trust = extensions::Trust::load(&atlas_core::paths::state_root());
        let disabled = Config::load().extensions.disabled;
        found
            .into_iter()
            .map(|e| {
                let status = extensions::status(&e, &trust, &disabled);
                (e, status)
            })
            .collect()
    }

    fn active_extensions(&self) -> Vec<atlas_core::extensions::Extension> {
        self.extension_list()
            .into_iter()
            .filter(|(_, s)| *s == atlas_core::extensions::Status::Active)
            .map(|(e, _)| e)
            .collect()
    }

    pub fn extensions(&self) -> QString {
        use atlas_core::extensions::Location;
        let list: Vec<_> = self
            .extension_list()
            .iter()
            .map(|(e, status)| {
                let commands = |c: &atlas_core::actions::Command| match c {
                    atlas_core::actions::Command::Argv(argv) => argv.join(" "),
                    atlas_core::actions::Command::Shell(s) => format!("sh -c {s}"),
                };
                serde_json::json!({
                    "name": e.name,
                    "title": e.title,
                    "version": e.version,
                    "description": e.description,
                    "dir": e.dir.to_string_lossy(),
                    "manifest": e.dir.join(atlas_core::extensions::MANIFEST).to_string_lossy(),
                    "location": if e.location == Location::User { "user" } else { "project" },
                    "status": status.as_str(),
                    "errors": e.errors,
                    "actions": e.actions.iter().map(|a| serde_json::json!({ "name": a.name, "command": commands(&a.command) })).collect::<Vec<_>>(),
                    "processors": e.processors.iter().map(|p| serde_json::json!({
                        "name": p.name, "applies_to": p.applies_to, "output": p.output.as_str(), "command": commands(&p.command),
                    })).collect::<Vec<_>>(),
                    "exporters": e.exporters.iter().map(|x| serde_json::json!({
                        "name": x.name, "applies_to": x.applies_to, "target": x.target, "command": commands(&x.command),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn refresh_extensions(mut self: Pin<&mut Self>) {
        let mut names = QStringList::default();
        for (e, status) in self.extension_list() {
            if status == atlas_core::extensions::Status::Untrusted {
                names.append(qs(&e.name));
            }
        }
        self.as_mut().rust_mut().untrusted_extensions = names;
        self.untrusted_extensions_changed();
    }

    pub fn trust_extension(mut self: Pin<&mut Self>, name: &QString) -> bool {
        let name = name.to_string();
        let mut trust = atlas_core::extensions::Trust::load(&atlas_core::paths::state_root());
        let Some((ext, _)) = self.extension_list().into_iter().find(|(e, _)| e.name == name) else { return false };
        let ok = match trust.trust(&ext) {
            Ok(()) => true,
            Err(err) => {
                log::warn!("cannot store trust: {err:#}");
                false
            }
        };
        self.as_mut().refresh_extensions();
        ok
    }

    pub fn set_extension_enabled(mut self: Pin<&mut Self>, name: &QString, enabled: bool) -> QString {
        let name = name.to_string();
        let mut config = Config::load().extensions;
        config.disabled.retain(|n| n != &name);
        if !enabled {
            config.disabled.push(name);
        }
        let result = match Config::save_extensions(&config) {
            Ok(()) => QString::default(),
            Err(err) => qs(format!("{err:#}")),
        };
        self.as_mut().refresh_extensions();
        result
    }

    pub fn new_extension(&self, title: &QString) -> QString {
        let root = atlas_core::extensions::user_root(&atlas_core::config::config_dir());
        let (base, text) = atlas_core::extensions::starter(&title.to_string());
        let (mut name, mut n) = (base.clone(), 2);
        while root.join(&name).exists() {
            name = format!("{base}-{n}");
            n += 1;
        }
        let manifest = root.join(&name).join(atlas_core::extensions::MANIFEST);
        let text = text.replace(&format!("name: {base}\n"), &format!("name: {name}\n"));
        match atlas_core::paths::write_atomic(&manifest, text.as_bytes()) {
            Ok(()) => qs(manifest.to_string_lossy()),
            Err(err) => {
                log::warn!("cannot create {}: {err:#}", manifest.display());
                QString::default()
            }
        }
    }

    fn processed_cache(&self) -> PathBuf {
        self.project
            .as_ref()
            .map(|p| p.cache_dir())
            .unwrap_or_else(|| atlas_core::paths::state_root().join("cache"))
            .join("processed")
    }

    pub fn processors_for(&self, path: &QString) -> QString {
        let file = path_of(path);
        let list: Vec<_> = self
            .active_extensions()
            .iter()
            .flat_map(|e| e.processors.iter())
            .filter(|p| p.applies(&file, self.root.as_deref()))
            .map(|p| {
                serde_json::json!({ "id": p.id, "name": p.name, "extension": p.extension, "output": p.output.as_str(), "auto": p.auto })
            })
            .collect();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn process_file(&self, id: &QString, path: &QString) {
        use atlas_core::extensions::Processed;
        let id_str = id.to_string();
        let file = path_of(path);
        let processor = self.active_extensions().into_iter().flat_map(|e| e.processors).find(|p| p.id == id_str);
        let (root, cache) = (self.root.clone(), self.processed_cache());
        let (id, path) = (id.clone(), path.clone());
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = match processor {
                None => Err(anyhow::anyhow!("Die Verarbeitung „{id_str}“ ist nicht (mehr) aktiv.")),
                Some(p) => p.process(&file, root.as_deref(), &cache),
            };
            let (kind, content, error) = match result {
                Ok(Processed::Markdown(text)) => ("markdown", text, String::new()),
                Ok(Processed::Text(text)) => ("text", text, String::new()),
                Ok(Processed::Pdf(pdf)) => ("pdf", pdf.to_string_lossy().into_owned(), String::new()),
                Err(err) => ("", String::new(), format!("{err:#}")),
            };
            let _ = thread.queue(move |app| app.file_processed(path, id, qs(kind), qs(content), qs(error)));
        });
    }

    pub fn exporters_for(&self, paths: &QStringList) -> QString {
        let files: Vec<PathBuf> = paths.iter().map(path_of).collect();
        let [file] = files.as_slice() else { return qs("[]") };
        let list: Vec<_> = self
            .active_extensions()
            .iter()
            .flat_map(|e| e.exporters.iter())
            .filter(|x| x.applies(file, self.root.as_deref()))
            .map(|x| {
                serde_json::json!({
                    "id": x.id, "name": x.name, "extension": x.extension, "target": x.target,
                    "output": x.default_output(file).to_string_lossy(),
                })
            })
            .collect();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn export_file(&self, id: &QString, path: &QString, output: &QString) {
        let id = id.to_string();
        let (file, out) = (path_of(path), path_of(output));
        let exporter = self.active_extensions().into_iter().flat_map(|e| e.exporters).find(|x| x.id == id);
        let root = self.root.clone();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let (ok, message, open) = match exporter {
                None => (false, format!("Der Exporter „{id}“ ist nicht (mehr) aktiv."), false),
                Some(x) => match x.export(&file, &out, root.as_deref()) {
                    Ok(()) => (
                        true,
                        format!("„{}“ exportiert: {}", x.name, out.file_name().unwrap_or_default().to_string_lossy()),
                        x.open,
                    ),
                    Err(err) => (false, format!("{err:#}"), false),
                },
            };
            let output = qs(out.to_string_lossy());
            let _ = thread.queue(move |app| app.file_exported(output, ok, qs(message), open));
        });
    }

    fn action_context(&self, paths: &QStringList, line: i32) -> atlas_core::actions::Context {
        atlas_core::actions::Context {
            project: self.root.clone(),
            paths: paths.iter().map(path_of).filter(|p| !is_terminal_path(p)).collect(),
            line: line.max(0) as u32,
        }
    }

    pub fn actions_for(&self, paths: &QStringList) -> QString {
        let (actions, _) = self.load_actions();
        let ctx = self.action_context(paths, 0);
        let applicable = atlas_core::actions::applicable(&actions, &ctx);
        // Two actions with the same name (e.g. from two extensions) say
        // where they come from.
        let twice = |name: &str| applicable.iter().filter(|a| a.name == name).count() > 1;
        let list: Vec<_> = applicable
            .iter()
            .map(|a| {
                let name = if twice(&a.name) { format!("{} ({})", a.name, a.source.label()) } else { a.name.clone() };
                serde_json::json!({
                    "id": a.id,
                    "name": name,
                    "description": a.description,
                    "source": a.source.label(),
                })
            })
            .collect();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn action_errors(&self) -> QString {
        let (_, errors) = self.load_actions();
        let list: Vec<_> = errors
            .iter()
            .map(|e| serde_json::json!({ "file": e.file.to_string_lossy(), "message": e.message }))
            .collect();
        qs(serde_json::Value::from(list).to_string())
    }

    pub fn prepare_action(&self, id: &QString, paths: &QStringList, line: i32) -> QString {
        let (actions, _) = self.load_actions();
        let id = id.to_string();
        let ctx = self.action_context(paths, line);
        let result = match actions.iter().find(|a| a.id == id) {
            None => serde_json::json!({ "error": format!("Die Aktion „{id}“ gibt es nicht mehr.") }),
            Some(action) if !action.applies(&ctx) => {
                serde_json::json!({ "error": format!("„{}“ passt nicht zur Auswahl.", action.name) })
            }
            Some(action) => match action.prepare(&ctx) {
                Ok(p) => serde_json::json!({
                    "name": action.name,
                    "argv": p.argv,
                    "display": p.display,
                    "cwd": p.cwd.to_string_lossy(),
                    "confirm": p.confirm,
                    "destructive": p.destructive,
                    "placement": match p.placement {
                        atlas_core::actions::Placement::Panel => "panel",
                        atlas_core::actions::Placement::Tab => "tab",
                    },
                }),
                Err(err) => serde_json::json!({ "error": format!("„{}“: {err:#}", action.name) }),
            },
        };
        qs(result.to_string())
    }

    pub fn is_runnable(&self, path: &QString) -> bool {
        atlas_core::actions::script_command(&path_of(path)).is_some()
    }

    pub fn prepare_script(&self, path: &QString) -> QString {
        let result = match atlas_core::actions::prepare_script(&path_of(path)) {
            Ok((p, name)) => serde_json::json!({
                "name": name,
                "argv": p.argv,
                "display": p.display,
                "cwd": p.cwd.to_string_lossy(),
            }),
            Err(err) => serde_json::json!({ "error": format!("{err:#}") }),
        };
        qs(result.to_string())
    }

    pub fn actions_file(&self, global: bool) -> QString {
        let file = if global {
            atlas_core::actions::user_file(&atlas_core::config::config_dir())
        } else {
            match self.project.as_ref() {
                Some(p) => atlas_core::actions::project_file(&p.atlas_dir()),
                None => return QString::default(),
            }
        };
        if !file.exists()
            && let Err(err) = atlas_core::paths::write_atomic(&file, atlas_core::actions::TEMPLATE.as_bytes())
        {
            log::warn!("cannot create {}: {err:#}", file.display());
            return QString::default();
        }
        qs(file.to_string_lossy())
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
        qs(atlas_core::tree::of_folder(&path_of(path), 3, 300))
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
            .map(|p| PathBuf::from(p.strip_prefix("file://").map(percent_decode).unwrap_or_else(|| p.clone())))
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

    pub fn copy_to_clipboard(&self, text: &QString) {
        crate::qt_helpers::clipboard_set_text(text);
    }

    /// # Safety
    /// `window` must be null or point to a live QObject (QML passes the
    /// window object).
    pub unsafe fn save_window_image(&self, window: *mut cxx_qt::QObject, path: &QString) -> bool {
        unsafe { crate::qt_helpers::save_window_image(window, path) }
    }

    /// # Safety
    /// As for `save_window_image`.
    pub unsafe fn send_wheel(&self, window: *mut cxx_qt::QObject, x: f64, y: f64, delta: i32, flags: i32) {
        unsafe { crate::qt_helpers::send_wheel(window, x, y, delta, flags) }
    }

    /// # Safety
    /// As for `save_window_image`.
    pub unsafe fn send_key(&self, window: *mut cxx_qt::QObject, key: i32, modifiers: i32) {
        unsafe { crate::qt_helpers::send_key(window, key, modifiers) }
    }

    /// # Safety
    /// Both must be null or live QObjects (passed from QML).
    pub unsafe fn set_owner(&self, object: *mut cxx_qt::QObject, owner: *mut cxx_qt::QObject) {
        unsafe { crate::qt_helpers::set_owner(object, owner) }
    }

    /// # Safety
    /// As for `save_window_image`.
    pub unsafe fn post_mouse(&self, window: *mut cxx_qt::QObject, kind: i32, x: f64, y: f64, delay_ms: i32) {
        unsafe { crate::qt_helpers::post_mouse(window, kind, x, y, delay_ms) }
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

/// Tabs that are no files: `terminal:<dir>#<n>`, `calendar:` and `ilias:`.
fn is_terminal_path(path: &Path) -> bool {
    let bytes = path.as_os_str().as_encoded_bytes();
    bytes.starts_with(b"terminal:") || bytes.starts_with(b"calendar:") || bytes.starts_with(b"ilias:")
}

/// Coarse file type used to pick a viewer.
pub fn file_kind(path: &Path) -> &'static str {
    if path.as_os_str().as_encoded_bytes().starts_with(b"calendar:") {
        return "calendar";
    }
    if path.as_os_str().as_encoded_bytes().starts_with(b"ilias:") {
        return "ilias";
    }
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
        // Notebooks from Atlas Tablet (ZIP); also when a sync tool appended ".zip".
        "atlasnote" => "atlasnote",
        "zip" if path.to_string_lossy().to_lowercase().ends_with(".atlasnote.zip") => "atlasnote",
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

/// `[ilias]` for QML.
/// The remembered university login for ILIAS (encrypted only).
fn ilias_secret() -> atlas_core::secret::SecretStore {
    atlas_core::secret::SecretStore::default_for("ilias")
}

fn ilias_json(c: &atlas_core::config::IliasConfig) -> QString {
    let saved = ilias_secret().kind() == Some(atlas_core::secret::Kind::Encrypted);
    let json = serde_json::json!({
        "base_url": c.base_url, "download_dir": c.download_dir, "show_in_places": c.show_in_places,
        "user": c.user, "login_host": c.login_host, "login_saved": saved && !c.user.is_empty(),
    });
    QString::from(json.to_string().as_str())
}

/// Order of new folder views for QML.
fn folder_sort_json(f: &atlas_core::config::FilesConfig) -> QString {
    QString::from(serde_json::json!({ "key": f.sort_key, "ascending": f.sort_ascending }).to_string().as_str())
}

/// `[context]` for QML.
fn context_json(c: &atlas_core::config::ContextConfig) -> QString {
    QString::from(serde_json::to_string(c).unwrap_or_else(|_| "{}".into()).as_str())
}

/// Runs `f` with the spellchecker, loaded on first use (about 0.1 s);
/// `None` if no dictionary was found.
fn with_speller<T>(f: impl FnOnce(&mut atlas_core::spell::Checker) -> T) -> Option<T> {
    use atlas_core::spell::{Checker, dictionary_dirs};
    static SPELLER: OnceLock<Mutex<Option<Checker>>> = OnceLock::new();
    let speller = SPELLER.get_or_init(|| {
        let personal = atlas_core::config::config_dir().join("dictionary.txt");
        Mutex::new(
            Checker::load(&dictionary_dirs(), Some(personal)).inspect_err(|e| log::info!("spellcheck: {e:#}")).ok(),
        )
    });
    let mut guard = speller.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_mut().map(f)
}

/// Downloads the formula model the first time (see
/// [`atlas_core::latex_ocr`]), telling `progress` about it now and then.
pub(crate) fn ensure_latex_models(progress: &dyn Fn(String)) -> anyhow::Result<()> {
    use atlas_core::latex_ocr;
    use std::time::{Duration, Instant};
    let dir = latex_ocr::model_dir();
    if latex_ocr::models_present(&dir) {
        return Ok(());
    }
    let last = std::sync::Mutex::new(Instant::now() - Duration::from_secs(1));
    latex_ocr::download_models(&dir, &|file, received, total| {
        let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
        if last.elapsed() < Duration::from_millis(250) {
            return;
        }
        *last = Instant::now();
        let mb = |b: u64| b as f64 / 1_048_576.0;
        progress(match total {
            Some(t) => format!("Lade das Formelmodell (einmalig): {file} {:.0} von {:.0} MB", mb(received), mb(t)),
            None => format!("Lade das Formelmodell (einmalig): {file} {:.0} MB", mb(received)),
        });
    })
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
