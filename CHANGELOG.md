<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Änderungen

Das Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
die Versionen [Semantic Versioning](https://semver.org/lang/de/).

## [Unveröffentlicht]

### Neu
- Kontextleiste: neue Module **Lernzeit** (Pomodoro-Timer: 25 Min. lernen,
  5 Min. Pause, nach vier Runden 15 Min.; Längen einstellbar),
  **Demnächst** (Termine, Arbeitsblöcke und Fristen der nächsten sieben
  Tage) und **Aufgaben** (abhaken, neue anlegen). Jedes lässt sich in den
  Einstellungen ein- und ausschalten. Der erste Reiter heißt jetzt
  „Übersicht“.
- Markdown → PDF eingebaut („Als PDF exportieren“ im Kontextmenü und in der
  Kontextleiste): gesetzt wie die Vorschau mit Formeln, Diagrammen und
  Code, auf hellem A4-Papier – ohne pandoc oder LaTeX.
- TeX → PDF („Zu PDF übersetzen“ für .tex-Dateien): latexmk, sonst
  pdflatex, im Terminal; Fehler bleiben sichtbar.
- PDF: Pfeiltasten – hoch/runter scrollen, links/rechts blättern.

### Behoben
- PDF-Dunkelmodus ruckelte beim Ändern der Fenstergröße: Die Umkehrung
  wird jetzt einmal beim Rendern gemacht statt bei jedem Neuzeichnen.
- Die Beispielaktion „Markdown als PDF exportieren“ (pandoc) scheiterte
  ohne LaTeX-Installation; die Vorlage für neue Aktionsdateien nutzt
  stattdessen Word (pandoc → .docx), PDF gibt es eingebaut.
- Einstellungsfenster scrollt auf kleinen Bildschirmen.

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

[Unveröffentlicht]: https://github.com/MlNUX/atlas/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/MlNUX/atlas/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/MlNUX/atlas/releases/tag/v1.0.0
