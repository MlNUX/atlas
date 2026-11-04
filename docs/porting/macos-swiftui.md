<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Atlas für macOS 26/27 mit SwiftUI – Plan für mehrere Agents

Stand 04.11.2026 · Basis `release-1.1.4` · Branch `port/macos-swiftui`

## Ziel

Atlas bekommt unter macOS eine **eigene, native SwiftUI-Oberfläche** auf dem bestehenden
**Rust-Kern** (`crates/atlas-core`). Linux behält die Qt/QML-Oberfläche. Beide Oberflächen
teilen sich alles, was nicht Darstellung ist: Projekte, Notizen und Links, Suche, Kalender
und Sync, Aktionen, Erweiterungen, Anmerkungen, Konfiguration, Kontextleisten-Profile.

- **Mindestversion macOS 26 (Tahoe)**, Apple Silicon und Intel (Universal Binary).
  macOS 27 wird geprüft, sobald die Beta erscheint (WWDC, Juni 2027), und ab der ersten
  Veröffentlichung in der CI mitgetestet.
- Swift 6 mit strikter Nebenläufigkeitsprüfung, `@Observable`, neue macOS-26-APIs dort, wo
  sie Arbeit sparen (SwiftUI-`WebView`/`WebPage`, Rich-Text im `TextEditor`, Liquid Glass
  kommt mit den Systemsteuerelementen von selbst).
- Verteilung außerhalb des App Stores: Developer-ID-Signatur, Notarisierung, Updates per
  Sparkle. (Der App-Store-Sandkasten verträgt sich nicht mit Terminal, Skripten und
  beliebigen Projektordnern.)

## Ausgangslage (gemessen im Repo)

| Teil | Umfang | Bedeutung für den Port |
|---|---|---|
| `crates/atlas-core` | ~23 400 Zeilen Rust, **keine Qt-Abhängigkeit** | wird wiederverwendet |
| `crates/atlas-app/src` | ~10 200 Zeilen, 255 Qt-Funktionen (`app.rs` allein 126) | Vorbild für die neue, neutrale API; nicht wiederverwendbar (cxx-qt) |
| `crates/atlas-app/qml` | ~25 600 Zeilen, ~770 JS-Funktionen | wird in SwiftUI neu gebaut; Logik darin wandert vorher nach Rust |
| `crates/atlas-app/web` | Vorschau-Seite mit KaTeX, Mermaid, highlight.js | 1:1 in `WKWebView` weiterverwendbar |

Plattformgebunden im Kern: PDF über poppler/cairo/glib (`crates/atlas-core/src/pdf/`, Trait
`Backend`), `prctl` und `/proc` (`background.rs`, `terminal/mod.rs`), Passwortspeicher über
`systemd-creds` (`secret.rs`), Suche über das Programm `rg` (`search.rs`, `ATLAS_RG_BIN`).

## Architektur

```
crates/
  atlas-core/        unverändert Kern; poppler wird optionales Feature "pdf-poppler"
  atlas-api/   NEU   neutrale Fassade: Dienste mit einfachen Rust-Typen, Ereignisse als
                     Callback-Trait; keine Qt-, keine Swift-Typen. Später nutzt auch die
                     Qt-Brücke sie (atlas-app wird dünner).
  atlas-ffi/   NEU   UniFFI-Export von atlas-api (Proc-Macros) → staticlib + Swift-Bindings
  atlas-app/         Qt/QML für Linux, wie heute
apps/macos/    NEU   Xcode-Projekt „Atlas“ (SwiftUI), Swift-Package „AtlasKit“ mit den
                     generierten Bindings und dem XCFramework
packaging/macos/ NEU Info.plist, Entitlements, Icon, bundle.sh, notarize.sh, Sparkle-Appcast
```

### Entscheidungen

| Bereich | Entscheidung | Warum |
|---|---|---|
| Rust ↔ Swift | **UniFFI** (Proc-Macro-Modus), XCFramework für arm64 + x86_64 | erprobt (Firefox iOS), erzeugt idiomatisches Swift, Callbacks und `async` |
| PDF-Anzeige | **PDFKit** (`PDFView` per `NSViewRepresentable`) | native Darstellung, Auswahl, Suche, Zoom; poppler/cairo/glib müssen nicht ins App-Bundle |
| PDF-Daten im Kern | Anmerkungen bleiben im Atlas-Format (`.atlas/annotations/*.json`, Punkte, Ursprung oben links) und werden in PDFKit-Annotationen umgerechnet (`y' = Seitenhöhe − y`); Seitentext für Suche/Index liefert Swift per Callback an den Kern | Daten bleiben zwischen Linux und Mac austauschbar |
| PDF-Export mit Anmerkungen | PDFKit schreibt die Kopie | ersetzt den cairo-Weg auf dem Mac |
| Markdown-Editor | `NSTextView` (TextKit 2) per `NSViewRepresentable`; Syntaxfarben, Listen-/Tabellenlogik, Wiki-Vervollständigung kommen **aus dem Kern** | der SwiftUI-`TextEditor` reicht für einen Code-Editor nicht |
| Syntaxhervorhebung | neu im Kern mit **`syntect`** (reines Rust): `highlight(text, sprache) → Spannen` | KSyntaxHighlighting gibt es auf dem Mac nicht; Qt kann es später auch nutzen |
| Markdown-Vorschau | Kern rendert HTML (wie heute), Anzeige in SwiftUI-`WebView` mit `crates/atlas-app/web/preview.*` | gleiche Vorschau wie unter Linux |
| Terminal | Spike entscheidet: **SwiftTerm** (eigene PTY, ausgereift) oder Raster aus `alacritty_terminal` im Kern + eigener Swift-Renderer | SwiftTerm ist schneller fertig; Kern-Variante hält Verhalten gleich |
| Suche | `rg` ins App-Bundle legen (universal), Pfad über `ATLAS_RG_BIN` | Kern bleibt unverändert |
| Passwörter | Schlüsselbund über das Crate `security-framework` in `platform/macos.rs` | ersetzt `systemd-creds` |
| Formel-OCR | `ort` mit mitgelieferter `libonnxruntime.dylib` (heute `load-dynamic`) | unverändert im Kern |
| Kontextleiste | SwiftUI `.inspector`; Profile-Logik (heute JS in `ContextProfiles.qml`) wandert nach Rust | beide Oberflächen gleich |

## Regeln für alle Agents (vor Phase 1 als `AGENTS.md` ins Repo, `CLAUDE.md` verweist darauf)

1. **Branches:** Integrationszweig ist `port/macos-swiftui`. Jede Aufgabe auf eigenem Branch
   `port/macos/<ID>-<kurz>` (z. B. `port/macos/P2.3-kalender`), Pull-Request zurück in den
   Integrationszweig. Keine Betriebssystem-Branches, kein Arbeiten direkt auf dem
   Integrationszweig.
2. **Datei-Besitz:** Jede Aufgabe nennt die Dateien, die sie ändern darf. Gemeinsame Dateien
   (`Cargo.toml` des Workspace, `atlas-api/src/lib.rs`, `ci.yml`, `CHANGELOG.md`,
   Xcode-Projektdatei) ändert nur der **Integrator**; andere melden den Bedarf im PR.
3. **Vertrag zuerst:** Die API (`docs/porting/api.md` + Rust-Signaturen in `atlas-api`)
   ändert nur die Aufgabe P2.1 bzw. danach der Integrator. Swift-Aufgaben bauen gegen die
   generierten Bindings, nie an der API vorbei.
4. **Fertig heißt:** Code formatiert (`cargo fmt`, `swift-format`), Tests grün (`cargo test`,
   Xcode-Tests), Linux-Selbsttest (`scripts/smoke-test.sh`) weiter BESTANDEN, wenn der Kern
   berührt wurde, kurze Notiz im PR: was, wie geprüft, was offen.
5. **Mac-VM:** Jeder Agent mit eigenem Klon (`~/atlas-<ID>`). Homebrew, Xcode und Toolchains
   installiert nur P0.3. Keine zwei großen Builds gleichzeitig.
6. **Sprache:** Oberfläche Deutsch (wie Linux), Code-Kommentare Englisch, Commits Deutsch.

## Phasen und Aufgaben

Legende: **Mac** = braucht die macOS-VM · **Linux** = geht in der Cloud · Modell: **stark** =
größtes verfügbares Modell (Entwurf, schwierige Integration), **schnell** = kleineres Modell
nach klarer Vorgabe.

### Phase 0 – Grundlagen und Erkundung (≈ 1 Woche, 4 Agents parallel)

| ID | Aufgabe | Ort | Modell | Dateien | Fertig, wenn |
|---|---|---|---|---|---|
| P0.1 | `AGENTS.md` (Regeln oben), `CLAUDE.md` mit Verweis, Aufgabenliste als GitHub-Issues | Linux | schnell | `AGENTS.md`, `CLAUDE.md` | Issues P0–P4 angelegt, je mit ID, Branch, Abnahme |
| P0.2 | **Spike UniFFI:** Mini-Crate mit 3 Funktionen + 1 Callback + 1 `async`, als XCFramework in einer leeren SwiftUI-App aufrufen | Mac | stark | `spikes/uniffi/` | App zeigt Werte aus Rust, Callback kommt im Main-Actor an; Bericht `docs/porting/spike-uniffi.md` |
| P0.3 | **Toolchain:** Xcode 26, Rust-Targets `aarch64/x86_64-apple-darwin`, Homebrew (`ripgrep`, `onnxruntime`), Skript `apps/macos/scripts/bootstrap.sh` | Mac | schnell | `apps/macos/scripts/` | frische VM baut nach einem Skriptaufruf |
| P0.4 | **Spike PDFKit:** PDF anzeigen, Atlas-Anmerkungen aus einer Beispiel-JSON einzeichnen, Auswahl → Rechtecke in Atlas-Koordinaten | Mac | stark | `spikes/pdfkit/` | Markierungen liegen pixelgenau auf denen der Linux-Version (Screenshot-Vergleich) |
| P0.5 | **Spike Terminal:** SwiftTerm vs. Kern-Raster, je 1 Tag; Entscheidung mit Begründung | Mac | stark | `spikes/terminal/` | Entscheidung in `docs/porting/spike-terminal.md` |
| P0.6 | **Kern ohne poppler auf dem Mac übersetzen** (Feature-Schalter vorbereiten, s. P1.1) | Mac | stark | – (Bericht) | Liste aller Stellen, die ohne poppler brechen |

Abhängigkeiten: P0.2, P0.4, P0.5 nach P0.3. P0.1 sofort.

### Phase 1 – Kern vorbereiten (≈ 1–2 Wochen, bis zu 5 Agents parallel, alles Linux)

Jede Aufgabe hält die Qt-Version unverändert lauffähig (Selbsttest grün).

| ID | Aufgabe | Modell | Dateien | Fertig, wenn |
|---|---|---|---|---|
| P1.1 | poppler als optionales Feature `pdf-poppler` (Standard an); `Backend`-Trait öffentlich; Seitentext von außen einspeisbar (`pdf::set_page_texts(path, pages)`) für Suche/Index | stark | `crates/atlas-core/src/pdf/`, `search.rs`, `Cargo.toml` (Kern) | `cargo check --no-default-features` grün, Suche findet eingespeisten Text (Test) |
| P1.2 | Plattform-Schicht `platform/{mod,linux,macos}.rs`: Prozess-Arbeitsverzeichnis, Elternende, Schlüsselbund (`security-framework`), Dateimanager-Orte (macOS: feste Liste + Finder-Favoriten, wenn lesbar) | stark | `crates/atlas-core/src/platform/`, `background.rs`, `terminal/mod.rs`, `secret.rs` | Linux-Tests grün; macOS-Teil übersetzt (P0.3-VM) |
| P1.3 | Kontextleisten-Profile von JS nach Rust: `context_profiles.rs` (Katalog, `layout`, `edit`, `move`, Seiten) mit Tests; QML ruft es über die Qt-Brücke auf | stark | `crates/atlas-core/src/context_profiles.rs`, `qml/ContextProfiles.qml`, `app.rs` (nur neue Funktionen) | alle Kontextleisten-Prüfungen im Selbsttest grün |
| P1.4 | Editor-Logik nach Rust: Tabellen ausrichten, Listen fortsetzen, Zelle finden, Wiki-Vervollständigung (heute JS in `TextEditor.qml`) → `markdown_edit.rs` | stark | `crates/atlas-core/src/markdown_edit.rs`, `qml/TextEditor.qml` | Tabellen-/Listen-Prüfungen grün, Rust-Tests decken dieselben Fälle |
| P1.5 | Syntaxhervorhebung im Kern mit `syntect`: Sprachen erkennen (Dateiendung, Codeblock-Info), Spannen liefern; Markdown mit eingebetteten Codeblöcken | schnell | `crates/atlas-core/src/highlight.rs` | Tests für Markdown, Rust, Python, MATLAB, LaTeX |

Abhängigkeiten: keine untereinander (Dateien getrennt). P1.3/P1.4 ändern `app.rs` nur durch
**neue** Funktionen am Dateiende; der Integrator führt zusammen.

### Phase 2 – `atlas-api` und `atlas-ffi` (≈ 2–3 Wochen)

**P2.1 API-Entwurf (1 Agent, stark, Linux, zuerst):** `docs/porting/api.md` und leere
Rust-Signaturen in `crates/atlas-api/src/*.rs`. Grundsätze: Dienste als Objekte
(`Project`, `Notes`, `Search`, `Calendar`, `Pdf`, `Terminal`, `Actions`, `Extensions`,
`Settings`, `Context`, `Ilias`, `LatexOcr`), einfache Datentypen (Records/Enums statt JSON),
lange Arbeiten `async` oder mit Fortschritt-Callback, Ereignisse über einen
`EventSink`-Trait (Datei geändert, Notizindex neu, Sync fertig …). Vorlage für den Umfang
sind die 255 Funktionen der Qt-Brücke (`crates/atlas-app/src/*.rs`).
Fertig, wenn: Integrator und ein zweites Modell (Gegenlesen) den Entwurf abgenommen haben.

Danach **parallel, je ein Agent pro Bereich** (Linux, meist schnell; Vorlage ist jeweils
die entsprechende Qt-Brücke):

| ID | Bereich | Vorlage (heute) | Dateien |
|---|---|---|---|
| P2.2 | Projekt, Dateien, Sitzungen, Papierkorb, Ordnerbaum | `app.rs` (Sitzungen), `file_ops.rs`, `tree_model.rs`, `folder_model.rs`, `trash_model.rs` | `atlas-api/src/project.rs`, `files.rs` |
| P2.3 | Notizen, Links, Tags, Graph, Zitate, Link-Reparatur | `app.rs` (notes…, contextGraph, projectQuotes) | `atlas-api/src/notes.rs` |
| P2.4 | Suche (Dateien, Text, PDF) | `search_model.rs`, `quickOpen` | `atlas-api/src/search.rs` |
| P2.5 | Kalender, Aufgaben, Sync (CalDAV, Google) | `calendar_model.rs` | `atlas-api/src/calendar.rs` |
| P2.6 | PDF-Daten: Anmerkungen, Zitate, Seitentext-Einspeisung, Formel-OCR | `annotations_model.rs`, `pdf_items.rs`, `latex*` | `atlas-api/src/pdf.rs`, `ocr.rs` |
| P2.7 | Aktionen, Hintergrundjobs, Erweiterungen | `app.rs` (actions…, extension…) | `atlas-api/src/actions.rs`, `extensions.rs` |
| P2.8 | Einstellungen, Kontextleiste (aus P1.3), Rechtschreibung, Hervorhebung (P1.5), Editor-Logik (P1.4) | `app.rs` (config, spell…) | `atlas-api/src/settings.rs`, `editor.rs` |
| P2.9 | Terminal (nur falls P0.5 „Kern-Raster“ wählt) | `terminal_item.rs` | `atlas-api/src/terminal.rs` |
| P2.10 | ILIAS (Anmeldung über WebView in Swift, Rest im Kern) | `app.rs` (ilias…), `IliasService.qml` | `atlas-api/src/ilias.rs` |

Jede: Rust-Tests für die Fassade; keine Logik doppeln, nur `atlas-core` aufrufen.

**P2.11 `atlas-ffi` + Build (1 Agent, stark, Mac):** UniFFI-Export aller Bereiche, Skript
`apps/macos/scripts/build-core.sh` (Universal-XCFramework, Swift-Bindings in `AtlasKit`),
Xcode-Build-Phase ruft es auf. Fertig, wenn `swift test` in `AtlasKit` jeden Dienst einmal
aufruft.

### Phase 3 – SwiftUI-Oberfläche (≈ 6–10 Wochen, Mac)

**P3.0 App-Gerüst (1 Agent, stark, zuerst):** `AtlasApp` mit `WindowGroup` je Projekt,
`NavigationSplitView` (Seitenleiste | Editor-Bereich | `.inspector` für die Kontextleiste),
Tab-/Split-Modell des Editor-Bereichs, `Commands` für die Menüleiste (Ablage, Bearbeiten,
Darstellung, Fenster, Hilfe; ⌘-Kürzel aus `docs/shortcuts.md`), Arbeitsleiste (⌘P),
`Settings`-Szene, Anbindung `AtlasKit` + `EventSink` → `@Observable`-Modelle.
Fertig, wenn ein Projektordner öffnet, Tabs/Splits gehen und Sitzungen gespeichert werden.

Danach **parallel nach Funktion** (eigene Swift-Ordner, eigene Ansichtsmodelle):

| ID | Funktion | Kernstücke | Ordner | Modell |
|---|---|---|---|---|
| P3.1 | Dateibaum, Ordneransicht, Orte, Papierkorb | `OutlineGroup`/`List`, Drag & Drop, Finder-Kontextmenüs, Schnellansicht über **QuickLook** (`QLPreviewPanel`) | `Features/Files/` | schnell |
| P3.2 | **PDF** | PDFKit-Ansicht, Werkzeuge (Markieren, Kommentar, Stift, Radierer), Seitenleiste (Miniaturen, Gliederung, Anmerkungen), Zitat in Notiz, Bereich als Bild, Präsentationsmodus, Dunkelmodus, Export | `Features/PDF/` | stark |
| P3.3 | **Markdown** | Editor (`NSTextView`, Farben/Listen/Tabellen aus dem Kern), Vorschau (`WebView` + `preview.html`), Geteilt mit Scroll-Sync, Einfügen (Bild, Tabelle, Diagramm, Ordnerbaum), Wiki-Links, Rechtschreibung (Kern oder `NSSpellChecker`) | `Features/Markdown/` | stark |
| P3.4 | Text-/Code-Editor, Symbole, Gehe zu Zeile, Suchen/Ersetzen | baut auf dem Editor aus P3.3 auf | `Features/Code/` | schnell |
| P3.5 | Terminal | nach Entscheidung P0.5 | `Features/Terminal/` | stark |
| P3.6 | Kontextleiste | Bausteine, Reiterseiten, Profile, Bearbeitungsmodus, Galerie (Logik aus `atlas-api`) | `Features/Context/` | schnell |
| P3.7 | Kalender | Monat/Woche, Einträge, Aufgaben, Sync-Einstellungen, Konflikte | `Features/Calendar/` | schnell |
| P3.8 | Suche | Fenster mit Typfiltern, Treffer markiert im Dokument | `Features/Search/` | schnell |
| P3.9 | Einstellungen | alle Bereiche aus `SettingsDialog.qml` | `Features/Settings/` | schnell |
| P3.10 | Aktionen und Erweiterungen | Liste, Ausführen, Hintergrundjobs, Erweiterungs-Ansichten (`WebView` + Brücke), Rechte-Dialog | `Features/Extensions/` | stark |
| P3.11 | ILIAS | Anmeldung (`WebView`), Favoriten als Ordner, Downloads | `Features/Ilias/` | schnell |
| P3.12 | Atlas-Notizen (`.atlasnote`), HTML-Ansicht, Bild-Fenster | | `Features/Viewers/` | schnell |

Abhängigkeiten: alle nach P3.0; P3.4 nach den Editor-Grundlagen aus P3.3; P3.6 nach P3.2/P3.3
(Gliederung, Anmerkungen); sonst unabhängig.

### Phase 4 – Qualität und Auslieferung (≈ 2 Wochen, parallel ab Mitte Phase 3)

| ID | Aufgabe | Ort | Modell |
|---|---|---|---|
| P4.1 | **UI-Tests** (XCUITest) nach dem Vorbild des Linux-Selbsttests (`crates/atlas-app/qml/SelfTest.qml`): gleiche Szenarien, gleiches Testprojekt (`scripts/smoke-test.sh` erzeugt es) | Mac | schnell |
| P4.2 | CI: Job `macos-26` (Kern bauen, `AtlasKit`-Tests, App bauen, UI-Tests), Linux-Jobs unverändert | Linux/Mac | schnell |
| P4.3 | Paket: App-Bundle mit `rg` und `libonnxruntime`, Hardened Runtime, Developer-ID-Signatur, Notarisierung, DMG | Mac | stark |
| P4.4 | Updates mit Sparkle, Appcast im Release-Workflow | Mac | schnell |
| P4.5 | Barrierefreiheit (VoiceOver-Beschriftungen), Tastaturbedienung, Dunkel/Hell, Liquid-Glass-Durchsicht | Mac | schnell |
| P4.6 | macOS 27: Beta prüfen, Abweichungen als Aufgaben anlegen | Mac | schnell |
| P4.7 | Doku: `README.md` (Installation macOS), `docs/porting/` aufräumen, CHANGELOG | Linux | schnell |

## Rollen

- **Integrator** (1 Agent, stark, dauerhaft): führt PRs in `port/macos-swiftui` zusammen,
  besitzt die gemeinsamen Dateien, lässt nach jedem Merge Linux-Selbsttest + Mac-Tests laufen,
  pflegt die Issue-Liste. Kann die Sitzung sein, die die anderen startet.
- **Prüfer** (anderes Modell, z. B. Codex oder ein zweites Claude): liest API-Entwurf (P2.1)
  und die schwierigen PRs (P2.11, P3.0, P3.2, P3.3) gegen.
- **Du:** Entscheidungen bei Spikes (P0.4, P0.5), Abnahme auf echtem Mac (Trackpad, Retina,
  Gefühl), Apple-Developer-Account für P4.3.

## Parallelität auf einen Blick

```
Woche  1     2     3     4     5     6     7 … 14    15    16
P0  ███ (4 Agents)
P1        ████████ (5 Agents, Linux)
P2.1          ██
P2.x             ██████ (9 Agents, Linux)  P2.11 ██ (Mac)
P3.0                         ███
P3.x                            ██████████████ (bis 6 Agents gleichzeitig; Mac-VM ist der Engpass)
P4                                       ██████████
```

Engpass ist die **Mac-VM**: Linux-Aufgaben (P1, P2) können breit parallel laufen,
Mac-Aufgaben (P0, P2.11, P3, P4) besser mit 2–3 Agents gleichzeitig, mehr nur mit mehr
Mac-Rechenzeit (zweite VM oder echter Mac).

## Grobe Gesamtdauer

Mit 3–5 parallelen Agents und regelmäßiger Integration **etwa 3–4 Monate** bis zu einer
Version, die die heutigen Funktionen abdeckt. Unsicher sind vor allem P3.2 (PDF-Werkzeuge),
P3.3 (Editor) und P4.3 (Signatur/Notarisierung beim ersten Mal).

## Risiken und Gegenmittel

| Risiko | Gegenmittel |
|---|---|
| API passt nicht zu SwiftUI, viele Nacharbeiten | Spike P0.2 vor P2.1; P2.1 von zweitem Modell prüfen lassen |
| PDFKit-Koordinaten weichen von poppler ab | Spike P0.4 mit Screenshot-Vergleich, Umrechnung zentral in `Features/PDF/Coordinates.swift` |
| Editor (Markdown/Code) wird aufwendiger als gedacht | Logik vorher in den Kern (P1.4, P1.5), Swift nur Darstellung |
| Qt- und SwiftUI-Version laufen auseinander | gemeinsame Logik nur in `atlas-core`/`atlas-api`; Funktionsliste als Paritätstabelle in `docs/porting/paritaet.md` (P4.7) |
| Zu viele Agents auf einer VM | max. 2–3 Mac-Aufgaben gleichzeitig, eigene Klone, ein Agent für Installationen |
