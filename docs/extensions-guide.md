<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Erweiterungen entwickeln – Handbuch

Dieses Handbuch erklärt Schritt für Schritt, wie eine Erweiterung für Atlas
entsteht, welche Schnittstellen Atlas anbietet und wie sich eigene
Programme (z. B. in Rust) einbinden lassen. Die knappe Formatübersicht
steht in [extensions.md](extensions.md), die Platzhalter und Regeln der
Befehle in [actions.md](actions.md). Alle Angaben hier entsprechen der
Erweiterungs-API **2** (`atlas_api: 2`) und sind am Code geprüft
(`crates/atlas-core/src/extensions.rs`, `actions.rs`,
`crates/atlas-app/web/extension.js`).

**Inhalt**

1. [Was eine Erweiterung kann – und was nicht](#1-was-eine-erweiterung-kann--und-was-nicht)
2. [Die erste Erweiterung in fünf Minuten](#2-die-erste-erweiterung-in-fünf-minuten)
3. [Orte, Laden, Status](#3-orte-laden-status)
4. [Manifest-Referenz](#4-manifest-referenz)
5. [Laufzeitvertrag für Programme](#5-laufzeitvertrag-für-programme)
6. [Ansichten und die JavaScript-Brücke](#6-ansichten-und-die-javascript-brücke)
7. [Rechte und Sicherheit](#7-rechte-und-sicherheit)
8. [Rezept: eine Erweiterung in Rust](#8-rezept-eine-erweiterung-in-rust)
9. [Debuggen und testen](#9-debuggen-und-testen)
10. [Weitergeben](#10-weitergeben)
11. [Grenzen und häufige Fragen](#11-grenzen-und-häufige-fragen)

---

## 1. Was eine Erweiterung kann – und was nicht

Eine Erweiterung ist ein **Ordner mit einer `extension.yaml`**. Sie
beschreibt deklarativ, was sie beiträgt. Eigener Code läuft nie in Atlas,
sondern immer als **eigener Prozess** (ein Programm oder Skript, das die
Erweiterung mitbringt oder das auf dem System installiert ist) oder als
**HTML-Seite** in einer abgeschotteten Ansicht.

| Beitrag | Wo er erscheint | Typischer Zweck |
|---|---|---|
| **Befehl** (`commands`) | Kontextmenü, Arbeitsleiste (Strg+Umschalt+P), Tastenkürzel | etwas mit Dateien tun, Ausgabe sichtbar im Terminal |
| **Dateiverarbeitung** (`processors`) | beim Öffnen einer Datei, „Anzeigen mit …“ | Dateien anzeigen, für die Atlas keine Ansicht hat (als Markdown, Text oder PDF) |
| **Exporter** (`exporters`) | Kontextmenü „Exportieren als …“, Arbeitsleiste | eine Datei in ein anderes Format schreiben |
| **Ansicht** (`views`) | eigener Tab, „Anzeigen mit …“, Tastenkürzel | eigene Oberfläche: Editor, Werkzeug, Dashboard |
| **Einstellungen** (`settings`) | Einstellungen von Atlas → Erweiterungen | Werte, die der Nutzer setzt; an Programme und Ansichten übergeben |
| **Karte in der Kontextleiste** (`context`) | rechte Kontextleiste, Reiter „Aktionen“ | eine Zeile je Ansicht und Befehl der Erweiterung mit Tastenkürzel, ein Klick entfernt |

**Nicht möglich** (Stand API 2):

- die Oberfläche von Atlas selbst verändern (Menüs außer den obigen Plätzen,
  Seitenleisten, Themes, Editorverhalten),
- auf Ereignisse reagieren („Datei gespeichert“, „Projekt geöffnet“) oder
  dauerhaft im Hintergrund laufen,
- Befehle anderer Erweiterungen aufrufen oder interne Daten von Atlas lesen
  (Sitzungen, Index, Annotationen) – außer über die Dateien im Projekt.

---

## 2. Die erste Erweiterung in fünf Minuten

**Schritt 1 – Ordner anlegen.** Am einfachsten über Atlas: Arbeitsleiste →
„Erweiterungen verwalten …“ → **„Neue Erweiterung …“**, einen Titel eingeben.
Atlas legt `~/.config/atlas/extensions/<name>/extension.yaml` aus einer
Vorlage an, in der alle Beispiele auskommentiert sind, und öffnet sie. Von
Hand geht es genauso:

```bash
mkdir -p ~/.config/atlas/extensions/hallo
```

**Schritt 2 – Manifest schreiben** (`~/.config/atlas/extensions/hallo/extension.yaml`):

```yaml
atlas_api: 2
name: hallo
title: Hallo
version: 0.1.0
description: Zählt die Wörter einer Markdown-Datei.

permissions:
  run: [wc]            # Programme, die Befehle starten dürfen
  files: project
  network: false

commands:
  - name: Wörter zählen
    applies_to: "*.md"
    command: [wc, -w, "{file}"]
```

**Schritt 3 – ausprobieren.** Rechtsklick auf eine `.md`-Datei → Aktionen →
„Wörter zählen“. Der Befehl läuft im Terminal von Atlas; die Ausgabe bleibt
stehen, Enter schließt.

Es gibt kein Neuladen und keinen Neustart: Atlas liest Erweiterungen jedes
Mal neu, wenn ein Menü, die Arbeitsleiste oder der Erweiterungsdialog
geöffnet wird. Fehler im Manifest zeigt der Erweiterungsdialog bei der
jeweiligen Erweiterung; die übrigen Einträge bleiben nutzbar.

---

## 3. Orte, Laden, Status

| Ort | Gilt für | Freigabe |
|---|---|---|
| `$XDG_CONFIG_HOME/atlas/extensions/<ordner>/` (meist `~/.config/atlas/extensions/`) | alle Projekte | sofort aktiv |
| `<Projekt>/.atlas/extensions/<ordner>/` | dieses Projekt | erst nach „Prüfen und freigeben …“ |

- Ein Ordner zählt als Erweiterung, wenn er eine `extension.yaml` enthält.
  Er wird nicht rekursiv durchsucht.
- Eine **Projekt-Erweiterung** mit demselben `name` ersetzt eine globale.
- `ATLAS_CONFIG_DIR` verlegt den Konfigurationsordner (nützlich zum Testen
  mit einer leeren Umgebung), `ATLAS_STATE_DIR` den Zustandsordner.

**Status** einer Erweiterung (im Erweiterungsdialog):

| Status | Bedeutung |
|---|---|
| aktiv | alles geladen; Fehler einzelner Einträge stehen trotzdem dabei |
| aus | vom Nutzer ausgeschaltet (`[extensions] disabled` in `~/.config/atlas/config.toml`) |
| nicht freigegeben | Projekt-Erweiterung, noch nicht oder seit einer Änderung nicht mehr freigegeben |
| defekt | Manifest unlesbar, API-Version fehlt oder ist neuer als die von Atlas, ungültiger Name, unlesbare `permissions` |

Was Atlas außerhalb des Ordners über eine Erweiterung speichert:

| Datei | Inhalt |
|---|---|
| `~/.config/atlas/extension-settings.toml` | geänderte Einstellungen, als `[name]`-Tabellen |
| Schlüsselbund / verschlüsselter Speicher | Geheimnisse (`type: secret`) unter `extension-<name>-<schlüssel>` |
| `~/.local/state/atlas/trusted-extensions.json` | Freigaben von Projekt-Erweiterungen |

**Deinstallieren** (Erweiterungsdialog oder Einstellungen) verschiebt den
Ordner in den Papierkorb und löscht Einstellungen, Geheimnisse und
Freigabe. Dateien, die die Erweiterung in Projekten angelegt hat, bleiben.

---

## 4. Manifest-Referenz

YAML, UTF-8. Unbekannte Felder werden ignoriert. Fehlerhafte Einträge
werden übersprungen und gemeldet, der Rest bleibt nutzbar.

### Kopf

| Feld | Pflicht | Wert |
|---|---|---|
| `atlas_api` | ja | `2` (Format 1 hieß `atlas_extension: 1` und läuft weiter). Neuere Versionen lädt Atlas nicht und sagt das. |
| `name` | nein (Standard: Ordnername) | Buchstaben, Ziffern, `-`, `_`; höchstens 64 Zeichen. Eindeutig; Präfix aller IDs. |
| `title` | nein (Standard: `name`) | Anzeigename |
| `version` | nein | Text, frei (z. B. `1.2.0`) |
| `description` | nein | Text für Dialog und Einstellungen |
| `permissions` | bei API 2 sinnvoll | siehe [Rechte](#7-rechte-und-sicherheit); fehlt es, gilt `run: []`, `files: project`, kein Netz |
| `context` | nein (`false`) | `true`: Karte in der Kontextleiste zeigen, bis der Nutzer sie ausschaltet. Ohne `context` kann der Nutzer sie selbst einschalten (Einstellungen → Kontextleiste). Die Karte hat einen Knopf je Ansicht und je Befehl mit `menu: palette`. |

### `permissions`

| Feld | Standard | Wert |
|---|---|---|
| `run` | `[]` | Liste von Programmnamen (ohne Leerzeichen), die Befehle starten dürfen. Eigene Dateien (`{extension}/…`) brauchen keinen Eintrag. Shell-Befehle (`shell: true`) brauchen `sh`. |
| `files` | `project` | `none`, `project`, `home`, `all` |
| `network` | `false` | `true`, `false` oder Liste von Rechnern |

### `settings` (Liste)

| Feld | Pflicht | Wert |
|---|---|---|
| `key` | ja | Kleinbuchstaben, Ziffern, `-`, `_`, höchstens 64 Zeichen, eindeutig |
| `title` | nein | Anzeigename (Standard: `key`) |
| `description` | nein | Hilfetext |
| `type` | nein (`string`) | `string`, `path`, `bool`, `number`, `choice`, `secret` |
| `choices` | bei `choice` | Liste der erlaubten Texte |
| `default` | nein | Standard: `""`, `false`, `0` bzw. erste Auswahl. Muss zum Typ passen. Geheimnisse haben keinen Standard. |

Weitergabe an Programme: als Platzhalter `{setting:key}` (nicht für
Geheimnisse) und als Umgebungsvariable `ATLAS_SETTING_KEY` (Großbuchstaben,
`-` → `_`). Werte kommen immer als Text: `true`/`false`, Zahlen wie `500`
oder `1.5`.

### `commands` (Liste; `actions` geht auch)

Format der [Aktionen](actions.md) plus Tastenkürzel und Menüplatz:

| Feld | Pflicht | Wert |
|---|---|---|
| `name` | ja | Anzeigename |
| `id` | nein | Standard: aus dem Namen |
| `description` | nein | Tooltip |
| `applies_to` | nein | Muster oder Liste (`*.md`, `docs/**/*.pdf`, `{a,b}`), Groß-/Kleinschreibung egal |
| `when` | nein | `file`, `dir`, `project` oder Liste (siehe actions.md) |
| `command` | ja | Liste `[programm, arg, …]` oder Text (wie in einer Shell zerlegt, aber ohne Shell) |
| `shell` | nein (`false`) | `true`: `command` ist ein Text für `sh -c` |
| `cwd` | nein | Arbeitsordner, Platzhalter erlaubt; Standard: Projektordner |
| `confirm` | nein (`false`) | vorher fragen; destruktiv aussehende Befehle fragen immer |
| `terminal` | nein (`panel`) | `panel`, `tab` oder `background` (ohne Terminal, siehe unten) |
| `keybinding` | nein | z. B. `Ctrl+Alt+Y`; braucht Strg, Alt, Meta oder eine F-Taste; kein Kürzel von Atlas |
| `menu` | nein (beides) | `context`, `palette`, beides oder `none` |

### `processors` (Liste)

| Feld | Pflicht | Wert |
|---|---|---|
| `name` | ja | Anzeigename („Anzeigen mit …“) |
| `id` | nein | Standard: aus dem Namen; Teil des Cache-Schlüssels |
| `applies_to` | **ja** | Muster oder Liste |
| `output` | nein (`text`) | `markdown`, `text`, `pdf` |
| `command`, `shell`, `cwd` | | wie bei Befehlen |
| `timeout` | nein (`30`) | Sekunden |
| `auto` | nein (**`true`**) | beim Öffnen passender Dateien ohne eigene Ansicht automatisch verwenden |

Bei `output: pdf` muss der Befehl `{output}` enthalten.

### `exporters` (Liste)

| Feld | Pflicht | Wert |
|---|---|---|
| `name` | ja | Anzeigename („Exportieren als …“) |
| `id` | nein | Standard: aus dem Namen |
| `applies_to` | nein | leer: alle Dateien |
| `target` | **ja** | Endung des Ergebnisses ohne Punkt, z. B. `html` |
| `command` | ja | **muss** `{output}` enthalten |
| `shell`, `cwd` | | wie bei Befehlen |
| `timeout` | nein (`300`) | Sekunden |
| `open` | nein (`true`) | Ergebnis danach in Atlas öffnen |

### `views` (Liste, nur API 2)

| Feld | Pflicht | Wert |
|---|---|---|
| `name` | ja | Tab-Titel, Menüeintrag |
| `id` | nein | Standard: aus dem Namen; der Tab heißt intern `view:<name>/<id>` |
| `page` | ja | HTML-Datei, relativ im Ordner der Erweiterung (kein `..`, nicht absolut, muss existieren) |
| `applies_to` | nein | Muster: „Anzeigen mit …“ für solche Dateien; leer: nur auf dem Projekt |
| `auto` | nein (`false`) | passende Dateien gleich mit der Ansicht öffnen (auch Text-Dateien; „Anzeigen mit → Atlas (Datei selbst)“ zeigt die Datei) |
| `keybinding` | nein | Tastenkürzel |

### Platzhalter

In `command` und `cwd` (siehe auch actions.md):

| Platzhalter | Wert |
|---|---|
| `{file}`, `{files}` | erste bzw. alle ausgewählten Dateien, absolut |
| `{name}`, `{basename}`, `{stem}`, `{ext}` | Dateiname, ohne Endung, Pfad ohne Endung, Endung |
| `{dir}`, `{relative}`, `{project}` | Ordner, Pfad relativ zum Projekt, Projektordner |
| `{line}` | Cursorzeile (Arbeitsleiste), sonst 1 |
| `{extension}` | Ordner der Erweiterung |
| `{output}` | Verarbeitung/Exporter: Datei, in die das Ergebnis geschrieben wird |
| `{setting:key}` | Wert einer Einstellung |

Platzhalter werden **nach** dem Zerlegen eingesetzt: Ein Dateiname mit
Leerzeichen oder `$(…)` bleibt ein einziges Argument und wird nie
ausgewertet. Unbekannte Platzhalter sind ein Fehler.

---

## 5. Laufzeitvertrag für Programme

Das wichtigste Kapitel für Programme in Rust, Python, Go, Shell …: wie Atlas
sie startet und was es von ihnen erwartet.

### Gemeinsam für alle

- **Start ohne Shell.** `command: [prog, a, b]` startet `prog` direkt mit
  den Argumenten. Ein Programm aus der Erweiterung heißt
  `"{extension}/bin/prog"`. Mit `shell: true` läuft der Text über `sh -c`.
- **Programme suchen.** Namen ohne `/` sucht das System im `PATH` von Atlas.
  Startet Atlas über den Desktop, ist das der `PATH` der Sitzung, nicht der
  einer Shell mit eigener `.bashrc`.
- **Arbeitsordner:** `cwd`, sonst der Projektordner, außerhalb eines
  Projekts der Ordner der Datei.
- **Umgebung:** die von Atlas plus `ATLAS_SETTING_<KEY>` für jede Einstellung.
  Weitere Variablen (Pfad der Datei o. Ä.) gibt es nicht: Alles Nötige kommt
  über Platzhalter in die Argumente.
- **Kodierung:** Pfade und Ausgaben als UTF-8 (ungültige Bytes werden
  ersetzt).

### Befehle (`commands`)

- Laufen **sichtbar im integrierten Terminal** (`terminal: panel` oder
  `tab`). stdin, stdout und stderr sind das Terminal: Ausgaben mit Farben
  und Fortschrittsanzeigen gehen, Rückfragen an den Nutzer auch.
- Nach dem Ende zeigt Atlas ✓ (Exit-Code 0) oder ✗ mit dem Code; die Ausgabe
  bleibt stehen, bis der Nutzer Enter drückt.
- Mit `terminal: background` läuft ein Befehl **unsichtbar im Hintergrund**
  – gedacht für Server und Dienste. Atlas startet ihn ohne Terminal (stdin
  leer, stdout und stderr in ein Log unter `~/.cache/atlas/background/`),
  höchstens einmal gleichzeitig, und beendet ihn mit Atlas (SIGTERM an die
  Prozessgruppe, nach ein paar Sekunden SIGKILL; auch wenn Atlas abstürzt).
  In der Arbeitsleiste steht dann „Beenden: <Name>“; eine Ansicht fragt den
  Zustand mit `atlas.commandStatus(id)` ab. Tipp: ein Startskript sollte das
  eigentliche Programm mit `exec` starten.
- Geheimnisse (`type: secret`) bekommen Befehle **nicht**.
- Kein Zeitlimit.

### Dateiverarbeitung (`processors`)

- Läuft **ohne Terminal**: stdin ist leer (`/dev/null`), stdout und stderr
  werden mitgelesen (je höchstens 16 MiB).
- **Ergebnis:** bei `markdown`/`text` die Standardausgabe – oder die Datei
  `{output}`, falls das Programm hineinschreibt. Bei `pdf` muss es nach
  `{output}` schreiben. `{output}` ist eine temporäre Datei im Cache.
- **Erfolg** heißt Exit-Code 0. Sonst zeigt Atlas „„Name“ ist mit Code N
  beendet“ und die **letzten 8 Zeilen von stderr**. Gute Fehlermeldungen
  gehören also nach stderr.
- **Zeitlimit** `timeout` (Standard 30 s); danach wird der Prozess beendet.
- **Cache:** Ergebnisse liegen in `<Projekt>/.atlas/cache/processed/`
  (außerhalb von Projekten in `~/.local/state/atlas/cache/processed/`).
  Der Schlüssel ist ID des Prozessors + Befehl + Einstellungen +
  **Inhalt der Datei**. Eine geänderte Datei wird neu umgewandelt; ein neu
  gebautes Programm dagegen **nicht** – beim Entwickeln den Cache-Ordner
  leeren (siehe [Debuggen](#9-debuggen-und-testen)).
- Geheimnisse kommen als Umgebungsvariable an.

### Exporter (`exporters`)

- Läuft ohne Terminal wie die Dateiverarbeitung; Zeitlimit Standard 300 s.
- `{output}` ist eine **temporäre Datei neben dem Ziel**
  (`.<ziel ohne endung>.atlas-export-<pid>.<target>`). Das Programm muss sie schreiben;
  Atlas benennt sie erst bei Exit-Code 0 in das Ziel um. Ein Abbruch
  hinterlässt nichts Halbes, die Quelldatei bleibt immer unverändert.
- Das Ziel ist `<datei>.<target>` neben der Datei. Gibt es das schon, fragt
  Atlas vor dem Ersetzen. Mit `open: true` öffnet Atlas das Ergebnis danach.
- Fehler wie bei der Dateiverarbeitung (Exit-Code, letzte Zeilen von stderr).
- Geheimnisse kommen als Umgebungsvariable an.

### Kurz: so verhält sich ein gutes Programm

```text
Argumente       → Eingaben (Pfade, Modi, Einstellungen über {setting:…})
ATLAS_SETTING_* → Einstellungen (auch Geheimnisse, nicht bei Befehlen)
stdout          → Ergebnis (Verarbeitung) bzw. Ausgabe (Befehl, im Terminal)
{output}        → Ergebnisdatei (Exporter, PDF-Verarbeitung)
stderr          → verständliche Fehlermeldung, letzte Zeilen werden gezeigt
Exit-Code       → 0 = Erfolg, alles andere = Fehler
```

---

## 6. Ansichten und die JavaScript-Brücke

Eine Ansicht ist eine HTML-Seite der Erweiterung, die Atlas in einem Tab
zeigt (Chromium, wie die Markdown-Vorschau). Sie ist für eigene
Oberflächen gedacht; Rechenarbeit kann sie über einen Befehl einem
Programm überlassen.

### Laden und Abschottung

- Adresse: `atlas://app/ext/<name>/<page>`. Weitere Dateien (CSS, JS,
  Bilder) lädt die Seite relativ dazu aus dem Ordner der Erweiterung.
- **Kein Zugriff auf `file://`.** Dateien des Nutzers gibt es nur über die
  Brücke.
- **Netz** nur mit `network` in den Rechten. Derzeit erlaubt jeder Eintrag
  das ganze Netz; die Liste der Rechner wird gezeigt, aber noch nicht
  einzeln durchgesetzt.
- Neue Fenster sind gesperrt; Links auf `http(s)` öffnet der Systembrowser.
- **F5** lädt die Seite neu. JavaScript-Fehler zeigt Atlas als Hinweis über
  der Seite (mit „Neu laden“).

### Einbinden

```html
<!doctype html>
<meta charset="utf-8">
<link rel="stylesheet" href="style.css">
<script src="atlas://app/web/extension.js"></script>
<script>
  atlas.ready.then(async (ctx) => {
    document.body.style.background = "var(--atlas-editor)"
    const files = await atlas.call("listFiles", { dir: ".", pattern: "*.md" })
    // …
  })
</script>
```

### Kontext (`await atlas.ready`)

| Feld | Inhalt |
|---|---|
| `extension` | Name der Erweiterung |
| `project` | Projektordner (absolut) oder `null` |
| `file` | Datei oder Ordner, auf dem die Ansicht geöffnet wurde |
| `view` | ID der Ansicht (`view:<name>/<id>`) |
| `settings` | Einstellungen als Objekt (ohne Geheimnisse) |
| `files` | angemeldeter Dateizugriff (`none`, `project`, `home`, `all`) |
| `theme` | Farben von Atlas; zusätzlich als CSS-Variablen gesetzt |

### Anfragen (`atlas.call(methode, parameter)` → Promise)

Jede Anfrage liefert ein Ergebnis oder wird mit einem `Error` abgelehnt
(Text der Fehlermeldung).

| Methode | Parameter | Ergebnis |
|---|---|---|
| `context` | – | wie oben, ohne `file`, `view`, `theme` |
| `listFiles` | `dir`, optional `pattern` (Muster wie `*.md`) | Liste `{name, path, dir, size, modified}` (ms seit 1970), nach Namen sortiert; versteckte Einträge fehlen; Ordner kommen unabhängig vom Muster |
| `readFile` | `path` | Text (höchstens 32 MiB, UTF-8) |
| `writeFile` | `path`, `text` | absoluter Pfad; atomar, legt Ordner an, höchstens 32 MiB |
| `exists` | `path` | `true`/`false` |
| `renameFile` | `path`, `name` | neuer absoluter Pfad (nur der Name ändert sich, der Ordner bleibt) |
| `trashFile` | `path` | `true`; in den Papierkorb |
| `renderMarkdown` | `text`, optional `path` (für relative Links) | `{html}` wie in der Markdown-Vorschau; Formeln als `<span class="math" data-tex=…>` zum Setzen mit KaTeX. Das HTML enthält Roh-HTML aus dem Text – vor dem Einfügen bereinigen |
| `atlas.open(path)` | | öffnet die Datei in einem Tab |
| `atlas.runCommand(id, path?)` | | startet den Befehl `id` **dieser** Erweiterung (mit derselben Rückfrage wie aus der Arbeitsleiste); `path` ist die Auswahl. Bei `terminal: background` scheitert er mit der Fehlermeldung, wenn er nicht startet (oder schon läuft) |
| `atlas.commandStatus(id)` | | Hintergrundbefehl `id` dieser Erweiterung: `{running, code, cwd, log}` (Ende des Logs, höchstens 16 KiB) oder `{}`, wenn er seit dem Start von Atlas nicht lief |
| `atlas.stopCommand(id)` | | beendet ihn; `false`, wenn er nicht läuft |
| `atlas.reveal(path)` | | zeigt sie im Explorer |
| `atlas.message(text)` | | kurze Meldung unten im Fenster |

**Pfade** sind absolut oder relativ zum Projekt. Atlas prüft **jeden** Pfad
gegen `permissions: files`: `..` ist verboten, symbolische Links werden vor
der Prüfung aufgelöst. Bei `files: project` ohne geöffnetes Projekt
scheitern alle Dateianfragen.

### Farben

CSS-Variablen auf `<html>`: `--atlas-window`, `--atlas-editor`,
`--atlas-header`, `--atlas-raised`, `--atlas-border`, `--atlas-text`,
`--atlas-strong`, `--atlas-muted`, `--atlas-faint`, `--atlas-accent`,
`--atlas-accent-soft`, `--atlas-ok`, `--atlas-warn`, `--atlas-error`.
KaTeX für Formeln liegt unter `atlas://app/web/vendor/katex/`.

Vollständige Beispiele sind die mitgelieferten Erweiterungen
`crates/atlas-core/bundled/bnki/` (`extension.yaml`, `editor.html`) und
`crates/atlas-core/bundled/jupyter/` (Ansicht mit `auto: true`, ein Server
als Hintergrundbefehl, den die Seite mit `runCommand` startet und mit
`commandStatus` beobachtet, Netz nur zu 127.0.0.1).

---

## 7. Rechte und Sicherheit

| Recht | Durchgesetzt? |
|---|---|
| `run` | **ja**, beim Laden: Ein Befehl, eine Verarbeitung oder ein Exporter darf nur ein Programm aus `run` starten, eine Datei der Erweiterung (`{extension}/…`, ohne `..`) oder ein Programm aus einer Einstellung vom Typ `string`/`path` (deren Standardwert leer oder selbst in `run` sein muss). Sonst wird der Eintrag nicht geladen und als Fehler gezeigt. |
| `files` | **ja** für Ansichten (Brücke). Für Programme nur angezeigt – sie laufen mit den Rechten des Nutzers; eine Sandbox ist geplant. |
| `network` | **ja** für Ansichten (alles oder nichts). Für Programme nur angezeigt. |
| Geheimnisse | nur als Umgebungsvariable, nur für Verarbeitung und Exporter, nie als Platzhalter, nie im Terminal |

**Projekt-Erweiterungen** werden erst nach einer Freigabe aktiv. Die
Freigabe gilt für eine Prüfsumme (BLAKE3) über den **ganzen Ordner**:
Dateien, Ausführrechte, Ziele symbolischer Links. Jede Änderung – auch ein
neu gebautes Binary – sperrt die Erweiterung wieder, bis sie erneut
freigegeben wird. Ordner mit mehr als 10 000 Dateien oder 512 MB lassen
sich nicht prüfen. Globale Erweiterungen hat der Nutzer selbst installiert;
sie sind sofort aktiv.

Faustregeln: so wenige Rechte wie nötig anmelden, Programme in
`{extension}/bin` mitliefern statt beliebige Programme in `run`, Befehle
mit Seiteneffekten mit `confirm: true` versehen.

---

## 8. Rezept: eine Erweiterung in Rust

Das Beispiel liegt fertig in
[`docs/examples/atlas-tabelle/`](examples/atlas-tabelle/): ein einziges
Rust-Programm, das drei Beiträge bedient. Es zeigt CSV-Dateien als Tabelle
(Dateiverarbeitung), zählt Zeilen und Spalten (Befehl) und exportiert HTML
(Exporter), mit zwei Einstellungen.

```text
docs/examples/atlas-tabelle/
├── Cargo.toml        eigenes Paket (eigener [workspace], ohne Abhängigkeiten)
├── src/main.rs       das Programm
├── extension.yaml    das Manifest
└── install.sh        bauen und nach ~/.config/atlas/extensions/tabelle kopieren
```

Installiert sieht die Erweiterung so aus – das Cargo-Projekt muss **nicht**
dort liegen, nur das gebaute Programm:

```text
~/.config/atlas/extensions/tabelle/
├── extension.yaml
└── bin/atlas-tabelle
```

### Das Manifest

```yaml
atlas_api: 2
name: tabelle
title: Tabellen (CSV)
version: 0.1.0
permissions:
  run: []              # nur das eigene Programm in {extension}/bin
  files: project
  network: false
settings:
  - key: trennzeichen
    type: choice
    choices: [auto, komma, semikolon, tab]
    default: auto
  - key: max_zeilen
    type: number
    default: 500
commands:
  - name: Tabelle auswerten
    applies_to: ["*.csv", "*.tsv"]
    command: ["{extension}/bin/atlas-tabelle", statistik, "{file}"]
    keybinding: Ctrl+Alt+Y
processors:
  - name: Tabelle
    applies_to: ["*.csv", "*.tsv"]
    output: markdown
    auto: false        # Atlas zeigt CSV selbst als Text; „Anzeigen mit → Tabelle“
    command: ["{extension}/bin/atlas-tabelle", markdown, "{file}"]
exporters:
  - name: HTML-Tabelle
    applies_to: ["*.csv", "*.tsv"]
    target: html
    command: ["{extension}/bin/atlas-tabelle", html, "{file}", "{output}"]
```

### Das Programm (Ausschnitt)

```rust
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["statistik", file] => statistik(file),          // Befehl → Terminal
        ["markdown", file] => markdown(file),            // Verarbeitung → stdout
        ["html", file, output] => html(file, output),    // Exporter → {output}
        _ => Err("Aufruf: atlas-tabelle statistik|markdown <datei> | html <datei> <ausgabe>".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("atlas-tabelle: {message}");       // Atlas zeigt die letzten Zeilen
            ExitCode::FAILURE
        }
    }
}

/// Einstellungen kommen als Umgebungsvariablen.
fn max_rows() -> usize {
    std::env::var("ATLAS_SETTING_MAX_ZEILEN").ok()
        .and_then(|v| v.parse::<f64>().ok())
        .map_or(500, |v| v.max(1.0) as usize)
}
```

### Bauen und installieren

```bash
cd docs/examples/atlas-tabelle && sh install.sh
```

Danach in Atlas: Rechtsklick auf eine CSV-Datei → „Anzeigen mit → Tabelle“,
„Exportieren als → HTML-Tabelle“, Aktionen → „Tabelle auswerten“ (oder
Strg+Alt+Y). Die Einstellungen stehen unter Einstellungen → Erweiterungen →
„Tabellen (CSV)“.

### Tipps für Rust

- **Ein Binary, mehrere Unterbefehle** (wie oben) hält die Erweiterung
  klein; das Manifest wählt den Modus über das erste Argument.
- **Keine Panics für erwartbare Fehler:** Meldung nach stderr, Exit-Code
  ≠ 0. Ein Panic geht auch (Code 101), die Meldung ist aber weniger
  freundlich.
- **Große Ausgaben:** Für Verarbeitungen mehr als 16 MiB lieber nach
  `{output}` schreiben als nach stdout.
- **Abhängigkeiten** wie `csv`, `serde`, `pulldown-cmark` sind normal
  nutzbar – es ist ein ganz gewöhnliches Cargo-Projekt.
- **Startzeit** zählt bei Verarbeitungen: ein Release-Build
  (`cargo build --release`) startet in Millisekunden.

---

## 9. Debuggen und testen

**Manifest prüfen.** Der Erweiterungsdialog zeigt pro Erweiterung Status,
Fehler, alle Beiträge und die **genauen Befehlszeilen**. Typische Meldungen:
„„prog“ fehlt in „permissions: run““, „der Befehl muss nach {output}
schreiben“, „unbekannte Einstellung {setting:…}“, „… ist schon belegt“ bei
Tastenkürzeln.

**Programm ohne Atlas testen.** Atlas ruft genau das auf, was im Manifest
steht – also lässt es sich im Terminal nachstellen:

```bash
cd ~/mein-projekt
```

```bash
ATLAS_SETTING_TRENNZEICHEN=auto ATLAS_SETTING_MAX_ZEILEN=20 ~/.config/atlas/extensions/tabelle/bin/atlas-tabelle markdown daten.csv; echo "Exit: $?"
```

Für Verarbeitungen gilt dabei: stdin ist leer (`< /dev/null` anhängen),
das Ergebnis ist stdout.

**Cache der Dateiverarbeitung leeren**, nachdem das Programm neu gebaut
wurde (sonst zeigt Atlas das alte Ergebnis, solange sich die Datei nicht
ändert):

```bash
rm -rf ~/mein-projekt/.atlas/cache/processed
```

Alternativ beim Entwickeln die `id` des Prozessors ändern.

**Logs von Atlas.** Atlas schreibt Meldungen nach stderr; ausführlicher
mit `RUST_LOG`:

```bash
RUST_LOG=atlas_core=debug atlas
```

**Ansichten:** F5 lädt neu. `console.error(…)` und nicht abgefangene Fehler
erscheinen als Hinweis über der Seite; `console.log` zeigt Atlas nicht an
(Nachrichten mit dem Präfix `atlas-ext:` sind der Brücke vorbehalten). Zum
Ausprobieren eignet sich `atlas.message(…)`.

**Saubere Testumgebung** ohne die eigenen Einstellungen:

```bash
ATLAS_CONFIG_DIR=/tmp/atlas-test ATLAS_STATE_DIR=/tmp/atlas-state atlas
```

---

## 10. Weitergeben

- **Für alle Projekte:** den Ordner (Manifest + `bin/` + Seiten) als Archiv
  weitergeben; der Empfänger entpackt ihn nach
  `~/.config/atlas/extensions/`.
- **Mit einem Projekt:** den Ordner nach `<Projekt>/.atlas/extensions/`
  legen und mit dem Projekt teilen (Git, Nextcloud …). Jeder, der das
  Projekt öffnet, muss die Erweiterung prüfen und freigeben; nach jeder
  Änderung erneut.
- **Binaries portabel bauen:** dynamisch gelinkte Programme laufen nur mit
  passenden Systembibliotheken (unter NixOS oft gar nicht ohne Weiteres).
  Statisch gelinkt mit musl laufen sie überall auf x86_64-Linux:

  ```bash
  rustup target add x86_64-unknown-linux-musl && cargo build --release --target x86_64-unknown-linux-musl
  ```

- Die **Version** (`version:`) hochzählen; Atlas zeigt sie im Dialog.

---

## 11. Grenzen und häufige Fragen

**Kann meine Erweiterung auf das Speichern einer Datei reagieren?** Nein,
es gibt keine Ereignisse. Ein Befehl mit Tastenkürzel ist der übliche Weg.

**Kann ein Programm Atlas etwas mitteilen (Datei öffnen, Meldung)?** Nur
Ansichten über die Brücke. Ein Exporter kann mit `open: true` sein Ergebnis
öffnen lassen; ein Befehl kann nur ins Terminal schreiben.

**Wie bekommt ein Befehl die aktuelle Cursorzeile?** Über `{line}` (wenn er
über die Arbeitsleiste läuft).

**Warum taucht mein Befehl nicht im Kontextmenü auf?** `applies_to` passt
nicht (Muster ohne `/` prüfen den Dateinamen, mit `/` den Pfad im Projekt),
`when` passt nicht zur Auswahl, `menu: none`/`palette`, oder der Eintrag
wurde wegen eines Fehlers nicht geladen (Erweiterungsdialog).

**Warum zeigt Atlas meine Dateiverarbeitung nicht automatisch?** `auto`
gilt nur für Dateien, die Atlas nicht selbst zeigen kann (Binärdateien
ohne eigene Ansicht) – also nicht für PDF, Markdown, Bilder und alles, was
als Text lesbar ist, z. B. CSV. Für solche Dateien gibt es „Anzeigen mit
…“.

**Darf eine Erweiterung ins Netz?** Ansichten nur mit `network`. Programme
können es technisch, sollen es aber angeben (`network: true` oder die
Rechner), damit der Nutzer es vor der Freigabe sieht.
