<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Erweiterungen

Eine Erweiterung ist ein Ordner mit einer `extension.yaml`. Sie beschreibt
deklarativ, was sie beiträgt und was sie dafür braucht; eigener Code läuft
nur als Befehl (Programm oder Skript im Erweiterungsordner), immer in einem
eigenen Prozess. Die Oberfläche von Atlas lässt sich (noch) nicht
verändern.

| Ort | Gilt für | Freigabe |
|---|---|---|
| `$XDG_CONFIG_HOME/atlas/extensions/<name>/` | alle Projekte | sofort aktiv |
| `.atlas/extensions/<name>/` | dieses Projekt | erst nach Prüfung („Prüfen und freigeben …“) |

Eine Projekt-Erweiterung mit gleichem Namen ersetzt eine globale.
Verwaltung über die Arbeitsleiste („Erweiterungen verwalten …“) oder das
Kontextmenü (Aktionen → Erweiterungen …): Status, Beiträge, die genauen
Befehle, Ein-/Ausschalten (in `config.toml`), Manifest öffnen, neue
Erweiterung aus einer Vorlage anlegen.

## Format (API 2)

```yaml
atlas_api: 2                  # Pflicht: Version der Erweiterungs-API
name: notizbuch               # Buchstaben, Ziffern, - und _
title: Notizbuch-Werkzeuge
version: 1.2.0
description: Jupyter-Notebooks anzeigen und exportieren.

permissions:                  # was die Erweiterung braucht (siehe unten)
  run: [jupyter, pandoc]      # Programme, die ihre Befehle starten
  files: project              # none, project (Standard), home oder all
  network: false              # true oder Liste: [cloud.example.org]

settings:                     # Werte, die du in Atlas einstellst
  - key: format               # Kleinbuchstaben, Ziffern, - und _
    title: Exportformat
    description: Zielformat von „Notebook exportieren“.
    type: choice              # string, path, bool, number, choice, secret
    choices: [html, markdown]
    default: html

commands:                     # wie Aktionen in actions.yaml (docs/actions.md)
  - name: Notebook exportieren
    applies_to: "*.ipynb"
    command: [jupyter, nbconvert, --to, "{setting:format}", "{file}"]
    keybinding: Ctrl+Alt+J    # optional
    menu: [context, palette]  # Standard: beides; none = nur per Kürzel

processors:                   # Dateiverarbeitung
  - name: Notebook
    applies_to: "*.ipynb"
    output: markdown          # markdown, text oder pdf
    command: [jupyter, nbconvert, --to, markdown, --stdout, "{file}"]
    auto: true                # beim Öffnen passender Dateien verwenden
    timeout: 30               # Sekunden

exporters:
  - name: PDF (pandoc)
    applies_to: "*.md"
    target: pdf               # Endung des Ergebnisses
    command: [pandoc, "{file}", -o, "{output}"]
    open: true                # Ergebnis danach in Atlas öffnen
    timeout: 300
```

Erweiterungen im Format 1 (`atlas_extension: 1`, `actions:` statt
`commands:`, ohne `permissions` und `settings`) laufen unverändert weiter;
im Format 1 sind keine Rechte angemeldet, ihre Befehle dürfen alles, was du
darfst. Braucht eine Erweiterung eine neuere API, als Atlas kennt
(`atlas_api: 3`), lädt Atlas sie nicht und sagt das.

### Rechte

`permissions` sagt, was die Erweiterung braucht; Atlas zeigt es in der
Liste und vor der Freigabe einer Projekt-Erweiterung.

- **`run`** wird durchgesetzt: Ein Befehl darf nur ein Programm aus dieser
  Liste starten, eine Datei der Erweiterung (`{extension}/…`) oder ein
  Programm, das du in einer Einstellung (`type: string` oder `path`)
  gewählt hast (`["{setting:python}", …]`; ihr Standardwert muss dann
  selbst in `run` stehen). Shell-Befehle
  (`shell: true`) brauchen `sh` in der Liste. Andere Befehle lädt Atlas
  nicht und meldet sie als Fehler.
- **`files`** und **`network`** sind Angaben der Erweiterung. Ihre Skripte
  können sie (noch) überschreiten; eine Sandbox, die sie durchsetzt, ist
  geplant.

### Befehle

Befehle haben das Format der [Aktionen](actions.md) und zusätzlich

| Feld | Bedeutung |
|---|---|
| `keybinding` | Tastenkürzel, z. B. `Ctrl+Alt+J` (auch `Strg`, `Umschalt`). Es braucht Strg, Alt oder Meta oder eine F-Taste und darf kein Kürzel von Atlas sein. Der Befehl läuft auf dem aktiven Dokument, wenn er dazu passt, sonst auf dem Projekt. Ist das Kürzel schon belegt (eigene Aktionen gehen vor, dann die erste Erweiterung), meldet Atlas das bei der späteren. |
| `menu` | wo der Befehl erscheint: `context` (Kontextmenü), `palette` (Arbeitsleiste), beides (Standard) oder `none` |

`keybinding` und `menu` gelten genauso in `actions.yaml`.

### Einstellungen

Atlas zeigt die Einstellungen unter „Erweiterungen“ → „Einstellungen …“
und speichert nur geänderte Werte in
`~/.config/atlas/extension-settings.toml`. Befehle bekommen jeden Wert als
Platzhalter `{setting:schlüssel}` und als Umgebungsvariable
`ATLAS_SETTING_SCHLÜSSEL` (großgeschrieben, `-` wird `_`).

| `type` | Wert |
|---|---|
| `string`, `path` | Text |
| `bool` | `true` / `false` |
| `number` | Zahl |
| `choice` | einer der `choices` |
| `secret` | Text, verschlüsselt gespeichert wie das Nextcloud-Passwort; nur als Umgebungsvariable und nur für Anzeigen und Exporter, nie als Platzhalter, nie für Befehle im Terminal |

Für alle Befehle gelten die Regeln der Aktionen: Programm und Argumente
ohne Shell, `shell: true` nur ausdrücklich, Platzhalter nach dem Zerlegen
eingesetzt. Zusätzlich gibt es

| Platzhalter | Bedeutung |
|---|---|
| `{output}` | Datei, in die das Ergebnis geschrieben wird (Verarbeitung: optional, Export: Pflicht) |
| `{extension}` | Ordner der Erweiterung, z. B. `["{extension}/konvert.sh", "{file}"]` |
| `{setting:schlüssel}` | Wert einer Einstellung (API 2) |

## Dateiverarbeitung

Öffnet man eine Datei, für die Atlas keine eigene Ansicht hat, und passt
eine Verarbeitung mit `auto: true`, zeigt Atlas deren Ergebnis an – als
Markdown-Vorschau, Text oder PDF, immer nur lesend. Über das Kontextmenü
„Anzeigen mit“ (oder die Arbeitsleiste) lässt sich jede passende
Verarbeitung auch für andere Dateien wählen.

- Ergebnis: die Standardausgabe des Befehls, oder die Datei `{output}`,
  falls der Befehl hineinschreibt (bei `output: pdf` Pflicht).
- Ergebnisse liegen in `.atlas/cache/processed/` (außerhalb von Projekten im
  Zustandsordner) und werden pro Dateiinhalt wiederverwendet; ändert sich
  die Datei, wird neu umgewandelt.
- Befehle laufen ohne Eingabe und werden nach `timeout` Sekunden beendet;
  bei Fehlern zeigt Atlas die letzten Zeilen der Fehlerausgabe.

## Exporter

„Exportieren als“ im Kontextmenü einer Datei (und „Exportieren: …“ in der
Arbeitsleiste) schreibt das Ergebnis neben die Datei
(`<name>.<target>`). Existiert es schon, fragt Atlas vor dem Ersetzen. Der
Befehl schreibt in eine temporäre Datei, die erst bei Erfolg umbenannt
wird; ein abgebrochener Export lässt nichts halb Geschriebenes zurück und
ändert nie die Quelldatei.

## Freigabe von Projekt-Erweiterungen

Erweiterungen, die mit einem Projekt kommen, können beim Öffnen von Dateien
Befehle ausführen. Atlas aktiviert sie deshalb erst, wenn sie geprüft und
freigegeben wurden; der Dialog zeigt dabei alle Befehle. Die Freigabe gilt
für den Ordner und **jede Datei darin** – Manifest, Skripte, Unterordner,
Ausführrechte und das Ziel symbolischer Links, bei einem Link auf einen
Ordner auch alles darin
(`$XDG_STATE_HOME/atlas/trusted-extensions.json`, BLAKE3 über den ganzen
Ordner). Ändert sich irgendetwas davon, ist die Erweiterung wieder
gesperrt, bis sie erneut freigegeben wird; geprüft wird bei jeder
Ausführung. Ordner mit mehr als 10 000 Dateien oder 512 MB lassen sich
nicht prüfen und bleiben aus. Globale Erweiterungen hat der Nutzer selbst
installiert; sie sind sofort aktiv.
