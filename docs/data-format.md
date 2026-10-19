<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Von Atlas erzeugte Daten

Atlas verändert Projektdateien nur auf ausdrückliche Aktion (Bearbeiten,
Dateioperationen, Linkaktualisierung nach Verschieben in Atlas, bestätigte
Linkreparatur). Alles Übrige liegt in den unten genannten Orten. Pfade in
Projektdaten sind immer **relativ zur Projektwurzel** und verwenden `/`.

## Im Projekt: `.atlas/`

| Datei | Inhalt | Wiederherstellbar? |
|---|---|---|
| `project.toml` | `format_version`, Projekt-`id` (UUID), `created_unix` | wird bei Fehlen neu angelegt |
| `sessions/<device-id>/<session>.json` | benannte Sessions dieses Geräts | nein (Nutzerzustand) |
| `sessions/<device-id>/active.json` | `{"session": "<id>"}` – aktive Session | ja (Standard-Session) |
| `metadata.sqlite` | Dokumentidentitäten und Anzeigezustand | teilweise (Positionen gehen verloren) |
| `index.sqlite` | Dateiliste mit Gerät/Inode | ja, wird bei jedem Start neu aufgebaut |
| `recovery/<hash>.json` | ungespeicherte Bearbeitungen | nein, bis gespeichert oder verworfen |
| `annotations/<name>-<8 hex>.json` | PDF-Annotationen, eine Datei je PDF | nein (Nutzerdaten) |
| `actions.yaml` | eigene Aktionen des Projekts ([actions.md](actions.md)) | nein (Nutzerdaten) |
| `cache/` | reserviert für abgeleitete Daten | ja |

Alle JSON-/TOML-Dateien werden atomar geschrieben (temporäre Datei, fsync,
rename, fsync des Ordners). `.atlas/` kann in `.gitignore` stehen; Sessions
anderer Geräte liegen in eigenen Unterordnern und stören sich nicht.

### Session (`<session>.json`)

```json
{
  "version": 1,
  "name": "Standard",
  "expanded": ["src", "docs/notes"],
  "documents": ["docs/paper.pdf", "README.md"],
  "layout": { "version": 2, "editor": { "groups": [ … ] }, "terminal": { … }, … }
}
```

`layout` gehört der Oberfläche (Splits, Tabs mit Viewer-Zustand wie
PDF-Seite und Zoom, Markdown-Modus, Seitenleiste, Suche, Terminals mit
Arbeitsordnern). Terminal-Tabs stehen als `terminal:<ordner>#<n>` darin.
Ältere Layouts (Phase 0: `panes`) werden beim Laden übernommen.

### `metadata.sqlite`

`PRAGMA user_version` = Schemaversion (aktuell 2, Migration von 1).

```sql
CREATE TABLE documents (           -- wiederfinden nach externem Verschieben
    path TEXT PRIMARY KEY, dev INTEGER, ino INTEGER,
    size INTEGER, head TEXT,       -- BLAKE3 der ersten 64 KiB
    seen_unix INTEGER);
CREATE TABLE document_state (      -- zuletzt gelesene Position je Dokument
    path TEXT PRIMARY KEY, state TEXT,   -- JSON des Viewers, z. B. {"page":3,"zoom":0}
    updated_unix INTEGER);
```

### `index.sqlite`

```sql
CREATE TABLE files (path TEXT PRIMARY KEY, dev INTEGER, ino INTEGER,
                    size INTEGER, mtime_ns INTEGER, is_dir INTEGER);
```

Beachtet `.gitignore`; wird in eine temporäre Datei gebaut und atomar
ersetzt. Ein beschädigter oder veralteter Index wird ohne Rückfrage neu
gebaut.

### Recovery (`recovery/<hash>.json`)

```json
{ "version": 1, "path": "/abs/pfad/notiz.md", "base_hash": "<blake3>",
  "saved_at": 1792355296, "text": "…" }
```

`<hash>` = die ersten 24 Hex-Zeichen von BLAKE3 des absoluten Pfads.
`base_hash` ist die Fassung auf der Platte, auf der die Bearbeitung beruht;
weicht sie beim Wiederherstellen ab, entsteht ein Konflikt statt eines
stillen Überschreibens.

### PDF-Annotationen (`annotations/*.json`)

Die PDF selbst wird nie verändert. Je annotierter PDF gibt es eine Datei;
sie wird mit der ersten Anmerkung angelegt und mit der letzten entfernt.

```json
{
  "version": 1,
  "document": {
    "path": "docs/paper.pdf",
    "blake3": "<BLAKE3 der ganzen Datei>", "size": 482113, "pages": 12,
    "pdf_id": "<permanente ID aus dem PDF-Trailer, falls vorhanden>"
  },
  "annotations": [
    { "id": "1a2b3c4d5e6f", "kind": "highlight", "page": 3,
      "rects": [[72.0, 140.5, 420.25, 16.0]], "text": "markierter Text",
      "comment": "", "color": "#ffd400", "group": "",
      "created": 1792355296, "modified": 1792355296 },
    { "id": "…", "kind": "ink", "page": 4, "ink": [[[80, 300], [150, 340]]],
      "width": 2.0, "color": "#ff6b6b", "created": …, "modified": … },
    { "id": "…", "kind": "bookmark", "page": 7, "title": "Methodik", … }
  ]
}
```

- `kind`: `highlight`, `underline`, `strikeout` (Textmarkierungen mit
  `rects` je Zeile und `text`), `note` (angehefteter Kommentar, `rects`
  = Symbol), `ink` (Freihandstriche), `bookmark` (Lesezeichen einer Seite).
- `page` ist 1-basiert wie in Links; Koordinaten sind PDF-Punkte, Ursprung
  oben links auf der unrotierten Seite.
- Markierungen über mehrere Seiten sind je Seite ein Eintrag mit gemeinsamer
  `group`.
- **Zuordnung:** zuerst über `path`; ist die PDF dort nicht mehr, über
  `blake3` oder `pdf_id` + `pages` (externes Verschieben – `path` wird dann
  angepasst). Verschieben und Umbenennen in Atlas aktualisiert `path`
  direkt. Weicht `blake3` am gleichen Pfad ab, zeigt Atlas einen Hinweis;
  mit „Verstanden“ wird der Fingerabdruck übernommen.
- **Export** schreibt eine Kopie mit echten PDF-Annotationen
  (Highlight/Underline/StrikeOut, Text, Ink); Lesezeichen haben dort keine
  Entsprechung. Das Original kann nicht als Ziel gewählt werden.
- PDFs außerhalb eines Projekts: gleiche Dateien unter
  `$XDG_STATE_HOME/atlas/annotations/`, `path` ist dann absolut.

## Pro Benutzer

| Ort | Inhalt |
|---|---|
| `$XDG_STATE_HOME/atlas/app.json` | `{"version": 1, "last_project": "/pfad"}` |
| `$XDG_STATE_HOME/atlas/device-id` | `<hostname>-<8 hex>`, einmal erzeugt |
| `$XDG_STATE_HOME/atlas/recovery/` | Recovery für Dateien außerhalb von Projekten |
| `$XDG_CONFIG_HOME/atlas/config.toml` | Einstellungen (siehe unten) |
| `$XDG_CONFIG_HOME/atlas/actions.yaml` | eigene Aktionen für alle Projekte ([actions.md](actions.md)) |

`ATLAS_STATE_DIR` und `ATLAS_CONFIG_DIR` ersetzen die beiden Ordner (Tests).

### `config.toml`

```toml
[editor]
command = "code --goto {file}:{line}:{column}"   # {file} {line} {column} {dir}
terminal = false                                 # im Atlas-Terminal ausführen

[terminal]
shell = "/run/current-system/sw/bin/fish"        # Standard: Login-Shell
```

Unbekannte Schlüssel bleiben beim Speichern aus Atlas erhalten.

## In Notizen

Atlas speichert nichts Verstecktes in Markdown-Dateien. Es liest Tags aus dem
YAML-Frontmatter (`tags: [a, b/c]`) und `title:`, und es schreibt Links nur in
der dokumentierten Form (`/pfad` ab Projektwurzel oder relativ, mit
`#L10-L20`, `#page=3` oder Überschriften-Anker).

**Wiki-Links** (`[[/docs/architektur.md#Parser|Architektur]]`): `/` ist die
Projektwurzel, sonst relativ zur Notiz; Pfade stehen ohne
Percent-Encoding; ohne Endung ist eine Markdown-Datei gemeint
(`[[/docs/architektur]]`). Nach `#` folgt eine Überschrift, nach `|` der
angezeigte Text. Verschieben in Atlas passt Wiki-Links, normale Links,
Bilder und Referenzdefinitionen (`[x]: pfad`) an und behält dabei die
Schreibweise (wurzelrelativ/relativ, mit/ohne Endung) bei.

PDF-Zitate sind normales Markdown; der Link führt auf Seite und Anmerkung
(`#page=8&annot=<id>`). Andere Programme öffnen damit die PDF (die Seite
verstehen viele Betrachter), Atlas springt zusätzlich zur Markierung:

```markdown
> Der markierte Text aus der PDF …
>
> — [paper.pdf, S. 8](/docs/paper.pdf#page=8&annot=1a2b3c4d5e6f)

Optionaler Kommentar
```
