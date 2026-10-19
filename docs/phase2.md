<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Phase 2 – Vertiefte Arbeitsabläufe: Fortschritt

Umsetzung nach [PHASES.md](../PHASES.md) und [CALENDAR.md](../CALENDAR.md);
nach jedem Schritt ein Commit.

## Entscheidungen zu Beginn (2026-10-19)

- Phase 2 beginnt auf Wunsch vor dem stabilen Phase-1-Release; die offenen
  Punkte aus [phase1.md](phase1.md) (Durchgang auf echtem Hyprland, Arch-Paket)
  bleiben offen.
- **Annotationen** liegen als JSON-Begleitdatei in `.atlas/annotations/`;
  die Original-PDF wird nie verändert, ein Export erzeugt immer eine Kopie.
- **Aktionen und Erweiterungen** werden in YAML beschrieben (wie im
  Konzept), mit Formatversion.
- **Kalender** als eigener Tab-Typ (Seitenleiste mit Projektbaum bleibt
  sichtbar, Dateien lassen sich auf Termine ziehen); lokale Daten im
  iCalendar-Format.

## Schritte

| # | Schritt | Stand |
|---|---|---|
| 1 | PDF-Annotationen und Zitate | ✅ |
| 2 | Notizverknüpfungen: Wiki-Links, Backlink-Filter, Kontextgraph | ✅ |
| 3 | Benutzerdefinierte Aktionen | ✅ |
| 4 | Kleines Erweiterungssystem | |
| 5 | Projektkalender (lokal) | |
| 6 | Nextcloud/CalDAV-Synchronisierung | |
| 7 | Tests, Dokumentation und Abnahme | |

### Schritt 1 – PDF-Annotationen und Zitate

Kern: `atlas_core::annotations` (Datenmodell, Ablage, Zuordnung, Zitate) und
`atlas_core::pdf` (Textauswahl, PDF-ID, Export) – mit Tests.

- **Werkzeuge im PDF-Viewer**: *Text auswählen* (Ziehen, auch über
  Seitengrenzen und mit automatischem Scrollen; Doppelklick wählt ein Wort,
  Umschalt+Klick erweitert), *Freihand* (Farbe und Strichstärke wählbar),
  *Notiz anheften* und *Lesezeichen* je Seite (Strg+D).
- Nach einer Auswahl erscheint eine kleine Leiste: **Markieren** in fünf
  Farben (Strg+H), Unterstreichen, Durchstreichen, *Markieren und
  kommentieren*, **❝ Zitat** (Strg+Q) und Kopieren (Strg+C).
- Klick auf eine Anmerkung öffnet Kommentar, Farbe, *Zitat in Notiz*,
  *Link kopieren* und *Löschen*. Kommentare erscheinen als 💬 an der
  Markierung.
- Seitenleiste **Anmerkungen**: alle Anmerkungen nach Seite, Klick springt
  hin; Lesezeichen auch in den Seitenminiaturen.
- **Nicht destruktiv**: alles liegt in `.atlas/annotations/` (Format in
  [data-format.md](data-format.md)); die PDF bleibt unverändert.
- **Zuordnung** über Projektpfad, Fingerabdruck (BLAKE3, Größe, Seitenzahl,
  permanente PDF-ID) und Seite: Verschieben/Umbenennen in Atlas passt den
  Pfad an, externes Verschieben wird über den Fingerabdruck erkannt, eine
  veränderte PDF am selben Ort erzeugt einen Hinweis statt stiller
  Verschiebungen.
- **Export** „Als PDF-Kopie exportieren …“ schreibt eine Kopie mit echten
  PDF-Annotationen (Markierungen, Kommentare, Freihand); das Original kann
  nicht als Ziel gewählt werden.
- **Zitat in Markdown**: Blockzitat mit Quelle und Seite,
  `— [paper.pdf, S. 8](/docs/paper.pdf#page=8&annot=<id>)`, optional mit
  Kommentar. Ziel ist eine offene Notiz (am Ende eingefügt), eine beliebige
  Notiz oder eine neue; die zuletzt gewählte steht oben, so dass ein Zitat
  mit Auswahl → Strg+Q → Enter festgehalten ist.
- **Rücksprung**: Der Link öffnet die PDF auf der Seite und hebt die
  Markierung hervor; fehlt die Anmerkung, gilt die Seite. Links folgen
  Umbenennungen in Atlas (Linkaktualisierung aus Phase 1).
- Arbeitsleiste: `@text` sucht in Zitaten, Kommentaren und Lesezeichen
  aller PDFs; längere Suchbegriffe zeigen passende Zitate auch ohne `@`.

Gefunden und behoben: `poppler_path_new_from_array` übernimmt den
Punktepuffer und gibt ihn selbst frei – mit einem Rust-Vektor führte der
Export von Freihandstrichen zu Speicherfehlern. Die Punkte werden jetzt mit
`g_malloc` übergeben, Freihand-Annotationen erst auf die Seite gelegt und
dann mit Strichen gefüllt.

Geprüft: Kern-Tests (Auswahl über eine und zwei Seiten, Export aller Arten,
Original unverändert, Zuordnung nach Verschieben/Kopie/Änderung) und neue
Rauchtest-Schritte: Auswahl → Markierung → Kommentar → Lesezeichen →
Freihand → Zitat mit Quelle und Seite → `@`-Suche → Rücksprung zur Stelle →
Export als Kopie → Umbenennen der PDF (Zitat-Link und Anmerkungen folgen).

### Schritt 2 – Notizverknüpfungen

Kern: `atlas_core::markdown` (Wiki-Links, Bilder und Referenzdefinitionen
als Links, Umschreiben) und `atlas_core::notes` (Linkart, Kontextzeile,
lokaler Graph) – mit Tests.

- **Wiki-Links** `[[/docs/architektur.md]]`, mit Überschrift und Alias
  `[[/docs/architektur.md#Parser|Architektur]]`; `/` ist die Projektwurzel,
  sonst relativ zur Notiz; ohne Endung ist eine Markdown-Datei gemeint.
  In der Vorschau gestrichelt unterstrichen, ohne Alias mit dem Notiznamen
  („architektur › Parser“); defekte Wiki-Links sind wie andere markiert und
  reparierbar.
- **Vervollständigung**: `[[` im Markdown-Editor schlägt Notizen und
  Dateien des Projekts vor (unscharfe Suche wie Strg+P) und setzt den
  wurzelrelativen Pfad samt `]]` ein.
- **Rückverweise mit Filtern** (⇠ in Markdown und PDF): nach Notiz
  gruppiert, je Link Zeile, Kontext (die Zeile des Links) und Ziel
  (Überschrift, Codezeile, Seite, Zitat); Filter nach Freitext, Art
  (Wiki-Links, Markdown-Links, Zitate, mit Seite, auf Überschrift,
  Bilder), Tag (hierarchisch) und Ordner der Quelle.
- **Lokaler Kontextgraph** (◎) für die aktuelle Markdown-Datei oder PDF:
  verlinkte und verlinkende Notizen, PDFs und andere Dateien, wahlweise mit
  Nachbarn der Nachbarn; Farben nach Dateiart, fehlende Ziele
  durchgestrichen, Zitate aus PDFs als gelbe Kanten, Pfeile in
  Linkrichtung. Klick öffnet, Strg-/Mittelklick im Split. Panel, Tab und
  Tiefe werden je Tab in der Session gespeichert.
- **PDFs im Netz**: Notizen, die eine PDF zitieren oder verlinken,
  erscheinen in deren Rückverweisen und Graph.
- **Robuste Linkaktualisierung** beim Verschieben/Umbenennen in Atlas:
  zusätzlich zu Phase 1 auch Wiki-Links (Schreibweise ohne Endung bleibt),
  Bilder `![](…)` und Referenzdefinitionen `[x]: pfad`; Fragmente wie
  `#page=3&annot=…` bleiben erhalten. Die Dateiliste der Arbeitsleiste
  folgt Dateioperationen in Atlas sofort.

Geprüft: Kern-Tests (Wiki-Links, Bilder, Definitionen inkl. Umschreiben,
Graph um Notiz und PDF, Rückverweise mit Art/Kontext) und Rauchtest
(Alias, Überschriftenziel, Rückverweis mit Tags, Graph der PDF mit
Zitatkante, Tiefe 2, Wiki-Link folgt Umbenennung, Kontextpanel in der
Session, Vervollständigung).

### Schritt 3 – Benutzerdefinierte Aktionen

Kern: `atlas_core::actions` (Format, Dateimuster, Platzhalter, Vorbereitung
des Befehls, Heuristik für destruktive Befehle) mit Tests; Format in
[actions.md](actions.md).

- **Deklarativ in YAML** (`version: 1`): `.atlas/actions.yaml` für das
  Projekt, `~/.config/atlas/actions.yaml` für alle Projekte; Projektaktionen
  ersetzen globale mit gleicher `id`. Beim ersten Bearbeiten legt Atlas eine
  kommentierte Vorlage an. Fehler in einzelnen Einträgen werden gemeldet,
  der Rest bleibt nutzbar.
- **Ohne Shell**: Befehle sind Programm + Argumentliste (als Liste oder als
  Text, der nur in Wörter zerlegt wird). `shell: true` schaltet `sh -c`
  ausdrücklich ein; Platzhalter werden dann shell-sicher gequotet.
- **Platzhalter** `{file}` `{files}` `{name}` `{basename}` `{stem}` `{ext}`
  `{dir}` `{relative}` `{project}` `{line}` – nach dem Zerlegen eingesetzt,
  Dateinamen bleiben ein Argument; unbekannte Platzhalter sind ein Fehler.
- **Kontext**: `applies_to` (Muster für Dateiname oder Projektpfad, auch
  mehrere) und `when: file | dir | project`; bei Mehrfachauswahl müssen
  alle Dateien passen.
- **Anzeige** im Kontextmenü (Untermenü „Aktionen“, auch auf freier Fläche
  für Projektaktionen, mit „… bearbeiten“) und in der Arbeitsleiste
  („Aktion: …“ für das aktive Dokument und das Projekt).
- **Sichtbare Ausführung** im Terminal-Panel oder als Terminal-Tab
  (`terminal: tab`): ein fester Wrapper reicht die Argumente unverändert
  durch, lässt die Ausgabe nach dem Ende stehen und zeigt ✓/✗ mit Exit-Code
  in Zeile und Tab-Titel. Aktions-Tabs werden nicht in der Session
  wiederhergestellt.
- **Reproduzierbar**: Arbeitsordner ist standardmäßig der Projektordner,
  abweichend über `cwd` (mit Platzhaltern).
- **Bestätigung** mit dem genauen Befehl und Arbeitsordner bei
  `confirm: true` und immer bei destruktiv wirkenden Befehlen (`rm`, `mv`,
  `dd`, `git clean/reset/push --force`, Umleitungen `>` …).

Geprüft: Kern-Tests (Parsen und Fehler, Muster für Dateien/Ordner/Projekt,
Platzhalter mit `$(…)` und Leerzeichen im Dateinamen, Shell-Quoting,
Heuristik, Wrapper mit Exit-Code, Vorrang der Projektaktionen, Vorlage) und
Rauchtest (Aktionen je Kontext, Programm ohne Shell im Projektordner,
Bestätigungspflicht bei Umleitung, Ausführung im Terminal mit Ergebnis und
Statuszeile).
