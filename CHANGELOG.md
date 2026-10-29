<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Änderungen

Das Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
die Versionen [Semantic Versioning](https://semver.org/lang/de/).

## [Unveröffentlicht]

### Hinzugefügt
- **Erweiterungen, API 2** (`atlas_api: 2`, docs/extensions.md); Format 1
  läuft unverändert weiter:
  - **Rechte** (`permissions`): welche Programme die Erweiterung startet,
    welche Dateien sie braucht, ob sie ins Netz geht. Atlas zeigt sie in der
    Liste und vor der Freigabe; Befehle mit nicht angemeldeten Programmen
    werden abgelehnt.
  - **Einstellungen** mit Typ (Text, Pfad, Schalter, Zahl, Auswahl,
    Geheimnis), die Atlas als Formular zeigt; in Befehlen als
    `{setting:…}` und `ATLAS_SETTING_…`, Geheimnisse verschlüsselt und nur
    in der Umgebung.
  - **Befehle** mit Tastenkürzel (`keybinding`) und Platz im Kontextmenü
    oder in der Arbeitsleiste (`menu`) – auch für eigene Aktionen.
  - Neuere API-Versionen werden erkannt und mit Hinweis nicht geladen.
- **Ansichten** für Erweiterungen (`views`): eine HTML-Seite der
  Erweiterung als Tab, mit einer Brücke zu Atlas (Dateien lesen, schreiben,
  umbenennen, in den Papierkorb, öffnen). Jeder Pfad wird gegen die
  angemeldeten Rechte geprüft; ins Netz nur mit `network`.
- **Mitgelieferte Erweiterungen**, mit einem Klick installiert (in den
  Einstellungen, im Erweiterungsdialog, in der Arbeitsleiste):
  - **bnki-Karten**: Karteikarten für die bnki-App schreiben (Strg+Alt+B) –
    mit Vorschau für Formeln und Code; jeder Stapel ist eine CSV-Datei im
    Projekt, sofort gespeichert und direkt in bnki importierbar.
- Die Einstellungen von Erweiterungen stehen direkt im Einstellungsdialog.
- **Erweiterungen deinstallieren** (Erweiterungsdialog und Einstellungen):
  der Ordner kommt in den Papierkorb, Einstellungen, Geheimnisse und
  Freigabe werden gelöscht.
- **Jupyter 1.6.0:** Nach dem Wechsel auf einen frisch gestarteten Kernel
  (z. B. eine gerade gebaute nix-Umgebung) liefen Zellen nicht: der
  Jupyter-Server leitete Antworten erst nach einem Neuverbinden weiter. Die
  Ansicht prüft jetzt nach dem Verbinden, ob der Kernel antwortet, und
  verbindet sich sonst neu. Ist der Server nicht erreichbar, steht das als
  Hinweis mit „Neu verbinden“ in der Ansicht statt als „Failed to fetch“ in
  Atlas.
- **Jupyter 1.5.0 (NixOS):** Neu **„Mit nix bauen“**: Conda-Umgebungsdateien
  werden mit nix zu `python3XY.withPackages` aus nixpkgs (Python-Version
  der Datei, alle Pakete der Datei), läuft unter NixOS ohne nix-ld und ist
  dort der empfohlene Weg. Das Python des Servers wird am laufenden
  Prozess abgelesen (hinter allen Hüllskripten); fehlt jedes Python, holt
  die Ansicht mit `nix` eins aus nixpkgs, in der Version der
  Umgebungsdatei. NixOS ohne nix-ld wird erkannt: Auswahl und Hilfe zeigen
  den nix-Weg und die nötige Einstellung für Conda, statt micromamba zu
  laden, das nicht startet; ein
  Conda-Programm, das nicht startet, gilt nicht als gefunden. Lange
  Hilfetexte in der Auswahl scrollen (mit „Zurück“).
- **Jupyter 1.4.0:** findet ein Python auch ohne `python3` im Pfad (wie
  oft unter NixOS): Einstellung, Pfad, Python des Servers (aus dessen Log,
  auch hinter nix-Hüllskripten) oder des Kernels; Pakete gehen mit dem
  Python des Kernels, Kernel abmelden braucht kein Jupyter-Programm mehr.
  micromamba wird mit curl/wget geladen. Aufgaben warten aufeinander statt
  abgelehnt zu werden.
- **Hintergrundbefehle neu starten:** Wurde ein Hintergrundbefehl beendet
  und gleich wieder gestartet (z. B. „Jupyter-Server neu starten“), traf
  das SIGKILL für den alten Prozess fünf Sekunden später den neuen.
- **Jupyter 1.3.0:** Rückmeldung beim Start des Servers (Schritte mit
  Sekunden und Log-Zeile, dann „Jupyter-Server gestartet“). Umgebungen
  bauen und suchen läuft jetzt als Hintergrundbefehl von Atlas statt im
  Terminal des Jupyter-Servers – „Kein Terminal: Failed to fetch“ bei
  Jupyter ohne Terminals ist damit weg, und Conda wird auch ohne Server
  gefunden.
- **Jupyter überarbeitet** (Erweiterung 1.2.0): neue Oberfläche mit
  schlanker Werkzeugleiste, Syntaxfarben, Ausführen-Knopf und Laufzeit an
  jeder Zelle, Einfügen zwischen Zellen, Befehlsmodus wie in Jupyter
  (A, B, D D, M, Y) und einklappbaren langen Ausgaben. Statt des Dialogs
  „Kernel …“ eine **Umgebungsauswahl** wie in VS Code: sie findet venvs,
  Conda-Umgebungen, Conda-Umgebungsdateien (`*.yml`, `*.sec`) und
  `requirements.txt`, baut daraus auf Klick die Umgebung, bindet sie als
  Kernel ein und verwendet sie; ohne Conda richtet sie micromamba ein oder
  baut mit pip.
- **Mitgelieferte Erweiterungen aktualisieren sich:** Ist die installierte
  Fassung älter als die in Atlas, erneuert Atlas sie beim Start (die
  Einstellungen bleiben).
- **Hintergrundbefehle:** Befehle von Erweiterungen und Aktionen mit
  `terminal: background` laufen ohne Terminal; Atlas schreibt ihre Ausgabe
  in ein Log, startet jeden höchstens einmal und beendet sie mit Atlas.
  „Beenden: …“ in der Arbeitsleiste stoppt sie; Ansichten sehen ihren
  Zustand über `atlas.commandStatus` und `atlas.stopCommand`.
- **Jupyter-Notebooks** (mitgelieferte Erweiterung „Jupyter-Notebooks“):
  `.ipynb` öffnen sich direkt in einer Notebook-Ansicht – Markdown mit
  Formeln, Ausgaben mit Tabellen, Bildern und farbigen Tracebacks. Einzelne
  Zellen (Umschalt+Enter, Strg+Enter) oder alle ausführen (hält beim ersten
  Fehler), unterbrechen, Kernel neu starten oder wechseln; Zellen
  bearbeiten, hinzufügen, verschieben, löschen. Gespeichert wird
  automatisch im Format von Jupyter. Der Server (`jupyter_server`, nur
  127.0.0.1, eigenes Token) startet beim Öffnen von selbst im Hintergrund
  und endet mit Atlas; fehlt Jupyter, richtet die Ansicht es in einer
  `.venv` im Projekt ein.
  Unter „Kernel …“ legt man virtuelle Umgebungen als Kernel an oder bindet
  vorhandene ein, installiert Pakete und entfernt Kernel – mit
  Live-Ausgabe im Dialog.
- **Ansichten mit `auto: true`** öffnen passende Dateien jetzt immer
  direkt, auch Text-Dateien; „Anzeigen mit → Atlas (Datei selbst)“ zeigt
  die Datei. Neu in der Brücke: `renderMarkdown` (HTML wie in der
  Vorschau) und `atlas.runCommand` (Befehl der eigenen Erweiterung).
- **Erweiterungen in der Kontextleiste:** Jede aktive Erweiterung mit
  Ansichten oder Befehlen kann als Karte in der Kontextleiste erscheinen –
  ein Knopf je Ansicht und Befehl. Schalter unter Einstellungen →
  Kontextleiste oder × auf der Karte (`[context.extensions]`); im Manifest
  zeigt `context: true` die Karte standardmäßig. **bnki-Karten** ist so
  eingestellt (nach „Erweiterung aktualisieren: bnki-Karten“).
- **Handbuch für Erweiterungen** (`docs/extensions-guide.md`): Laufzeitvertrag
  für Programme (Start, Umgebung, Ausgabe, Fehler, Cache), vollständige
  Manifest- und Brücken-Referenz, Rechte, Debugging, Weitergabe; mit einer
  Beispiel-Erweiterung in Rust (`docs/examples/atlas-tabelle/`: CSV als
  Tabelle anzeigen, auswerten, als HTML exportieren).
- **`.atlasnote`-Dateien von Atlas Tablet öffnen** (auch `.atlasnote.zip`):
  Seiten untereinander mit Papier, Handschrift (inkl. Druckstärke),
  Textmarker, Bleistift, Textfeldern, eingefügten Bildern und
  PDF-Hintergründen; Zoom mit Strg+Mausrad. Nur lesend: das Archiv wird
  einmal in `~/.cache/atlas/atlasnote` entpackt und die Seiten werden dort
  gerendert; die Datei selbst bleibt unverändert.

### Sicherheit
- **Freigabe von Projekt-Erweiterungen gilt für den ganzen Ordner**, nicht
  mehr nur für die `extension.yaml`: Ein Skript, das die Erweiterung
  aufruft (`{extension}/…`), ließ sich nach der Freigabe austauschen und
  lief ohne neue Nachfrage. Jetzt sperrt jede Änderung einer Datei, eines
  Ausführrechts oder eines Link-Ziels (bei Links auf Ordner auch deren
  Inhalt) die Erweiterung wieder; geprüft wird
  bei jeder Ausführung. Bereits freigegebene Projekt-Erweiterungen müssen
  einmal neu freigegeben werden.

### Geändert
- **Weniger Arbeitsspeicher:**
  - Geschlossene PDFs werden freigegeben: Schließt der letzte Tab einer
    PDF, schließen alle PDF-Threads das Dokument; jeder hält ohnehin nur
    noch wenige offen (die zuletzt benutzten). Vorher blieb jede einmal
    geöffnete PDF bis zum Beenden bis zu viermal im Speicher.
  - Die Markdown-Vorschau (ein Chromium-Browser mit eigenem Prozess)
    entsteht erst, wenn sie gezeigt wird – Notizen im Bearbeiten-Modus und
    nach dem Start noch nicht angesehene Tabs brauchen keine; nach
    5 Minuten verdeckt wird sie geschlossen und beim Zeigen neu erstellt.
  - Bilder im Bild-Tab werden höchstens in Bildschirmgröße geladen (ein
    Foto mit 24 Megapixeln brauchte sonst rund 92 MiB); das Vergrößern-
    Fenster gibt sein Bild beim Schließen frei.
  - Seitenbilder der PDF-Ansicht: Zwischenspeicher 128 statt 256 MB, ältere
    Fassungen einer geänderten Datei fliegen sofort raus, die Grenze wird
    nie überschritten.
  - PDF-Text der Suche: Zwischenspeicher hält seine 64 MB wirklich ein
    (vorher erst nach dem Einfügen geprüft); pro PDF höchstens 32 MB Text.
  - Das Formelmodell verlässt den Speicher nach 3 Minuten ohne Nutzung.
  - Der Notizindex überspringt Build- und Abhängigkeitsordner
    (`node_modules`, `target`, …) auch ohne `.gitignore`.

## [1.1.2] – 2026-10-27

### Neu
- **Neue Projektsuche** (Strg+Umschalt+F) in einem großen Fenster vor
  Atlas statt in der Seitenleiste: Filter nach Dokumenttyp (Markdown, PDF,
  LaTeX, Code, Text), Groß-/Kleinschreibung beachten oder ignorieren,
  „Ganzes Wort“, regulärer Ausdruck, Dateimuster. Durchsucht jetzt auch den
  **Text aller PDFs** im Projekt (Treffer mit Seitenzahl). Jeder Fund ist
  in der Liste markiert, rechts steht eine Vorschau mit den Zeilen davor
  und danach (bzw. der PDF-Seite); Dateien lassen sich einklappen.
  ↑/↓ wählen, Enter öffnet, Strg+Enter im Split – das gefundene Wort ist
  dann im Dokument markiert: im Editor ausgewählt, in der Markdown-Vorschau
  hervorgehoben, im PDF über die Suche des Viewers auf der richtigen Seite.
- **Formel als LaTeX**: In PDFs mit ∑ einen Rahmen um eine Formel ziehen
  (oder markierten Text über ∑ im Auswahlmenü) – Atlas erkennt sie auf
  deinem Rechner mit dem eingebauten pix2tex-Modell (ONNX Runtime, ohne
  Python), kopiert den LaTeX-Code und zeigt ihn zum Prüfen, auch als
  `$…$`/`$$…$$`. Das Modell (etwa 170 MB) lädt Atlas beim ersten Mal selbst
  ([docs/latex-ocr.md](docs/latex-ocr.md)).
- **ILIAS-Anmeldung merken**: Benutzername und Passwort der Hochschule in
  den Einstellungen speichern – das Passwort nur verschlüsselt mit
  systemd-creds, sonst gar nicht. Läuft die Sitzung ab, meldet Atlas dich
  selbst wieder an; eingesetzt wird nur auf der einmal bestätigten
  Anmeldeseite (z. B. idp.scc.kit.edu) und nur ein Versuch, damit ein
  falsches Passwort nicht in einer Schleife landet.
- **Text der Seite mit LaTeX**: „T“ in der PDF-Leiste gibt die Formeln der
  Seite jetzt als `$…$` bzw. `$$…$$` aus statt als Zeichensalat (`ng2 n`).
  Atlas erkennt sie an den Mathe-Schriften und lässt sie vom eingebauten
  Modell lesen; der übrige Text kommt sofort, mit „Kopieren“-Knopf.

- **PDF präsentieren** wie mit PowerPoint: F5 von vorn, Umschalt+F5 (oder
  F11, ▶ in der PDF-Leiste) ab der aktuellen Seite. Im Vollbild steht immer
  genau eine Seite auf Schwarz; weiter mit Klick, →, ↓, Leertaste, Enter,
  Bild↓ oder Mausrad, zurück mit Rechtsklick, ←, ↑, Rücktaste, Bild↑;
  Pos1/Ende, Seitenzahl + Enter springt; B/W schaltet auf Schwarz/Weiß;
  der Mauszeiger verschwindet nach 2 s. Anmerkungen bleiben sichtbar. Esc
  kehrt zur zuletzt gezeigten Seite in den Tab zurück.

- **Markdown-Editor**:
  - Enter setzt Aufzählungen, Checkboxen (neu offen), Nummerierung und
    Zitate fort; Enter auf einem leeren Punkt beendet die Liste.
  - **Bilder** per Strg+V (auch Bildschirmfotos) oder Ziehen: landen in
    `assets/` des Projekts und werden relativ zur Notiz verlinkt.
  - Menü **„＋ Einfügen“**: Bild, Tabelle, Diagramm (Mermaid-Vorlagen:
    Fluss, Sequenz, Klassen, Zustände, ER, Gantt, Mindmap, Kreis,
    Zeitstrahl) und **Ordnerbaum** aus einem echten Ordner.
  - **#Tags im Text** zählen wie die im YAML-Kopf; `#` schlägt die Tags des
    Projekts vor.
  - **Rechtschreibprüfung** Deutsch und Englisch zugleich, auf dem Rechner
    (Hunspell-Wörterbücher, im Nix-Paket enthalten): rot unterstrichen,
    Rechtsklick für Vorschläge oder „zum Wörterbuch hinzufügen“;
    abschaltbar in den Einstellungen.
- **Markdown-Vorschau**:
  - Codeblöcke mit „Kopieren“, bash-/sh-Blöcke mit „▶ Ausführen“ im
    Terminal von Atlas (im Ordner der Notiz).
  - Überschriften klappen ihren Abschnitt ein und aus.
  - ` ```tree `-Blöcke zeichnen eine eingerückte Liste als Ordnerbaum
    (├── └──).
- **PDF-Zitate**: Formeln in markiertem Text kommen beim „In Notiz
  übernehmen“ als `$…$` in die Notiz. Neues Werkzeug **„Bild als Zitat“**:
  Rahmen um eine Abbildung ziehen, Notiz wählen – das Bild landet in
  `assets/`, darunter als Bildunterschrift der Link auf die Seite.
- **Bilder in der Vorschau** vergrößern: Knopf ⤢ (oder Doppelklick) öffnet
  das Bild in einem Fenster vor Atlas (wie der Graph) – passend eingepasst,
  Mausrad zoomt zur Mausposition, +/−, 1:1, ziehen zum Verschieben,
  Esc schließt.

### Geändert
- **Scrollen wie im Browser** (PDF, Editor, Dateiliste, Projektbaum): Das
  Touchpad bewegt den Inhalt genau mit den Fingern, auch ganz langsame
  Bewegungen unter einem Pixel, einmal pro Bild; beim Loslassen gleitet die
  Ansicht sofort weiter (Wayland meldet das Ende der Geste) und läuft aus
  wie in Firefox – mit Reibung (pro Millisekunde ein fester Anteil der
  Geschwindigkeit, `apz.fling_friction`) statt gleichmäßigem Abbremsen mit
  hartem Halt. Das Mausrad
  scrollt weich statt in Sprüngen. Vorher hielt Atlas Touchpad-Schritte von
  genau 10, 20, 30 … Pixeln für Mausrad-Rasten (Qt meldet unter Wayland
  das 12-Fache der Bewegung als Winkel) und sprang dabei um eine ganze
  Stufe; Schritte unter einem Pixel gingen verloren. Das Abheben der Finger
  (libinput meldet dabei kurz ein „Festhalten“) beendet den Schwung nicht
  mehr; erst ein echtes Auflegen hält ihn an.
- **Suchfenster übersichtlicher**: Groß/klein, ganzes Wort und regulärer
  Ausdruck als kleine Schalter im Suchfeld; die Typfilter zeigen ihre
  Trefferzahl; Dateimuster und Build-Ordner unter „Weitere Filter“. Pro
  Datei erst drei Treffer („+ N weitere“ klappt den Rest auf), alle Dateien
  auf einen Klick ein- und ausklappen, Tastenhilfe in der Fußzeile. „Öffnen“ öffnet den Treffer als
  festen Tab im aktiven Split, „Im Split“ (Strg+Enter) daneben – kein
  temporäres Fenster mehr.
- **PDFs schneller**:
  - Zoomen und Größe ändern ruckeln nicht mehr (die Seite wird skaliert
    statt bei jedem Bild neu gemalt): 16 statt bis zu 115 ms pro Schritt.
  - Seiten werden auf bis zu drei Kernen parallel gerendert, bei Folien mit
    Bildern etwa dreimal so schnell.
  - Zuletzt gezeigte Seiten kommen aus einem Zwischenspeicher (256 MB), beim
    Zurückscrollen ohne neues Rendern.
  - Anmerkungen werden beim Zoomen nicht mehr einzeln neu berechnet, und ein
    Strich, Radieren oder Markieren kostet nur noch die eigene Seite statt
    aller Anmerkungen der PDF.
- Repository aufgeräumt: Konzept und Entwürfe unter `docs/concept/`,
  Phasenberichte unter `docs/development/`, Übersicht in `docs/README.md`.

### Behoben
- Rechtschreibprüfung: keine Flut von „QTextCursor::setPosition: Position …
  out of range“ mehr im Terminal. Beim Laden einer anderen Datei (oder
  derselben nach einer Änderung) wurden kurz die Unterstreichungen des
  alten, längeren Texts gezeichnet.
- PDF im Split: Ein Klick irgendwo in den Split (Leiste, Rand neben den
  Seiten, Knöpfe) gibt die Pfeiltasten der PDF – vorher musste man erst in
  eine Seite klicken, und ein angeklickter Knopf behielt die Tasten.
- **PDF-Präsentation:** Beim ersten Start blieb die Seite manchmal in
  Tab-Größe oben links im schwarzen Vollbild (der Tab setzte die Größe der
  Ansicht zurück). Jetzt wandert nur der sichtbare Inhalt ins
  Präsentationsfenster und füllt es; die Statuszeile ist ausgeblendet.
- **PDF-Zitate:** Als LaTeX erkannte Formeln wurden beim Übernehmen
  maskiert (`\$…\$`) und in der Vorschau nicht gesetzt.
- **Mermaid-Diagramme** passen sich dem dunklen Hintergrund der Vorschau
  an (im PDF-Export weiter hell).
- **Markdown-Vorschau:** Zitate bekommen auch das schließende „”“;
  verschachtelte Checklisten sind deutlicher eingerückt.
- **Tabs:** Beim Wechsel des Tabs blieb der Tastaturfokus im vorherigen Tab
  (z. B. Pfeiltasten im Ordner gingen ins Leere); jetzt bekommt ihn der
  neue Tab – Ordneransicht, Editor, PDF, Terminal
  ([#9](https://github.com/MlNUX/atlas/issues/9)).
- **PDF:** Bei „Ganze Seite“ stimmte die angezeigte Seitenzahl nicht
  (immer 1), weil die schmalere Seite mittig steht.
- **PDF:** Strg+Mausrad scrollte beim Zoomen die Seiten weg; jetzt bleibt
  die Stelle unter dem Mauszeiger stehen, auch auf späteren Seiten.

## [1.1.1] – 2026-10-23

### Neu
- **ILIAS** (z. B. KIT) unter „Orte“: deine Favoriten vom Dashboard als
  Ordner mit allen Unterordnern, Sitzungen und Dateien. Ein Klick fragt
  nach dem Zielordner (vorgeschlagen: wo die Datei schon lag, sonst der
  zuletzt gewählte), lädt die Datei direkt dorthin und öffnet sie; ein
  vergebener Name bekommt eine Nummer. Zeigt ILIAS eine neue Version, steht
  „neue Version“ daneben und der Klick lädt sie neu. Anmelden im
  eingebauten Fenster der Hochschule – Atlas merkt sich nur die Sitzung,
  nie das Passwort. In den Einstellungen: die Adresse des ILIAS deiner
  Hochschule (geprüft, `https://` wird ergänzt), der vorgeschlagene
  Ordner und Abmelden. Ein Protokoll zur Fehlersuche öffnet sich aus der
  ILIAS-Leiste (nur Cookie-Namen, nie Werte).
- Einstellungen: **Kalender für neue Termine** direkt wählbar (bisher nur
  über den Stern im Verbindungsdialog).
- Kontextleiste: **Lernzeit** – ein Pomodoro-Timer fest am unteren Rand
  (auf allen Reitern) mit Ring, Runden und Start/Pause, Neu beginnen,
  Überspringen; 25 Min. lernen, 5 Min. Pause, nach vier Runden 15 Min.,
  Längen einstellbar. Oben in der „Übersicht“ (so heißt der erste Reiter
  jetzt) **Heute** mit den Terminen, Blöcken und Fristen des Tages und
  **Aufgaben** zum Abhaken und Anlegen. Alle drei lassen sich in den
  Einstellungen ein- und ausschalten.
- Markdown → PDF eingebaut („Als PDF exportieren“ im Kontextmenü und in der
  Kontextleiste): gesetzt wie die Vorschau mit Formeln, Diagrammen und
  Code, auf hellem A4-Papier – ohne pandoc oder LaTeX.
- TeX → PDF („Zu PDF übersetzen“ für .tex-Dateien): latexmk, sonst
  pdflatex, im Terminal; Fehler bleiben sichtbar.
- PDF: Pfeiltasten – hoch/runter scrollen, links/rechts blättern.
- PDF-Anmerkungen rückgängig machen und wiederholen: Strg+Z,
  Strg+Umschalt+Z oder Strg+Y und Knöpfe in der PDF-Leiste – für Zeichnen,
  Markieren, Kommentare, Farben, Lesezeichen und Löschen
  ([#6](https://github.com/MlNUX/atlas/issues/6)).
- Natürliche Sortierung in Dateiansicht und Projektbaum: „2.pdf“ vor
  „10.pdf“; schaltbar in den Einstellungen. Die zuletzt gewählte
  Sortierung gilt für neue Ordneransichten
  ([#5](https://github.com/MlNUX/atlas/issues/5)).
- Schwung beim Touchpad-Scrollen: nach schnellem Wischen läuft die Ansicht
  weiter und bremst ab (PDFs, Editor, Projektbaum, Ordner); Finger
  auflegen hält sie an (Wayland-Haltegeste, z. B. Hyprland, KDE, GNOME);
  schaltbar ([#4](https://github.com/MlNUX/atlas/issues/4)).
- PDF-Radierer: neben dem Stift; wer damit über Striche fährt, löscht sie
  ganz. Ein Radier-Zug lässt sich mit einem Strg+Z zurückholen.

### Behoben
- Google-Kalender: neu angelegte Termine erschienen doppelt. Google legt
  einen neuen Termin unter eigenem Namen ab und schickt kein ETag zurück;
  die alte Kopie blieb im Cache. Jetzt gibt es je Termin nur einen Eintrag,
  vorhandene Doppelte verschwinden beim nächsten Abgleich.
- Gelöschte Dateien: ihr Tab schließt sich – beim Löschen in Atlas sofort,
  bei Dateien, die außerhalb gelöscht wurden, nach höchstens zwei Sekunden
  (Tabs mit ungespeicherten Änderungen bleiben offen).
- „Checkliste als Aufgaben übernehmen“ erscheint nur noch bei
  Markdown-Dateien mit Checkliste.
- Markdown-Vorschau blieb nach einem Workspace-Wechsel schwarz, bis man
  scrollte. Jetzt zeigt sie beim Zurückkommen sofort ihren letzten Inhalt
  (Momentaufnahme beim Verlassen), bis Chromium neu gezeichnet hat – ohne
  schwarzes Aufblitzen ([#2](https://github.com/MlNUX/atlas/issues/2)).
- Markdown-Vorschau zeigt keine „#“ mehr vor Überschriften
  ([#3](https://github.com/MlNUX/atlas/issues/3)).
- Markdown-Split: Scrollen in der Vorschau bewegt jetzt auch den Editor,
  die Vorschau läuft im Gleichschritt mit (ohne Nachziehen und Ruckeln),
  und in der Mitte der Ansicht steht derselbe Inhalt links und rechts auf
  gleicher Höhe – auch direkt nach dem Öffnen und nach Größenänderungen.
  Neurendern beim Tippen bewegt den Editor nicht mehr
  ([#7](https://github.com/MlNUX/atlas/issues/7)).
- PDF-Dunkelmodus ruckelte beim Ändern der Fenstergröße: Die Umkehrung
  wird jetzt einmal beim Rendern gemacht statt bei jedem Neuzeichnen.
- Die Beispielaktion „Markdown als PDF exportieren“ (pandoc) scheiterte
  ohne LaTeX-Installation; die Vorlage für neue Aktionsdateien nutzt
  stattdessen Word (pandoc → .docx), PDF gibt es eingebaut.
- Einstellungsfenster scrollt auf kleinen Bildschirmen.
- Ordneransicht: PDF-Vorschaubilder zeigen die erste Seite jetzt über
  Atlas' eigene PDF-Darstellung. Qt's PDF-Bildplugin scheiterte an vielen
  Dateien („Error decoding … Dieser Typ von Bilddaten wird nicht
  unterstützt“) und zeigte dann nur das Symbol.
- Keine „Poppler-CRITICAL … poppler_document_get_page“-Meldungen mehr im
  Terminal, wenn nach einer Seite gefragt wird, die die PDF (nicht mehr)
  hat – z. B. nachdem sie kürzer neu erzeugt wurde.

### Geändert
- ILIAS steht als letzter Eintrag unter „Orte“.
- Keine Vorschau-Tabs mehr: ein einfacher Klick auf eine Datei öffnet sie
  als normalen Tab, der beim nächsten Klick nicht ersetzt wird (bisher erst
  nach Doppelklick). Nur der temporäre Split der Schnellansicht wechselt
  seinen Inhalt weiterhin.

## [1.1.0] – 2026-10-22

Graph fürs ganze Projekt, Verknüpfungen und Ordner-Knoten, eine Session je
Projektordner, Google-Kalender, PDF-Dunkelmodus und Links in PDFs.

Beim Update: Projekte mit mehreren benannten Sessions behalten die zuletzt
benutzte; Erweiterungen aus der alten Vorlage heißen danach „Core“.

### Neu
- Google-Kalender als Alternative zu Nextcloud: Anmeldung im Browser
  (eigener OAuth-Client, Anleitung in docs/google-kalender.md), Termine
  lesen und schreiben, offline wie bei Nextcloud.
- „Verknüpfen mit …“ verbindet zwei Dateien direkt, z. B. zwei PDFs – im
  Kontextmenü, in der Arbeitsleiste und in der Kontextleiste. Im Graph als
  grüne gestrichelte Linie, unter „Verknüpfungen“ mit Liste zum Öffnen und
  Lösen; folgt Umbenennungen.
- Ordner-Knoten: Rechtsklick auf einen Ordner → „Als Ordner-Knoten
  verknüpfen“. Im Graph entsteht ein Knoten mit dem Ordnernamen, an dem
  alle Dateien des Ordners hängen – auch später hinzugefügte.
- Graph in eigenem großen Fenster („Groß anzeigen“).
- Markdown-Editor mit Zeilennummern; Tab/Umschalt+Tab rücken Listenpunkte
  ein und aus; Zeilenumbrüche erscheinen in der Vorschau.
- Ordner lassen sich verknüpfen, auch Ordner-Knoten untereinander.
- PDF-Dunkelmodus (Mond in der PDF-Leiste): Seiten mit umgekehrten Farben
  auf dunkelgrauem Papier, sofort umschaltbar ohne neues Rendern. Gilt für
  alle PDFs und wird in `config.toml` gemerkt.
- Links in PDFs: Verweise innerhalb des Dokuments springen zur Zielstelle,
  Webadressen öffnen sich im Browser (Adresse als Tooltip).

### Behoben
- Markdown-Vorschau: Formeln, die KaTeX anders als LaTeX noch annimmt (z. B.
  `array` mit weniger Spalten als angegeben), schreiben keine Warnung
  „LaTeX-incompatible input“ mehr ins Terminal – bei jeder Aktualisierung.
- PDF-Vorschaubilder in der Dateiansicht haben wieder weißes Papier statt
  eines durchsichtigen Hintergrunds.
- Kalender-Abgleich scheiterte manchmal mit „Interrupted system call“, wenn
  gleichzeitig ein Programm im Terminal endete; solche Anfragen werden jetzt
  wiederholt.
- Terminal: horizontal und vertikal teilen teilen das aktive Terminal.
- PDF springt beim Zoomen mit Strg+Mausrad nicht mehr zur Seite;
  „PDF-Kopie exportieren“ nur mit Anmerkungen.
- Doppelte Aktionen aus neu angelegten Erweiterungen; neue Erweiterungen
  bekommen einen eigenen Namen.
- Nextcloud-Kalender: Fehlerseiten des Servers erscheinen als kurzer Text mit
  Anfrage und Adresse statt als großes HTML; aus dem Browser kopierte
  Server-Adressen und Nextcloud in Unterordnern werden gefunden.
- Wiki-Link-Vorschläge konnten nach dem Neuberechnen des Graphen leer sein:
  Qt verlor bei der schrittweisen Speicherbereinigung Werte; Atlas sammelt
  jetzt in einem Zug.

### Geändert
- Mausrad in PDFs scrollt deutlich weiter pro Raste (Touchpads etwas
  schneller als bisher).
- Erweiterungen aus der alten Vorlage hießen alle „Beispiel-Erweiterung“;
  sie heißen jetzt „Core“, „Core 2“ … (Ordner und Name werden beim Start
  umbenannt, ausgeschaltete bleiben aus). Das Erweiterungsfenster ist neu
  gestaltet wie die Einstellungen.
- Aktionen in der Kontextleiste als aufgeräumte Liste („Für diese Datei“,
  „Eigene Aktionen“); nicht verfügbare sagen, warum; nichts ragt mehr über
  den Rand.
- Neues Desktop-Symbol (das „A“ der Titelleiste); Hauptmenü ohne Einträge,
  die es in den Einstellungen gibt.
- Kontextmenü und Hauptmenü gestrafft (ohne Standardprogramm, Dateimanager,
  „Editor teilen“, „Splits untereinander“; „Neu …“ nur auf Ordnern),
  Einstellungsfenster neu gestaltet.
- „Lokaler Graph“ heißt jetzt „Graph“ und zeigt in einer Ansicht das ganze
  Projekt – alle Dateien des Projektordners außer versteckten: die aktuelle
  Datei in der Mitte, verknüpfte Dateien um sie herum, den Rest gleichmäßig
  außen; Zoom mit Strg+Mausrad, Verschieben durch Ziehen.
- Eine Session ist jetzt ein Projektordner: jeder Ordner hat genau eine
  Session (Tabs, Splits, Baum), benannt nach dem Ordner oder mit eigenem
  Namen. Das Sessionfenster listet die Sessions aller geöffneten Ordner,
  ein Klick wechselt dorthin; „Neue Session“ wählt einen Ordner. Bisher
  verschwanden nach „Neue Session“ in einem anderen Ordner die Sessions des
  vorigen aus der Liste und wirkten überschrieben.
- Vor dem Wechsel in einen anderen Projektordner wird die aktuelle Session
  gespeichert.

## [1.0.0] – 2026-10-20

Erste öffentliche Version.

### Arbeitsbereich
- Projektordner mit Verzeichnisbaum, Orten (auf Wunsch genau wie in Dolphin)
  und Dateiansicht mit Symbolen oder Details, Pfadleiste und Zoom.
- Tabs und Splits mit Ziehen und Ablegen; benannte Sessions je Projekt, die
  automatisch gespeichert werden.
- Schnellansicht mit der Leertaste für PDFs, Bilder, Markdown und Code.
- Arbeitsleiste (Strg+P) für Dateien, Aktionen, Tags, Zitate, Termine und
  Volltextsuche; projektweite Suche mit ripgrep (`.gitignore`-bewusst).
- Dateioperationen mit Papierkorb, Eigenschaften sowie Besitzer,
  Gruppe und Unix-Rechten.

### Dokumente
- PDF-Anzeige mit Markierungen, Freihand, Kommentaren und Lesezeichen. Die
  PDF selbst bleibt unverändert (Anmerkungen liegen in `.atlas/`), Export als
  PDF-Kopie mit echten Anmerkungen; Zoom auch mit Strg+Mausrad und Touchpad.
- Markdown mit Live-Vorschau, KaTeX-Formeln, Mermaid-Diagrammen,
  Codehervorhebung, anklickbaren Checklisten und Frontmatter-Tags.
- Notizverknüpfungen: Wiki-Links, Backlinks, lokaler Graph und PDF-Zitate,
  die direkt zur zitierten Stelle springen; Übersicht aller Zitate im Projekt.
- Codevorschau mit Syntaxhervorhebung, Übergabe an einen externen Editor.
- HTML-Dateien als Webseite anzeigen; Inhalte aus dem Internet nur nach
  Freigabe für die einzelne Datei.

### Werkzeuge
- Eingebautes Terminal (mehrere Terminals, Splits, Kopieren und Einfügen).
- Eigene Aktionen pro Projekt oder global, Skripte direkt ausführen.
- Kleines Erweiterungssystem für Befehle und Aktionen.
- Projektkalender mit Monats-, Wochen- und Agendaansicht, Fristen aus
  Notizen und Synchronisierung mit Nextcloud (CalDAV). Das App-Passwort wird
  einmal eingegeben und verschlüsselt mit `systemd-creds` gespeichert oder
  von einem Passwort-Befehl geholt.

### Paketierung
- Nix-Flake mit Paket, App, Overlay und Entwicklungsumgebung.
- PKGBUILD für das AUR (`atlas-workspace`).

[Unveröffentlicht]: https://github.com/MlNUX/atlas/compare/v1.1.2...HEAD
[1.1.2]: https://github.com/MlNUX/atlas/compare/v1.1.1...v1.1.2
[1.1.1]: https://github.com/MlNUX/atlas/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/MlNUX/atlas/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/MlNUX/atlas/releases/tag/v1.0.0
