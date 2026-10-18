<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Phase 1 – Funktionsfähiger Kern: Fortschritt

Umsetzung in der Reihenfolge aus [PHASES.md](../PHASES.md); nach jedem
Schritt ein Commit und ein Zwischenstand.

## Entscheidungen zu Beginn (2026-10-18)

- PDF-Engine: **Poppler** (Begründung und Messwerte in [phase0.md](phase0.md)).
  PDFium wurde entfernt.
- Markdown-Vorschau: **QtWebEngine** mit lokal gebündeltem KaTeX, Mermaid
  und Syntaxhervorhebung (kommt mit Schritt 5).
- Ein Commit pro Schritt, Zwischenstand nach jedem Schritt.

## Schritte

| # | Schritt | Stand |
|---|---|---|
| 1 | Wayland-Fenster, Projektordner und Verzeichnisbaum | ✅ |
| 2 | Dateioperationen und Dateisystem-Watcher | ✅ |
| 3 | Tabs, Splits und Sitzungsmodell | ✅ |
| 4 | PDF-Anzeige und Schnellansicht | ✅ |
| 5 | Markdown sowie Metadaten und Linkverhalten | ✅ |
| 6 | Codevorschau und externer Editor | offen |
| 7 | Terminal-Integration | offen |
| 8 | Suche und universelle Arbeitsleiste | offen |
| 9 | Berechtigungen, Besitzer, Gruppe und Linux-Politur | offen |
| 10 | Paketierung, Tests und Stabilisierung | offen |

### Schritt 1 – Wayland-Fenster, Projektordner und Verzeichnisbaum

Aus Phase 0 übernommen: natives Wayland-Fenster, Projektordner wählen und
dauerhaft merken, Explorer-Baum mit Watcher, versteckte Dateien ein-/ausblenden.

Neu:

- **Feste Systemorte** („Orte“) oberhalb des Projektbaums: Persönlicher Ordner,
  Schreibtisch, Dokumente, Downloads, Papierkorb – nach XDG-Benutzerordnern,
  nur vorhandene, ohne Doppelte. Einklappbar, keine Favoriten.
- **Dateiliste** (Ordneransicht) als eigener Pane-Typ: Spalten Name, Größe,
  Geändert, Typ; Klick auf den Spaltenkopf sortiert (Ordner bleiben oben),
  Doppelklick/Enter öffnet, Strg/Mittelklick öffnet in neuem Split,
  ↑-Knopf, Alt+↑ oder Rücktaste wechseln in den übergeordneten Ordner.
  Externe Änderungen werden als minimaler Diff übernommen (Scrollposition
  und Auswahl bleiben erhalten). Sortierung und „versteckte Dateien“ werden
  pro Pane in der Sitzung gespeichert.
- Projektbaum: Strg-/Mittelklick auf einen Ordner oder das ☰-Symbol beim
  Überfahren öffnet ihn als Dateiliste.
- Pfade aus `--open` werden normalisiert.

Die Ordner-Tabs mit Vor-/Zurück-Navigation und Breadcrumbs folgen in Schritt 3.

### Schritt 2 – Dateioperationen und Dateisystem-Watcher

Kern: `atlas_core::fileops` (getestet, inkl. Papierkorb-Integrationstest mit
eigenem `XDG_DATA_HOME`).

- **Neue Datei / neuer Ordner** im ausgewählten Ordner (Projektbaum: Ordner
  der Auswahl, sonst Projektwurzel; Dateiliste: angezeigter Ordner). Knöpfe
  „+“ und „📁“ im Kopf des Projektbaums, Kontextmenü. Eine neue Datei wird
  sofort geöffnet.
- **Umbenennen** (F2, Kontextmenü) mit Namensprüfung im Dialog; vorhandene
  Namen werden nie überschrieben.
- **Kopieren, Ausschneiden, Einfügen** (Strg+C/X/V, Kontextmenü). Die
  Zwischenablage nutzt `text/uri-list` sowie die GNOME- und KDE-Kennzeichnung
  für „Ausschneiden“ – Kopieren/Einfügen funktioniert also auch zwischen Atlas
  und Dolphin/Nautilus.
- **Namenskonflikte**: Dialog mit „Ersetzen“ (die vorhandene Datei wandert in
  den Papierkorb), „Beide behalten“ („Bericht (Kopie).pdf“), „Überspringen“.
  Kopieren in denselben Ordner erzeugt immer eine Kopie; Ordner können nicht
  in sich selbst kopiert/verschoben werden.
- **Verschieben** per `rename`, über Dateisystemgrenzen hinweg als Kopie und
  anschließendes Entfernen der Quelle.
- **Löschen nur über den Papierkorb** (Entf, Kontextmenü) nach
  freedesktop.org-Spezifikation (Crate `trash`), inklusive `.Trash-$UID` auf
  anderen Dateisystemen.
- **Papierkorb-Ansicht** (Ort „Papierkorb“): Name, ursprünglicher Ort,
  Löschdatum, Größe; Wiederherstellen (bricht bei Namenskonflikt ab, statt zu
  überschreiben), endgültig löschen und Papierkorb leeren mit Rückfrage.
- **Dateiliste**: Mehrfachauswahl (Strg/Umschalt, Strg+A), Kontextmenü auch
  auf freier Fläche.
- **Offene Panes folgen** Dateien, die Atlas verschiebt oder umbenennt; zeigt
  ein Pane eine gelöschte Datei, erscheint ein Hinweis statt veralteter Inhalte.
- Langlaufende Operationen laufen im Hintergrund mit Fortschritt in der
  Statusleiste; Fehler erscheinen als Dialog.

Die Aktualisierung von Markdown-Links beim Verschieben folgt mit Schritt 5,
Drag-and-drop mit Schritt 3.

### Schritt 3 – Tabs, Splits und Sitzungsmodell

- **Splits mit Tabs** (bis zu vier Splits, horizontal oder vertikal). Tabs für
  Dateien und Ordner; dieselbe Datei darf in mehreren Splits offen sein.
- **Vorschau-Tabs** (kursiv): ein Klick im Explorer öffnet eine Vorschau, die
  beim nächsten Klick ersetzt wird; Doppelklick (Explorer oder Tab) heftet an.
  Strg+W schließt den Tab, Strg+Tab/Strg+Umschalt+Tab wechseln.
- **Temporärer Split** (gelber Rahmen, „Temporär“): Suchtreffer öffnen dort;
  weitere temporäre Öffnungen ersetzen ihn, 📌 heftet ihn an.
- **Breadcrumbs und Vor/Zurück** je Tab (Alt+←/→, Maus-Seitentasten); Klick
  auf einen Pfadteil öffnet den Ordner im selben Tab, Mittelklick in neuem Tab.
- **Drag-and-drop**: Dateien aus Explorer, Dateiliste oder anderen Programmen
  auf einen Split ziehen öffnet sie; am linken/rechten Rand entsteht ein neuer
  Split. Auf Ordner (Explorer, Dateiliste) gezogen werden sie verschoben, mit
  Strg kopiert. Gezogene Dateien lassen sich auch in andere Programme ablegen.
- **Sessions** (Dropdown in der Titelleiste): Standard-Session wird automatisch
  angelegt; neue (leere) Session, Duplizieren (aktueller Zustand), Umbenennen,
  „Jetzt speichern“ und Löschen im Untermenü. Vor jedem Wechsel wird die aktive
  Session gespeichert; die aktive Session wird erst gelöscht, nachdem eine
  andere aktiviert oder angelegt wurde. Ablage: `.atlas/sessions/<device-id>/<id>.json`,
  `active.json` zeigt auf die aktive Session (Phase-0-Format wird übernommen).
- Je Session gespeichert: Splits und Tabs (inkl. Vorschau-Status, PDF-Seite und
  Zoom, Sortierung der Dateilisten), aufgeklappte Ordner, Seitenleiste
  (sichtbar, Breite, Explorer/Suche; Strg+B blendet aus), Suchanfrage und
  Filter, Terminal-Layout mit Arbeitsverzeichnissen (`/proc/<pid>/cwd`).
  Laufende Terminals bleiben beim Session-Wechsel bestehen; neu angelegt werden
  sie beim Start.
- Dateilisten blenden in schmalen Splits Typ- und Datumsspalte aus.

### Schritt 4 – PDF-Anzeige und Schnellansicht

- **Seitenleiste** im PDF-Viewer (☰): Seitenminiaturen und Inhaltsverzeichnis
  (Gliederung des PDFs inkl. benannter Ziele; Klick springt zur Seite).
- **Volltextsuche im Dokument** (Strg+F): alle Treffer werden auf den Seiten
  markiert, der aktuelle hervorgehoben; Enter/F3 und Umschalt+F3 springen
  vor/zurück. Die Suche läuft im PDF-Worker, die Oberfläche bleibt bedienbar.
- **Navigation**: Seitenzahl eingeben, Bild↑/↓, Strg+Pos1/Ende;
  **Zoom**: Strg+Mausrad, Strg++/−, Seitenbreite (Strg+0), ganze Seite.
- **Letzte Seite und Zoom pro Dokument** werden in `.atlas/metadata.sqlite`
  (Tabelle `document_state`, Schema v2 mit Migration) gemerkt und beim
  nächsten Öffnen wiederhergestellt – auch in anderen Sessions. Die Session
  selbst speichert zusätzlich Seite, Zoom und Seitenleiste je Tab.
- Zwei PDFs oder PDF und Markdown nebeneinander: über Splits (Schritt 3).
- **Schnellansicht** (Leertaste in Explorer und Dateiliste): großes Overlay
  ohne Tab und Split; ←/→ blättern durch die Liste, Enter öffnet einen festen
  Tab, Leertaste/Esc schließt.
- Das Test-PDF des Fixture-Generators enthält jetzt ein Inhaltsverzeichnis.

### Schritt 5 – Markdown sowie Metadaten und Linkverhalten

Kern: `atlas_core::markdown` (Rendering, Links, Tags, Umschreiben),
`atlas_core::notes` (Notizindex, Backlinks, Linkreparatur) und
`atlas_core::textfile` (atomares Speichern, Konflikte, Recovery) – alle mit
Tests.

- **Markdown-Ansicht** mit drei Modi: *Bearbeiten*, *Geteilt* (Editor und
  Live-Vorschau nebeneinander, Scrollen synchronisiert) und *Vorschau*. Der
  Modus wird je Tab in der Session gespeichert. Markdown-Dateien sind normale
  Dateien in beliebigen Ordnern; es gibt keinen Notizordner. Neue Dateien
  entstehen im ausgewählten Ordner (Vorgabe „Neue Datei.md“).
- **Vorschau** in QtWebEngine: Tabellen, Aufgabenlisten, Fußnoten,
  Syntaxhervorhebung in Codeblöcken (highlight.js), LaTeX (`$…$`, `$$…$$`,
  KaTeX) und Mermaid-Diagramme (erst bei Bedarf geladen). Alle Bibliotheken
  sind gebündelt (`crates/atlas-app/web/vendor`, Lizenzen dort), die Vorschau
  braucht kein Netz. Seite und lokale Bilder kommen über das eigene Schema
  `atlas://`; eine Content-Security-Policy verhindert, dass HTML in Notizen
  Skripte ausführt.
- **Links**: führender `/` = Projektwurzel, sonst relativ zur Datei;
  `#L120`/`#L120-L145` springt zu Codezeilen, `#page=8` (auch `?page=8`) zu
  PDF-Seiten, sonstige Fragmente zu Überschriften (GitHub-Anker). Klick folgt
  dem Link im selben Tab (Alt+← zurück), Strg-/Mittelklick öffnet einen Split,
  externe Links gehen an den Browser. Nicht auflösbare Links sind rot
  durchgestrichen markiert; die Werkzeugleiste zeigt ihre Anzahl (⚠).
- **Tags** im YAML-Frontmatter (`tags: [Architektur/Backend, rust]`),
  hierarchisch, Vergleich ohne Groß-/Kleinschreibung, Schreibweise bleibt.
  Anzeige in Vorschau und Werkzeugleiste; Abfrage `notesWithTag` (für Suche
  und Filter in Schritt 8).
- **Notizindex** (im Speicher, beim Öffnen des Projekts im Hintergrund aus
  allen Markdown-Dateien gebaut, `.gitignore` beachtet; nach Speichern
  aktualisiert): **Rückverweise** (⇠-Leiste) zeigen, welche Notizen auf die
  Datei verweisen.
- **Verschieben/Umbenennen in Atlas** aktualisiert betroffene Links in allen
  Notizen automatisch (auch relative Links einer verschobenen Notiz selbst);
  Meldung in der Statusleiste. Grundlage ist der Stand auf der Platte; Dateien,
  die sich zwischenzeitlich geändert haben, werden nicht überschrieben.
- **Externe Verschiebungen**: Für defekte Links sucht Atlas (Klick auf ⚠ oder
  den Link) das Ziel über Gerät/Inode aus dem Dateiindex oder einen eindeutigen
  Dateinamen und schlägt die Reparatur in einem Dialog vor. Geändert wird erst
  nach Bestätigung.
- **Editor** (auch für Textdateien): KSyntaxHighlighting (KDE Frameworks, Thema
  „Breeze Dark“), Zeilennummern bei Code, Suche im Dokument (Strg+F), Gehe zu
  Zeile (Strg+G). Ab 2 MiB ohne Hervorhebung, ab 8 MiB nur lesen, ab 64 MiB
  nicht als Text geöffnet.

#### Autosave und Recovery

- Speichern 1 s nach der letzten Änderung und mit Strg+S, außerdem beim
  Schließen eines Tabs, Session-Wechsel und Beenden. Geschrieben wird atomar
  (temporäre Datei, fsync, rename, fsync des Ordners) auf einem
  Hintergrund-Thread; Berechtigungen bleiben erhalten, symbolische Links
  werden zum Ziel durchgeschrieben.
- Während ungespeicherter Änderungen liegt 300 ms nach jeder Eingabe eine
  Recovery-Kopie in `.atlas/recovery/<hash>.json` (außerhalb eines Projekts
  im Atlas-Zustandsordner); nach erfolgreichem Speichern wird sie entfernt.
  Beim nächsten Öffnen bietet Atlas *Wiederherstellen* oder *Verwerfen* an.
  Hat sich die Datei seit der Kopie geändert, landet die Wiederherstellung im
  Konfliktzustand – keine Fassung geht verloren.
- **Externe Änderungen** (Ordner-Watcher, Inhaltsvergleich per BLAKE3):
  unveränderte Dokumente laden still neu; bei ungespeicherten Änderungen hält
  Autosave an und Atlas bietet *Neu laden*, *Meine Fassung behalten* oder
  *Als neue Datei speichern …* an. Gelöschte Dateien: *Erneut speichern* oder
  *Als neue Datei speichern*.
- Schreibgeschützte Dateien (Datei oder Ordner nicht schreibbar, kein gültiges
  UTF-8, zu groß) werden nur angezeigt und nie überschrieben.

Gefunden und behoben: Das Syntax-Highlighting meldet Formatänderungen als
Textänderung – Atlas hätte jede geöffnete Datei sofort neu geschrieben und
Recovery-Kopien verworfen. Es zählen jetzt nur echte Textänderungen.

Geprüft (offscreen): Rendering inkl. KaTeX/Mermaid/Bild, Autosave, Backlinks,
Tags, Überschriften-Link mit Zurück-Navigation, Konflikt bei externer
Änderung, Recovery nach Neustart, CSP blockiert `onerror`-Skripte.
