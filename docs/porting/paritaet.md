<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Paritätstabelle Linux ↔ macOS

Alle Funktionen der Linux-Version (QML-Oberfläche und Qt-Brücke in `crates/atlas-app`)
mit ihrem Gegenstück auf dem Mac: welcher Dienst von `atlas-api` sie trägt, in welchem
Swift-Ordner sie entsteht (Aufgabe aus [`macos-swiftui.md`](macos-swiftui.md)) und wie weit
sie ist. Ziel ist volle Parität (E6). Die Tabelle wird mit jeder Aufgabe aus Phase 3
nachgetragen (P4.7 räumt sie am Ende auf).

**Stand API** (Spalte „API“):

| Zeichen | Bedeutung |
|---|---|
| ✅ | im Integrationszweig `release-1.1.5` |
| 🔶 #nn | im offenen PR #nn |
| 🟦 | braucht keine API: macht die Oberfläche (SwiftUI, PDFKit, SwiftTerm, WebKit, AppKit) |
| ⚠️ | **Lücke:** fehlt in `atlas-api`, siehe [Lücken](#lücken) |

**Stand Swift** (Spalte „Mac“): ⬜ offen · 🟨 begonnen · ✅ fertig und im UI-Test
([`testkatalog.md`](testkatalog.md)). Zu Beginn von Phase 3 steht überall ⬜.

Dienste werden kurz geschrieben: `Files::quick_open` heißt `Project::files()` →
`Files::quick_open`. Linux-Namen sind die QML-Datei oder die Funktion der Brücke.

## Fenster, Projekte, Sitzungen

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Projektordner öffnen (Strg+O, eigener Ordnerdialog) | `Main.qml`, `FolderChooser.qml`, `open_project` | `Atlas::open_project` | `App/` (P3.0), Ordnerauswahl `NSOpenPanel` | ✅ | ⬜ |
| Ein Fenster je Projekt, zuletzt geöffnete Projekte | `Main.qml`, `app.rs` | `Atlas::known_projects`, `forget_project` | `App/` (P3.0) | ✅ | ⬜ |
| Sitzungen: anlegen, wechseln, umbenennen, löschen, Kopie | `SessionMenu.qml`, `app/sessions.rs` | `Project::sessions`, `create_session`, `switch_session`, `rename_session`, `delete_session` | `App/` (P3.0) | ✅ | ⬜ |
| Sitzungen anderer Projekte, „In Session wechseln“ | `SessionMenu.qml`, `all_sessions` | `Atlas::known_projects`, `rename_project_session` | `App/` (P3.0) | ✅ | ⬜ |
| Tabs, Splits, Ziehen zwischen Splits, Rand → neuer Split | `EditorArea.qml`, `EditorGroup.qml` | `Project::save_session` (Layout als JSON) | `App/EditorArea` (P3.0) | ✅ | ⬜ |
| Sitzungszustand je Dokument (Seite, Zoom, Ansicht) | `document_state`, `save_document_state` | `Project::document_state`, `save_document_state` | je Feature-Ordner | ✅ | ⬜ |
| Vorschau-Tab, Doppelklick heftet an; Einzelklick öffnet bleibend | `EditorGroup.qml` | – | `App/EditorArea` (P3.0) | 🟦 | ⬜ |
| Leeres Fenster ohne Schließen-Knopf | `EditorGroup.qml` | – | `App/EditorArea` (P3.0) | 🟦 | ⬜ |
| Zurück / vor (Alt+←/→) | `EditorGroup.qml` | – | `App/` (P3.0) | 🟦 | ⬜ |
| Seitenleiste ein/aus (Strg+B), Explorer (Strg+Umschalt+E) | `Main.qml` | – | `App/` (P3.0), `NavigationSplitView` | 🟦 | ⬜ |
| Arbeitsleiste (Strg+P): Dateien, `>` Aktionen, `#` Tags, `@` Zitate/Anmerkungen, `!` Termine, `:` Zeile, `?` Volltext, `pfad:42` | `CommandPalette.qml`, `quick_open`, `fuzzy_score` | `Files::quick_open`, `Actions::actions_for`, `Notes::tags`, `Search::annotations`, `Calendar::search`, `Search::start` | `App/CommandPalette` (P3.0) | ✅ (Kalender 🔶 #79, Aktionen 🔶 #82) | ⬜ |
| Arbeitsleiste mit Aktionen (Strg+Umschalt+P) | `CommandPalette.qml` | `Actions::actions_for` | `App/CommandPalette` (P3.0) | 🔶 #82 | ⬜ |
| Farbschema dunkel/hell/System für alle Fenster | `Theme.qml`, `SettingsDialog.qml` | `Settings::get`/`save` | `Settings/` (P3.9), Asset-Farben | ✅ | ⬜ |
| Datei-Ereignisse: geänderte Ordner neu lesen, geänderte Dateien neu laden | `qt_helpers.rs` (Watcher) | `Files::watch`, `Event::FilesChanged`, `Event::FileModified` | `App/` (P3.0) | ✅ | ⬜ |
| Index im Hintergrund, Statusanzeige | `app.rs` | `Project::index_state`, `Event::IndexStatus` | `App/` (P3.0) | ✅ | ⬜ |
| Speichern beim Beenden, laufende Jobs beenden | `main.rs`, `request_save_all` | `Project::flush_session`, `Atlas::stop_all_background_jobs` | `App/` (P3.0) | ✅ | ⬜ |

## Dateien, Orte, Papierkorb

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Projektbaum mit Auf-/Zuklappen (gemerkt), Projektordner-Zeile | `ProjectTree.qml`, `tree_model.rs` | `Files::list`, `Project::session_expanded` | `Files/` (P3.1) | ✅ | ⬜ |
| Ordneransicht (Symbole/Details, Statusleiste, Zoom mit Strg+Mausrad) | `FolderView.qml`, `folder_model.rs` | `Files::list`, `Settings::folder_sort` | `Files/` (P3.1) | ✅ | ⬜ |
| Natürliche Sortierung, Sortierung gemerkt | `set_natural_sort`, `remember_folder_sort` | `Settings::set_folder_sort` | `Files/` (P3.1) | ✅ | ⬜ |
| Pfadleiste, Pfad eintippen (Strg+L) | `PathBar.qml` | `Files::exists`, `is_dir` | `Files/` (P3.1) | ✅ | ⬜ |
| Markierungsrechteck, Mehrfachauswahl | `FolderView.qml` | – | `Files/` (P3.1) | 🟦 | ⬜ |
| Neue Datei, neuer Ordner, Umbenennen (F2), Namensprüfung | `FileActions.qml`, `file_ops.rs` | `Files::new_file`, `new_folder`, `rename`, `check_name` | `Files/` (P3.1) | ✅ | ⬜ |
| Kopieren/Ausschneiden/Einfügen, Konflikte (Ersetzen, Überspringen, Beide behalten), Fortschritt | `file_ops.rs` | `Files::conflicts`, `transfer`, `Event::FileOperation` | `Files/` (P3.1), `NSPasteboard` (Finder-kompatibel) | ✅ | ⬜ |
| Ziehen und Ablegen (Baum, Ordner, aus Finder) | `ProjectTree.qml`, `FolderView.qml` | `Files::transfer` | `Files/` (P3.1) | ✅ | ⬜ |
| In den Papierkorb (Entf), Papierkorb ansehen, wiederherstellen, löschen | `TrashView.qml`, `trash_model.rs` | `Files::move_to_trash`, `trash_items`, `restore_from_trash`, `delete_from_trash` | `Files/` (P3.1) | ✅ | ⬜ |
| Offener Tab schließt sich, wenn die Datei gelöscht wird | `EditorArea.qml` | `Event::FilesChanged`, `Notes::files_trashed` | `App/` (P3.0) | ✅ | ⬜ |
| Eigenschaften (Rechte, Gruppe, Größe) | `PropertiesDialog.qml` | `Files::properties`, `set_mode`, `set_group`, `own_groups`, `tree_size` | `Files/` (P3.1) | ✅ | ⬜ |
| Schnellansicht (Leertaste, Strg+Klick; ←/→, Enter) | `QuickLook.qml`, `PreviewPool.qml` | – | `Files/` (P3.1), `QLPreviewPanel` | 🟦 | ⬜ |
| PDF-Vorschau in der Ordneransicht | `FolderView.qml` | – | `Files/` (P3.1), QuickLook-Miniaturen | 🟦 | ⬜ |
| Orte (wie Dolphin), Laufwerksbelegung | `PlacesList.qml`, `refresh_places`, `disk_usage` | `Atlas::places`, `Files::disk_space` | `Files/` (P3.1); auf dem Mac Finder-Seitenleiste statt Dolphin | ✅ | ⬜ |
| Kontextmenü Datei: Öffnen, im Split, im Editor, Terminal hier, Pfad kopieren, Verknüpfen, Aktionen, Ausführen, Als Aufgabe einplanen, Checkliste als Aufgaben, Exporter, „Anzeigen mit“ | `FileActions.qml` | `Files::file_kind`, `Actions`, `Extensions::exporters_for`/`processors_for`, `Calendar::task_for_file`/`checklist_of` | `Files/` (P3.1) | ✅ (🔶 #79, #82) | ⬜ |
| Im externen Editor öffnen (auch im Terminal: vim, helix …) | `open_in_editor`, `editor_in_terminal` | `Editor::external_launch`, `set_external_editor` | `Files/` (P3.1), `NSWorkspace`/SwiftTerm | ✅ | ⬜ |
| Viewer-Art einer Datei | `file_kind` | `Files::file_kind` | `App/` (P3.0) | ✅ | ⬜ |
| Datei-Links `atlas:`/`file:` in Pfade auflösen | `url_to_path` | `Atlas::url_to_path` | `App/` (P3.0) | ✅ | ⬜ |

## PDF

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Anzeigen, blättern, Zoom (Strg+Mausrad an der Mausstelle), Seitenbreite | `PdfViewer.qml`, `pdf_items.rs` | – | `PDF/` (P3.2), PDFKit | 🟦 | ⬜ |
| Text auswählen, kopieren, Volltextsuche (Strg+F, F3) | `PdfViewer.qml`, `PdfSelectionMenu.qml` | – | `PDF/` (P3.2) | 🟦 | ⬜ |
| Seitentext für Projektsuche und Index | `extract_text` | `PdfDocument::set_page_texts` | `PDF/` (P3.2) | 🔶 #81 | ⬜ |
| Anmerkungen laden (Abgleich über Pfad und Fingerabdruck, Hinweis bei geänderter oder verschobener PDF) | `annotations_model.rs` | `PdfDocument::attach`, `status`, `acknowledge_change` | `PDF/` (P3.2) | 🔶 #81 | ⬜ |
| Markieren, Unterstreichen, Durchstreichen (Strg+H, zuletzt gewählte Farbe), über mehrere Seiten | `PdfToolbar.qml`, `PdfAnnotationPopup.qml` | `PdfDocument::add`, `update`, `remove` | `PDF/` (P3.2), Umrechnung `Coordinates.swift` (P0.4) | 🔶 #81 | ⬜ |
| Kommentar, Farbe, Art wechseln, Link kopieren, Löschen | `PdfAnnotationPopup.qml` | `PdfDocument::update`, `link` | `PDF/` (P3.2) | 🔶 #81 | ⬜ |
| Lesezeichen (Strg+D), Notiz-Symbol | `PdfToolbar.qml` | `PdfDocument::add` | `PDF/` (P3.2) | 🔶 #81 | ⬜ |
| Stift und Radierer (ganze Striche), Rückgängig/Wiederholen | `PdfViewer.qml` | `PdfDocument::snapshot`, `restore` | `PDF/` (P3.2), `UndoManager` | 🔶 #81 | ⬜ |
| Zitat in Notiz (Strg+Q), Zitat-Link öffnet die Stelle | `quote_markdown`, `SendToNoteDialog.qml` | `PdfDocument::quote_markdown`, `Notes::append` | `PDF/` (P3.2) | 🔶 #81, ✅ | ⬜ |
| Bereich als Bild zitieren (PNG in `assets/`, Bildunterschrift mit Link) | `pdf_area_to_note` | – | `PDF/` (P3.2) | ⚠️ L2 | ⬜ |
| Seitenleiste: Miniaturen, Gliederung, Anmerkungen | `PdfSidebar.qml` | `PdfDocument::annotations` | `PDF/` (P3.2), `PDFThumbnailView`, `PDFOutline` | 🔶 #81 | ⬜ |
| PDF-Links (intern, extern) | `pdf_items.rs` | – | `PDF/` (P3.2) | 🟦 | ⬜ |
| Präsentation (F5): Vollbild, Pfeile, Klick, Zahl+Enter, B schwarz, Esc | `PdfViewer.qml` | – | `PDF/` (P3.2) | 🟦 | ⬜ |
| Dunkelmodus der Seiten | `PdfViewer.qml` | – | `PDF/` (P3.2), Core-Image-Filter | 🟦 | ⬜ |
| Export als Kopie mit Anmerkungen (Original bleibt) | `export_copy` | `PdfDocument::default_export_path`, `annotations` | `PDF/` (P3.2), PDFKit-Annotationen | 🔶 #81 | ⬜ |
| Formel als LaTeX: Rahmen aufziehen | `PdfLatexPopup.qml`, `formula_ocr.rs` | `LatexOcr::recognize`, `status`, `warm_up` | `PDF/` (P3.2), Bild aus `PDFPage.thumbnail` | 🔶 #81 | ⬜ |
| Formeln im Seitentext und in Zitaten als LaTeX | `pdf_items.rs` (`selection_latex`, Glyphen) | – | `PDF/` (P3.2) | ⚠️ L6 | ⬜ |
| Pfeiltasten ohne Klick, Fokus beim Tab-Wechsel | `PdfViewer.qml` | – | `PDF/` (P3.2) | 🟦 | ⬜ |
| Anmerkungen anderer Geräte (Sync) erscheinen | `reload` | `PdfDocument::reload`, `Event::AnnotationsChanged` | `PDF/` (P3.2) | 🔶 #81 | ⬜ |

## Markdown

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Editor, Hervorhebung (auch Codeblöcke nach Sprache), Zeilennummern | `TextEditor.qml`, `text_document.rs` | `Editor::highlighter`, `language_for_fence` | `Markdown/` (P3.3), `NSTextView` | ✅ | ⬜ |
| Automatisches Speichern nach 1 s, Strg+S, Wiederherstellung | `text_document.rs` | `Files::write_text`, `read_text` | `Markdown/` (P3.3) | ✅ | ⬜ |
| Listen: Enter setzt fort, Tab rückt ein, leere Zeile beendet | `markdown_edit.rs` (P1.4) | `Editor::continue_list`, `indent_list_items` | `Markdown/` (P3.3) | ✅ | ⬜ |
| Tabellen: Tab/Enter, ausrichten, formatieren | `markdown_edit.rs` | `Editor::table_tab`, `table_enter`, `align_table_at`, `format_table` | `Markdown/` (P3.3) | ✅ | ⬜ |
| Checkboxen umschalten (Editor und Vorschau) | `markdown_edit.rs`, `MarkdownView.qml` | `Editor::toggle_task` | `Markdown/` (P3.3) | ✅ | ⬜ |
| Wiki-Links `[[` mit Vorschlägen, Alias, Überschrift; `#Tags` | `TextEditor.qml` | `Editor::completion_at`, `completions`, `accept_completion` | `Markdown/` (P3.3) | ✅ | ⬜ |
| Vorschau: KaTeX, Mermaid (mit Vorlagen), Tabellen, Bilder, Ordnerbäume, Checklisten, Tags, Zitate | `MarkdownView.qml`, `web/preview.html` | `Editor::render_markdown` | `Markdown/` (P3.3), `WKWebView` + `preview.html` | ✅ | ⬜ |
| Vorschau: Kopieren-Knopf, „Ausführen“ bei bash, Abschnitte einklappen, defekte Links markiert | `web/preview.html` | `Editor::render_markdown`, `code_block` | `Markdown/` (P3.3) | ✅ | ⬜ |
| Geteilt (Editor + Vorschau) mit Scroll-Sync | `MarkdownView.qml` | – | `Markdown/` (P3.3) | 🟦 | ⬜ |
| Bild vergrößern im eigenen Fenster | `ImageWindow.qml` | – | `Viewers/` (P3.12) | 🟦 | ⬜ |
| Einfügen: Bild aus der Zwischenablage, Dateien (in `assets/`) | `paste_into_note`, `import_into_note` | – | `Markdown/` (P3.3) | ⚠️ L3 | ⬜ |
| Einfügen: Ordnerbaum, Tabelle, Diagramm-Vorlage | `folder_tree`, `MarkdownView.qml` | `Editor::format_table` | `Markdown/` (P3.3) | ⚠️ L4 (Ordnerbaum) | ⬜ |
| Rechtschreibprüfung Deutsch/Englisch, eigenes Wörterbuch | `spelling.rs` | `Spelling::available`, `check`, `suggest`, `add` (sonst `NSSpellChecker`) | `Markdown/` (P3.3) | ✅ | ⬜ |
| Markdown als PDF exportieren | `MarkdownPdfExport.qml` | `Editor::render_markdown` | `Markdown/` (P3.3), `WKWebView.createPDF` | ✅ | ⬜ |
| Gliederung (Überschriften), Sprung zur Überschrift | `ContextPanel.qml` | `Editor::render_markdown`, `Notes::heading_line` | `Markdown/` (P3.3), `Context/` (P3.6) | ✅ | ⬜ |
| Notiz-Fristen aus dem Front Matter | `note_deadlines` | `Notes::deadlines` | `Context/` (P3.6), `Calendar/` (P3.7) | 🔶 #78 | ⬜ |

## Notizen, Links, Graph

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Rückverweise mit Art, Kontext und Filtern | `backlinks`, `ContextPanel.qml` | `Notes::backlinks` | `Context/` (P3.6) | 🔶 #78 | ⬜ |
| Tags (hierarchisch), Notizen mit Tag | `note_tags`, `notes_with_tag`, `tags_of_note` | `Notes::tags`, `notes_with_tag`, `tags_of` | `Context/` (P3.6) | 🔶 #78 | ⬜ |
| Zitate in einer Notiz | `ContextPanel.qml` | `Notes::quotes` | `Context/` (P3.6) | 🔶 #78 | ⬜ |
| Zitate im Projekt, nach PDF gruppiert | `project_quotes` | – | `Context/` (P3.6) | ⚠️ L1 | ⬜ |
| Graph der Umgebung und des ganzen Projekts, eigenes Fenster | `GraphDialog.qml`, `context_graph` | `Notes::graph` | `Context/` (P3.6), Canvas | 🔶 #78 | ⬜ |
| Dateien von Hand verknüpfen, Ordner-Knoten | `RelateDialog.qml`, `relate_files`, `set_folder_node` | `Notes::related`, `relate`, `unrelate`, `set_folder_node` | `Context/` (P3.6) | 🔶 #78 | ⬜ |
| Links, Zitate, Verknüpfungen und Anmerkungen folgen Umbenennungen | `files_moved` | `Notes::files_moved`, `Calendar::files_moved` | `Files/` (P3.1) | 🔶 #78, #79 | ⬜ |
| Links reparieren nach Verschieben außerhalb von Atlas | `find_link_repairs`, `apply_link_repairs` | `Notes::find_link_repairs`, `Event::LinkRepairsFound` | `App/` (P3.0) | 🔶 #78 | ⬜ |
| Text an Notiz anhängen (Terminal-Ausgabe, Zitat) | `append_to_note`, `SendToNoteDialog.qml` | `Notes::append` | `App/` (P3.0) | 🔶 #78 | ⬜ |

## Text und Code

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Editor mit Hervorhebung für viele Sprachen | `TextEditor.qml` | `Editor::language`, `highlighter` | `Code/` (P3.4) | ✅ | ⬜ |
| Symbole (Funktionen, Typen) | `symbols` | `Editor::symbols` | `Code/` (P3.4) | ✅ | ⬜ |
| Suchen/Ersetzen im Dokument (Strg+F), Gehe zu Zeile (Strg+G) | `TextEditor.qml` | – | `Code/` (P3.4) | 🟦 | ⬜ |
| Skripte ausführen („Ausführen“ nur bei Skripten) | `is_runnable`, `prepare_script` | `Actions::is_runnable`, `prepare_script` | `Code/` (P3.4), `Terminal/` (P3.5) | 🔶 #82 | ⬜ |
| Touchpad-Schwung, Mausrad ohne Schwung | `TouchpadFling.qml` | – | macOS macht es selbst | 🟦 | ⬜ |

## Suche

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Projektweite Suche (Strg+Umschalt+F) in Notizen, Code und PDFs | `SearchDialog.qml`, `search_model.rs` | `Search::start`, `cancel`, `Event::SearchMatches`/`SearchFinished` | `Search/` (P3.8) | ✅ | ⬜ |
| Groß-/Kleinschreibung, ganze Wörter, Typfilter, ohne Build-Ordner | `SearchDialog.qml` | `SearchQuery` | `Search/` (P3.8) | ✅ | ⬜ |
| Vorschau mit Kontext (auch PDF-Seite) | `context` | `Search::context` | `Search/` (P3.8) | ✅ | ⬜ |
| Treffer öffnet und markiert im Dokument (Editor, Vorschau, PDF) | `SearchDialog.qml` | – | `Search/` (P3.8), je Viewer | 🟦 | ⬜ |
| Suche in Anmerkungen (`@`) | `annotation_search` | `Search::annotations` | `App/CommandPalette` (P3.0) | ✅ | ⬜ |

## Terminal

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Terminal-Panel (Strg+\`), Terminal-Tab, neues Terminal | `TerminalPanel.qml`, `TerminalPane.qml`, `terminal_item.rs` | `Terminal::shell` | `Terminal/` (P3.5), SwiftTerm | ✅ | ⬜ |
| Panel nebeneinander/untereinander teilen, als Baum in der Sitzung | `TerminalGroup.qml` | `Project::save_session` | `Terminal/` (P3.5) | ✅ | ⬜ |
| Startordner, aktueller Ordner des Prozesses | `terminal_item.rs` | `Terminal::process_cwd` | `Terminal/` (P3.5) | ✅ | ⬜ |
| Farben passend zum Schema | `set_terminal_light` | `Terminal::palette` | `Terminal/` (P3.5) | ✅ | ⬜ |
| Kopieren/Einfügen, Ausgabe in Notiz übernehmen | `TerminalPane.qml` | `Notes::append` | `Terminal/` (P3.5) | 🔶 #78 | ⬜ |
| Terminal läuft weiter, wenn der Tab gezogen wird | `EditorArea.qml` | – | `Terminal/` (P3.5) | 🟦 | ⬜ |

## Kontextleiste

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Bausteine: Anmerkungen, Gliederung, Links, Tags, Dateiinfo, Graph, Aktionen, Heute, Woche, Aufgaben, Pomodoro | `ContextPanel.qml`, `ContextBlock.qml`, `ContextModules.qml` | `Context::catalog`, `layout` | `Context/` (P3.6) | ✅ | ⬜ |
| Reiterseiten, schmal als Symbole, breit ausgeschrieben | `ContextPanel.qml` | `Context::options`, `new_page`, `rename_page`, `remove_tab` | `Context/` (P3.6) | ✅ | ⬜ |
| Bearbeitungsmodus: einklappen, verschieben (Alt+↑), ausblenden, anheften, Galerie | `ContextEditor.qml` | `Context::move_block`, `move_by`, `set_hidden`, `set_collapsed`, `unplace` | `Context/` (P3.6) | ✅ | ⬜ |
| Profile nach Dateityp, „Gilt für“, Rückgängig, zurücksetzen | `ContextProfiles.qml` (P1.3) | `Context::profiles`, `profile_for`, `snapshot`, `restore`, `reset` | `Context/` (P3.6) | ✅ | ⬜ |
| Karten der Erweiterungen (Ansichten, Befehle) | `context_extensions`, `ExtensionCards.qml` | – | `Context/` (P3.6) | ⚠️ L5 | ⬜ |
| Pomodoro (Lernzeit, Pause) | `PomodoroTimer.qml` | `Context::options` | `Context/` (P3.6) | ✅ | ⬜ |
| Inspector (Datei-Aktionen der Leiste) | `Inspector.qml` | `Actions`, `Extensions` | `Context/` (P3.6) | 🔶 #82 | ⬜ |

## Kalender

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Kalender als Tab, Monat, Woche, Agenda; Heute (T), blättern | `CalendarView.qml`, `CalendarMonth.qml`, `CalendarWeek.qml` | `Calendar::occurrences`, `view_state` | `Calendar/` (P3.7) | 🔶 #79 | ⬜ |
| Einträge: Termin, Arbeitsblock, Frist, Aufgabe; Dialog mit Wiederholung, Erinnerungen, Checkliste, Tags | `EntryDialog.qml`, `calendar_model/edit.rs` | `Calendar::save`, `entry`, `remove`, `Recurrence` | `Calendar/` (P3.7) | 🔶 #79 | ⬜ |
| Ziehen: verschieben (15-min-Raster), Dauer ändern, neue Zeit aufziehen | `CalendarWeek.qml` | `Calendar::move_occurrence`, `save` | `Calendar/` (P3.7) | 🔶 #79 | ⬜ |
| Aufgaben ohne Zeit, einplanen und zurück | `CalendarView.qml` | `Calendar::tasks`, `schedule` | `Calendar/` (P3.7) | 🔶 #79 | ⬜ |
| Dateien verknüpfen, Datei auf freie Zeit → Arbeitsblock, Checkliste als Aufgaben | `link_file`, `block_for_file`, `add_tasks` | `Calendar::link_file`, `block_for_file`, `task_for_file`, `checklist_of`, `add_tasks` | `Calendar/` (P3.7) | 🔶 #79 | ⬜ |
| Erinnerungen | `due_reminders` | `Calendar::due_reminders` | `Calendar/` (P3.7), `UNUserNotificationCenter` | 🔶 #79 | ⬜ |
| Kalenderauswahl, nur in diesem Projekt, Standardkalender | `CalendarSync.qml` | `Calendar::calendars`, `set_calendar_visible`, `set_calendar_project_only`, `set_default_calendar` | `Calendar/` (P3.7) | 🔶 #79 | ⬜ |
| Nextcloud/CalDAV: Verbindung testen, Konto, Kalender anlegen | `CalendarSync.qml`, `calendar_model/account.rs` | `Calendar::test_connection`, `save_account`, `create_remote_calendar` | `Calendar/` (P3.7) | 🔶 #79 | ⬜ |
| Google-Anmeldung im Browser | `google_sign_in` | `Calendar::google_sign_in`, `google_sign_in_finished` | `Calendar/` (P3.7), `NSWorkspace.open` | 🔶 #79 | ⬜ |
| Passwort: eingeben, merken, Passwort-Befehl | `CalendarSync.qml` | `Calendar::set_password`, `forget_password`, `retry_password_command` | `Calendar/` (P3.7), Schlüsselbund | 🔶 #79 | ⬜ |
| Synchronisieren, Status, Konflikte lösen, Cache leeren, trennen | `calendar_model/remote.rs` | `Calendar::sync_now`, `sync_state`, `conflicts`, `resolve_conflict`, `clear_cache`, `disconnect` | `Calendar/` (P3.7) | 🔶 #79 | ⬜ |
| macOS-Kalender (EventKit) neben den Atlas-Einträgen | – (neu) | – | `Calendar/EventKit/` (P3.13) | 🟦 | ⬜ |

## Aktionen und Erweiterungen

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Eigene Aktionen (Projekt, Benutzer), Fehler in den Dateien, Datei öffnen | `app/actions.rs` | `Actions::actions_for`, `errors`, `actions_file` | `Extensions/` (P3.10) | 🔶 #82 | ⬜ |
| Aktion ausführen im Panel, Tab oder Hintergrund; Rückfrage bei Gefährlichem | `prepare_action` | `Actions::prepare`, `start_background` | `Extensions/` (P3.10), `Terminal/` (P3.5) | 🔶 #82 | ⬜ |
| Hintergrundjobs: Status, Ausgabe, Stopp | `background_status`, `stop_background` | `Atlas::background_jobs`, `background_job`, `stop_background_job` | `Extensions/` (P3.10) | 🔶 #82 | ⬜ |
| Tastenkürzel von Aktionen und Ansichten | `keybindings` | `Actions::keybindings` | `App/` (P3.0), Abbildung nach E10 | 🔶 #82 | ⬜ |
| Erweiterungen: Liste, Freigabe, Ein/Aus, Einstellungen (Geheimnisse), neu, entfernen | `ExtensionsDialog.qml`, `ExtensionSettings.qml` | `Extensions::list`, `trust`, `set_enabled`, `set_setting`, `create`, `uninstall` | `Extensions/` (P3.10) | 🔶 #82 | ⬜ |
| Mitgelieferte Erweiterungen (bnki, Jupyter) | `bundled_extensions`, `install_bundled` | `Extensions::bundled`, `install_bundled` | `Extensions/` (P3.10) | 🔶 #82 | ⬜ |
| Ansichten der Erweiterungen (Web-Seite mit Brücke, Atlas-Farben und KaTeX) | `ExtensionView.qml` | `Extensions::views`, `view`, `call` | `Extensions/` (P3.10), `WKWebView` + `WKScriptMessageHandler` | 🔶 #82 | ⬜ |
| Verarbeiter („Anzeigen mit“, unbekannte Typen) | `ProcessedView.qml`, `process_file` | `Extensions::processors_for`, `process` | `Extensions/` (P3.10) | 🔶 #82 | ⬜ |
| Exporter (z. B. „Zu PDF übersetzen“) | `exporters_for`, `export_file` | `Extensions::exporters_for`, `export` | `Extensions/` (P3.10) | 🔶 #82 | ⬜ |

## Weitere Ansichten

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Atlas-Notizen vom Tablet (`.atlasnote`): Seiten, Vorschau | `AtlasNoteView.qml`, `atlas_note_info`/`page`/`preview` | – | `Viewers/` (P3.12) | ⚠️ L7 | ⬜ |
| HTML-Ansicht: gerendert und Quelltext, lädt beim Speichern neu, Hinweis bei Web-Inhalten | `HtmlView.qml` | `Files::read_text` | `Viewers/` (P3.12), `WKWebView` | ✅ | ⬜ |
| Bilder (Formate des Systems) | `DocumentView.qml`, `ImageWindow.qml` | – | `Viewers/` (P3.12) | 🟦 | ⬜ |
| ILIAS: Anmeldung, Favoriten als Ordner, Download mit Zielordner, neue Versionen | `IliasView.qml`, `IliasService.qml`, `ilias_backend.rs` | `Ilias::*` | `Ilias/` (P3.11), `WKWebView` + `WKHTTPCookieStore` | ✅ | ⬜ |

## Einstellungen

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Darstellung (Farbschema), Seitenleiste (Orte) | `SettingsDialog.qml` | `Settings::get`, `save` | `Settings/` (P3.9) | ✅ | ⬜ |
| Dateien (natürliche Sortierung, Touchpad-Schwung), Rechtschreibung | `SettingsDialog.qml` | `Settings`, `Spelling::languages` | `Settings/` (P3.9) | ✅ | ⬜ |
| ILIAS (Adresse, Anmeldung merken/vergessen, abmelden) | `SettingsDialog.qml` | `Ilias::config`, `save_config`, `remember_login`, `forget_login`, `logout` | `Settings/` (P3.9) | ✅ | ⬜ |
| Kontextleiste (Profile, leere Bausteine, Reiter, Lernzeit, zurücksetzen) | `SettingsDialog.qml` | `Context::options`, `set_options`, `reset` | `Settings/` (P3.9) | ✅ | ⬜ |
| Programme: externer Editor | `SettingsDialog.qml` | `Editor::external_editor`, `set_external_editor` | `Settings/` (P3.9) | ✅ | ⬜ |
| Kalender: Konto, neue Termine in … | `SettingsDialog.qml`, `CalendarSync.qml` | `Calendar::account`, `set_default_calendar` | `Settings/` (P3.9) | 🔶 #79 | ⬜ |
| Erweiterungen und Aktionen (Projekt/global) | `SettingsDialog.qml` | `Extensions`, `Actions::actions_file` | `Settings/` (P3.9) | 🔶 #82 | ⬜ |
| Version | `SettingsDialog.qml` | `Atlas::version` | `Settings/` (P3.9) | ✅ | ⬜ |

## Nur unter Linux (kein Gegenstück nötig)

| Funktion | Linux | Grund |
|---|---|---|
| Wayland-Touchpad-Erkennung für Strg+Mausrad | `TouchpadFling.qml`, Selbsttest Schritt 62 | AppKit liefert Gesten (`magnify`) direkt |
| Orte aus Dolphin (`user-places.xbel`) | `refresh_places` | auf dem Mac Finder-Seitenleiste bzw. eigene Orte (P3.1) |
| Selbsttest, Bildschirmfotos, Zeitmessung (`--self-test`, `test_driver.rs`) | `SelfTest.qml` | XCUITest (P4.1) nach [`testkatalog.md`](testkatalog.md) |
| `set_owner` (QML-Objektbesitz), `pdf_users` (poppler-Dokumente offen halten) | `app.rs` | Qt-/poppler-intern; PDFKit verwaltet seine Dokumente selbst |
| Vorschau-Pool, Momentaufnahme der Vorschau beim Verlassen | `PreviewPool.qml`, `WebRepaint.qml` | Umgehung für QtWebEngine; WKWebView braucht sie nicht |

## Lücken

Funktionen der Linux-Version ohne Dienst in `atlas-api`. Vorschlag: eine kleine Aufgabe
**P2.13 „Restliche Brücke“** (LLM2), bevor die betroffenen Phase-3-Aufgaben beginnen.

| Nr. | Funktion | Linux | Vorschlag für `atlas-api` | gebraucht von |
|---|---|---|---|---|
| L1 | Zitate im Projekt nach PDF gruppiert | `project_quotes` | `Notes::marked_passages() -> Vec<MarkedPassage>` (PDF, Seite, Text, Kommentar, Farbe, zitierende Notizen) | P3.6 |
| L2 | Bereich einer PDF als Bild zitieren | `pdf_area_to_note` | `Notes::image_target(note, pdf, page) -> ImageTarget` (Pfad in `assets/`) und `Notes::image_quote(note, image, pdf, page) -> String` (Markdown mit Bildunterschrift); das PNG rendert PDFKit | P3.2 |
| L3 | Bild aus der Zwischenablage, Dateien in eine Notiz einfügen | `paste_into_note`, `import_into_note` | `Notes::new_image_path(note) -> String`, `Notes::import_files(note, paths) -> String` (über `atlas_core::assets`); die Zwischenablage liest `NSPasteboard` | P3.3 |
| L4 | Ordnerbaum einfügen | `folder_tree` | `Editor::folder_tree(path) -> String` (`atlas_core::text_tree`) | P3.3 |
| L5 | Karten der Erweiterungen in der Kontextleiste | `context_extensions` | `Extensions::context_cards() -> Vec<ExtensionCard>` (Ansichten und Palettenbefehle, „gezeigt“ aus `Config.context`) | P3.6 |
| L6 | Formeln im Seitentext und in Zitaten als LaTeX | `selection_latex`, Glyphen aus poppler | `PdfDocument::formula_regions(page, glyphs)`: PDFKit liefert Zeichen mit Rahmen und Schrift (`PDFSelection` je Zeichen), der Kern findet Formeln (`pdf::formulas`) | P3.2 |
| L7 | `.atlasnote`-Seiten | `atlas_note_info`, `atlas_note_page`, `atlas_note_preview` | `AtlasNote` (Objekt): `info() -> Vec<PageInfo>`, `async page(index, width) -> String` (PNG-Pfad aus `atlasnote::NoteFile::rendered`), `preview(index)` | P3.12 |
