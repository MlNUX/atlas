<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Erweiterungen

> Ausführliche Anleitung mit Laufzeitvertrag, Brücken-Referenz, Debugging
> und einem vollständigen Beispiel in Rust:
> [extensions-guide.md](extensions-guide.md).

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

**Deinstallieren** („Deinstallieren …“ im Erweiterungsdialog oder in den
Einstellungen): Der Ordner der Erweiterung kommt in den Papierkorb (von
dort wiederherstellbar), ihre Einstellungen, gespeicherten Geheimnisse und
ihre Freigabe löscht Atlas. Dateien, die sie in Projekten angelegt hat
(z. B. die Stapel von bnki-Karten), bleiben. Eine Projekt-Erweiterung
verschwindet damit für alle, die das Projekt teilen.

## Mitgelieferte Erweiterungen

In den Einstellungen (Abschnitt „Erweiterungen“), im Erweiterungsdialog
(„Mitgeliefert …“) und in der Arbeitsleiste installiert ein Klick sie nach
`~/.config/atlas/extensions/<name>/`; erneut installieren aktualisiert die
Dateien, die Einstellungen bleiben. Bringt eine neue Atlas-Version eine
neuere Fassung mit, spielt Atlas sie beim Start selbst ein. Quelle: `crates/atlas-core/bundled/`.

### bnki-Karten

Karteikarten für die bnki-App schreiben, in einem eigenen Tab
(**Strg+Alt+B**, „Öffnen: bnki-Karten“ in der Arbeitsleiste, oder auf einer
CSV-Datei „Anzeigen mit → bnki-Karten“).

- Jeder Stapel ist eine CSV-Datei (`front;back;hint;tags`) im Ordner
  `bnki/` des Projekts (Einstellung „Ordner der Stapel“). Jede Änderung
  wird sofort gespeichert; ein gelöschter Stapel liegt im Papierkorb.
- Vorschau wie in der App: Formeln `$…$` und `$$…$$` (KaTeX von Atlas,
  ohne Internet), Codeblöcke mit ` ``` `.
- Aufs Handy: „Im Explorer“ zeigt die Datei; in bnki „Stapel öffnen →
  CSV importieren“.
- Rechte: nur Dateien im Projekt, kein Netzwerk, keine Programme.

### Jupyter-Notebooks

Notebooks (`.ipynb`) öffnen sich nach der Installation direkt in einer
eigenen Ansicht: Markdown-Zellen mit Formeln, Code mit Syntaxfarben,
Ausgaben mit Tabellen von pandas, Bildern von matplotlib und farbigen
Tracebacks. Ausführen lassen sich einzelne Zellen und alle.

- **Umgebung wählen** (Pille oben rechts, wie die Kernel-Auswahl in VS
  Code; der Punkt zeigt den Zustand des Kernels). Die Liste findet von
  selbst:
  - **Umgebungen:** `.venv`, `venv`, `env` im Notebook-Ordner und darüber
    bis zum Projekt, dazu alle Conda-Umgebungen (conda, mamba oder
    micromamba). Ein Klick bindet sie ein – ipykernel kommt bei Bedarf
    dazu, die Umgebung wird als Kernel angemeldet (`ipykernel install
    --user`) und das Notebook läuft damit.
  - **Aus Datei:** Umgebungsdateien im Conda-Format (`*.yml`, `*.yaml`,
    `*.sec` mit `dependencies:`) und `requirements*.txt` in denselben
    Ordnern. „erstellen“ baut die Umgebung (Conda: `env create`, mit dem
    Namen aus der Datei; requirements: `.venv` daneben), bindet sie ein und
    verwendet sie; „aktualisieren“ spielt die Pakete der Datei erneut ein.
    Mit `nix` (z. B. NixOS) gibt es außerdem **„Mit nix bauen“**: Python in
    der Version der Datei und alle Pakete aus nixpkgs als
    `python3XY.withPackages`, abgelegt als Link unter
    `~/.local/share/atlas/envs/<name>` – läuft unter NixOS ohne nix-ld, mit
    den Versionen von nixpkgs statt der festgelegten.
    Ohne conda, mamba und micromamba bietet die Ansicht an, **micromamba
    einzurichten** (ein Programm unter `~/.local/share/atlas/micromamba`,
    von micro.mamba.pm oder GitHub; die Umgebung bekommt dann genau die
    Python-Version der Datei; geladen mit curl oder wget, sonst mit
    Python) oder die Pakete **mit pip als `.venv-<name>`** zu bauen.
  - Ein Python für neue Umgebungen sucht die Ansicht der Reihe nach:
    Einstellung „Python für neue Umgebungen“, `python3`/`python` im Pfad
    von Atlas, das Python des laufenden Jupyter-Servers (am Prozess
    abgelesen, also auch hinter nix-Hüllskripten), das des laufenden
    Kernels und – mit `nix` – eines aus nixpkgs, bei einer Umgebungsdatei in
    deren Version (`python=3.13` → `nixpkgs#python313`). Pakete installiert
    sie mit dem Python des Kernels selbst.
  - **NixOS:** Ohne nix-ld starten dort keine fremden Programme – weder
    micromamba noch das Python aus Conda noch Wheels mit kompiliertem Code
    (numpy, torch …). Die Ansicht erkennt das, empfiehlt „Mit nix bauen“
    und zeigt für Conda die Einstellung:
    `programs.nix-ld.enable = true;` (mit `programs.nix-ld.libraries`
    für z. B. `stdenv.cc.cc zlib libGL glib`), danach `nixos-rebuild
    switch` und neu anmelden.
  - **Weitere Kernel**, die schon angemeldet sind; ✕ meldet einen ab.
  - **+ Neue .venv** und **Pakete installieren** (pip in die aktive
    Umgebung).

  Was dafür auf dem Rechner laufen muss, läuft als Hintergrundbefehl von
  Atlas (Skript unter `.atlas/cache/jupyter/`, ausgeführt von `job.sh`),
  auch ohne laufenden Jupyter-Server; Fortschritt und Log stehen im Feld
  unten rechts, dort lässt es sich auch abbrechen. conda und micromamba
  sucht die Ansicht im `PATH` von Atlas und an den üblichen Orten
  (`~/miniforge3`, `~/miniconda3`, `~/anaconda3`, `~/micromamba`, `/opt/…`). Das Notebook merkt sich seine Umgebung
  (`metadata.kernelspec`).
- **Server:** Öffnet man ein Notebook, startet die Ansicht den
  Jupyter-Server von selbst im Hintergrund – ohne Terminal, nur auf
  127.0.0.1, Port aus den Einstellungen (8899), mit einem eigenen Token in
  `.atlas/cache/jupyter/token` des Projekts – und er endet mit Atlas. Er
  läuft aus der `.venv` des Projekts, wenn sie Jupyter enthält, sonst mit
  dem Programm aus den Einstellungen („Jupyter-Programm“, Standard
  `jupyter`). Wechselt man das Projekt, löst der neue Server den alten ab.
  Neu starten oder beenden: unten in der Umgebungsauswahl, im Menü ⋯ oder
  „Beenden: Jupyter-Server“ in der Arbeitsleiste. Beim Start zeigt die
  Ansicht, was gerade passiert (mit Sekunden und der letzten Zeile aus dem
  Log des Servers) und meldet „Jupyter-Server gestartet“, sobald der Kernel
  bereit ist; startet er nicht, steht die Meldung aus seinem Log da.
- **Ohne Jupyter:** „Jupyter im Projekt einrichten“ legt eine `.venv` mit
  `jupyter_server` und `ipykernel` an (mit dem Python aus den
  Einstellungen) und startet danach den Server aus ihr.
- **Ausführen:** ▶ links an der Zelle (während sie läuft: ■ unterbricht),
  **Umschalt+Enter** (ausführen und weiter), **Strg+Enter** (ausführen),
  **Alt+Enter** (ausführen, neue Zelle darunter). „Alle ausführen“ läuft der
  Reihe nach und hält beim ersten Fehler; unter jeder Zelle stehen ✓ oder
  ✗ und die Laufzeit. Neustart (auch mit „alle ausführen“ oder „Ausgaben
  löschen“) am Knopf ▾, „ab/bis hier ausführen“ im Menü ⋯.
- **Bearbeiten:** Zellen zwischen zwei Zellen einfügen (+ Code, + Markdown
  beim Überfahren), duplizieren, verschieben, löschen, Art wechseln.
  **Esc** wechselt in den Befehlsmodus wie in Jupyter: **A**/**B** neue
  Zelle darüber/darunter, **D D** löschen, **M**/**Y**/**R** Markdown/Code/Roh,
  **↑**/**↓** wählen, **Enter** bearbeiten. Tab rückt (auch mehrere Zeilen)
  ein, Umschalt+Tab aus. Lange Ausgaben sind eingeklappt. Gespeichert wird
  automatisch im Format von Jupyter (nbformat 4), Ausgaben inklusive.
- Jedes Notebook hat eine Sitzung auf dem Server; öffnet man es wieder,
  läuft derselbe Kernel weiter (Variablen bleiben).
- HTML aus Ausgaben und Markdown wird bereinigt, Skripte laufen nie
  (Widgets von ipywidgets zeigt die Ansicht als Text).
- Die JSON-Datei selbst: „Anzeigen mit → Atlas (Datei selbst)“.
- Rechte: Dateien im Projekt, die Programme `jupyter` und `python3`,
  Netzwerk nur zu 127.0.0.1.

## Format (API 2)

```yaml
atlas_api: 2                  # Pflicht: Version der Erweiterungs-API
name: notizbuch               # Buchstaben, Ziffern, - und _
title: Notizbuch-Werkzeuge
version: 1.2.0
description: Jupyter-Notebooks anzeigen und exportieren.
context: true                 # Karte in der Kontextleiste (Standard: false)

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

### Kontextleiste

Jede aktive Erweiterung mit Ansichten oder Befehlen (in der Arbeitsleiste)
kann als **Karte in der Kontextleiste** erscheinen, mit einem Knopf je
Ansicht und Befehl. Ein- und ausschalten unter Einstellungen →
Kontextleiste („Erweiterung: …“) oder mit × auf der Karte; gespeichert in
`[context.extensions]` der `config.toml`. Mit `context: true` erscheint die
Karte, solange der Nutzer sie nicht ausschaltet (bnki-Karten tut das).
Befehle laufen dort wie über ihr Tastenkürzel: auf dem aktiven Dokument,
wenn sie dazu passen, sonst auf dem Projekt.

### Ansichten

Eine Ansicht ist eine HTML-Seite der Erweiterung, die Atlas als Tab zeigt
– für eigene Editoren und Werkzeuge:

```yaml
views:
  - id: editor
    name: bnki-Karten
    page: editor.html           # im Ordner der Erweiterung
    applies_to: "*.csv"         # optional: „Anzeigen mit“ für solche Dateien
    auto: false                 # true: passende Dateien gleich so öffnen
    keybinding: Ctrl+Alt+B      # optional
```

Geöffnet wird sie über die Arbeitsleiste („Öffnen: …“) oder das Kürzel
auf dem Projektordner, über „Anzeigen mit“ auf einer passenden Datei. Mit
`auto: true` öffnen passende Dateien gleich in der Ansicht (statt im
Editor von Atlas); „Anzeigen mit → Atlas (Datei selbst)“ zeigt dann die
Datei selbst. Die
Seite läuft im Prozess von Chromium (wie die Markdown-Vorschau), unter
`atlas://app/ext/<name>/…`; sie erreicht nur Dateien ihres eigenen Ordners
und das Netz nur mit `network` in den Rechten. Mit Atlas spricht sie über
die Brücke:

```html
<script src="atlas://app/web/extension.js"></script>
<script>
  atlas.ready.then(async (ctx) => {        // {extension, project, file, settings, files, theme}
    const text = await atlas.call("readFile", { path: "notizen/a.md" })
    await atlas.call("writeFile", { path: "export/a.txt", text })
  })
</script>
```

| Anfrage | Ergebnis |
|---|---|
| `context` | Projekt, Datei oder Ordner des Tabs (`file`), Einstellungen (ohne Geheimnisse), `files` |
| `listFiles {dir, pattern?}` | Einträge `{name, path, dir, size, modified}` |
| `readFile {path}` / `writeFile {path, text}` | Text; Schreiben ist atomar und legt Ordner an |
| `exists {path}` | `true`/`false` |
| `renameFile {path, name}` | neuer Pfad |
| `trashFile {path}` | in den Papierkorb |
| `renderMarkdown {text, path?}` | `{html}` wie in der Vorschau von Atlas (Formeln als `span.math[data-tex]`) |
| `atlas.open(path)`, `atlas.reveal(path)`, `atlas.message(text)` | Datei in Atlas öffnen, im Explorer zeigen, Meldung |
| `atlas.runCommand(id, path?)` | einen Befehl dieser Erweiterung starten (wie aus der Arbeitsleiste) |
| `atlas.commandStatus(id)`, `atlas.stopCommand(id)` | Zustand und Ende eines Hintergrundbefehls (`terminal: background`) |

Pfade sind absolut oder relativ zum Projekt. Atlas prüft **jeden** Pfad
gegen `permissions: files` – `project` heißt nur im Projekt (auch nicht
über symbolische Links oder `..` hinaus), `home` im persönlichen Ordner,
`none` gar nicht. Die Farben von Atlas stehen als CSS-Variablen bereit
(`--atlas-editor`, `--atlas-text`, `--atlas-accent`, …), KaTeX unter
`atlas://app/web/vendor/katex/`.

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
- **`files`** und **`network`** gelten für Ansichten: Ihre Seite kommt nur
  an Dateien, die `files` erlaubt, und ins Netz nur mit `network`. Für
  Skripte der Erweiterung sind es Angaben, die sie (noch) überschreiten
  können; eine Sandbox, die sie durchsetzt, ist geplant.

### Befehle

Befehle haben das Format der [Aktionen](actions.md) und zusätzlich

| Feld | Bedeutung |
|---|---|
| `keybinding` | Tastenkürzel, z. B. `Ctrl+Alt+J` (auch `Strg`, `Umschalt`). Es braucht Strg, Alt oder Meta oder eine F-Taste und darf kein Kürzel von Atlas sein. Der Befehl läuft auf dem aktiven Dokument, wenn er dazu passt, sonst auf dem Projekt. Ist das Kürzel schon belegt (eigene Aktionen gehen vor, dann die erste Erweiterung), meldet Atlas das bei der späteren. |
| `menu` | wo der Befehl erscheint: `context` (Kontextmenü), `palette` (Arbeitsleiste), beides (Standard) oder `none` |

`keybinding` und `menu` gelten genauso in `actions.yaml`.

### Einstellungen

Atlas zeigt die Einstellungen in seinen Einstellungen (Abschnitt
„Erweiterungen“) und unter „Erweiterungen“ → „Einstellungen …“ und
speichert nur geänderte Werte in
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
