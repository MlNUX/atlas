<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Eigene Aktionen

Aktionen sind lokale Befehle, die Atlas im Kontextmenü („Aktionen“) und in
der Arbeitsleiste (Strg+Umschalt+P, „Aktion: …“) anbietet und **sichtbar im
integrierten Terminal** ausführt. Nach dem Ende bleibt die Ausgabe stehen;
Zeile und Tab-Titel zeigen ✓ oder ✗ mit Exit-Code, Enter schließt.

## Orte

| Datei | Gilt für |
|---|---|
| `.atlas/actions.yaml` | dieses Projekt (Kontextmenü → Aktionen → „Projektaktionen bearbeiten …“) |
| `$XDG_CONFIG_HOME/atlas/actions.yaml` | alle Projekte („Globale Aktionen bearbeiten …“) |

Beide werden bei jedem Öffnen des Menüs neu gelesen. Eine Projektaktion
ersetzt eine globale mit gleicher `id`. Fehlerhafte Einträge werden
übersprungen und gemeldet; die übrigen bleiben nutzbar.

## Format (Version 1)

> Markdown → PDF und TeX → PDF sind eingebaut („Als PDF exportieren“,
> „Zu PDF übersetzen“ im Kontextmenü und in der Kontextleiste). Das
> Beispiel unten mit pandoc braucht zusätzlich eine LaTeX-Installation
> (`pdflatex`); ohne sie bricht pandoc mit „pdflatex not found“ ab.

```yaml
version: 1
actions:
  - name: Markdown als PDF exportieren      # Pflicht, Anzeige
    id: pdf-export                          # optional, sonst aus dem Namen
    description: Mit pandoc neben die Datei  # optional, Tooltip
    applies_to: "*.md"                      # Muster oder Liste von Mustern
    when: file                              # file, dir, project (oder Liste)
    command: [pandoc, "{file}", -o, "{stem}.pdf"]
    cwd: "{dir}"                            # Standard: Projektordner
    confirm: false                          # vor dem Start fragen
    terminal: panel                         # panel (Standard) oder tab
```

### Befehle

- **Liste**: `command: [programm, arg1, "{file}"]` – Programm und Argumente
  werden direkt gestartet, ohne Shell.
- **Text**: `command: pandoc {file} -o "{stem}.pdf"` – wird wie von einer
  Shell in Wörter zerlegt (Anführungszeichen, `\`), aber **ohne** Shell
  ausgeführt: keine Variablen, Umleitungen, Pipes oder Globs.
- **Shell nur ausdrücklich**: mit `shell: true` läuft der Text über
  `sh -c`; Platzhalter werden dann shell-sicher gequotet eingesetzt.

```yaml
  - name: Bilder verkleinern
    applies_to: ["*.png", "*.jpg"]
    shell: true
    command: "for f in {files}; do convert \"$f\" -resize 50% \"$f\"; done"
    confirm: true
```

### Platzhalter

| Platzhalter | Wert (Beispiel für `/p/docs/Bericht.md`) |
|---|---|
| `{file}` | erste Auswahl, absolut: `/p/docs/Bericht.md` |
| `{files}` | alle ausgewählten Dateien; in Listen ein Argument je Datei |
| `{name}` | `Bericht.md` |
| `{basename}` | `Bericht` |
| `{stem}` | Pfad ohne Endung: `/p/docs/Bericht` |
| `{ext}` | `md` |
| `{dir}` | Ordner der Datei bzw. der gewählte Ordner: `/p/docs` |
| `{relative}` | relativ zum Projekt: `docs/Bericht.md` |
| `{project}` | Projektordner: `/p` |
| `{line}` | Cursorzeile im aktiven Editor (Arbeitsleiste), sonst 1 |

Platzhalter werden **nach** dem Zerlegen eingesetzt: Dateinamen mit
Leerzeichen, Anführungszeichen oder `$(…)` bleiben ein einziges Argument
und werden nie ausgewertet. Unbekannte Platzhalter (`{datei}`) sind ein
Fehler.

### Wann eine Aktion erscheint

- `when: file` (Standard, sobald der Befehl einen Datei-Platzhalter nutzt
  oder `applies_to` gesetzt ist): für ausgewählte Dateien; alle müssen zu
  `applies_to` passen.
- `when: dir`: für einen ausgewählten Ordner.
- `when: project` (Standard ohne Datei-Platzhalter): ohne Auswahl, z. B. im
  Kontextmenü auf freier Fläche oder in der Arbeitsleiste.
- Muster ohne `/` gelten für den Dateinamen (`*.md`), Muster mit `/` für
  den Pfad im Projekt (`docs/**/*.pdf`); `{a,b}` und `**` sind möglich,
  Groß-/Kleinschreibung zählt nicht.

### Bestätigung

`confirm: true` fragt vor jedem Start und zeigt den genauen Befehl samt
Arbeitsordner. Atlas fragt außerdem immer, wenn ein Befehl destruktiv
aussieht (`rm`, `mv`, `dd`, `shred`, `mkfs…`, `chmod`/`chown`,
`git clean`/`reset`/`push --force`, `find -delete`, `rsync --delete`,
Umleitungen `>` in Shell-Befehlen).

### Arbeitsordner

Standard ist der Projektordner – unabhängig davon, wo geklickt wurde –, so
dass eine Aktion immer gleich abläuft. `cwd` kann Platzhalter enthalten
(`cwd: "{dir}"`). Außerhalb eines Projekts gilt der Ordner der Datei.
