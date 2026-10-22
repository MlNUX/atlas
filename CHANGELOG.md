<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Änderungen

Das Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
die Versionen [Semantic Versioning](https://semver.org/lang/de/).

## [Unveröffentlicht]

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

### Behoben
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

[1.0.0]: https://github.com/MlNUX/atlas/releases/tag/v1.0.0
