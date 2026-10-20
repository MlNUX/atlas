<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Änderungen

Das Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
die Versionen [Semantic Versioning](https://semver.org/lang/de/).

## [Unveröffentlicht]

### Geändert
- „Lokaler Graph“ heißt jetzt „Graph“ und zeigt in einer Ansicht das ganze
  Projekt: die aktuelle Datei in der Mitte, verknüpfte Dateien um sie
  herum, den Rest außen; mit Zoom und Verschieben.

### Behoben
- Sessions in anderen Projektordnern: Das Sessionfenster zeigt jetzt die
  Sessions aller geöffneten Projekte, nach Ordner gruppiert; ein Klick
  wechselt in den Ordner und die Session. Bisher waren nach „Neue Session“
  in einem anderen Ordner die Sessions des vorigen Ordners nicht mehr zu
  sehen und wirkten überschrieben.
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
