// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Application controller: project selection, the project's `.atlas/`
//! folder (session, metadata, index) and small file helpers for QML.
//!
//! The bridge and the state are here, the methods by topic in `app/`:
//! `settings`, `sessions`, `editing` (external editor, quick open), `notes`,
//! `relations`, `extensions` and `actions`. ILIAS, spelling, formula
//! recognition and the test driver are QObjects of their own.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

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

use crate::util::qs;

use crate::cli::Cli;

mod actions;
mod editing;
mod extensions;
mod notes;
mod relations;
mod sessions;
mod settings;

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
        /// Design of the app: "dark", "light" or "system" (`[appearance]
        /// theme` in config.toml).
        #[qproperty(QString, theme, READ, WRITE = set_theme, NOTIFY)]
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
        #[qproperty(QString, screenshot_path, cxx_name = "screenshotPath", READ, CONSTANT)]
        #[qproperty(i32, quit_after_ms, cxx_name = "quitAfterMs", READ, CONSTANT)]
        #[qproperty(bool, self_test, cxx_name = "selfTest", READ, CONSTANT)]
        /// `ATLAS_TIMING` is set: log how long opening files takes (Timing.qml).
        #[qproperty(bool, timing, READ, CONSTANT)]
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
        fn set_theme(self: Pin<&mut AppController>, theme: QString);

        /// Terminals in the light palette (the design actually shown, also
        /// when `theme` follows the system).
        #[qinvokable]
        #[cxx_name = "setTerminalLight"]
        fn set_terminal_light(self: &AppController, on: bool);
        fn set_natural_sort(self: Pin<&mut AppController>, on: bool);
        fn set_touchpad_fling(self: Pin<&mut AppController>, on: bool);
        fn set_spellcheck(self: Pin<&mut AppController>, on: bool);

        /// Viewers keeping `path` open in the PDF service (smoke test).
        #[qinvokable]
        #[cxx_name = "pdfUsers"]
        fn pdf_users(self: &AppController, path: &QString) -> i32;

        /// Atlas' state folder (`$XDG_STATE_HOME/atlas`) and config folder.
        #[qinvokable]
        #[cxx_name = "stateDir"]
        fn state_dir(self: &AppController) -> QString;
        #[qinvokable]
        #[cxx_name = "configDir"]
        fn config_dir(self: &AppController) -> QString;

        /// Remembers the order chosen in a folder view for new views.
        #[qinvokable]
        #[cxx_name = "rememberFolderSort"]
        fn remember_folder_sort(self: Pin<&mut AppController>, key: &QString, ascending: bool);

        /// Switches a module of the context bar on or off (`calendar`,
        /// `tasks`, `pomodoro`) or sets a pomodoro length in minutes.
        #[qinvokable]
        #[cxx_name = "setContextModule"]
        fn set_context_module(self: Pin<&mut AppController>, key: &QString, value: i32);

        /// Stores a profile of the context bar (JSON {tabs, overview, pinned,
        /// hidden, collapsed}, each optional); "" removes it.
        #[qinvokable]
        #[cxx_name = "setContextProfile"]
        fn set_context_profile(self: Pin<&mut AppController>, name: &QString, json: &QString);

        /// Sets `empty`, `tab_labels` or `by_kind` ("true"/"false") of `[context]`.
        #[qinvokable]
        #[cxx_name = "setContextOption"]
        fn set_context_option(self: Pin<&mut AppController>, key: &QString, value: &QString);

        /// Tags of one note (from the note index), as written.
        #[qinvokable]
        #[cxx_name = "tagsOfNote"]
        fn tags_of_note(self: &AppController, path: &QString) -> QStringList;

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

        /// Active extensions that can appear as a card in the context bar:
        /// JSON [{name, title, description, shown, entries: [{id, name, kind, keybinding}]}].
        #[qinvokable]
        #[cxx_name = "contextExtensions"]
        fn context_extensions(self: &AppController) -> QString;

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
        /// `light`: dark pages on white paper with darkened ink (light theme).
        #[qinvokable]
        #[cxx_name = "atlasNotePage"]
        fn atlas_note_page(self: &AppController, path: &QString, index: i32, width: i32, light: bool) -> QString;

        /// Any rendering of page `index` that exists already, at another size
        /// ("" if none): shown scaled until `atlasNotePage` has the right one.
        #[qinvokable]
        #[cxx_name = "atlasNotePreview"]
        fn atlas_note_preview(self: &AppController, path: &QString, index: i32, light: bool) -> QString;

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

        /// Starts a prepared command (`argv`: JSON list) in the background
        /// as job `key` (an action id); error text or "".
        #[qinvokable]
        #[cxx_name = "startBackground"]
        fn start_background(
            self: &AppController,
            key: &QString,
            name: &QString,
            argv: &QString,
            cwd: &QString,
        ) -> QString;

        /// Background job `key`: JSON {key, name, cwd, running, code, log}
        /// or {} if it never ran.
        #[qinvokable]
        #[cxx_name = "backgroundStatus"]
        fn background_status(self: &AppController, key: &QString) -> QString;

        /// All background jobs: JSON list of {key, name, cwd, running, code}.
        #[qinvokable]
        #[cxx_name = "backgroundJobs"]
        fn background_jobs(self: &AppController) -> QString;

        /// Ends background job `key`; false if it does not run.
        #[qinvokable]
        #[cxx_name = "stopBackground"]
        fn stop_background(self: &AppController, key: &QString) -> bool;

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

        /// Sets one setting of extension `name` (`value`: JSON; null resets
        /// it, a secret is a text); error text or "".
        #[qinvokable]
        #[cxx_name = "setExtensionSetting"]
        fn set_extension_setting(
            self: Pin<&mut AppController>,
            name: &QString,
            key: &QString,
            value: &QString,
        ) -> QString;

        /// Extensions that come with Atlas: JSON list of {name, title,
        /// description, installed}.
        #[qinvokable]
        #[cxx_name = "bundledExtensions"]
        fn bundled_extensions(self: &AppController) -> QString;

        /// Installs (or updates) the bundled extension `name`; error text
        /// or "".
        #[qinvokable]
        #[cxx_name = "installBundled"]
        fn install_bundled(self: Pin<&mut AppController>, name: &QString) -> QString;

        /// Uninstalls the extension in folder `dir`: the folder goes to the
        /// trash, its settings, secrets and trust are forgotten; error text
        /// or "".
        #[qinvokable]
        #[cxx_name = "uninstallExtension"]
        fn uninstall_extension(self: Pin<&mut AppController>, dir: &QString) -> QString;

        /// Views of active extensions: JSON list of {id, name, extension,
        /// keybinding}.
        #[qinvokable]
        #[cxx_name = "extensionViews"]
        fn extension_views(self: &AppController) -> QString;

        /// The page of view `id` (`view:<extension>/<id>`): JSON {url, name,
        /// extension, network} or {error}.
        #[qinvokable]
        #[cxx_name = "extensionView"]
        fn extension_view(self: &AppController, id: &QString) -> QString;

        /// A request of a view's page (see atlas_core::extensions::call):
        /// JSON {ok, value} or {ok: false, error}.
        #[qinvokable]
        #[cxx_name = "extensionCall"]
        fn extension_call(self: &AppController, name: &QString, method: &QString, params: &QString) -> QString;

        /// Key sequences of actions and active extensions: JSON list of
        /// {sequence, id, name}.
        #[qinvokable]
        fn keybindings(self: &AppController) -> QString;

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

        /// Makes `owner` responsible for deleting `object` (QObject parent),
        /// whatever its visual parent.
        #[qinvokable]
        #[cxx_name = "setOwner"]
        unsafe fn set_owner(self: &AppController, object: *mut QObject, owner: *mut QObject);
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
    theme: QString,
    context_modules: QString,
    natural_sort: bool,
    touchpad_fling: bool,
    spellcheck: bool,
    folder_sort: QString,
    /// Watches the file managers' places files.
    places_watcher: Option<atlas_core::watcher::DirWatcher>,
    screenshot_path: QString,
    quit_after_ms: i32,
    self_test: bool,
    timing: bool,
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
            theme: qs(Config::load().appearance.theme()),
            context_modules: context_json(&Config::load().context),
            natural_sort: Config::load().files.natural_sort,
            touchpad_fling: Config::load().files.touchpad_fling,
            spellcheck: Config::load().editor.spellcheck.unwrap_or(true),
            folder_sort: folder_sort_json(&Config::load().files),
            places_watcher: None,
            screenshot_path: qs(cli.screenshot.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()),
            quit_after_ms: cli.quit_after_ms.unwrap_or(0) as i32,
            self_test: cli.self_test,
            timing: std::env::var_os("ATLAS_TIMING").is_some(),
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

/// Note texts read once per call, for the quoted text of quote links.
#[derive(Default)]
struct NoteTexts(std::collections::HashMap<PathBuf, Option<String>>);

impl NoteTexts {
    fn quote(&mut self, note: &Path, line: usize) -> Option<String> {
        let text = self.0.entry(note.to_path_buf()).or_insert_with(|| std::fs::read_to_string(note).ok());
        atlas_core::notes::quote_text(text.as_deref()?, line)
    }
}

/// How Atlas notes are drawn in the current theme.
fn note_look(light: bool) -> atlas_core::atlasnote::Look {
    if light { atlas_core::atlasnote::Look::Light } else { atlas_core::atlasnote::Look::AsWritten }
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
        qs(atlas_core::text::percent_decode(rest))
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

    pub fn atlas_note_page(&self, path: &QString, index: i32, width: i32, light: bool) -> QString {
        let Ok(note) = atlas_core::atlasnote::NoteFile::open_cached(&path_of(path)) else { return QString::default() };
        let (index, width) = (index.max(0) as usize, width.max(1) as u32);
        let look = note_look(light);
        match note.rendered_look(index, width, look) {
            Some(file) => qs(format!("file://{}", file.display())),
            None => {
                note.render_in_background_look(index, width, look);
                QString::default()
            }
        }
    }

    pub fn atlas_note_preview(&self, path: &QString, index: i32, light: bool) -> QString {
        atlas_core::atlasnote::NoteFile::open_cached(&path_of(path))
            .ok()
            .and_then(|note| note.any_rendered_look(index.max(0) as usize, note_look(light)))
            .map_or_else(QString::default, |file| qs(format!("file://{}", file.display())))
    }

    pub fn copy_to_clipboard(&self, text: &QString) {
        crate::qt_helpers::clipboard_set_text(text);
    }

    /// # Safety
    /// Both must be null or live QObjects (passed from QML).
    pub unsafe fn set_owner(&self, object: *mut cxx_qt::QObject, owner: *mut cxx_qt::QObject) {
        unsafe { crate::qt_helpers::set_owner(object, owner) }
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

/// Coarse file type used to pick a viewer (see [`atlas_core::listing::viewer_kind`]).
pub fn file_kind(path: &Path) -> &'static str {
    atlas_core::listing::viewer_kind(path)
}

/// `segment` for a URL path (everything but unreserved characters escaped).
fn url_escape(segment: &str) -> String {
    segment
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Order of new folder views for QML.
fn folder_sort_json(f: &atlas_core::config::FilesConfig) -> QString {
    QString::from(serde_json::json!({ "key": f.sort_key, "ascending": f.sort_ascending }).to_string().as_str())
}

/// `[context]` for QML.
fn context_json(c: &atlas_core::config::ContextConfig) -> QString {
    QString::from(serde_json::to_string(c).unwrap_or_else(|_| "{}".into()).as_str())
}
