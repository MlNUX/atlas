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
| 2 | Dateioperationen und Dateisystem-Watcher | offen |
| 3 | Tabs, Splits und Sitzungsmodell | offen |
| 4 | PDF-Anzeige und Schnellansicht | offen |
| 5 | Markdown sowie Metadaten und Linkverhalten | offen |
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
