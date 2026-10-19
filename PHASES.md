# Atlas Workspace – Phasenkonzept

## Zweck

Dieses Dokument übersetzt das Produktkonzept in umsetzbare Entwicklungsphasen. Jede Phase soll als eigenständig nutzbarer Produktstand abgeschlossen werden. Neue Funktionen werden erst begonnen, wenn die Abnahmekriterien der vorherigen Phase erfüllt sind.

## Produktbasis

Atlas ist ein Linux-first Desktop-Workspace, der einen fest ausgewählten Projektordner mit Dateiverwaltung, PDF-Anzeige, Markdown, Codevorschau und Terminal verbindet.

Grundregeln über alle Phasen hinweg:

- reale Dateien und Ordner bleiben die primäre Datenbasis
- kein verpflichtender Cloud-Dienst und kein Benutzerkonto
- natives Wayland; XWayland ist keine Voraussetzung
- Hyprland gehört zu den primären Testumgebungen
- NixOS ist die erste Entwicklungs- und Zielplattform; Arch Linux folgt zum Ende der Stabilisierung
- Atlas wird als Open-Source-Software unter GPL-3.0-or-later entwickelt
- Funktionen bleiben kontextbezogen und überladen die Hauptoberfläche nicht

---

## Phase 0: Technischer Prototyp

### Ziel

Die risikoreichsten technischen Grundlagen validieren, bevor die vollständige Oberfläche entwickelt wird.

### Umfang

- natives Qt-6-/QML-Fenster unter Wayland
- Qt 6/QML für die Oberfläche und Rust für den System- und Anwendungskern
- Projektordner auswählen und Verzeichnisbaum darstellen
- grundlegender Dateisystem-Watcher
- Vergleichsprototyp für Poppler und PDFium; die performanteste ausreichend korrekte Engine gewinnt
- eingebetteter Terminal-Prototyp mit Bedienverhalten wie das VS-Code-Terminal
- Split-Layout mit zwei unterschiedlichen Dokumenttypen
- Nix-Flake für reproduzierbare Entwicklung
- projektbezogene Zustände ausschließlich außerhalb des Projektordners speichern
- `ripgrep` als Suchbasis integrieren

### Zu klärende Technik

- reproduzierbarer Poppler-/PDFium-Benchmark mit kleinen, großen, textlastigen und bildlastigen PDFs
- konkrete Terminal-Emulation und PTY-Anbindung für VS-Code-artige Tabs, Splits und Größenänderung
- CXX-Qt als bevorzugte Grenzfläche zwischen QML und Rust; nur bei einem konkreten technischen Blocker neu bewerten
- robuste Wayland-Integration unter Hyprland

### Festgelegtes Datenmodell für den Prototyp

- Atlas legt im ausgewählten Projektordner einen versteckten `.atlas/`-Ordner an.
- Projektkonfiguration, Sitzung, Index und UI-Zustand werden projektbezogen in `.atlas/` gespeichert.
- Nutzerdokumente bleiben außerhalb von `.atlas/` als normale Dateien und sind vollständig ohne Atlas verwendbar.
- Rekonstruierbare Daten wie Suchindex und Vorschaudaten werden getrennt von dauerhaft relevanten Metadaten abgelegt.
- Dateien werden primär über Pfad plus Geräte-ID und Inode verfolgt.
- Ein leichter Fingerabdruck dient als Reparaturhilfe bei externem Verschieben oder Umbenennen.
- Phase 0 und Phase 1 unterstützen zunächst lokale Dateisysteme; Netzwerkdateisysteme sind nicht Teil der Abnahme.

Vorgesehene Grundstruktur:

```text
Projektarbeit/
├ .atlas/
│  ├ project.toml
│  ├ sessions/
│  │  └ <device-id>/
│  │     ├ active.json
│  │     ├ literaturrecherche.json
│  │     └ programmierung.json
│  ├ metadata.sqlite
│  ├ index.sqlite
│  └ cache/
├ src/
├ docs/
└ README.md
```

`project.toml`, die Session-Dateien und `metadata.sqlite` enthalten dauerhafte Atlas-Daten. `index.sqlite` und `cache/` dürfen gelöscht und aus dem Projekt neu aufgebaut werden. Git-spezifisches Verhalten ist nicht Bestandteil von Phase 0 oder Phase 1.

### Nicht-Ziele

- fertiges Design
- vollständige Dateioperationen
- Notizen, Annotationen oder Backlinks
- Plugin-System

### Abnahmekriterien

- Atlas startet nativ unter Wayland auf NixOS.
- Ein Projektordner mit mindestens 10.000 Einträgen kann ohne blockierende Oberfläche geladen werden.
- PDF, Markdown-Datei und Terminal können gleichzeitig in Splits angezeigt werden.
- Änderungen außerhalb von Atlas werden zuverlässig im Projektbaum sichtbar.
- Entwicklung und Build sind über die Nix-Flake reproduzierbar.
- `ripgrep` liefert eine nicht blockierende Projektsuche und berücksichtigt `.gitignore`.
- Terminalprogramme mit Vollbildoberfläche, Unicode und Farben funktionieren im eingebetteten Terminal.
- Die ausgewählte PDF-Engine gewinnt den dokumentierten Benchmark und erfüllt Darstellung, Textauswahl und Suche korrekt.
- `.atlas/` wird sicher und atomar geschrieben; ein beschädigter oder gelöschter Index kann ohne Verlust der Nutzerdokumente neu aufgebaut werden.

---

## Phase 1: Funktionsfähiger Kern

### Ziel

Eine täglich nutzbare lokale Anwendung für die Arbeit mit einem Projektordner, PDFs, Markdown, Quellcode und Terminal bereitstellen.

### 1. Projekt- und Dateiverwaltung

- genau einen Projektordner auswählen und dauerhaft merken
- feste Systemorte oberhalb des Projektbaums anzeigen
- Projektbaum nach dem Explorer-Prinzip von VS Code
- Kopieren, Verschieben, Umbenennen und neue Ordner beziehungsweise Dateien anlegen
- Löschen über den Linux-Papierkorb
- versteckte Dateien ein- und ausblenden
- Unix-Berechtigungen lesen und ändern
- Besitzer und Gruppe anzeigen und mit vorhandenen Systemrechten ändern
- externe Dateiänderungen automatisch übernehmen

### 2. Arbeitsfläche

- Tabs für Ordner und Dateien
- horizontale und vertikale Splits
- temporäre Splits, die angeheftet werden können
- große Schnellansicht per Leertaste
- Drag-and-drop zwischen Projektbaum, Dateiliste und Arbeitsfläche
- Breadcrumbs sowie Vor- und Zurücknavigation

### 3. PDF-Grundfunktionen

- schnelle PDF-Darstellung
- Seitenminiaturen und Inhaltsverzeichnis
- Volltextsuche im geöffneten Dokument
- Zoom und Seitennavigation
- letzte Seite, Scrollposition und Zoom speichern
- zwei PDFs oder PDF und Markdown nebeneinander anzeigen

PDF-Markierungen und Kommentare gehören ausdrücklich nicht zu Phase 1.

### 4. Markdown und Notizen

- Markdown-Dateien dürfen frei in jedem Unterordner des Projektordners liegen
- kein zentraler oder bevorzugter Notizordner
- neue Markdown-Dateien werden standardmäßig im aktuell ausgewählten Ordner angelegt
- Atlas behandelt Notizen wie normale Dateien und erzwingt keine separate Bibliothek oder Vault-Struktur
- Live-Vorschau beziehungsweise kombinierter Lese-/Bearbeitungsmodus
- technische Codeblöcke mit Syntaxhervorhebung
- Tabellen, LaTeX und Mermaid-Diagramme
- Links auf Dateien, Überschriften, Codezeilen und PDF-Seiten
- Tags werden im YAML-Frontmatter der jeweiligen Markdown-Datei gespeichert
- Tags werden für Suche und Filter ohne Beachtung der Groß-/Kleinschreibung verglichen; die Schreibweise in der Datei bleibt unverändert
- hierarchische Tags wie `architektur/backend` sind erlaubt
- Dateilinks werden vom Projektwurzelordner aus aufgelöst; ein führender `/` bezeichnet die Projektwurzel
- Beispiel: `[Parser](/src/parser.rs#L120)`
- Pfade verwenden `/`, UTF-8 und bei Bedarf Prozentkodierung; `#L120-L145` bezeichnet einen Codezeilenbereich
- nicht auflösbare Links werden sichtbar als defekt markiert
- Atlas aktualisiert betroffene Links bei Verschieben oder Umbenennen innerhalb der Anwendung
- bei externen Änderungen bietet Atlas eine erkannte Linkreparatur an, verändert Dateien aber nicht ohne Bestätigung
- einfache Backlinks werden bereits in Phase 1 aus dem lokalen Index angezeigt
- dieselben Markdown-Funktionen stehen für README, Dokumentation und persönliche Notizen zur Verfügung

### Autosave und Recovery

- bearbeitete Markdown- und Textdateien nach einer kurzen Schreibpause automatisch speichern
- weiterhin `Strg + S` für sofortiges Speichern unterstützen
- atomar über temporäre Datei und Umbenennen schreiben
- während einer Bearbeitung eine Recovery-Kopie unter `.atlas/recovery/` führen
- Recovery nach erfolgreichem Speichern bereinigen
- bei externer Änderung Autosave für diese Datei pausieren und Neu laden, lokale Fassung behalten oder als neue Datei speichern anbieten
- schreibgeschützte Dateien niemals still überschreiben

### 5. Codevorschau

- Syntaxhervorhebung und Zeilennummern
- Symbolübersicht, soweit zuverlässig verfügbar
- Suche im geöffneten Dokument
- kleine Textänderungen
- Sprung zu einer Datei und Zeile
- konfigurierbare Aktion „Im Editor öffnen“

Atlas ersetzt in Phase 1 keine vollständige IDE.

### 6. Terminal

- Terminal als Tab, Split oder unteres Panel
- Start im Projektstamm oder ausgewählten Unterordner
- mehrere Terminal-Tabs
- horizontale und vertikale Terminal-Splits
- Bash, Zsh und Fish
- Drag-and-drop fügt Dateipfade ein
- Ausgabe als Codeblock in eine Notiz übernehmen

Das Terminal verhält sich wie ein normales Terminal. Atlas bewertet oder blockiert eingegebene Befehle nicht.

Das detaillierte visuelle Terminaldesign wird vorerst nicht weiter ausgearbeitet. Phase 0 validiert nur die technische Einbettung und das erforderliche Verhalten.

### 7. Suche und Arbeitsleiste

- universelle Arbeitsleiste über `Strg + P`
- `ripgrep` als technische Suchbasis
- Dateien, Ordner, Notizen und Aktionen gemeinsam finden
- projektweite Volltextsuche
- reguläre Ausdrücke und Dateimuster
- `.gitignore` berücksichtigen
- typische Build-Verzeichnisse ausschließen
- Suche nach `TODO` und `FIXME`
- Treffer im temporären Split öffnen

### 8. Sessions

- dauerhaft sichtbares Session-Dropdown in der oberen Titelleiste
- aktive Session direkt im Dropdown anzeigen
- benannte Sessions anlegen, wechseln, duplizieren, umbenennen und löschen
- mindestens eine automatisch erzeugte Standard-Session pro Projekt
- „Neue Session“ erzeugt eine leere Arbeitsanordnung; „Duplizieren“ übernimmt den aktuellen Zustand
- dieselbe Datei darf mehrfach in verschiedenen Splits und an unterschiedlichen Positionen geöffnet sein
- aktive Session erst löschen, nachdem eine andere Session aktiviert oder eine Ersatzsession angelegt wurde
- keine künstliche Obergrenze für die Anzahl benannter Sessions
- aktive Session vor jedem Wechsel automatisch speichern
- kontinuierlich automatisch speichern; keine permanente Schaltfläche „Session speichern“ in der Titelleiste
- manuelles Speichern nur als unauffällige Aktion im Session-Dropdown und in der Arbeitsleiste anbieten
- Tabs und Split-Anordnung je Session speichern
- PDF-Seite, Scrollposition und Zoom je Session speichern
- Suchanfragen, Filter und sichtbare Seitenleisten je Session speichern
- Terminal-Layouts und Arbeitsverzeichnisse speichern; laufende Prozesse müssen nach einem Neustart nicht fortgesetzt werden
- nach einem Neustart die zuletzt aktive Session automatisch öffnen
- Session-Dateien pro Gerät unter `.atlas/sessions/<device-id>/` ablegen

### Nicht-Ziele von Phase 1

- Git-Oberfläche
- automatisch erkannte Build- und Testaufgaben
- PDF-Annotationen und Zitatbibliothek
- Arbeitskorb
- KI-Assistent
- Plugin-Marktplatz
- SSH-, Container- oder Remote-Entwicklung

### Abnahmekriterien

- Ein Projektordner lässt sich ohne vorherige Einrichtung öffnen und wiederverwenden.
- Ein lokales PDF ist in höchstens zwei Interaktionen geöffnet.
- Schnellansicht, permanente Tabs und temporäre Splits funktionieren konsistent.
- Nach einem Neustart werden Projekt, Tabs, Splits und Dokumentpositionen wiederhergestellt.
- Das Session-Dropdown erlaubt einen Wechsel, ohne Änderungen an Projektdateien oder der zuvor aktiven Session zu verlieren.
- Autosave verliert bei normalem Beenden, Absturz oder externer Dateiänderung keine bestätigte Fassung.
- Terminal, PDF, Markdown und Codevorschau können gleichzeitig verwendet werden.
- Projektweite Suche berücksichtigt `.gitignore` und blockiert die Oberfläche nicht.
- grundlegende Dateioperationen funktionieren unter NixOS inklusive Papierkorb.
- die Anwendung läuft unter Hyprland nativ über Wayland.

### Ergebnis

Phase 1 ist eine eigenständig nutzbare Version für tägliche lokale Projektarbeit. Sie bildet die Basis für alle weiteren Funktionen.

---

## Phase 2: Vertiefte Arbeitsabläufe

### Ziel

Dateien, PDF-Quellen und Notizen stärker verbinden und Atlas durch kontrollierbare lokale Aktionen erweiterbar machen.

### 1. PDF-Annotationen

- Text markieren und kommentieren
- Freihandmarkierungen und Lesezeichen
- Annotationen nicht destruktiv speichern
- Annotationen standardmäßig unter `.atlas/annotations/` speichern
- PDF über Projektpfad, Fingerabdruck und Seitenreferenz zuordnen
- expliziter Export in eine PDF-Kopie
- markierte Stelle als Zitat in Markdown übernehmen
- stabiler Rücksprung zu Datei und Seite

Die Original-PDF wird nur auf ausdrücklichen Befehl verändert. Eine projektweite Zitatbibliothek ist nicht Teil von Phase 2.

### 2. Notizverknüpfungen

- projektwurzelrelative Wiki-Links wie `[[/docs/architektur.md]]`
- Aliase und Überschriftenziele wie `[[/docs/architektur.md#Parser|Architektur]]`
- erweiterte Backlink-Filter
- lokaler Kontextgraph für die aktuelle Markdown- oder PDF-Datei
- Verbindungen zwischen Markdown-Dateien und PDFs darstellen
- robuste Aktualisierung interner Links beim Verschieben innerhalb von Atlas

Der konkrete Umfang folgt aus dem in Phase 1 beschlossenen Notizmodell.

### 3. Benutzerdefinierte Aktionen

- deklarative Aktionen, beispielsweise in YAML
- Befehle standardmäßig als Programm plus Argumentliste ohne Shell definieren
- Shell-Ausführung nur über eine explizite Option aktivieren
- Aktionen nach Dateityp und Kontext filtern
- Anzeige in Kontextmenü und Arbeitsleiste
- sichtbare Ausführung im integrierten Terminal
- Platzhalter für Datei, Ordner und Projektstamm

### 4. Kleines Erweiterungssystem

- eigene Aktionen
- eigene Dateiverarbeitung und Exporter
- klar versioniertes Konfigurationsformat
- keine beliebige UI-Erweiterung in der ersten Ausbaustufe

### 5. Projektkalender und Nextcloud

- Woche, Monat und Agenda
- Termine, Arbeitsblöcke, Fristen und ungeplante Aufgaben
- Verknüpfungen mit Projektdateien und benannten Sessions
- CalDAV-Verbindung zu Nextcloud
- Auswahl sichtbarer Nextcloud-Kalender
- globaler und optional projektspezifischer Standardkalender für neue Termine
- Zielkalender beim Erstellen eines einzelnen Termins änderbar
- neue und geänderte lokale Termine bei bestehender Verbindung sofort synchronisieren
- Offline-Warteschlange mit automatischer Übertragung nach Wiederherstellung der Verbindung
- Konflikte sichtbar auflösen statt Einträge still zu überschreiben
- nach Bestätigung einen eigenen Nextcloud-Kalender wie „Atlas – Projektarbeit“ anlegen dürfen
- Markdown-Checklisten nur über eine bewusste Aktion in Kalenderaufgaben übernehmen
- Wiederholungen zunächst anzeigen und grundlegend bearbeiten; komplexe Serienausnahmen später
- Erinnerungen aus CalDAV übernehmen
- Dateiverknüpfungen über Termin-UID in `.atlas/calendar/links.json` speichern
- kein Linux-Schlüsselbund: App-Passwort pro Atlas-Start einmal abfragen und nur im Arbeitsspeicher halten

### Abnahmekriterien

- Eine PDF-Markierung kann mit Quelle und Seite in eine Notiz übernommen werden.
- Ein Zitat öffnet zuverlässig wieder die zugehörige PDF-Seite.
- Die Original-PDF bleibt ohne ausdrücklichen Export unverändert.
- Der lokale Graph zeigt Beziehungen der aktuellen Markdown- oder PDF-Datei.
- Eigene Aktionen werden reproduzierbar im richtigen Arbeitsverzeichnis ausgeführt.
- Externe Programme können Markdown- und PDF-Dateien weiterhin ohne Atlas verwenden.
- Ein lokal erstellter Termin erscheint nach erfolgreicher Synchronisierung im ausgewählten Nextcloud-Kalender.
- Offline erstellte Termine werden nach Wiederherstellung der Verbindung ohne Duplikate übertragen.

---

## Zukunfts-Backlog

Diese Punkte sind bewusst keiner verbindlichen Phase zugeordnet:

- Git-Status, Diff und weitere Git-Funktionen
- automatisch erkannte Aufgaben aus `Cargo.toml`, `package.json`, `Makefile` oder `justfile`
- universelle Diff-Ansicht
- Aktivitäts- und Änderungschronik
- Beziehungen zwischen beliebigen Dateien
- projektweite Zitatbibliothek
- sitzungsbezogener Arbeitskorb
- quellengebundener KI-Assistent – niedrige Priorität; erst nach Phase 2 erneut bewerten
- OCR für gescannte PDFs
- SSH und Remote-Dateisysteme
- Container und Dev Containers
- Nix-Entwicklungsumgebungen innerhalb eines Projekts erkennen und aktivieren
- Cloud-Synchronisierung und Zusammenarbeit
- Office-Bearbeitung
- umfassendes Plugin-System und Plugin-Marktplatz

Ein Backlog-Punkt wird erst einer Phase zugeordnet, wenn sein konkretes Problem, sein gewünschtes Verhalten und seine Priorität geklärt sind.

---

## Veröffentlichungsstrategie

### Phase 0

Interne Entwickler-Builds, keine Stabilitätszusage.

Das Quellcode-Repository enthält von Beginn an den vollständigen GPL-3.0-Lizenztext, die SPDX-Kennung `GPL-3.0-or-later` und klare Copyright-Hinweise.

### Phase 1 Alpha

- Nix-Flake und NixOS-Paket
- Fokus auf Datenintegrität und grundlegende Arbeitsabläufe
- bewusst noch veränderliche Oberfläche und Konfiguration

### Phase 1 Beta

- Migrationen für gespeicherte Sitzungsdaten
- Performance-Tests mit großen Projektordnern und PDFs
- dokumentierte Tastenkürzel
- Fehlerberichte mit klaren Systeminformationen für Wayland und NixOS
- Arch-`PKGBUILD` und Arch-Kompatibilität gegen Ende der Beta

### Phase 1 Stable

- keine bekannten Fehler mit Datenverlust
- stabile Sitzungswiederherstellung
- reproduzierbare Pakete für NixOS und Arch
- dokumentiertes Dateiformat für alle von Atlas erzeugten Metadaten

Phase 2 beginnt erst nach einem stabilen Phase-1-Release.

## Reihenfolge innerhalb von Phase 1

1. Wayland-Fenster, Projektordner und Verzeichnisbaum
2. Dateioperationen und Dateisystem-Watcher
3. Tabs, Splits und Sitzungsmodell
4. PDF-Anzeige und Schnellansicht
5. Markdown sowie Festlegung von Metadaten und Linkverhalten
6. Codevorschau und externer Editor
7. Terminal-Integration
8. Suche und universelle Arbeitsleiste
9. Berechtigungen, Besitzer, Gruppe und Linux-Politur
10. Paketierung, Tests und Stabilisierung

