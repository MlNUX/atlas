<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Paritätstabelle Linux ↔ macOS

Alle Funktionen der Linux-Version (QML-Oberfläche und Qt-Brücke in `crates/atlas-app`)
mit ihrem Gegenstück auf dem Mac: welcher Dienst von `atlas-api` sie trägt, in welchem
Swift-Ordner sie entsteht (Aufgabe aus [`macos-swiftui.md`](macos-swiftui.md)) und wie weit
sie ist. Ziel ist volle Parität (E6).

**Stand:** 06.11.2026, abgeglichen mit dem Code im Integrationszweig `release-1.1.5` und den
offenen PRs #104 (Dateien, P3.1) und #110 (Terminal, P3.5) – P4.7. Was noch fehlt, steht
gesammelt unter [Offen auf dem Mac](#offen-auf-dem-mac).

**Stand API** (Spalte „API“):

| Zeichen | Bedeutung |
|---|---|
| ✅ | im Integrationszweig `release-1.1.5` (die Lücken L1–L7 schloss P2.13, #87) |
| 🟦 | braucht keine API: macht die Oberfläche (SwiftUI, PDFKit, SwiftTerm, WebKit, AppKit) |

**Stand Swift** (Spalte „Mac“):

| Zeichen | Bedeutung |
|---|---|
| ✅ | im Integrationszweig, mit Unit-Tests bzw. CI-Bildschirmfoto der Aufgabe |
| 🔶 #nn | im offenen PR #nn |
| 🟨 | teilweise; was fehlt, steht dahinter |
| ⬜ | fehlt noch |

Die UI-Tests (XCUITest, P4.1) folgen dem [Testkatalog](testkatalog.md); welche Szenarien es
schon gibt, steht dort.

Dienste werden kurz geschrieben: `Files::quick_open` heißt `Project::files()` →
`Files::quick_open`. Linux-Namen sind die QML-Datei oder die Funktion der Brücke.

## Fenster, Projekte, Sitzungen

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Projektordner öffnen (Strg+O, eigener Ordnerdialog) | `Main.qml`, `FolderChooser.qml`, `open_project` | `Atlas::open_project` | `App/` (P3.0), Ordnerauswahl `NSOpenPanel` | ✅ | ✅ |
| Ein Fenster je Projekt, zuletzt geöffnete Projekte | `Main.qml`, `app.rs` | `Atlas::known_projects`, `forget_project` | `App/` (P3.0) | ✅ | ✅ |
| Sitzungen: anlegen, wechseln, umbenennen, löschen, Kopie | `SessionMenu.qml`, `app/sessions.rs` | `Project::sessions`, `create_session`, `switch_session`, `rename_session`, `delete_session` | `App/` (P3.0) | ✅ | ⬜ |
| Sitzungen anderer Projekte, „In Session wechseln“ | `SessionMenu.qml`, `all_sessions` | `Atlas::known_projects`, `rename_project_session` | `App/` (P3.0) | ✅ | ⬜ |
| Tabs, Splits, Ziehen zwischen Splits, Rand → neuer Split | `EditorArea.qml`, `EditorGroup.qml` | `Project::save_session` (Layout als JSON) | `App/EditorArea` (P3.0) | ✅ | 🟨 ohne Ziehen |
| Sitzungszustand je Dokument (Seite, Zoom, Ansicht) | `document_state`, `save_document_state` | `Project::document_state`, `save_document_state` | je Feature-Ordner | ✅ | 🟨 Tab-Zustand (#102), PDF-Seite/Zoom offen |
| Vorschau-Tab, Doppelklick heftet an; Einzelklick öffnet bleibend | `EditorGroup.qml` | – | `App/EditorArea` (P3.0) | 🟦 | ⬜ |
| Leeres Fenster ohne Schließen-Knopf | `EditorGroup.qml` | – | `App/EditorArea` (P3.0) | 🟦 | ✅ |
| Zurück / vor (Alt+←/→) | `EditorGroup.qml` | – | `App/` (P3.0) | 🟦 | ⬜ |
| Seitenleiste ein/aus (Strg+B), Explorer (Strg+Umschalt+E) | `Main.qml` | – | `App/` (P3.0), `NavigationSplitView` | 🟦 | 🟨 ⌃⌘S vom System, Strg+Umschalt+E fehlt |
| Arbeitsleiste (Strg+P): Dateien, `>` Aktionen, `#` Tags, `@` Zitate/Anmerkungen, `!` Termine, `:` Zeile, `?` Volltext, `pfad:42` | `CommandPalette.qml`, `quick_open`, `fuzzy_score` | `Files::quick_open`, `Actions::actions_for`, `Notes::tags`, `Search::annotations`, `Calendar::search`, `Search::start` | `App/CommandPalette` (P3.0) | ✅ | 🟨 nur Dateien und `pfad:42` |
| Arbeitsleiste mit Aktionen (Strg+Umschalt+P) | `CommandPalette.qml` | `Actions::actions_for` | `App/CommandPalette` (P3.0) | ✅ | ✅ |
| Farbschema dunkel/hell/System für alle Fenster | `Theme.qml`, `SettingsDialog.qml` | `Settings::get`/`save` | `Settings/` (P3.9), Asset-Farben | ✅ | ✅ |
| Datei-Ereignisse: geänderte Ordner neu lesen, geänderte Dateien neu laden | `qt_helpers.rs` (Watcher) | `Files::watch`, `Event::FilesChanged`, `Event::FileModified` | `App/` (P3.0) | ✅ | ✅ |
| Index im Hintergrund, Statusanzeige | `app.rs` | `Project::index_state`, `Event::IndexStatus` | `App/` (P3.0) | ✅ | ✅ |
| Speichern beim Beenden, laufende Jobs beenden | `main.rs`, `request_save_all` | `Project::flush_session`, `Atlas::stop_all_background_jobs` | `App/` (P3.0) | ✅ | ✅ |

## Dateien, Orte, Papierkorb

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Projektbaum mit Auf-/Zuklappen (gemerkt), Projektordner-Zeile | `ProjectTree.qml`, `tree_model.rs` | `Files::list`, `Project::session_expanded` | `Files/` (P3.1) | ✅ | 🔶 #104 |
| Ordneransicht (Symbole/Details, Statusleiste, Zoom mit Strg+Mausrad) | `FolderView.qml`, `folder_model.rs` | `Files::list`, `Settings::folder_sort` | `Files/` (P3.1) | ✅ | 🔶 #104 (ohne Zoom) |
| Natürliche Sortierung, Sortierung gemerkt | `set_natural_sort`, `remember_folder_sort` | `Settings::set_folder_sort` | `Files/` (P3.1) | ✅ | 🟨 #104 (Sortierung nicht gemerkt) |
| Pfadleiste, Pfad eintippen (Strg+L) | `PathBar.qml` | `Files::exists`, `is_dir` | `Files/` (P3.1) | ✅ | 🟨 #104 (ohne Eintippen) |
| Markierungsrechteck, Mehrfachauswahl | `FolderView.qml` | – | `Files/` (P3.1) | 🟦 | 🟨 #104 (Mehrfachauswahl, kein Rechteck) |
| Neue Datei, neuer Ordner, Umbenennen (F2), Namensprüfung | `FileActions.qml`, `file_ops.rs` | `Files::new_file`, `new_folder`, `rename`, `check_name` | `Files/` (P3.1) | ✅ | 🔶 #104 |
| Kopieren/Ausschneiden/Einfügen, Konflikte (Ersetzen, Überspringen, Beide behalten), Fortschritt | `file_ops.rs` | `Files::conflicts`, `transfer`, `Event::FileOperation` | `Files/` (P3.1), `NSPasteboard` (Finder-kompatibel) | ✅ | 🟨 #104 (ohne Fortschrittsanzeige) |
| Ziehen und Ablegen (Baum, Ordner, aus Finder) | `ProjectTree.qml`, `FolderView.qml` | `Files::transfer` | `Files/` (P3.1) | ✅ | 🔶 #104 |
| In den Papierkorb (Entf), Papierkorb ansehen, wiederherstellen, löschen | `TrashView.qml`, `trash_model.rs` | `Files::move_to_trash`, `trash_items`, `restore_from_trash`, `delete_from_trash` | `Files/` (P3.1) | ✅ | 🔶 #104 |
| Offener Tab schließt sich, wenn die Datei gelöscht wird | `EditorArea.qml` | `Event::FilesChanged`, `Notes::files_trashed` | `App/` (P3.0) | ✅ | 🟨 #104 (nur Papierkorb aus Atlas) |
| Eigenschaften (Rechte, Gruppe, Größe) | `PropertiesDialog.qml` | `Files::properties`, `set_mode`, `set_group`, `own_groups`, `tree_size` | `Files/` (P3.1) | ✅ | 🟨 #104 (Rechte/Gruppe nur lesen) |
| Schnellansicht (Leertaste, Strg+Klick; ←/→, Enter) | `QuickLook.qml`, `PreviewPool.qml` | – | `Files/` (P3.1), `QLPreviewPanel` | 🟦 | 🔶 #104 |
| PDF-Vorschau in der Ordneransicht | `FolderView.qml` | – | `Files/` (P3.1), QuickLook-Miniaturen | 🟦 | ⬜ |
| Orte (wie Dolphin), Laufwerksbelegung | `PlacesList.qml`, `refresh_places`, `disk_usage` | `Atlas::places`, `Files::disk_space` | `Files/` (P3.1); auf dem Mac Finder-Seitenleiste statt Dolphin | ✅ | 🔶 #104 |
| Kontextmenü Datei: Öffnen, im Split, im Editor, Terminal hier, Pfad kopieren, Verknüpfen, Aktionen, Ausführen, Als Aufgabe einplanen, Checkliste als Aufgaben, Exporter, „Anzeigen mit“ | `FileActions.qml` | `Files::file_kind`, `Actions`, `Extensions::exporters_for`/`processors_for`, `Calendar::task_for_file`/`checklist_of` | `Files/` (P3.1) | ✅ | 🟨 #104 (ohne Verknüpfen, Aufgabe, Checkliste, Exporter) |
| Im externen Editor öffnen (auch im Terminal: vim, helix …) | `open_in_editor`, `editor_in_terminal` | `Editor::external_launch`, `set_external_editor` | `Files/` (P3.1), `NSWorkspace`/SwiftTerm | ✅ | 🟨 nur Einstellung |
| Viewer-Art einer Datei | `file_kind` | `Files::file_kind` | `App/` (P3.0) | ✅ | ✅ |
| Datei-Links `atlas:`/`file:` in Pfade auflösen | `url_to_path` | `Atlas::url_to_path` | `App/` (P3.0) | ✅ | ⬜ |

## PDF

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Anzeigen, blättern, Zoom (Strg+Mausrad an der Mausstelle), Seitenbreite | `PdfViewer.qml`, `pdf_items.rs` | – | `PDF/` (P3.2), PDFKit | 🟦 | ✅ |
| Text auswählen, kopieren, Volltextsuche (Strg+F, F3) | `PdfViewer.qml`, `PdfSelectionMenu.qml` | – | `PDF/` (P3.2) | 🟦 | ✅ |
| Seitentext für Projektsuche und Index | `extract_text` | `PdfDocument::set_page_texts` | `PDF/` (P3.2) | ✅ | ✅ |
| Anmerkungen laden (Abgleich über Pfad und Fingerabdruck, Hinweis bei geänderter oder verschobener PDF) | `annotations_model.rs` | `PdfDocument::attach`, `status`, `acknowledge_change` | `PDF/` (P3.2) | ✅ | ✅ |
| Markieren, Unterstreichen, Durchstreichen (Strg+H, zuletzt gewählte Farbe), über mehrere Seiten | `PdfToolbar.qml`, `PdfAnnotationPopup.qml` | `PdfDocument::add`, `update`, `remove` | `PDF/` (P3.2), Umrechnung `Coordinates.swift` (P0.4) | ✅ | ✅ |
| Kommentar, Farbe, Art wechseln, Link kopieren, Löschen | `PdfAnnotationPopup.qml` | `PdfDocument::update`, `link` | `PDF/` (P3.2) | ✅ | ✅ |
| Lesezeichen (Strg+D), Notiz-Symbol | `PdfToolbar.qml` | `PdfDocument::add` | `PDF/` (P3.2) | ✅ | ✅ |
| Stift und Radierer (ganze Striche), Rückgängig/Wiederholen | `PdfViewer.qml` | `PdfDocument::snapshot`, `restore` | `PDF/` (P3.2), `UndoManager` | ✅ | 🟨 Stift; Radierer und Rückgängig prüfen |
| Zitat in Notiz (Strg+Q), Zitat-Link öffnet die Stelle | `quote_markdown`, `SendToNoteDialog.qml` | `PdfDocument::quote_markdown`, `Notes::append` | `PDF/` (P3.2) | ✅ | ✅ |
| Bereich als Bild zitieren (PNG in `assets/`, Bildunterschrift mit Link) | `pdf_area_to_note` | – | `PDF/` (P3.2) | ✅ | ✅ |
| Seitenleiste: Miniaturen, Gliederung, Anmerkungen | `PdfSidebar.qml` | `PdfDocument::annotations` | `PDF/` (P3.2), `PDFThumbnailView`, `PDFOutline` | ✅ | ✅ |
| PDF-Links (intern, extern) | `pdf_items.rs` | – | `PDF/` (P3.2) | 🟦 | ✅ |
| Präsentation (F5): Vollbild, Pfeile, Klick, Zahl+Enter, B schwarz, Esc | `PdfViewer.qml` | – | `PDF/` (P3.2) | 🟦 | ✅ |
| Dunkelmodus der Seiten | `PdfViewer.qml` | – | `PDF/` (P3.2), Core-Image-Filter | 🟦 | ✅ |
| Export als Kopie mit Anmerkungen (Original bleibt) | `export_copy` | `PdfDocument::default_export_path`, `annotations` | `PDF/` (P3.2), PDFKit-Annotationen | ✅ | ✅ |
| Formel als LaTeX: Rahmen aufziehen | `PdfLatexPopup.qml`, `formula_ocr.rs` | `LatexOcr::recognize`, `status`, `warm_up` | `PDF/` (P3.2), Bild aus `PDFPage.thumbnail` | ✅ | ✅ |
| Formeln im Seitentext und in Zitaten als LaTeX | `pdf_items.rs` (`selection_latex`, Glyphen) | – | `PDF/` (P3.2) | ✅ | ⬜ |
| Pfeiltasten ohne Klick, Fokus beim Tab-Wechsel | `PdfViewer.qml` | – | `PDF/` (P3.2) | 🟦 | 🟨 ungeprüft |
| Anmerkungen anderer Geräte (Sync) erscheinen | `reload` | `PdfDocument::reload`, `Event::AnnotationsChanged` | `PDF/` (P3.2) | ✅ | ✅ |

## Markdown

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Editor, Hervorhebung (auch Codeblöcke nach Sprache), Zeilennummern | `TextEditor.qml`, `text_document.rs` | `Editor::highlighter`, `language_for_fence` | `Markdown/` (P3.3), `NSTextView` | ✅ | ✅ |
| Automatisches Speichern nach 1 s, Strg+S, Wiederherstellung | `text_document.rs` | `Files::write_text`, `read_text` | `Markdown/` (P3.3) | ✅ | ✅ (ohne Wiederherstellung) |
| Listen: Enter setzt fort, Tab rückt ein, leere Zeile beendet | `markdown_edit.rs` (P1.4) | `Editor::continue_list`, `indent_list_items` | `Markdown/` (P3.3) | ✅ | ✅ |
| Tabellen: Tab/Enter, ausrichten, formatieren | `markdown_edit.rs` | `Editor::table_tab`, `table_enter`, `align_table_at`, `format_table` | `Markdown/` (P3.3) | ✅ | ✅ |
| Checkboxen umschalten (Editor und Vorschau) | `markdown_edit.rs`, `MarkdownView.qml` | `Editor::toggle_task` | `Markdown/` (P3.3) | ✅ | ✅ |
| Wiki-Links `[[` mit Vorschlägen, Alias, Überschrift; `#Tags` | `TextEditor.qml` | `Editor::completion_at`, `completions`, `accept_completion` | `Markdown/` (P3.3) | ✅ | ✅ |
| Vorschau: KaTeX, Mermaid (mit Vorlagen), Tabellen, Bilder, Ordnerbäume, Checklisten, Tags, Zitate | `MarkdownView.qml`, `web/preview.html` | `Editor::render_markdown` | `Markdown/` (P3.3), `WKWebView` + `preview.html` | ✅ | ✅ |
| Vorschau: Kopieren-Knopf, „Ausführen“ bei bash, Abschnitte einklappen, defekte Links markiert | `web/preview.html` | `Editor::render_markdown`, `code_block` | `Markdown/` (P3.3) | ✅ | ✅ |
| Geteilt (Editor + Vorschau) mit Scroll-Sync | `MarkdownView.qml` | – | `Markdown/` (P3.3) | 🟦 | ✅ |
| Bild vergrößern im eigenen Fenster | `ImageWindow.qml` | – | `Viewers/` (P3.12) | 🟦 | ✅ |
| Einfügen: Bild aus der Zwischenablage, Dateien (in `assets/`) | `paste_into_note`, `import_into_note` | – | `Markdown/` (P3.3) | ✅ | ✅ |
| Einfügen: Ordnerbaum, Tabelle, Diagramm-Vorlage | `folder_tree`, `MarkdownView.qml` | `Editor::format_table` | `Markdown/` (P3.3) | ✅ | ✅ |
| Rechtschreibprüfung Deutsch/Englisch, eigenes Wörterbuch | `spelling.rs` | `Spelling::available`, `check`, `suggest`, `add` (sonst `NSSpellChecker`) | `Markdown/` (P3.3) | ✅ | ✅ |
| Markdown als PDF exportieren | `MarkdownPdfExport.qml` | `Editor::render_markdown` | `Markdown/` (P3.3), `WKWebView.createPDF` | ✅ | ✅ |
| Gliederung (Überschriften), Sprung zur Überschrift | `ContextPanel.qml` | `Editor::render_markdown`, `Notes::heading_line` | `Markdown/` (P3.3), `Context/` (P3.6) | ✅ | ✅ |
| Notiz-Fristen aus dem Front Matter | `note_deadlines` | `Notes::deadlines` | `Context/` (P3.6), `Calendar/` (P3.7) | ✅ | ✅ |

## Notizen, Links, Graph

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Rückverweise mit Art, Kontext und Filtern | `backlinks`, `ContextPanel.qml` | `Notes::backlinks` | `Context/` (P3.6) | ✅ | ✅ |
| Tags (hierarchisch), Notizen mit Tag | `note_tags`, `notes_with_tag`, `tags_of_note` | `Notes::tags`, `notes_with_tag`, `tags_of` | `Context/` (P3.6) | ✅ | ✅ |
| Zitate in einer Notiz | `ContextPanel.qml` | `Notes::quotes` | `Context/` (P3.6) | ✅ | ⬜ |
| Zitate im Projekt, nach PDF gruppiert | `project_quotes` | – | `Context/` (P3.6) | ✅ | ⬜ |
| Graph der Umgebung und des ganzen Projekts, eigenes Fenster | `GraphDialog.qml`, `context_graph` | `Notes::graph` | `Context/` (P3.6), Canvas | ✅ | 🟨 Baustein, ohne eigenes Fenster |
| Dateien von Hand verknüpfen, Ordner-Knoten | `RelateDialog.qml`, `relate_files`, `set_folder_node` | `Notes::related`, `relate`, `unrelate`, `set_folder_node` | `Context/` (P3.6) | ✅ | ⬜ |
| Links, Zitate, Verknüpfungen und Anmerkungen folgen Umbenennungen | `files_moved` | `Notes::files_moved`, `Calendar::files_moved` | `Files/` (P3.1) | ✅ | 🔶 #104 |
| Links reparieren nach Verschieben außerhalb von Atlas | `find_link_repairs`, `apply_link_repairs` | `Notes::find_link_repairs`, `Event::LinkRepairsFound` | `App/` (P3.0) | ✅ | ⬜ |
| Text an Notiz anhängen (Terminal-Ausgabe, Zitat) | `append_to_note`, `SendToNoteDialog.qml` | `Notes::append` | `App/` (P3.0) | ✅ | 🟨 Zitat ✅, Terminal 🔶 #110 |

## Text und Code

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Editor mit Hervorhebung für viele Sprachen | `TextEditor.qml` | `Editor::language`, `highlighter` | `Code/` (P3.4) | ✅ | ✅ |
| Symbole (Funktionen, Typen) | `symbols` | `Editor::symbols` | `Code/` (P3.4) | ✅ | ✅ |
| Suchen/Ersetzen im Dokument (Strg+F), Gehe zu Zeile (Strg+G) | `TextEditor.qml` | – | `Code/` (P3.4) | 🟦 | ✅ |
| Skripte ausführen („Ausführen“ nur bei Skripten) | `is_runnable`, `prepare_script` | `Actions::is_runnable`, `prepare_script` | `Code/` (P3.4), `Terminal/` (P3.5) | ✅ | 🔶 #110 (ohne Terminal im Hintergrund) |
| Touchpad-Schwung, Mausrad ohne Schwung | `TouchpadFling.qml` | – | macOS macht es selbst | 🟦 | ✅ |

## Suche

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Projektweite Suche (Strg+Umschalt+F) in Notizen, Code und PDFs | `SearchDialog.qml`, `search_model.rs` | `Search::start`, `cancel`, `Event::SearchMatches`/`SearchFinished` | `Search/` (P3.8) | ✅ | ✅ |
| Groß-/Kleinschreibung, ganze Wörter, Typfilter, ohne Build-Ordner | `SearchDialog.qml` | `SearchQuery` | `Search/` (P3.8) | ✅ | ✅ |
| Vorschau mit Kontext (auch PDF-Seite) | `context` | `Search::context` | `Search/` (P3.8) | ✅ | ✅ |
| Treffer öffnet und markiert im Dokument (Editor, Vorschau, PDF) | `SearchDialog.qml` | – | `Search/` (P3.8), je Viewer | 🟦 | ✅ |
| Suche in Anmerkungen (`@`) | `annotation_search` | `Search::annotations` | `App/CommandPalette` (P3.0) | ✅ | ⬜ |

## Terminal

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Terminal-Panel (Strg+\`), Terminal-Tab, neues Terminal | `TerminalPanel.qml`, `TerminalPane.qml`, `terminal_item.rs` | `Terminal::shell` | `Terminal/` (P3.5), SwiftTerm | ✅ | 🔶 #110 |
| Panel nebeneinander/untereinander teilen, als Baum in der Sitzung | `TerminalGroup.qml` | `Project::save_session` | `Terminal/` (P3.5) | ✅ | 🔶 #110 |
| Startordner, aktueller Ordner des Prozesses | `terminal_item.rs` | `Terminal::process_cwd` | `Terminal/` (P3.5) | ✅ | 🔶 #110 |
| Farben passend zum Schema | `set_terminal_light` | `Terminal::palette` | `Terminal/` (P3.5) | ✅ | 🔶 #110 |
| Kopieren/Einfügen, Ausgabe in Notiz übernehmen | `TerminalPane.qml` | `Notes::append` | `Terminal/` (P3.5) | ✅ | 🔶 #110 |
| Terminal läuft weiter, wenn der Tab gezogen wird | `EditorArea.qml` | – | `Terminal/` (P3.5) | 🟦 | 🔶 #110 |

## Kontextleiste

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Bausteine: Anmerkungen, Gliederung, Links, Tags, Dateiinfo, Graph, Aktionen, Heute, Woche, Aufgaben, Pomodoro | `ContextPanel.qml`, `ContextBlock.qml`, `ContextModules.qml` | `Context::catalog`, `layout` | `Context/` (P3.6) | ✅ | ✅ |
| Reiterseiten, schmal als Symbole, breit ausgeschrieben | `ContextPanel.qml` | `Context::options`, `new_page`, `rename_page`, `remove_tab` | `Context/` (P3.6) | ✅ | ✅ |
| Bearbeitungsmodus: einklappen, verschieben (Alt+↑), ausblenden, anheften, Galerie | `ContextEditor.qml` | `Context::move_block`, `move_by`, `set_hidden`, `set_collapsed`, `unplace` | `Context/` (P3.6) | ✅ | ✅ |
| Profile nach Dateityp, „Gilt für“, Rückgängig, zurücksetzen | `ContextProfiles.qml` (P1.3) | `Context::profiles`, `profile_for`, `snapshot`, `restore`, `reset` | `Context/` (P3.6) | ✅ | ✅ |
| Karten der Erweiterungen (Ansichten, Befehle) | `context_extensions`, `ExtensionCards.qml` | – | `Context/` (P3.6) | ✅ | ✅ |
| Pomodoro (Lernzeit, Pause) | `PomodoroTimer.qml` | `Context::options` | `Context/` (P3.6) | ✅ | ✅ |
| Inspector (Datei-Aktionen der Leiste) | `Inspector.qml` | `Actions`, `Extensions` | `Context/` (P3.6) | ✅ | ✅ |

## Kalender

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Kalender als Tab, Monat, Woche, Agenda; Heute (T), blättern | `CalendarView.qml`, `CalendarMonth.qml`, `CalendarWeek.qml` | `Calendar::occurrences`, `view_state` | `Calendar/` (P3.7) | ✅ | ✅ |
| Einträge: Termin, Arbeitsblock, Frist, Aufgabe; Dialog mit Wiederholung, Erinnerungen, Checkliste, Tags | `EntryDialog.qml`, `calendar_model/edit.rs` | `Calendar::save`, `entry`, `remove`, `Recurrence` | `Calendar/` (P3.7) | ✅ | ✅ |
| Ziehen: verschieben (15-min-Raster), Dauer ändern, neue Zeit aufziehen | `CalendarWeek.qml` | `Calendar::move_occurrence`, `save` | `Calendar/` (P3.7) | ✅ | ✅ |
| Aufgaben ohne Zeit, einplanen und zurück | `CalendarView.qml` | `Calendar::tasks`, `schedule` | `Calendar/` (P3.7) | ✅ | ✅ |
| Dateien verknüpfen, Datei auf freie Zeit → Arbeitsblock, Checkliste als Aufgaben | `link_file`, `block_for_file`, `add_tasks` | `Calendar::link_file`, `block_for_file`, `task_for_file`, `checklist_of`, `add_tasks` | `Calendar/` (P3.7) | ✅ | ⬜ |
| Erinnerungen | `due_reminders` | `Calendar::due_reminders` | `Calendar/` (P3.7), `UNUserNotificationCenter` | ✅ | ✅ |
| Kalenderauswahl, nur in diesem Projekt, Standardkalender | `CalendarSync.qml` | `Calendar::calendars`, `set_calendar_visible`, `set_calendar_project_only`, `set_default_calendar` | `Calendar/` (P3.7) | ✅ | ✅ |
| Nextcloud/CalDAV: Verbindung testen, Konto, Kalender anlegen | `CalendarSync.qml`, `calendar_model/account.rs` | `Calendar::test_connection`, `save_account`, `create_remote_calendar` | `Calendar/` (P3.7) | ✅ | ✅ |
| Google-Anmeldung im Browser | `google_sign_in` | `Calendar::google_sign_in`, `google_sign_in_finished` | `Calendar/` (P3.7), `NSWorkspace.open` | ✅ | ✅ |
| Passwort: eingeben, merken, Passwort-Befehl | `CalendarSync.qml` | `Calendar::set_password`, `forget_password`, `retry_password_command` | `Calendar/` (P3.7), Schlüsselbund | ✅ | ✅ |
| Synchronisieren, Status, Konflikte lösen, Cache leeren, trennen | `calendar_model/remote.rs` | `Calendar::sync_now`, `sync_state`, `conflicts`, `resolve_conflict`, `clear_cache`, `disconnect` | `Calendar/` (P3.7) | ✅ | ✅ |
| macOS-Kalender (EventKit) neben den Atlas-Einträgen | – (neu) | – | `Calendar/EventKit/` (P3.13) | 🟦 | ✅ |

## Aktionen und Erweiterungen

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Eigene Aktionen (Projekt, Benutzer), Fehler in den Dateien, Datei öffnen | `app/actions.rs` | `Actions::actions_for`, `errors`, `actions_file` | `Extensions/` (P3.10) | ✅ | ✅ |
| Aktion ausführen im Panel, Tab oder Hintergrund; Rückfrage bei Gefährlichem | `prepare_action` | `Actions::prepare`, `start_background` | `Extensions/` (P3.10), `Terminal/` (P3.5) | ✅ | ✅ |
| Hintergrundjobs: Status, Ausgabe, Stopp | `background_status`, `stop_background` | `Atlas::background_jobs`, `background_job`, `stop_background_job` | `Extensions/` (P3.10) | ✅ | ✅ |
| Tastenkürzel von Aktionen und Ansichten | `keybindings` | `Actions::keybindings` | `App/` (P3.0), Abbildung nach E10 | ✅ | ⬜ |
| Erweiterungen: Liste, Freigabe, Ein/Aus, Einstellungen (Geheimnisse), neu, entfernen | `ExtensionsDialog.qml`, `ExtensionSettings.qml` | `Extensions::list`, `trust`, `set_enabled`, `set_setting`, `create`, `uninstall` | `Extensions/` (P3.10) | ✅ | ✅ |
| Mitgelieferte Erweiterungen (bnki, Jupyter) | `bundled_extensions`, `install_bundled` | `Extensions::bundled`, `install_bundled` | `Extensions/` (P3.10) | ✅ | ✅ |
| Ansichten der Erweiterungen (Web-Seite mit Brücke, Atlas-Farben und KaTeX) | `ExtensionView.qml` | `Extensions::views`, `view`, `call` | `Extensions/` (P3.10), `WKWebView` + `WKScriptMessageHandler` | ✅ | ✅ |
| Verarbeiter („Anzeigen mit“, unbekannte Typen) | `ProcessedView.qml`, `process_file` | `Extensions::processors_for`, `process` | `Extensions/` (P3.10) | ✅ | ✅ |
| Exporter (z. B. „Zu PDF übersetzen“) | `exporters_for`, `export_file` | `Extensions::exporters_for`, `export` | `Extensions/` (P3.10) | ✅ | ✅ |

## Weitere Ansichten

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Atlas-Notizen vom Tablet (`.atlasnote`): Seiten, Vorschau | `AtlasNoteView.qml`, `atlas_note_info`/`page`/`preview` | – | `Viewers/` (P3.12) | ✅ | ✅ |
| HTML-Ansicht: gerendert und Quelltext, lädt beim Speichern neu, Hinweis bei Web-Inhalten | `HtmlView.qml` | `Files::read_text` | `Viewers/` (P3.12), `WKWebView` | ✅ | ✅ |
| Bilder (Formate des Systems) | `DocumentView.qml`, `ImageWindow.qml` | – | `Viewers/` (P3.12) | 🟦 | ✅ |
| ILIAS: Anmeldung, Favoriten als Ordner, Download mit Zielordner, neue Versionen | `IliasView.qml`, `IliasService.qml`, `ilias_backend.rs` | `Ilias::*` | `Ilias/` (P3.11), `WKWebView` + `WKHTTPCookieStore` | ✅ | ✅ |

## Einstellungen

| Funktion | Linux | API | Swift (Aufgabe) | API | Mac |
|---|---|---|---|---|---|
| Darstellung (Farbschema), Seitenleiste (Orte) | `SettingsDialog.qml` | `Settings::get`, `save` | `Settings/` (P3.9) | ✅ | ✅ |
| Dateien (natürliche Sortierung, Touchpad-Schwung), Rechtschreibung | `SettingsDialog.qml` | `Settings`, `Spelling::languages` | `Settings/` (P3.9) | ✅ | 🟨 ohne Sprachwahl der Rechtschreibung |
| ILIAS (Adresse, Anmeldung merken/vergessen, abmelden) | `SettingsDialog.qml` | `Ilias::config`, `save_config`, `remember_login`, `forget_login`, `logout` | `Settings/` (P3.9) | ✅ | ✅ |
| Kontextleiste (Profile, leere Bausteine, Reiter, Lernzeit, zurücksetzen) | `SettingsDialog.qml` | `Context::options`, `set_options`, `reset` | `Settings/` (P3.9) | ✅ | ✅ |
| Programme: externer Editor | `SettingsDialog.qml` | `Editor::external_editor`, `set_external_editor` | `Settings/` (P3.9) | ✅ | ✅ |
| Kalender: Konto, neue Termine in … | `SettingsDialog.qml`, `CalendarSync.qml` | `Calendar::account`, `set_default_calendar` | `Settings/` (P3.9) | ✅ | ✅ |
| Erweiterungen und Aktionen (Projekt/global) | `SettingsDialog.qml` | `Extensions`, `Actions::actions_file` | `Settings/` (P3.9) | ✅ | ✅ |
| Version | `SettingsDialog.qml` | `Atlas::version` | `Settings/` (P3.9) | ✅ | ✅ |

## Nur unter Linux (kein Gegenstück nötig)

| Funktion | Linux | Grund |
|---|---|---|
| Wayland-Touchpad-Erkennung für Strg+Mausrad | `TouchpadFling.qml`, Selbsttest Schritt 62 | AppKit liefert Gesten (`magnify`) direkt |
| Orte aus Dolphin (`user-places.xbel`) | `refresh_places` | auf dem Mac Finder-Seitenleiste bzw. eigene Orte (P3.1) |
| Selbsttest, Bildschirmfotos, Zeitmessung (`--self-test`, `test_driver.rs`) | `SelfTest.qml` | XCUITest (P4.1) nach [`testkatalog.md`](testkatalog.md) |
| `set_owner` (QML-Objektbesitz), `pdf_users` (poppler-Dokumente offen halten) | `app.rs` | Qt-/poppler-intern; PDFKit verwaltet seine Dokumente selbst |
| Vorschau-Pool, Momentaufnahme der Vorschau beim Verlassen | `PreviewPool.qml`, `WebRepaint.qml` | Umgehung für QtWebEngine; WKWebView braucht sie nicht |

## Lücken der API (erledigt)

Die Lücken L1–L7 aus P2.12 (Zitate im Projekt, Bereich als Bild, Bilder und Dateien in
Notizen, Ordnerbaum, Karten der Erweiterungen, Formeln als LaTeX, `.atlasnote`-Seiten) hat
P2.13 geschlossen (#87): `Notes::marked_passages`, `image_target`, `image_quote`,
`new_image_path`, `import_files`, `Editor::folder_tree`, `Extensions::context_cards`,
`PdfDocument::formula_clips`/`text_with_latex`, `AtlasNote`. Neue Lücken sind seitdem
nicht aufgetaucht.

## Offen auf dem Mac

Alles mit ⬜ oder 🟨 oben, nach Bereich – Vorlage für Folgeaufgaben (die API dafür ist
überall schon da):

| Bereich | Was fehlt | API |
|---|---|---|
| App (P3.0) | Sitzungen: Menü zum Anlegen, Wechseln, Umbenennen, Löschen; Sitzungen anderer Projekte | `Project::sessions` …, `Atlas::known_projects`, `rename_project_session` |
| App (P3.0) | Tabs zwischen Splits ziehen (auch an den Rand → neuer Split); Vorschau-Tab; Zurück/Vor (⌥⌘←/→) | – |
| App (P3.0) | Dokumentzustand außerhalb des Tabs (PDF-Seite und Zoom über Sitzungen hinweg) | `Project::document_state`, `save_document_state` |
| App (P3.0) | Arbeitsleiste mit Präfixen `#` Tags, `@` Zitate/Anmerkungen, `!` Termine, `?` Volltext | `Notes::tags`, `Search::annotations`, `Calendar::search`, `Search::start` |
| App (P3.0) | Links reparieren nach Verschieben außerhalb von Atlas; `atlas:`-/`file:`-Links auflösen | `Notes::find_link_repairs`, `apply_link_repairs`, `Atlas::url_to_path` |
| App (P3.0) | Tastenkürzel von Aktionen und Ansichten der Erweiterungen | `Actions::keybindings` |
| Dateien (P3.1) | Pfad eintippen, Zoom und Markierungsrechteck in der Ordneransicht, PDF-Miniaturen, Sortierung merken, Fortschritt beim Kopieren, Rechte/Gruppe ändern, im externen Editor öffnen, fehlende Einträge im Kontextmenü | `Settings::set_folder_sort`, `Files::set_mode`, `set_group`, `Editor::external_launch`, `Calendar::task_for_file`, `checklist_of`, `Extensions::exporters_for` |
| PDF (P3.2) | Formeln im Seitentext und in Zitaten als LaTeX; Radierer und Rückgängig prüfen | `PdfDocument::formula_clips`, `text_with_latex`, `snapshot`, `restore` |
| Notizen (P3.6) | Zitate einer Notiz und im Projekt, Dateien verknüpfen und Ordner-Knoten, Graph im eigenen Fenster | `Notes::quotes`, `marked_passages`, `relate`, `unrelate`, `set_folder_node`, `graph` |
| Suche (P3.8) | Suche in Anmerkungen (`@` der Arbeitsleiste) | `Search::annotations` |
| Kalender (P3.7) | Dateien verknüpfen, Datei auf freie Zeit, Checkliste als Aufgaben | `Calendar::link_file`, `block_for_file`, `add_tasks` |
| Einstellungen (P3.9) | Sprachen der Rechtschreibung | `Spelling::languages` |
