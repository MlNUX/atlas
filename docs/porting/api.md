<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# `atlas-api` – Vertrag zwischen Kern und Oberfläche

Stand 04.11.2026 · Aufgabe P2.1 · Code: `crates/atlas-api/src/*.rs` (Signaturen, Rümpfe
`todo_task!("P2.x")`) · gilt für die P2-Aufgaben und alle Swift-Aufgaben (Regel 3 in
`AGENTS.md`: Änderungen nur durch den Integrator).

## Grundsätze

1. **Dienste als Objekte.** Einstieg ist `Atlas` (einmal je Prozess); `Atlas::open_project`
   liefert ein `Project` je Fenster (E9). Das Projekt gibt seine Dienste heraus (`files()`,
   `notes()`, `search()`, `calendar()`, `pdf(path)`, `actions()`, `extensions()`,
   `editor()`, `context()`). App-weit gibt es `settings()`, `ilias()` und die
   Hintergrundjobs. `LatexOcr` und `Terminal` haben eigene Konstruktoren.
2. **Einfache Typen statt JSON.** Records und Enums. Pfade sind absolute UTF-8-`String`s,
   Zeiten Unix-Sekunden (`i64`). Kalenderzeiten sind ISO-8601-Text in der Zone von
   `Calendar::time_zone`, weil Ganztägiges und Ortszeit sonst verloren gehen. JSON bleibt
   nur, wo das Format nicht Atlas gehört: Sitzungs-Layout und Dokumentzustand (Dateiformat,
   E12), Kalender-Ansicht, Anfragen von Erweiterungs-Ansichten.
   **Positionen in Texten sind UTF-16-Einheiten** (wie `NSString` und QML-Strings): Editor
   (Spannen, Änderungen, Vervollständigung), Rechtschreibung, Suchtreffer. Die Kernmodule
   aus P1.4/P1.5 rechnen schon so; Swift muss nichts umrechnen.
3. **Ein Fehlertyp** `AtlasError` mit den Fällen `NotFound`, `Invalid`, `Conflict`,
   `Unavailable`, `Auth` und `Failed`, jeweils mit deutscher Meldung für die Oberfläche.
   Swift sieht ihn als `throws`.
4. **Schnell oder `async`.** Methoden ohne `async` kehren in wenigen Millisekunden zurück
   und dürfen auf dem Main-Thread laufen. Lange einmalige Arbeit ist `async`: Sie läuft
   auf einem eigenen Thread und meldet sich über einen Oneshot-Kanal, ohne Tokio (Spike
   P0.2). Laufende Ströme liefern Ereignisse mit einer Id: Suche, Datei-Operationen, Sync.
5. **Ereignisse über einen Callback.** `EventSink::on_event(Event)` gibt es einmal für
   `Atlas` (app-weit) und einmal je `Project` (Fenster). Der Kern ruft ihn aus
   Hintergrund-Threads auf; Swift reicht die Ereignisse über die Main-Queue weiter (FIFO,
   `MainActorEventSink`). Ereignisse sagen nur, *was* sich geändert hat; Daten holt die
   Oberfläche danach selbst (Revisionen statt Kopien).
6. **Keine Logik doppeln.** Jede Methode ruft `atlas-core` (bzw. `atlas-spell`,
   `atlas-ilias`) auf. Fehlt dort etwas, kommt es in den Kern (wie P1.3–P1.5), nicht in
   die Fassade. Was heute in QML/JS steckt, wandert vorher in den Kern.
7. **UniFFI nur hinter dem Feature `ffi`.** Die Ableitungen stehen als
   `#[cfg_attr(feature = "ffi", …)]` an den Typen, damit die Typen nicht doppelt
   beschrieben werden müssen. `atlas-ffi` (P2.11) schaltet das Feature ein und baut
   `staticlib` und XCFramework; das Swift-Modul heißt `atlas`. Ohne Feature hängt
   `atlas-api` nicht an UniFFI und kann auch der Qt-Brücke dienen.
8. **Thread-sicher.** Alle Objekte sind `Send + Sync`, jede Methode ist von jedem Thread
   aus aufrufbar.

Geprüft: `cargo check -p atlas-api` und `--features ffi` ohne Warnungen. Probeweise
erzeugte Swift-Bindings (cdylib + `uniffi-bindgen generate --library`) entstehen ohne
Fehler: 15 657 Zeilen, rund 440 öffentliche Typen und Methoden.

## Bereiche

| Datei | Objekt(e) | Aufgabe | Vorlage in der Qt-Brücke |
|---|---|---|---|
| `app.rs` | `Atlas`, `Place`, `KnownProject`, `BackgroundJob` | P2.2 (Orte, Projekte), P2.7 (Jobs), P2.8 (Einstellungen), P2.10 (ILIAS) | `app.rs` (Orte, `open_project`, Sitzungen anderer Projekte), `app/actions.rs` (Jobs) |
| `project.rs` | `Project`, `SessionSummary`, `IndexState` | P2.2 | `app/sessions.rs`, `app.rs` (Projekt, Dokumentzustand) |
| `files.rs` | `Files`, `FileEntry`, `TrashItem`, `FileProperties`, `QuickOpenHit` … | P2.2 | `file_ops.rs`, `tree_model.rs`, `folder_model.rs`, `trash_model.rs`, `quickOpen` |
| `notes.rs` | `Notes`, `Backlink`, `Tag`, `Deadline`, `Quote`, `Graph`, `LinkRepair` | P2.3 | `app/notes.rs`, `app/relations.rs` |
| `search.rs` | `Search`, `SearchQuery`, `SearchMatch`, `SearchSummary`, `AnnotationHit` | P2.4 | `search_model.rs`, `annotation_search` |
| `calendar.rs` | `Calendar`, `Entry`, `Occurrence`, `Account`, `SyncState`, `Conflict` … | P2.5 | `calendar_model.rs`, `calendar_model/{account,edit,remote}.rs` |
| `pdf.rs` | `PdfDocument`, `Annotation`, `AnnotationPatch`, `PdfStatus`, `Snapshot` | P2.6 | `annotations_model.rs`, `pdf_items.rs` (nur Daten) |
| `ocr.rs` | `LatexOcr`, `GrayImage`, `OcrStatus` | P2.6 | `formula_ocr.rs` |
| `actions.rs` | `Actions`, `ActionInfo`, `Prepared`, `KeyBinding` | P2.7 | `app/actions.rs` |
| `extensions.rs` | `Extensions`, `ExtensionInfo`, `ViewPage` … | P2.7 | `app/extensions.rs` |
| `settings.rs` | `Settings`, `AppSettings`, `Context` (Kontextleiste mit `ContextTarget` „nur dieses Profil“/„alle“, Rückgängig über `ContextSnapshot`), `Spelling` | P2.8 | `app/settings.rs`, `spelling.rs`, `ContextProfiles.qml` → P1.3 |
| `editor.rs` | `Editor`, `Highlighter` (inkrementell), `Span`/`HighlightKind`, `Edit` (`anchor`/`cursor`), `CompletionContext`/`Completion` (Wiki-Links und Tags), `Symbol` | P2.8 | `app/editing.rs`, `TextEditor.qml` → P1.4, Hervorhebung → P1.5 |
| `terminal.rs` | `Terminal` (Shell starten, Arbeitsverzeichnis) | P2.2 | `terminal_item.rs`; das Raster entfällt (SwiftTerm, P0.5) |
| `ilias.rs` | `Ilias`, `IliasConfig`, `Cookie`, `IliasItem` | P2.10 | `ilias_backend.rs`, `IliasService.qml` |
| `events.rs` | `EventSink`, `Event`, `Level` | P2.1 (Integrator) | Qt-Signale aller Brücken |
| `error.rs` | `AtlasError` | P2.1 (Integrator) | – |

## Was bewusst *nicht* in der API steht

- **PDF anzeigen, rendern, auswählen, exportieren:** Das macht PDFKit (P0.4). Der Kern
  bekommt nur Seitenzahl und PDF-Id (`PdfDocument::attach`) und den Seitentext
  (`set_page_texts`). Die Anmerkungen bleiben im Atlas-Format.
- **Terminal-Raster:** SwiftTerm (P0.5).
- **Item-Modelle, Rollen, Zeilen-Diffs der Qt-Modelle:** SwiftUI vergleicht selbst.
  `Files::list` liefert ganze Listen, `Event::FilesChanged` sagt, welche Ordner neu zu
  lesen sind.
- **Selbsttest, Bildschirmfotos, Zeitmessung** (`test_driver.rs`, `--self-test`): Das
  übernehmen auf dem Mac XCUITest und P4.1.
- **macOS-Kalender (EventKit):** reines Swift (P3.13).

## Regeln für P2.2–P2.10

- Nur die eigene Datei füllen. Fehlt eine Methode, einen Typ oder ein Ereignis: in den PR
  schreiben, der Integrator ergänzt den Vertrag.
- Jede Methode bekommt mindestens einen Rust-Test in der eigenen Datei, mit einem
  Projekt in `tempfile` und ohne Netz.
- Interne Zustände (offene Indizes, Watcher) gehören in die Objekte. Gemeinsames für
  mehrere Dienste kommt in ein `pub(crate)`-Modul, das der Integrator anlegt.
- `Project` und `Atlas` halten bisher nur Platzhalterfelder. P2.2 legt das gemeinsame
  Projekt-Innenleben an (Index, Writer, Watcher, Sink), die anderen Bereiche bekommen
  es über `Project`.

## Eingearbeitet nach dem Gegenlesen (LLM2, #68)

1. UTF-16 statt UTF-8-Bytes für alle Textpositionen (Grundsatz 2).
2. `Editor` an P1.4 angeglichen: `continue_list`, `indent_list_items`, `table_tab`,
   `table_enter`, `align_table_at` (Cursor bleibt in der Zelle), `align_tables`,
   `format_table` und `toggle_task`. `EditorKey::Pipe` ist gestrichen, `Edit` hat
   `anchor`/`cursor`.
3. Vervollständigung: `completion_at` → `completions` → `accept_completion`, für Wiki-Links
   *und* Tags. Das `]]`-Schließen und die Cursorposition übernimmt der Kern.
4. `Context` vollständig gegenüber `ContextProfiles.qml`: `ContextTarget::All` („Gilt für
   alle Dateitypen“), `set_hidden`, `unplace`, `rename_page`, `remove_tab`, `is_set`,
   `snapshot`/`restore`. Die Erweiterungs-Karten holt `Context` selbst vom Projekt;
   `move_by` richtet sich nach der sichtbaren Reihenfolge.
5. Inkrementelle Hervorhebung über `Editor::highlighter(language)`; `Span::kind` ist ein
   Enum. Sprachen sind syntect-Namen und für die Oberfläche opak.
6. `pdf.rs` verweist für die Umrechnung auf `PageGeometry` (P0.4). `docs/data-format.md`
   ist korrigiert (#65).
7. `Point` ohne Druck, weil das Dateiformat nur `[x, y]` speichert.
8. Google-Anmeldung: `google_sign_in` (URL) und `async google_sign_in_finished`,
   abbrechbar.
9. Terminal-Palette (`Terminal::palette`) und `async Files::read_text` sind im
   P2.2-Branch, UTF-16-Bereiche der Suchtreffer im P2.4-Branch, um Konflikte zu vermeiden.
## Änderungen am Vertrag

| Aufgabe | Änderung | Grund |
|---|---|---|
| P2.2 | `SessionSummary` hat `tabs`, `splits`, `kind`, `saved` statt `open_files`/`last_used` | so liefert es der Kern (`ProjectSession::summary`) |
| P2.2 | `Project::save_session(layout, expanded, documents)` und `session_expanded()` statt `save_session_layout`/`expanded_dirs`/`set_expanded_dirs` | Layout, Ordner und Dokumente werden zusammen gespeichert (wie Linux) |
| P2.2 | `create_session(name, layout, expanded)`: Kopie über das übergebene Layout | die Oberfläche kennt den aktuellen Stand, der Kern nur den gespeicherten |
| P2.2 | neu: `Project::resolve_document`, `Atlas::rename_project_session`, `Files::disk_space` | Funktionen der Qt-Brücke, die gefehlt haben |
| P2.2 | `KnownProject` mit der aktiven Sitzung statt einer Liste | wie das Sitzungsmenü unter Linux |
| P2.2 | `FileEntry::kind` ist der lesbare Typ; die Viewer-Art kommt aus `Files::file_kind` | `file_kind` steckte in der Qt-Brücke und liegt jetzt im Kern (`listing::viewer_kind`) |
| P2.4 | `SearchQuery`: `case_sensitive: bool`, `build_dirs`, kein `text`/`pdf` mehr (über `kinds`) | wie das Suchfenster unter Linux; Optionen → Query jetzt im Kern (`search::Options`) |
| P2.4 | `Search::context` liefert `Vec<ContextLine>` | Zeilennummern je Zeile (PDF: Zeile auf der Seite) |
| P2.4 | `AnnotationHit` mit `relative`, `kind`, `title` | wie die Anmerkungssuche unter Linux; Logik jetzt im Kern (`annotations::search`) |
| P2.8 | `Editor::render_markdown` liefert `RenderedMarkdown` (HTML, Titel, Tags, Überschriften, kaputte Links) | wie die Vorschau unter Linux (Gliederung, Hinweis auf kaputte Links) |
| P2.8 | `Editor::external_launch` → `ExternalLaunch` (System/losgelöst/Terminal) statt den Editor selbst zu starten | Prozesse startet die Oberfläche (NSWorkspace, `Process`, SwiftTerm) |
| P2.8 | `AppSettings` ohne `spell_languages`/`other`/Editor-Befehl; Ordner-Sortierung ist eine (global), `ContextOptions` neu | so speichert es `config.toml`; Editor-Befehl über `Editor::set_external_editor` |
| P2.8 | `Spelling::available`, `add` gibt `Result` | ohne Wörterbücher prüft der Kern nichts (macOS: siehe unten) |

**Rechtschreibung unter macOS (Entscheidung nach E16):** Der Kern nutzt Hunspell-Wörterbücher
(`atlas_spell::dictionary_dirs`). Auf dem Mac gibt es die nur über Homebrew oder
mitgelieferte Dateien. P3.3 nimmt deshalb `NSSpellChecker`, solange
`Spelling::available()` `false` ist; das eigene Wörterbuch (`dictionary.txt`) bleibt
gemeinsam.
| P2.10 | `Ilias::set_session(cookies, user_agent)` statt `set_cookies`; `Cookie` nur `name`/`value`/`domain` | der Kern sendet die Cookies passend zum Host (wie Linux mit dem Web-Profil); WKWebView liefert sie über `WKHTTPCookieStore` |
| P2.10 | `scan` → `IliasIndex` (mit Zeitpunkt), `last_index` → `Option<IliasIndex>`, `download(…, download: Option<String>, …)`, neu `logout` | wie der gespeicherte Index unter Linux (`index.json`) |
