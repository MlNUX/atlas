<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Atlas für macOS 26/27 mit SwiftUI – Plan für mehrere Agents

Stand 04.11.2026 · Integrationszweig **`release-1.1.5`** (nach dessen Veröffentlichung der
jeweils nächste Release-Branch, z. B. `release-1.1.6`) · Aufgaben-Branches `port/macos/<ID>-<kurz>`

## Ziel

Atlas bekommt unter macOS eine **eigene, native SwiftUI-Oberfläche** auf dem bestehenden
**Rust-Kern** (`crates/atlas-core`). Linux behält die Qt/QML-Oberfläche. Beide Oberflächen
teilen sich alles, was nicht Darstellung ist: Projekte, Notizen und Links, Suche, Kalender
und Sync, Aktionen, Erweiterungen, Anmerkungen, Konfiguration, Kontextleisten-Profile.

- **Mindestversion macOS 26 (Tahoe), nur Apple Silicon (arm64).** macOS 27 unterstützt
  ohnehin nur noch Apple Silicon. macOS 27 wird geprüft, sobald die Beta erscheint (WWDC,
  Juni 2027), und in der CI mitgetestet, sobald GitHub ein Runner-Image dafür anbietet.
- **Kein eigener Mac nötig zum Start:** Mac-Arbeit läuft über die macOS-Runner von GitHub
  Actions (arm64, Apple-Hardware). Ab Phase 3 beschleunigt ein Apple-Silicon-Mac (Mac mini
  oder Cloud-Mac) das interaktive Arbeiten; er ist keine Voraussetzung. **Keine
  macOS-VM auf Fremdhardware** (Apples Lizenz erlaubt macOS-VMs nur auf Apple-Hardware;
  ohne Metal und arm64 wäre sie für die Oberfläche ohnehin unbrauchbar).
- Swift 6 mit strikter Nebenläufigkeitsprüfung, `@Observable`, neue macOS-26-APIs dort, wo
  sie Arbeit sparen (SwiftUI-`WebView`/`WebPage`, Rich-Text im `TextEditor`, Liquid Glass
  kommt mit den Systemsteuerelementen von selbst).
- Verteilung außerhalb des App Stores: Developer-ID-Signatur, Notarisierung, Updates per
  Sparkle. (Der App-Store-Sandkasten verträgt sich nicht mit Terminal, Skripten und
  beliebigen Projektordnern.)

## Ausgangslage (gemessen im Repo)

| Teil | Umfang | Bedeutung für den Port |
|---|---|---|
| `crates/atlas-core` | ~20 500 Zeilen Rust, **keine Qt-Abhängigkeit** | wird wiederverwendet |
| `crates/atlas-terminal`, `atlas-spell`, `atlas-ilias` | ~1 100 / ~300 / ~1 600 Zeilen, **ohne Qt** (`atlas-ilias` nutzt `atlas-core`) | werden wiederverwendet |
| `crates/atlas-app/src` | ~10 400 Zeilen, 255 Qt-Funktionen: `app.rs` + `app/` (nach Themen: `sessions`, `notes`, `relations`, `actions`, `extensions`, `settings`, `editing`), `calendar_model/`, `ilias_backend.rs`, `spelling.rs`, `formula_ocr.rs`, Modelle | Vorbild für die neue, neutrale API; nicht wiederverwendbar (cxx-qt) |
| `crates/atlas-app/qml` | ~25 800 Zeilen, ~775 JS-Funktionen | wird in SwiftUI neu gebaut; Logik darin wandert vorher nach Rust |
| `crates/atlas-app/web` | Vorschau-Seite mit KaTeX, Mermaid, highlight.js | 1:1 in `WKWebView` weiterverwendbar |

Plattformgebunden im Kern: PDF über poppler/cairo/glib (`crates/atlas-core/src/pdf/`, Trait
`Backend`), `prctl` (`atlas-core/src/background.rs`) und `/proc`
(`crates/atlas-terminal/src/lib.rs`), Passwortspeicher über
`systemd-creds` (`secret.rs`), Suche über das Programm `rg` (`search.rs`, `ATLAS_RG_BIN`).

## Entscheidungen

Alle festgelegt (04.11.2026) – gilt für alle Agents, nicht neu verhandeln. Wer auf eine
Frage stößt, die hier nicht beantwortet ist, entscheidet nach dem Geist dieser Liste,
dokumentiert es im PR und macht weiter.

| # | Frage | Entscheidung |
|---|---|---|
| E1 | Mac-Zugang | GitHub-Actions-Runner (macOS, arm64); keine VM auf Fremdhardware; ab Phase 3 optional ein Apple-Silicon-Mac |
| E2 | Architektur | nur arm64 (Apple Silicon), kein Intel/Universal |
| E3 | Mindestversion | macOS 26; macOS 27 mittesten, sobald verfügbar |
| E4 | Branches | Integrationszweig ist der aktuelle Release-Branch (jetzt `release-1.1.5`, danach der nächste); Aufgaben auf kurzlebigen Branches `port/macos/<ID>-<kurz>`, abgezweigt vom Integrationszweig, PR zurück dorthin, nach dem Merge gelöscht; keine Betriebssystem-Branches |
| E5 | Name, Bundle-ID | `Atlas`, `de.minux.atlas`. Apple-Developer-Account kommt noch: bis dahin unsignierte Test-Builds (ad hoc signiert), Signatur/Notarisierung (P4.3) so vorbereiten, dass nur Team-ID und Zertifikate fehlen |
| E6 | Umfang Version 1 | **volle Parität** mit der Linux-Version (alle Funktionen inkl. Kalender-Sync, ILIAS, Erweiterungen mit Ansichten, Formel-OCR, `.atlasnote`); keine kleinere Veröffentlichung vorher |
| E7 | Weg nach `main` | über den Release-Branch wie jede andere Änderung (Release-Branch → `main`); die Mac-App ist ab W7 (App-Gerüst läuft) Teil der Veröffentlichungen, als „Vorschau“ gekennzeichnet, bis die volle Parität erreicht ist |
| E8 | Aussehen | natives macOS (Liquid Glass, Systemfarben, San Francisco, Hell/Dunkel vom System), einzige Markenfarbe ist die Atlas-Akzentfarbe |
| E9 | Fenster und Tabs | ein Fenster pro Projekt, darin Atlas-Tabs und -Splits wie unter Linux; Sitzungen zwischen Linux und Mac austauschbar |
| E10 | Tastenkürzel | macOS-Konvention gewinnt; Atlas-Kürzel nur, wo frei, sonst Ersatzkürzel (Liste in `docs/shortcuts.md` um eine Mac-Spalte ergänzen) |
| E11 | Sprache | String Catalogs, Deutsch als Ausgangssprache, übersetzbar angelegt |
| E12 | Projekte Linux ↔ Mac | austauschbar, **bindend**: gleiches `.atlas/`-Format (Anmerkungen, Sitzungen, Index, Kontextleiste); Kern-Änderungen müssen es einhalten, Tests mit Projekten beider Seiten |
| E13 | Ort der Einstellungen | `~/Library/Application Support/Atlas` (heute ergäbe `dirs::config_dir()` dort `…/atlas` klein; P1.2 legt den Mac-Pfad in der Plattform-Schicht fest) |
| E14 | Kalender | eigener Atlas-Kalender mit CalDAV/Google-Sync wie Linux **und zusätzlich** der macOS-Kalender über EventKit (Termine anzeigen und anlegen, Zielkalender wählbar) – Aufgabe P3.13 |
| E15 | Verteilung | DMG auf GitHub-Releases, Updates per Sparkle; Homebrew-Cask später |
| E16 | Spikes ohne klare Antwort | die Variante, die schneller fertig ist; Agents entscheiden selbst, dokumentieren und informieren |

## Architektur

```
crates/
  atlas-core/        Kern; poppler wird optionales Feature "pdf-poppler"
  atlas-terminal/    Terminal (alacritty_terminal), atlas-spell/ Rechtschreibung,
  atlas-ilias/       ILIAS – alle ohne Qt, unverändert weiterverwendet
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
| Rust ↔ Swift | **UniFFI** (Proc-Macro-Modus), XCFramework für arm64 (E2) | erprobt (Firefox iOS), erzeugt idiomatisches Swift, Callbacks und `async` |
| PDF-Anzeige | **PDFKit** (`PDFView` per `NSViewRepresentable`) – bestätigt durch Spike P0.4: Markierungen deckungsgleich mit Linux, auch mit CropBox und `/Rotate` | native Darstellung, Auswahl, Suche, Zoom; poppler/cairo/glib müssen nicht ins App-Bundle |
| PDF-Daten im Kern | Anmerkungen bleiben im Atlas-Format (`.atlas/annotations/*.json`, Punkte, Ursprung oben links) und werden in PDFKit-Annotationen umgerechnet (`y' = Seitenhöhe − y`); Seitentext für Suche/Index liefert Swift per Callback an den Kern | Daten bleiben zwischen Linux und Mac austauschbar |
| PDF-Export mit Anmerkungen | PDFKit schreibt die Kopie | ersetzt den cairo-Weg auf dem Mac |
| Markdown-Editor | `NSTextView` (TextKit 2) per `NSViewRepresentable`; Syntaxfarben, Listen-/Tabellenlogik, Wiki-Vervollständigung kommen **aus dem Kern** | der SwiftUI-`TextEditor` reicht für einen Code-Editor nicht |
| Syntaxhervorhebung | neu im Kern mit **`syntect`** (reines Rust): `highlight(text, sprache) → Spannen` | KSyntaxHighlighting gibt es auf dem Mac nicht; Qt kann es später auch nutzen |
| Markdown-Vorschau | Kern rendert HTML (wie heute), Anzeige in SwiftUI-`WebView` mit `crates/atlas-app/web/preview.*` | gleiche Vorschau wie unter Linux |
| Terminal | **SwiftTerm** (`LocalProcessTerminalView`, eigene PTY) – entschieden in Spike P0.5 (`docs/porting/spike-terminal.md`); P2.9 entfällt | gleich schnell wie die Kern-Variante, bringt Tastatur/IME, Auswahl, Maus und Bedienungshilfen mit |
| Suche | `rg` (arm64) ins App-Bundle legen, Pfad über `ATLAS_RG_BIN` | Kern bleibt unverändert |
| Passwörter | Schlüsselbund über das Crate `security-framework` in `platform/macos.rs` | ersetzt `systemd-creds` |
| Formel-OCR | `ort` mit mitgelieferter `libonnxruntime.dylib` (heute `load-dynamic`) | unverändert im Kern |
| Mac-Builds und -Tests | GitHub Actions, macOS-Runner arm64; ein gemeinsamer Einrichtungs-Workflow (`.github/workflows/macos-setup.yml`, `workflow_call`), je Aufgabe eine eigene kleine Workflow-Datei, die ihn aufruft; Logs und Bildschirmfotos als Artefakte | legal, arm64 wie die Nutzer, Agents können pushen und Ergebnisse lesen |
| Kontextleiste | SwiftUI `.inspector`; Profile-Logik (heute JS in `ContextProfiles.qml`) wandert nach Rust | beide Oberflächen gleich |

## Regeln für alle Agents (vor Phase 1 als `AGENTS.md` ins Repo, `CLAUDE.md` verweist darauf)

1. **Branches:** Integrationszweig ist der aktuelle Release-Branch, jetzt `release-1.1.5`
   (nach dessen Veröffentlichung der nächste, z. B. `release-1.1.6` – der Integrator sagt
   Bescheid). Jede Aufgabe auf eigenem Branch `port/macos/<ID>-<kurz>`
   (z. B. `port/macos/P2.3-kalender`), abgezweigt vom aktuellen Integrationszweig,
   Pull-Request zurück dorthin, nach dem Merge löschen. Keine Betriebssystem-Branches, kein
   Arbeiten direkt auf dem Integrationszweig. Weil auf `release-1.1.5` auch andere Arbeit
   landet: vor jedem PR auf den aktuellen Stand rebasen.
2. **Datei-Besitz:** Jede Aufgabe nennt die Dateien, die sie ändern darf. Gemeinsame Dateien
   (`Cargo.toml` des Workspace, `atlas-api/src/lib.rs`, `ci.yml`, `CHANGELOG.md`,
   Xcode-Projektdatei) ändert nur der **Integrator**; andere melden den Bedarf im PR.
3. **Vertrag zuerst:** Die API (`docs/porting/api.md` + Rust-Signaturen in `atlas-api`)
   ändert nur die Aufgabe P2.1 bzw. danach der Integrator. Swift-Aufgaben bauen gegen die
   generierten Bindings, nie an der API vorbei.
4. **Fertig heißt:** Code formatiert (`cargo fmt`, `swift-format`), Tests grün (`cargo test`,
   Xcode-Tests), Linux-Selbsttest (`scripts/smoke-test.sh`) weiter BESTANDEN, wenn der Kern
   berührt wurde, kurze Notiz im PR: was, wie geprüft, was offen.
5. **macOS über CI:** Jede Mac-Aufgabe bekommt eine eigene Workflow-Datei
   (`.github/workflows/mac-<ID>.yml`), die nur auf ihren Branches (`port/macos/<ID>-*`)
   läuft und `macos-setup.yml` aufruft. Installationen nur im Einrichtungs-Workflow (gehört
   dem Integrator), nie von Hand. Ergebnisse (Logs, `screencapture`-Bilder, `.xcresult`) als
   Artefakte hochladen; Agents lesen sie über die GitHub-Anbindung. Caches für Cargo und
   Homebrew nutzen; Läufe nur bei Änderungen in den eigenen Pfaden (`paths:`).
6. **Sprache:** Oberfläche Deutsch (wie Linux), Code-Kommentare Englisch, Commits Deutsch.

## Phasen und Aufgaben

Legende: **Mac** = macOS-Runner der CI (ab Phase 3 optional zusätzlich ein Apple-Silicon-Mac)
· **Linux** = geht in der Cloud · Modell: **stark** =
größtes verfügbares Modell (Entwurf, schwierige Integration), **schnell** = kleineres Modell
nach klarer Vorgabe.

### Phase 0 – Grundlagen und Erkundung (≈ 1 Woche, 4 Agents parallel)

| ID | Aufgabe | Ort | Modell | Dateien | Fertig, wenn |
|---|---|---|---|---|---|
| P0.1 | `AGENTS.md` (Regeln oben), `CLAUDE.md` mit Verweis, Aufgabenliste als GitHub-Issues | Linux | schnell | `AGENTS.md`, `CLAUDE.md` | Issues P0–P4 angelegt, je mit ID, Branch, Abnahme |
| P0.2 | **Spike UniFFI:** Mini-Crate mit 3 Funktionen + 1 Callback + 1 `async`, als XCFramework in einer leeren SwiftUI-App aufrufen | Mac | stark | `spikes/uniffi/`, `.github/workflows/mac-P0.2.yml` | CI-Test ruft die Werte aus Rust ab, Callback kommt im Main-Actor an; Bericht `docs/porting/spike-uniffi.md` |
| P0.3 | **CI-Grundlage:** `macos-setup.yml` (`workflow_call`): neuestes macOS-arm64-Image (26, sobald verfügbar), Xcode 26, Rust-Target `aarch64-apple-darwin`, Homebrew (`ripgrep`, `onnxruntime`), Caches; dasselbe als Skript `apps/macos/scripts/bootstrap.sh` für einen späteren echten Mac | Linux (schreiben) + Mac-CI (prüfen) | schnell | `.github/workflows/macos-setup.yml`, `.github/workflows/mac-P0.3.yml`, `apps/macos/scripts/` | Testlauf baut ein leeres Xcode-Projekt und `cargo test -p atlas-core --no-default-features` (bzw. listet, was ohne poppler bricht), Artefakte hochgeladen |
| P0.4 | **Spike PDFKit:** PDF anzeigen, Atlas-Anmerkungen aus einer Beispiel-JSON einzeichnen, Auswahl → Rechtecke in Atlas-Koordinaten | Mac | stark | `spikes/pdfkit/`, `.github/workflows/mac-P0.4.yml` | Bildschirmfoto aus der CI zeigt die Markierungen deckungsgleich mit der Linux-Version (Vergleichsbild im Bericht) |
| P0.5 | **Spike Terminal:** SwiftTerm vs. Kern-Raster, je 1 Tag; Entscheidung mit Begründung | Mac | stark | `spikes/terminal/`, `.github/workflows/mac-P0.5.yml` | Entscheidung in `docs/porting/spike-terminal.md`, mit CI-Bildschirmfotos beider Varianten |
| P0.6 | **Kern ohne poppler für macOS übersetzen** (Feature-Schalter vorbereiten, s. P1.1) | Mac | stark | `.github/workflows/mac-P0.6.yml` (Bericht) | Liste aller Stellen, die ohne poppler brechen |

Abhängigkeiten: P0.2, P0.4, P0.5, P0.6 nach P0.3 (sie rufen `macos-setup.yml` auf). P0.1 sofort.

### Phase 1 – Kern vorbereiten (≈ 1–2 Wochen, bis zu 5 Agents parallel, alles Linux)

Jede Aufgabe hält die Qt-Version unverändert lauffähig (Selbsttest grün).

| ID | Aufgabe | Modell | Dateien | Fertig, wenn |
|---|---|---|---|---|
| P1.1 | poppler als optionales Feature `pdf-poppler` (Standard an); `Backend`-Trait öffentlich; Seitentext von außen einspeisbar (`pdf::set_page_texts(path, pages)`) für Suche/Index | stark | `crates/atlas-core/src/pdf/`, `search.rs`, `Cargo.toml` (Kern) | `cargo check --no-default-features` grün, Suche findet eingespeisten Text (Test) |
| P1.2 | Plattform-Schicht `platform/{mod,linux,macos}.rs`: Prozess-Arbeitsverzeichnis, Elternende, Schlüsselbund (`security-framework`), Dateimanager-Orte (macOS: feste Liste + Finder-Favoriten, wenn lesbar) | stark | `crates/atlas-core/src/platform/`, `background.rs`, `secret.rs`; Arbeitsverzeichnis per `cfg` direkt in `crates/atlas-terminal/src/lib.rs` (das Crate hängt nicht von `atlas-core` ab) | Linux-Tests grün; macOS-Teil übersetzt in der macOS-CI (`mac-P1.2.yml`) |
| P1.3 | Kontextleisten-Profile von JS nach Rust: `context_profiles.rs` (Katalog, `layout`, `edit`, `move`, Seiten) mit Tests; QML ruft es über die Qt-Brücke auf | stark | `crates/atlas-core/src/context_profiles.rs`, `qml/ContextProfiles.qml`, `crates/atlas-app/src/app/settings.rs` (nur neue Funktionen) | alle Kontextleisten-Prüfungen im Selbsttest grün |
| P1.4 | Editor-Logik nach Rust: Tabellen ausrichten, Listen fortsetzen, Zelle finden, Wiki-Vervollständigung (heute JS in `TextEditor.qml`) → `markdown_edit.rs` | stark | `crates/atlas-core/src/markdown_edit.rs`, `qml/TextEditor.qml`, `crates/atlas-app/src/app/editing.rs` | Tabellen-/Listen-Prüfungen grün, Rust-Tests decken dieselben Fälle |
| P1.5 | Syntaxhervorhebung im Kern mit `syntect`: Sprachen erkennen (Dateiendung, Codeblock-Info), Spannen liefern; Markdown mit eingebetteten Codeblöcken | schnell | `crates/atlas-core/src/highlight.rs` | Tests für Markdown, Rust, Python, MATLAB, LaTeX |

Abhängigkeiten: keine untereinander (Dateien getrennt). P1.3 ergänzt Funktionen in
`app/settings.rs`, P1.4 in `app/editing.rs`; die Aufzählung der Qt-Funktionen in `app.rs`
ändert nur der Integrator.

### Phase 2 – `atlas-api` und `atlas-ffi` (≈ 2–3 Wochen)

**P2.1 API-Entwurf (1 Agent, stark, Linux, zuerst):** `docs/porting/api.md` und leere
Rust-Signaturen in `crates/atlas-api/src/*.rs`. Grundsätze: Dienste als Objekte
(`Project`, `Notes`, `Search`, `Calendar`, `Pdf`, `Terminal`, `Actions`, `Extensions`,
`Settings`, `Context`, `Ilias`, `LatexOcr`), einfache Datentypen (Records/Enums statt JSON),
lange Arbeiten `async` oder mit Fortschritt-Callback, Ereignisse über einen
`EventSink`-Trait (Datei geändert, Notizindex neu, Sync fertig …). Vorlage für den Umfang
sind die 255 Funktionen der Qt-Brücke (`crates/atlas-app/src/**/*.rs`).
Fertig, wenn: Integrator und ein zweites Modell (Gegenlesen) den Entwurf abgenommen haben.

Danach **parallel, je ein Agent pro Bereich** (Linux, meist schnell; Vorlage ist jeweils
die entsprechende Qt-Brücke):

| ID | Bereich | Vorlage (heute) | Dateien |
|---|---|---|---|
| P2.2 | Projekt, Dateien, Sitzungen, Papierkorb, Ordnerbaum | `app/sessions.rs`, `file_ops.rs`, `tree_model.rs`, `folder_model.rs`, `trash_model.rs` | `atlas-api/src/project.rs`, `files.rs` |
| P2.3 | Notizen, Links, Tags, Graph, Zitate, Link-Reparatur | `app/notes.rs`, `app/relations.rs` | `atlas-api/src/notes.rs` |
| P2.4 | Suche (Dateien, Text, PDF) | `search_model.rs`, `quickOpen` | `atlas-api/src/search.rs` |
| P2.5 | Kalender, Aufgaben, Sync (CalDAV, Google) | `calendar_model.rs` + `calendar_model/` (`account`, `edit`, `remote`) | `atlas-api/src/calendar.rs` |
| P2.6 | PDF-Daten: Anmerkungen, Zitate, Seitentext-Einspeisung, Formel-OCR | `annotations_model.rs`, `pdf_items.rs`, `formula_ocr.rs` | `atlas-api/src/pdf.rs`, `ocr.rs` |
| P2.7 | Aktionen, Hintergrundjobs, Erweiterungen | `app/actions.rs`, `app/extensions.rs` | `atlas-api/src/actions.rs`, `extensions.rs` |
| P2.8 | Einstellungen, Kontextleiste (aus P1.3), Rechtschreibung, Hervorhebung (P1.5), Editor-Logik (P1.4) | `app/settings.rs`, `app/editing.rs`, `spelling.rs` (+ Crate `atlas-spell`) | `atlas-api/src/settings.rs`, `editor.rs` |
| P2.9 | Terminal (nur falls P0.5 „Kern-Raster“ wählt) | `terminal_item.rs` (+ Crate `atlas-terminal`) | `atlas-api/src/terminal.rs` |
| P2.10 | ILIAS (Anmeldung über WebView in Swift, Rest im Kern) | `ilias_backend.rs` (+ Crate `atlas-ilias`), `IliasService.qml` | `atlas-api/src/ilias.rs` |

Jede: Rust-Tests für die Fassade; keine Logik doppeln, nur `atlas-core` aufrufen.

**P2.11 `atlas-ffi` + Build (1 Agent, stark, Mac):** UniFFI-Export aller Bereiche, Skript
`apps/macos/scripts/build-core.sh` (XCFramework arm64, Swift-Bindings in `AtlasKit`),
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
| P3.5 | Terminal | SwiftTerm (P0.5); Arbeitsverzeichnis über `proc_pidinfo` wie `atlas-terminal` (P1.2) | `Features/Terminal/` | stark |
| P3.6 | Kontextleiste | Bausteine, Reiterseiten, Profile, Bearbeitungsmodus, Galerie (Logik aus `atlas-api`) | `Features/Context/` | schnell |
| P3.7 | Kalender | Monat/Woche, Einträge, Aufgaben, Sync-Einstellungen, Konflikte | `Features/Calendar/` | schnell |
| P3.8 | Suche | Fenster mit Typfiltern, Treffer markiert im Dokument | `Features/Search/` | schnell |
| P3.9 | Einstellungen | alle Bereiche aus `SettingsDialog.qml` | `Features/Settings/` | schnell |
| P3.10 | Aktionen und Erweiterungen | Liste, Ausführen, Hintergrundjobs, Erweiterungs-Ansichten (`WebView` + Brücke), Rechte-Dialog | `Features/Extensions/` | stark |
| P3.11 | ILIAS | Anmeldung (`WebView`), Favoriten als Ordner, Downloads | `Features/Ilias/` | schnell |
| P3.12 | Atlas-Notizen (`.atlasnote`), HTML-Ansicht, Bild-Fenster | | `Features/Viewers/` | schnell |
| P3.13 | **macOS-Kalender (EventKit)** | Zugriff anfragen (Info.plist `NSCalendarsFullAccessUsageDescription`, Kalender-Entitlement), Termine und Erinnerungen der macOS-Kalender neben den Atlas-Einträgen anzeigen, beim Anlegen Zielkalender wählen (Atlas oder macOS), Änderungen über `EKEventStoreChanged` | `Features/Calendar/EventKit/` | stark |

Abhängigkeiten: alle nach P3.0; P3.4 nach den Editor-Grundlagen aus P3.3; P3.6 nach P3.2/P3.3
(Gliederung, Anmerkungen); P3.13 nach P3.7 (Kalenderansichten); sonst unabhängig.

### Phase 4 – Qualität und Auslieferung (≈ 2 Wochen, parallel ab Mitte Phase 3)

| ID | Aufgabe | Ort | Modell |
|---|---|---|---|
| P4.1 | **UI-Tests** (XCUITest) nach dem Vorbild des Linux-Selbsttests (`crates/atlas-app/qml/SelfTest.qml`): gleiche Szenarien, gleiches Testprojekt (`scripts/smoke-test.sh` erzeugt es) | Mac | schnell |
| P4.2 | CI ausbauen: dauerhafter Workflow `macos.yml` (Kern bauen, `AtlasKit`-Tests, App bauen, UI-Tests) für Integrationszweig und PRs; die `mac-<ID>.yml` der erledigten Aufgaben entfernen; Linux-Jobs unverändert | Linux/Mac | schnell |
| P4.3 | Paket: App-Bundle mit `rg` und `libonnxruntime`, Hardened Runtime, Developer-ID-Signatur, Notarisierung, DMG | Mac | stark |
| P4.4 | Updates mit Sparkle, Appcast im Release-Workflow | Mac | schnell |
| P4.5 | Barrierefreiheit (VoiceOver-Beschriftungen), Tastaturbedienung, Dunkel/Hell, Liquid-Glass-Durchsicht | Mac | schnell |
| P4.6 | macOS 27: Beta prüfen, Abweichungen als Aufgaben anlegen | Mac | schnell |
| P4.7 | Doku: `README.md` (Installation macOS), `docs/porting/` aufräumen, CHANGELOG | Linux | schnell |

## Aufteilung auf zwei Modelle (LLM1, LLM2)

Zwei Agents arbeiten in **Wellen** gleichzeitig. In jeder Welle ändern sie nur ihre eigenen
Dateien; am Ende der Welle führt LLM1 beide PRs in den Integrationszweig (`release-1.1.5`)
zusammen, lässt die
Tests laufen, und beide starten die nächste Welle vom neuen Stand (`git rebase`).

### Damit es keine Konflikte gibt

- **LLM1 ist Integrator** und besitzt allein: Workspace-`Cargo.toml`, `crates/atlas-core/Cargo.toml`,
  `crates/atlas-core/src/lib.rs`, `crates/atlas-api/src/lib.rs`, `crates/atlas-app/src/app.rs`
  (Aufzählung der Qt-Funktionen), `.github/workflows/ci.yml`,
  `.github/workflows/macos-setup.yml`,
  `CHANGELOG.md`, Xcode-Projekt (`apps/macos/Atlas.xcodeproj`), `AGENTS.md` (nach P0.1).
  LLM2 schreibt benötigte Abhängigkeiten oder `pub mod`-Zeilen in die PR-Beschreibung,
  LLM1 trägt sie beim Zusammenführen ein.
- **Gerüste mit Platzhaltern:** P2.1 legt *alle* Dateien von `atlas-api` mit Signaturen an,
  P3.0 *alle* Feature-Ordner mit einer Platzhalter-Ansicht und einer leeren `Commands`-Datei
  je Funktion. Spätere Aufgaben füllen nur ihre eigenen Dateien und ändern Router, Menüleiste
  und Projektdatei nicht.
- **Xcode:** synchronisierte Ordner (bzw. ein lokales Swift-Paket je Funktion), damit neue
  Swift-Dateien die `project.pbxproj` nicht ändern.
- **macOS-CI:** jede Mac-Aufgabe hat ihre eigene Workflow-Datei `mac-<ID>.yml`; der gemeinsame
  `macos-setup.yml` gehört LLM1. Das Kern-XCFramework baut nach jedem Zusammenführen nur
  LLM1 und legt es als Artefakt ab; LLM2 lädt es in seinen Läufen herunter.
- **Kern-Änderungen in Phase 1** halten die Qt-Version lauffähig; Selbsttest
  (`scripts/smoke-test.sh`) grün vor jedem PR.

### Wellen

| Welle | LLM1 (Integrator) | LLM2 | Wo | Danach |
|---|---|---|---|---|
| W0 | **P0.3** CI-Grundlage `macos-setup.yml` | **P0.1** `AGENTS.md`, `CLAUDE.md`, GitHub-Issues | Mac-CI / Linux | LLM1 führt zusammen |
| W1 | **P0.2** Spike UniFFI (`spikes/uniffi/`) | **P0.4** Spike PDFKit (`spikes/pdfkit/`) | Mac / Mac | |
| W2 | **P0.6** Kern ohne poppler (Bericht) | **P0.5** Spike Terminal (`spikes/terminal/`) | Mac / Mac | PDF- und Terminal-Weg nach den Spikes festhalten (bei Unklarheit E16) |
| W3 | **P1.1** poppler optional, dann **P1.2** Plattform-Schicht (`pdf/`, `search.rs`, `platform/`, `background.rs`, `atlas-terminal/src/lib.rs`, `secret.rs`) | **P1.3** Kontextleisten-Profile nach Rust, dann **P1.4** Editor-Logik nach Rust (`context_profiles.rs`, `markdown_edit.rs`, `ContextProfiles.qml`, `TextEditor.qml`, `app/settings.rs`, `app/editing.rs`) | Linux / Linux | |
| W4 | **P2.1** API-Entwurf (`docs/porting/api.md`, alle `atlas-api`-Dateien als Signaturen) | **P1.5** Syntaxhervorhebung (`highlight.rs`), danach **Prüfer** für P2.1 | Linux / Linux | Entwurf erst nach LLM2-Abnahme zusammenführen |
| W5 | **P2.2** Projekt/Dateien, **P2.4** Suche, **P2.8** Einstellungen/Editor, **P2.10** ILIAS (P2.9 entfällt, SwiftTerm) | **P2.3** Notizen, **P2.5** Kalender, **P2.6** PDF-Daten/OCR, **P2.7** Aktionen/Erweiterungen | Linux / Linux | je Bereich ein PR |
| W6 | **P2.11** `atlas-ffi`, XCFramework, `AtlasKit` | Paritätstabelle `docs/porting/paritaet.md` + Testkatalog aus `SelfTest.qml` für P4.1 | Mac / Linux | |
| W7 | **P3.0** App-Gerüst mit allen Platzhaltern | **P4.2** dauerhafter Workflow `macos.yml` (eigene Datei) | Mac / Linux | ab hier füllt jeder nur seine Feature-Ordner |
| W8 | **P3.2** PDF (`Features/PDF/`) | **P3.3** Markdown (`Features/Markdown/`) | Mac / Mac | |
| W9 | **P3.8** Suche, **P3.11** ILIAS | **P3.4** Code-Editor (baut auf P3.3), **P3.1** Dateien | Mac / Mac | |
| W10 | **P3.6** Kontextleiste (braucht P3.2/P3.3) | **P3.5** Terminal, **P3.7** Kalender | Mac / Mac | |
| W11 | **P3.10** Aktionen und Erweiterungen, **P3.13** macOS-Kalender (EventKit) | **P3.9** Einstellungen, **P3.12** weitere Ansichten | Mac / Mac | |
| W12 | **P4.3** Bundle, Signatur, Notarisierung, **P4.4** Sparkle | **P4.1** UI-Tests, **P4.5** Barrierefreiheit | Mac / Mac | |
| W13 | **P4.6** macOS 27 (sobald Beta) | **P4.7** Doku, README, CHANGELOG-Text (LLM1 trägt ein) | Mac / Linux | Version fertig |

Ab W8 bauen beide gleichzeitig Swift-Code – jeder in seinen eigenen CI-Läufen (bzw. mit
eigenem Klon auf einem echten Mac, falls vorhanden). Große Rust-Builds (Kern, XCFramework)
macht nur LLM1, einmal pro Zusammenführung.

### Auftrag an jedes Modell (Vorlage)

> Du bist **LLM1|LLM2** im Atlas-macOS-Port. Lies `docs/porting/macos-swiftui.md` und
> `AGENTS.md`. Deine Aufgaben in dieser Welle: **<IDs>**. Arbeite auf Branch
> `port/macos/<ID>-<kurz>`, abgezweigt vom aktuellen Integrationszweig `release-1.1.5`
> (vor dem PR rebasen). Mac-Builds und
> -Tests laufen über deine eigene Workflow-Datei `.github/workflows/mac-<ID>.yml` (ruft
> `macos-setup.yml` auf); lies Logs und Bildschirmfotos aus den Artefakten. Ändere nur die
> in der Aufgabe genannten Dateien; brauchst du eine andere,
> schreib es in den PR. Fertig heißt: Abnahmekriterium der Aufgabe erfüllt, Tests grün,
> PR mit kurzer Notiz (was, wie geprüft, was offen).

## Rollen

- **Integrator** (1 Agent, stark, dauerhaft): führt PRs in den Integrationszweig
  (`release-1.1.5`, danach den nächsten Release-Branch) zusammen,
  besitzt die gemeinsamen Dateien, lässt nach jedem Merge Linux-Selbsttest + Mac-Tests laufen,
  pflegt die Issue-Liste. Kann die Sitzung sein, die die anderen startet.
- **Prüfer** (anderes Modell, z. B. Codex oder ein zweites Claude): liest API-Entwurf (P2.1)
  und die schwierigen PRs (P2.11, P3.0, P3.2, P3.3) gegen.
- **Du:** Abnahme auf echtem Mac (Trackpad, Retina,
  Gefühl), Apple-Developer-Account für P4.3; bis Phase 3 möglichst einen Apple-Silicon-Mac
  (Mac mini oder Cloud-Mac) für schnelleres Arbeiten an der Oberfläche – sonst geht P3 nur
  über CI, langsamer.

## Parallelität auf einen Blick

```
Woche  1     2     3     4     5     6     7 … 14    15    16
P0  ███ (4 Agents)
P1        ████████ (5 Agents, Linux)
P2.1          ██
P2.x             ██████ (9 Agents, Linux)  P2.11 ██ (Mac)
P3.0                         ███
P3.x                            ██████████████ (bis 6 Agents gleichzeitig; Mac-Rechenzeit ist der Engpass)
P4                                       ██████████
```

Engpass ist die **Mac-Rechenzeit**: Linux-Aufgaben (P1, P2) können breit parallel laufen.
Mac-Aufgaben (P0, P2.11, P3, P4) laufen über CI-Runner (einige Minuten pro Versuch,
Kontingent für macOS-Minuten beachten) und ab Phase 3 bevorzugt auf einem echten Mac.

## Grobe Gesamtdauer

Mit 3–5 parallelen Agents und regelmäßiger Integration **etwa 3–4 Monate** bis zu einer
Version mit voller Parität (E6), zuzüglich etwa einer Woche für EventKit (E14). Mit nur zwei
Modellen (LLM1/LLM2, Wellen W0–W13) eher am oberen Ende oder darüber. Unsicher sind vor allem P3.2 (PDF-Werkzeuge),
P3.3 (Editor) und P4.3 (Signatur/Notarisierung beim ersten Mal).

## Risiken und Gegenmittel

| Risiko | Gegenmittel |
|---|---|
| API passt nicht zu SwiftUI, viele Nacharbeiten | Spike P0.2 vor P2.1; P2.1 von zweitem Modell prüfen lassen |
| PDFKit-Koordinaten weichen von poppler ab | Spike P0.4 mit Screenshot-Vergleich, Umrechnung zentral in `Features/PDF/Coordinates.swift` |
| Editor (Markdown/Code) wird aufwendiger als gedacht | Logik vorher in den Kern (P1.4, P1.5), Swift nur Darstellung |
| Qt- und SwiftUI-Version laufen auseinander | gemeinsame Logik nur in `atlas-core`/`atlas-api`; Funktionsliste als Paritätstabelle in `docs/porting/paritaet.md` (P4.7) |
| Lange Wartezeiten und Minutenverbrauch in der macOS-CI | Caches, Läufe nur bei Änderungen in eigenen Pfaden, Kern-XCFramework als Artefakt wiederverwenden; ab Phase 3 Apple-Silicon-Mac für die Oberfläche |
| GitHub hat noch kein macOS-26-Image | bis dahin neuestes Image mit Xcode 26 nutzen; App-Ziel macOS 26 lässt sich dort bauen, nur nicht ausführen – dann UI-Tests erst, wenn das Image da ist bzw. auf echtem Mac |
