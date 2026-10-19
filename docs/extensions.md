<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Erweiterungen

Eine Erweiterung ist ein Ordner mit einer `extension.yaml`. Sie beschreibt
deklarativ, was sie beiträgt; eigener Code läuft nur als Befehl (Programm
oder Skript im Erweiterungsordner). Die Oberfläche von Atlas lässt sich in
dieser ersten Ausbaustufe nicht verändern.

| Ort | Gilt für | Freigabe |
|---|---|---|
| `$XDG_CONFIG_HOME/atlas/extensions/<name>/` | alle Projekte | sofort aktiv |
| `.atlas/extensions/<name>/` | dieses Projekt | erst nach Prüfung („Prüfen und freigeben …“) |

Eine Projekt-Erweiterung mit gleichem Namen ersetzt eine globale.
Verwaltung über die Arbeitsleiste („Erweiterungen verwalten …“) oder das
Kontextmenü (Aktionen → Erweiterungen …): Status, Beiträge, die genauen
Befehle, Ein-/Ausschalten (in `config.toml`), Manifest öffnen, neue
Erweiterung aus einer Vorlage anlegen.

## Format (Version 1)

```yaml
atlas_extension: 1            # Pflicht: Formatversion
name: pandoc-werkzeuge        # Buchstaben, Ziffern, - und _
title: Pandoc-Werkzeuge
version: 1.2.0
description: Word-Dateien anzeigen und Markdown exportieren.

actions:                      # wie in actions.yaml (docs/actions.md)
  - name: Wörter zählen
    applies_to: "*.md"
    command: [wc, -w, "{file}"]

processors:                   # Dateiverarbeitung
  - name: Word-Dokument
    applies_to: ["*.docx", "*.odt"]
    output: markdown          # markdown, text oder pdf
    command: [pandoc, "{file}", -t, gfm]
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

Für alle Befehle gelten die Regeln der Aktionen: Programm und Argumente
ohne Shell, `shell: true` nur ausdrücklich, Platzhalter nach dem Zerlegen
eingesetzt. Zusätzlich gibt es

| Platzhalter | Bedeutung |
|---|---|
| `{output}` | Datei, in die das Ergebnis geschrieben wird (Verarbeitung: optional, Export: Pflicht) |
| `{extension}` | Ordner der Erweiterung, z. B. `["{extension}/konvert.sh", "{file}"]` |

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
für den Ordner und den Inhalt der `extension.yaml`
(`$XDG_STATE_HOME/atlas/trusted-extensions.json`, BLAKE3 des Manifests) –
ändert sich das Manifest, wird erneut gefragt. Globale Erweiterungen hat
der Nutzer selbst installiert; sie sind sofort aktiv.
