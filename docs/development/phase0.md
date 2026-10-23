<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Phase 0 – Technischer Prototyp: Bericht

Stand: 2026-10-18 · Version 0.0.1 · Plattform: NixOS 26.11, Hyprland (Wayland), Qt 6.11.2, Rust 1.98

Dieses Dokument hält fest, was in Phase 0 gebaut wurde, welche technischen
Fragen aus [Phasenkonzept](../concept/phases.md) beantwortet sind und wie die
Abnahmekriterien geprüft wurden.

## Architektur

```text
crates/
  atlas-core/   Rust, ohne Qt – Dateisystem, Watcher, Suche, PDF, Terminal, Zustand
  atlas-app/    Qt 6/QML-Oberfläche; cxx-qt-Bridges als dünne Adapter auf atlas-core
    src/        QObjects und Item-Modelle (Rust, cxx-qt)
    cpp/        wenige C++-Helfer, die cxx-qt-lib nicht abdeckt (Fontmetrik, Zwischenablage, Fenster-Screenshot)
    qml/        Oberfläche
```

Grundsatz: Alles, was ohne Display testbar ist, liegt in `atlas-core`
(derzeit 33 Unit-Tests). `atlas-app` übersetzt nur zwischen Qt und dem Kern.

### Threading

| Aufgabe | Thread | Ergebnis zurück an Qt |
|---|---|---|
| Verzeichnis auflisten | kurzlebiger Hintergrund-Thread pro Auflistung | `CxxQtThread::queue` → minimaler Diff ins Modell |
| Dateisystem-Watcher | notify (inotify) + Debounce-Thread | `queue` → betroffene Ordner neu auflisten |
| Projektsuche | `rg --json` als Kindprozess, Parser-Thread | Treffer in Stapeln (alle 50 ms) |
| PDF | ein Worker-Thread („Actor“) je Engine, besitzt Bibliothek und Dokumente | `queue` → `QImage` im `PdfPage`-Item |
| Terminal | PTY-I/O-Thread von alacritty_terminal | Redraw-Ereignis (zusammengefasst) → `update()` |
| Projektindex (`.atlas/index.sqlite`) | Hintergrund-Thread beim Öffnen des Projekts | `queue` → Statusleiste, Wiederherstellung verschobener Dokumente |
| Sitzung + Metadaten schreiben | ein Writer-Thread pro Projekt (fsync, SQLite-Transaktion) | – (beim Beenden: `flush`) |

Der Qt-Thread führt nur Diffs, Modell-Signale und das Zeichnen aus.

## Entscheidungen zu „Zu klärende Technik“

### Grenzfläche QML ↔ Rust: cxx-qt 0.10

- QObjects, Item-Modelle (`QAbstractListModel`) und `QQuickPaintedItem`s werden
  direkt in Rust definiert (`#[cxx_qt::bridge]`), QML importiert sie als Modul
  `Atlas`.
- Asynchrone Ergebnisse kommen ausschließlich über `cxx_qt::Threading`
  (`qt_thread().queue(...)`) auf den Qt-Thread; Generationszähler verwerfen
  veraltete Antworten (Projektwechsel, neue Suche, Zoomänderung).
- Eigener C++-Code beschränkt sich auf `cpp/helpers.cpp` (~40 Zeilen).
- Build: `cxx-qt-build` fragt `qmake -query`. Unter Nix liegt jedes Qt-Modul in
  einem eigenen Store-Pfad; die Flake erzeugt deshalb ein zusammengeführtes
  Qt-Präfix mit `qt.conf` (siehe `qtEnv` in `flake.nix`).

### Terminal: alacritty_terminal + eigenes QML-Item

- VT-Emulation, Scrollback und PTY: `alacritty_terminal` 0.26.
- Zeichnen: `TerminalView` (QQuickPaintedItem) malt einen Qt-unabhängigen
  `Frame` aus dem Kern. ASCII-Läufe werden als ein Text gezeichnet, jedes
  andere Zeichen zellgenau, damit Fallback-Fonts (CJK, Emoji) das Raster nicht
  verschieben. Die Zellbreite ist ganzzahlig, die Glyphen werden per
  Letter-Spacing exakt darauf eingepasst.
- Eingabe: Qt-Tastencodes → xterm-Sequenzen (`terminal/input.rs`), inkl.
  Application-Cursor-Mode, Modifier-Kodierung, F-Tasten, Bracketed Paste,
  SGR-/X10-Mausberichte und Alternate Scroll.
- VS-Code-artige Bedienung: Panel mit Tabs (`Strg+Umschalt+``), Splits je Tab,
  Ein-/Ausblenden mit `Strg+``, Kopieren/Einfügen mit `Strg+Umschalt+C/V`,
  Mittelklick fügt ein, Größenänderung wird an die PTY gemeldet, Prozesse laufen
  beim Ausblenden weiter, beendete Shells schließen ihr Terminal.
- Shell: `ATLAS_SHELL`, sonst Login-Shell aus `/etc/passwd`, sonst `$SHELL`.
  (Die Login-Shell hat Vorrang, weil z. B. `nix develop` ein nicht-interaktives
  bash als `$SHELL` exportiert.)
- OSC 52 *Lesen* der Zwischenablage ist bewusst deaktiviert (Exfiltration),
  Schreiben ist erlaubt.

### Wayland/Hyprland

- Ist `WAYLAND_DISPLAY` gesetzt und `QT_QPA_PLATFORM` nicht, setzt Atlas
  `QT_QPA_PLATFORM=wayland` – ohne xcb-Fallback, XWayland ist nie nötig.
- Wayland-`app_id` ist `atlas` (`setDesktopFileName`), passend zur
  mitinstallierten `atlas.desktop` (Hyprland-Fensterregeln: `class:atlas`).
- Die Statusleiste zeigt das aktive QPA-Plugin (grün = `wayland`).
- Geprüft unter Hyprland: `hyprctl clients` meldet `class: atlas`,
  `xwayland: false`.

### PDF: Poppler und PDFium im Vergleich

Beide Engines liegen hinter derselben Schnittstelle (`atlas_core::pdf`), in der
Oberfläche lässt sich die Engine pro Dokument umschalten (Toolbar-Auswahl, mit
Anzeige von Öffnungs-/Renderzeit und Textextraktion).

Messung (`cargo run --release -p atlas-core --example pdf_compare -- --pages 20 --width 1600 …`,
i7-8550U, 1600 px Breite; `paper.pdf` ist das generierte Testdokument, die
übrigen sind TeX-Dokumentationen aus dem Nix-Store):

| Datei | Engine | Seiten | Öffnen ms | Rendern Ø ms | Rendern max ms | Text S.1 ms |
|---|---|---:|---:|---:|---:|---:|
| paper.pdf | Poppler | 40 | 2.5 | 33.8 | 51.3 | 1.7 |
| paper.pdf | PDFium | 40 | 7.6 | 37.5 | 48.1 | 1.5 |
| asymptote.pdf | Poppler | 204 | 100.9 | 15.8 | 27.4 | 3.7 |
| asymptote.pdf | PDFium | 204 | 8.2 | 16.0 | 75.3 | 0.5 |
| eplain.pdf | Poppler | 107 | 30.8 | 15.0 | 26.7 | 1.5 |
| eplain.pdf | PDFium | 107 | 5.0 | 11.7 | 18.9 | 0.2 |
| tug2005.pdf | Poppler | 19 | 69.1 | 431.1 | 846.5 | 5.8 |
| tug2005.pdf | PDFium | 19 | 1.5 | 102.4 | 167.0 | 0.3 |

Bewertung nach den Kriterien aus dem Phasenkonzept:

| Kriterium | Poppler (poppler-glib + cairo) | PDFium (pdfium-render) |
|---|---|---|
| Rendering-Qualität | gleichwertig (Pixelvergleich RMSE ≈ 2–3 %, nur Antialiasing) | gleichwertig |
| Rendering-Tempo | bei typischen Seiten gleichauf; bei bildlastigen Seiten bis 4× langsamer | schneller bei schweren Seiten |
| Öffnen großer Dokumente | langsamer: Seitengrößen nur über Page-Objekte (≈ 0,5 ms/Seite) | sehr schnell |
| API | GObject-API, Text, Links, Annotationen anlegen/speichern | sehr umfangreich (Text mit Zeichenboxen, Suche, Annotationen, Formulare, Speichern) |
| Paketierung NixOS | nixpkgs, aus Quellen | nixpkgs, aus Quellen (gn/ninja) |
| Paketierung Arch | offizielles Paket `poppler-glib` | nur AUR (`pdfium-binaries` o. ä.) |
| Lizenz | GPL-2.0-or-later – kompatibel mit GPL-3.0-or-later | Apache-2.0 / BSD-3-Clause – kompatibel |
| Laufzeit-Kopplung | normal gelinkt | per `dlopen` geladen (Pfad wird beim Nix-Build eingebettet) |

**Empfehlung:** Poppler als Standard-Engine für Phase 1.
Ausschlaggebend sind die Paketierung (offizielles Arch-Paket, normale
Bibliotheksabhängigkeit) und die gleichwertige Darstellung. Die Geschwindigkeitsnachteile
sind durch asynchrones Rendern im Worker-Thread kaum spürbar; das langsamere
Öffnen lässt sich in Phase 1 beheben, indem Seitengrößen schrittweise
nachgeladen werden. Die Engine-Schnittstelle bleibt bestehen, sodass PDFium
optional (z. B. Cargo-Feature) oder bei Problemen mit einzelnen Dokumenten
nachrüstbar bleibt.

> **Entschieden (2026-10-18): Poppler.** Zu Beginn von Phase 1 wurden PDFium,
> die Engine-Umschaltung und das Vergleichsprogramm `pdf_compare` entfernt;
> der Vergleichsstand ist im Commit `14de0dd` erhalten.

Gefundener Fehler beim Vergleich: pdfium-render liefert standardmäßig RGBA
(`FPDF_REVERSE_BYTE_ORDER`); Atlas arbeitet durchgängig mit BGRA. Behoben und
durch einen Test abgesichert, der für beide Engines die Kanalreihenfolge prüft.

## Datenmodell

Umgesetzt nach dem festgelegten Datenmodell aus dem Phasenkonzept (`atlas_core::project`):

```text
Projektarbeit/
├ .atlas/
│  ├ project.toml                     dauerhaft – Formatversion, Projekt-ID, Anlagezeit
│  ├ sessions/<device-id>/active.json dauerhaft – Layout, offene Dokumente, aufgeklappte Ordner
│  ├ metadata.sqlite                  dauerhaft – Dateiidentitäten der Sitzungsdokumente
│  ├ index.sqlite                     rekonstruierbar – Dateiliste (Pfad, Gerät/Inode, Größe, mtime)
│  └ cache/                           rekonstruierbar – derzeit leer
└ … Nutzerdokumente (werden nie verändert)
```

- Alle in `.atlas/` gespeicherten Pfade sind relativ zum Projektordner; ein
  Projekt kann als Ganzes verschoben oder synchronisiert werden. Pfade mit
  `..` oder absolute Pfade aus `.atlas/` werden abgewiesen.
- `<device-id>` = `<hostname>-<8 Hex-Ziffern>`, einmalig erzeugt und unter
  `$XDG_STATE_HOME/atlas/device-id` abgelegt. Dort liegt außerdem nur noch
  `app.json` (zuletzt geöffnetes Projekt) – beides gehört zum Gerät, nicht
  zum Projekt.
- **Sicher und atomar schreiben:** JSON/TOML werden in eine temporäre Datei im
  selben Ordner geschrieben, mit `fsync` gesichert, per `rename` ersetzt und
  der Ordner anschließend ebenfalls gesichert. `metadata.sqlite` nutzt
  Transaktionen mit Rollback-Journal und `synchronous = FULL` (kein WAL, damit
  Sync-Werkzeuge keine halben Zustände aus `-wal`/`-shm` übertragen). Das
  Schreiben läuft in einem eigenen Thread pro Projekt; mehrere wartende
  Speicheraufträge werden zum neuesten zusammengefasst, beim Beenden wird
  geflusht.
- **Index wiederaufbauen:** Beim Öffnen prüft Atlas `index.sqlite`
  (`PRAGMA quick_check`, Schemaversion) und baut ihn im Hintergrund neu auf –
  in eine temporäre Datenbank, die atomar an die Stelle der alten tritt. Ein
  fehlender, veralteter oder beschädigter Index wird so ohne Rückfrage ersetzt;
  die Statusleiste meldet den Vorgang („neu aufgebaut (war beschädigt)“).
  Nutzerdokumente und dauerhafte Daten werden dabei nicht berührt (getestet mit
  Müll-Datei, abgeschnittener Datenbank und gelöschter Datei).
- Dateiidentität: Pfad + Geräte-ID + Inode; Fingerabdruck = Größe + BLAKE3 der
  ersten 64 KiB. Fehlt ein Sitzungsdokument, sucht Atlas es über
  `metadata.sqlite` und den Index (gleiche Inode, sonst gleicher Fingerabdruck
  unter Dateien gleicher Größe) und öffnet es am neuen Ort.
- `--no-session` öffnet ein Projekt, ohne `.atlas/` anzulegen; ist der Ordner
  schreibgeschützt, läuft Atlas ohne Speichern weiter und zeigt den Grund in
  der Statusleiste.
- Offen laut Produktkonzept: die genaue Aufteilung zwischen versionierbaren,
  gerätespezifischen und rekonstruierbaren Dateien (z. B. ob `sessions/` oder
  `metadata.sqlite` in Git gehören). Atlas legt derzeit keine `.gitignore` an.

## Abnahmekriterien

| Kriterium | Ergebnis | Nachweis |
|---|---|---|
| Atlas startet nativ unter Wayland auf NixOS | ✅ | Hyprland: `class: atlas`, `xwayland: false`; Statusleiste „wayland“ |
| Projektordner mit ≥ 10.000 Einträgen lädt ohne blockierende Oberfläche | ✅ | Fixture mit 20.000 Dateien, `flat/` (10.000 Einträge) aufgeklappt: 10.210 Zeilen, größte UI-Blockade 8 ms (Debug-Build) |
| PDF, Markdown und Terminal gleichzeitig in Splits | ✅ | `atlas FIXTURE --open docs/paper.pdf --open notes/README.md` |
| Externe Änderungen erscheinen zuverlässig im Baum | ✅ | Datei angelegt/gelöscht und Ordner angelegt, während Atlas lief → nach kurzer Entprellung (120 ms) im Baum; aufgeklappte Unterordner bleiben erhalten |
| Entwicklung und Build reproduzierbar über die Nix-Flake | ✅ | `nix develop`, `nix build`, `nix run`; Abhängigkeiten über `flake.lock` und `Cargo.lock` fixiert |
| ripgrep: nicht blockierende Suche, `.gitignore` | ✅ | Unit-Tests + UI: 4 erwartete Treffer, `ignored/` und `*.log` korrekt ausgeschlossen; Ergebnisse streamen (`--line-buffered`), 84–114 ms für 20.000 Dateien im Release-Build |
| Vollbild-TUIs, Unicode und Farben im Terminal | ✅ | btop, nano; 16/256/Truecolor, fett/kursiv/unterstrichen/durchgestrichen/invers/gedimmt, CJK, Emoji, Braille, Box-Drawing |
| `.atlas/` sicher und atomar geschrieben; beschädigter oder gelöschter Index wird ohne Verlust von Nutzerdokumenten neu aufgebaut | ✅ | Unit-Tests (`project::*`, `paths::*`); End-to-End: PDF extern verschoben und umbenannt, `index.sqlite` mit Müll überschrieben → Index neu aufgebaut (10.113 Einträge, 99 ms), Dokument am neuen Ort wieder geöffnet, Sitzung aktualisiert |

### Prüfung reproduzieren

```sh
nix develop
cargo test -p atlas-core                                   # Kern-Tests
cargo run -p atlas-core --example fixture -- /tmp/atlas-fixture 10000
cargo run -p atlas-app -- /tmp/atlas-fixture --open docs/paper.pdf --open notes/README.md
# automatisiert, ohne Fenster:
QT_QPA_PLATFORM=offscreen ATLAS_STATE_DIR=/tmp/atlas-state \
  cargo run -p atlas-app -- /tmp/atlas-fixture --search needle --screenshot /tmp/atlas.png
```

Die Test-Hooks `--screenshot`, `--screenshot-delay`, `--quit-after` und
`--search` geben zusätzlich die größte gemessene UI-Blockade aus
(`probe: max UI stall … ms`).

## Bekannte Grenzen und offene Punkte für Phase 1

- Markdown wird mit Qts eingebautem CommonMark/GFM-Renderer angezeigt
  (Tabellen, Codeblöcke, Aufgabenlisten). Syntaxhervorhebung, LaTeX, Mermaid
  und Bearbeiten fehlen noch.
- Markdown- und Textansichten laden bei externen Änderungen nicht automatisch
  neu (Knopf „Neu laden“); der Projektbaum schon.
- Terminal: noch kein Input-Method-Support (IME) für CJK-Eingabe, keine
  Hyperlinks (OSC 8), kein Cursor-Blinken, keine Suche im Scrollback.
  Das Malen nutzt QPainter; bei sehr großen Terminals kann ein
  Scenegraph-/Glyph-Atlas-Renderer nötig werden.
- PDF: keine Miniaturen, kein Inhaltsverzeichnis, keine Suche im Dokument.
- Netzwerkdateisysteme sind nicht getestet (laut Plan nicht Teil der Abnahme).
- Der Watcher beobachtet nur aufgeklappte Ordner. Das ist gewollt (begrenzte
  inotify-Watches), Änderungen in zugeklappten Ordnern werden erst beim
  Aufklappen sichtbar.
- cxx-qt verlangt einen anderen Linker als `ld.bfd`; unter Nix wird das
  gewrappte `ld.gold` aus binutils verwendet (Rust warnt, dass gold veraltet
  ist). Alternative für später: gewrapptes `lld` oder `mold`.
- Arch-`PKGBUILD` folgt laut Plan erst gegen Ende der Stabilisierung.
