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
| 2 | Notizverknüpfungen: Wiki-Links, Backlink-Filter, Kontextgraph | |
| 3 | Benutzerdefinierte Aktionen | |
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
