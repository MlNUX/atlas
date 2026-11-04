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
| `settings.rs` | `Settings`, `AppSettings`, `Context` (Kontextleiste), `Spelling` | P2.8 | `app/settings.rs`, `spelling.rs`, `ContextProfiles.qml` → P1.3 |
| `editor.rs` | `Editor`, `Span`, `Edit`, `EditorKey`, `Completion`, `Symbol` | P2.8 | `app/editing.rs`, `TextEditor.qml` → P1.4, Hervorhebung → P1.5 |
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

## Änderungen am Vertrag

| Aufgabe | Änderung | Grund |
|---|---|---|
| P2.2 | `SessionSummary` hat `tabs`, `splits`, `kind`, `saved` statt `open_files`/`last_used` | so liefert es der Kern (`ProjectSession::summary`) |
| P2.2 | `Project::save_session(layout, expanded, documents)` und `session_expanded()` statt `save_session_layout`/`expanded_dirs`/`set_expanded_dirs` | Layout, Ordner und Dokumente werden zusammen gespeichert (wie Linux) |
| P2.2 | `create_session(name, layout, expanded)`: Kopie über das übergebene Layout | die Oberfläche kennt den aktuellen Stand, der Kern nur den gespeicherten |
| P2.2 | neu: `Project::resolve_document`, `Atlas::rename_project_session`, `Files::disk_space` | Funktionen der Qt-Brücke, die gefehlt haben |
| P2.2 | `KnownProject` mit der aktiven Sitzung statt einer Liste | wie das Sitzungsmenü unter Linux |
| P2.2 | `FileEntry::kind` ist der lesbare Typ; die Viewer-Art kommt aus `Files::file_kind` | `file_kind` steckte in der Qt-Brücke und liegt jetzt im Kern (`listing::viewer_kind`) |
