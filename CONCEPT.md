# Konzept: Atlas Workspace

## Kurzfassung

Atlas Workspace ist ein lokaler, dateizentrierter Desktop-Arbeitsplatz für Menschen, die hauptsächlich mit Ordnern und PDFs arbeiten, aber gelegentlich strukturierte Notizen benötigen. Das Programm verbindet die vertraute Bedienung eines Dateimanagers mit einem integrierten PDF-Arbeitsbereich und leichtgewichtigen, verknüpfbaren Markdown-Notizen.

Der entscheidende Grundsatz lautet: **Die Dateien bleiben das Original.** Atlas baut keine geschlossene Datenbank um die Arbeit des Nutzers. Vorhandene Ordnerstrukturen, Dateinamen und Programme bleiben kompatibel.

## Zielgruppe

- Einzelpersonen, Studierende, Forschende und Wissensarbeiter
- Nutzer, deren Arbeitsablauf bei einem Projektordner beginnt
- Menschen, die viele PDFs lesen, vergleichen, kommentieren und wiederfinden müssen
- Nutzer, denen Obsidian für den Alltag zu notizzentriert und Dolphin für Dokumentarbeit zu passiv ist

## Produktversprechen

> Öffne einen Ordner und arbeite mit seinen Dateien, PDFs und Notizen, ohne ständig zwischen Anwendungen und Fenstern zu wechseln.

## Bestätigte Produktrichtung

- Die dunkle, dreispaltige Hauptansicht ist die visuelle und strukturelle Basis.
- Eine große temporäre Schnellansicht wird per Leertaste geöffnet.
- Markierte PDF-Stellen können als Zitat mit stabilem Seitenlink in Notizen übernommen werden.
- Der Notizmodus bietet Markdown, Wiki-Links, Tags, Backlinks und einen lokalen Graphen, bleibt aber in die reale Projektordnerstruktur eingebettet.
- Die linke Navigation trennt feste Systemorte vom aktuell geöffneten Projektordner. Eigene Favoriten in Atlas entfallen; die Orte des Dateimanagers (Dolphin/KDE, GTK) werden auf Wunsch übernommen, sodass es nur eine Liste gibt. Der Projektordner erscheint als persistenter Explorer-Baum nach VS-Code-Vorbild.
- Für Informatik- und Linux-Projekte erhält Atlas ein integriertes Terminal, technische Markdown-Darstellung, Codevorschau, Entwicklersuche und benutzerdefinierte Aktionen.

## Leitprinzipien

1. **Dateisystem zuerst:** Reale Dateien und Ordner sind die primäre Organisation.
2. **Sofortige Vorschau:** Inhalte sollen ohne Moduswechsel sichtbar werden.
3. **Kontext bleibt erhalten:** Offene Tabs, Lesepositionen, Splits und Filter werden wiederhergestellt.
4. **Notizen sind optional:** Das Programm ist bereits ohne Wissensgraph vollständig nutzbar.
5. **Offene Formate:** Markdown, PDF-Metadaten und exportierbare Annotationen verhindern Lock-in.
6. **Lokal und schnell:** Grundfunktionen benötigen weder Konto noch Cloud.

## Informationsarchitektur

### Linke Seitenleiste: Navigation

- kompakter Bereich „Orte“ für Persönlicher Ordner, Schreibtisch, Dokumente, Downloads und Papierkorb
- keine eigenen Favoriten; darunter optional (Einstellungen, standardmäßig an) die lokalen Orte aus Dolphin/KDE und GTK-Dateimanagern
- darunter genau ein aktuell geöffneter Projektordner nach dem Explorer-Prinzip von VS Code
- vollständiger, aufklappbarer Verzeichnisbaum dieses Projektordners
- Aktion „Projektordner öffnen“ zum Wechseln der Arbeitsbasis
- derselbe Projektbaum bleibt in Datei-, PDF- und Notizmodus erhalten

### Mitte: Arbeitsfläche

- Dateiliste oder Rasteransicht
- Tabs für Ordner, PDFs, Markdown, Bilder und Textdateien
- horizontale und vertikale Split Views
- Schnellvorschau per Leertaste
- Drag-and-drop zwischen Ordnern, Tabs und Splits

### Rechte Seitenleiste: Kontext

- Dateiinformationen und Schlagwörter
- PDF-Gliederung, Miniaturen und Anmerkungen
- verknüpfte Notizen und Backlinks
- Aufgaben, die zur aktuellen Datei gehören
- Aktivitäts- und Versionsinformationen

## Kernfunktionen

### Dateiverwaltung

- Kopieren, Verschieben, Umbenennen, Löschen und Wiederherstellen
- Linux-Papierkorb statt unmittelbarem endgültigem Löschen
- versteckte Dateien ein- und ausblenden
- Unix-Dateiberechtigungen lesen und ändern
- Besitzer und Gruppe anzeigen und – mit den vorhandenen Systemrechten – ändern
- Tabs, Split View, Breadcrumbs und Verlauf
- Listen-, Detail- und Rasteransicht
- Batch-Umbenennung und Dateivorlagen
- optionale Farben und Tags ohne Veränderung der Ordnerstruktur

### Universelle Arbeitsleiste

`Strg/Cmd + P` öffnet eine zentrale Palette für Dateien, Ordner und Aktionen. Sie dient nicht nur als Suche, sondern als schneller Zugang zu fast allen häufigen Arbeitsabläufen.

```text
┌──────────────────────────────────────────────────────────────┐
│  🔎  literatur                                               │
├──────────────────────────────────────────────────────────────┤
│  DATEIEN                                                     │
│  PDF  Literatur.pdf                         02_Texte/Quellen │
│  MD   Literaturnotizen.md                    02_Texte/Notizen│
├──────────────────────────────────────────────────────────────┤
│  AKTIONEN                                                    │
│  Literatur.pdf rechts im Split öffnen                       │
│  Neue Notiz zu Literatur.pdf erstellen                      │
│  Zitate aus Literatur.pdf anzeigen                          │
└──────────────────────────────────────────────────────────────┘
```

Grundregeln:

- Ergebnisse erscheinen in den Gruppen Dateien, Ordner, Notizen, Zitate und Aktionen.
- Die Suche akzeptiert unscharfe Eingaben und zeigt Pfad sowie Dateityp.
- Pfeiltasten wählen einen Treffer, Enter führt ihn aus und Esc schließt die Palette.
- Der Präfix `>` beschränkt auf Aktionen, `#` auf Tags und `@` auf Zitate oder Annotationen.
- Aktionen beziehen den aktuellen Kontext ein, etwa das geöffnete PDF oder den aktiven Split.
- Häufige und zuletzt ausgeführte Aktionen erscheinen bei leerem Suchfeld, aber nicht als permanente Favoriten.

Beispielablauf: Der Nutzer drückt `Strg + P`, tippt „literatur“, wählt „Literatur.pdf rechts im Split öffnen“ und erhält sofort einen temporären zweiten Arbeitsbereich, ohne Menüs oder Drag-and-drop zu benötigen.

### PDF-Arbeitsbereich

- schnelle Darstellung großer Dokumente
- Inhaltsverzeichnis, Seitenminiaturen und Volltextsuche
- Markierungen, Kommentare, Freihand und Lesezeichen ab Phase 2
- zwei PDFs oder PDF und Notiz nebeneinander
- Zitate mit Link auf Datei und Seitenzahl in eine Markdown-Notiz übernehmen
- letzte Leseposition automatisch speichern
- Speicherformat und Export von Annotationen werden in Phase 2 festgelegt

### Notizen

- Markdown-Dateien direkt in jedem beliebigen Projektordner erstellen
- kein zentraler oder bevorzugter Notizordner; Speicherort bestimmt allein der Nutzer
- Notizen bleiben normale Dateien im Projektbaum und benötigen keine separate Bibliothek
- Notiz zu einer Datei, einem Ordner oder einer PDF-Seite anheften
- `[[Verknüpfungen]]`, Backlinks und Tags
- Tags als portable YAML-Frontmatter direkt in der jeweiligen Markdown-Datei speichern
- Tags ohne Beachtung der Groß-/Kleinschreibung vergleichen und hierarchische Formen wie `architektur/backend` erlauben
- Links mit führendem `/` vom Projektwurzelordner aus auflösen, etwa `[Parser](/src/parser.rs#L120)`
- Codezeilenbereiche als `#L120-L145` unterstützen und defekte Links sichtbar markieren
- Links bei Dateioperationen innerhalb von Atlas automatisch aktualisieren
- bei externen Verschiebungen Reparatur vorschlagen, aber Markdown nicht ungefragt verändern
- einfache Backlinks bereits in Phase 1 anzeigen
- tägliche Notizen und Graphansicht nur als optionale Erweiterungen
- Lesemodus und Editor ohne separate Bibliothek

### Suche

- gemeinsame Suche über Dateinamen, PDF-Text, Markdown und Metadaten
- Filter nach Typ, Ordner, Datum, Tag und Annotation
- OCR für gescannte PDFs als späteres Modul
- Aktionen direkt aus den Suchergebnissen

### Sitzungen und Arbeitsbereiche

Ein Arbeitsbereich speichert keine Kopien der Dateien, sondern eine Sicht auf die Arbeit:

- genau einen vom Nutzer fest ausgewählten Projektordner
- Tabs und Split-Anordnung
- Scroll- und Lesepositionen
- aktive Suche und Filter
- ein- oder ausgeblendete Seitenleisten

Atlas unterstützt bereits in Phase 1 mehrere benannte Sessions – beispielsweise „Literaturrecherche“, „Programmierung“ und „Dokumentation“. In der oberen Titelleiste befindet sich dauerhaft ein kompaktes Session-Dropdown nach dem Vorbild des erweiterten Mockups. Es zeigt die aktive Session und ermöglicht Wechseln, Anlegen, Duplizieren, Umbenennen und Löschen.

Beim Wechsel speichert Atlas die aktive Session automatisch und stellt anschließend Tabs, Splits, Dokumentpositionen, Seitenleisten, Suchzustand und Terminal-Layout der gewählten Session wieder her. Projektdateien werden dabei weder kopiert noch verändert. Terminal-Arbeitsverzeichnisse und Layouts werden wieder geöffnet, laufende Prozesse jedoch nicht zwischen Programmstarts rekonstruiert.

Sessions werden kontinuierlich automatisch gespeichert. Eine manuelle Aktion „Session speichern“ bleibt im Dropdown und in der universellen Arbeitsleiste verfügbar, erscheint jedoch nicht als dauerhafte Schaltfläche in der Titelleiste.

„Neue Session“ beginnt leer, während „Duplizieren“ den aktuellen Zustand übernimmt. Dieselbe Datei darf mehrfach in unterschiedlichen Splits geöffnet sein. Die aktive Session kann erst gelöscht werden, nachdem eine andere Session aktiv oder eine Ersatzsession angelegt wurde. Eine künstliche Obergrenze für Sessions gibt es nicht.

Projektbezogene Atlas-Daten liegen im versteckten Ordner `.atlas/` innerhalb des ausgewählten Projektordners. Dazu gehören Konfiguration, Sitzung, Dateiidentitäten, Linkinformationen und rekonstruierbare Suchdaten. Nutzerdokumente werden nicht in diesen Verwaltungsordner verschoben.

### Zitatbibliothek (Zukunfts-Backlog)

- projektweite Übersicht aller PDF-Markierungen und Zitate
- Filter nach Quelldatei, Seite, Farbe, Tag und Kommentar
- direkter Rücksprung zur ursprünglichen PDF-Seite
- Drag-and-drop eines Zitats in eine Markdown-Notiz
- stabile Quellenangabe bleibt beim Verschieben der zugehörigen Notiz erhalten

### Temporäre Splits

- Datei oder PDF kurz neben dem Hauptinhalt öffnen, ohne einen dauerhaften Tab anzulegen
- Aufruf per Arbeitsleiste, Kontextmenü oder Ziehen an den Fensterrand
- der temporäre Split kann angeheftet und dadurch dauerhaft gemacht werden
- ein nicht angehefteter Split schließt sich beim bewussten Wechsel des Arbeitskontexts

### Arbeitskorb (Zukunfts-Backlog)

Der Arbeitskorb ist eine temporäre Sammlung von Dateien aus verschiedenen Unterordnern. Er ersetzt keine Projektstruktur und keine Favoriten, sondern bündelt Material für die aktuelle Aufgabe.

- Dateien per Drag-and-drop oder Aktion „Zum Arbeitskorb“ hinzufügen
- Originaldateien werden weder kopiert noch verschoben
- Reihenfolge frei anordnen und Dateien gemeinsam öffnen oder vergleichen
- Inhalt gehört standardmäßig zur Sitzung
- ein Arbeitskorb kann später optional als benannte Sammlung gespeichert werden

### Terminal-Integration

Das Terminal ist ein regulärer Arbeitsbereich und kann als Tab, Split oder ausklappbares unteres Panel erscheinen. Es startet standardmäßig im aktuell geöffneten Projektordner.

- mehrere Terminal-Tabs sowie horizontale und vertikale Teilung
- Profile für Bash, Zsh und Fish
- Terminal im Projektstamm oder im ausgewählten Unterordner öffnen
- laufende Prozesse bleiben beim Ausblenden des Panels aktiv
- Dateien und Ordner können hineingezogen werden; Atlas fügt korrekt quotierte Pfade ein
- Terminalausgabe kann als Codeblock in eine Markdown-Notiz übernommen werden

Das Terminal verhält sich wie ein normales Terminal. Atlas bewertet oder blockiert dort eingegebene Befehle nicht und fügt keine besonderen Warnstufen für `sudo` oder destruktive Kommandos hinzu.

### Technisches Markdown

- Syntaxhervorgehobene Code- und Befehlsblöcke
- LaTeX-Formeln, Tabellen und Mermaid-Diagramme
- kopierbare Befehle mit der sicheren Aktion „Ins Terminal einfügen“
- Links auf Dateien, Überschriften und konkrete Codezeilen
- Links auf PDF-Seiten bleiben mit technischen Notizen kombinierbar

Beispiele für projektinterne Verweise:

```text
[[/src/parser.rs#L120]]
[[/README.md#Installation]]
[[/docs/architecture.pdf?page=8]]
```

### Codevorschau

Atlas ist keine vollständige IDE, stellt Quellcode aber als nativen Dateityp dar:

- Syntaxhervorhebung und Zeilennummern
- Symbolübersicht und Sprung zu einer bestimmten Zeile
- Suche im geöffneten Dokument
- kleine Änderungen im integrierten Texteditor
- frei konfigurierbare Aktion „Im Editor öffnen“, etwa für Neovim, Kate oder VS Code
- Übergabe von Datei und Arbeitsverzeichnis an das integrierte Terminal

### Autosave und Recovery

- Markdown- und Textdateien nach kurzer Schreibpause automatisch und atomar speichern
- `Strg + S` löst weiterhin ein sofortiges Speichern aus
- Recovery-Kopien während der Bearbeitung unter `.atlas/recovery/` führen
- bei externen Änderungen Autosave pausieren und eine bewusste Konfliktentscheidung verlangen
- schreibgeschützte Dateien nicht automatisch überschreiben

### Entwicklersuche

- projektweite Volltextsuche mit regulären Ausdrücken und Dateimustern wie `*.rs` oder `*.md`
- `.gitignore` berücksichtigen
- typische Build-Ordner wie `.git`, `node_modules` und `target` ausschließen
- Suche nach `TODO`, `FIXME` und ähnlichen Markierungen
- Treffer mit Dateipfad und Zeilennummer in einem temporären Split öffnen

### Benutzerdefinierte Aktionen

Nutzer können lokale, projektbezogene Aktionen definieren. Sie erscheinen im Kontextmenü und in der universellen Arbeitsleiste; die Ausführung erfolgt sichtbar im Terminal.

```yaml
name: Markdown als PDF exportieren
applies_to: "*.md"
command: "pandoc {file} -o {stem}.pdf"
confirmation: true
```

Mögliche Aktionen sind Tests ausführen, Dateien formatieren, Prüfsummen erzeugen, Bilder konvertieren oder Dokumente exportieren. Platzhalter für Dateipfade müssen sicher gequotet werden. Bei potenziell destruktiven Befehlen ist eine Bestätigung erforderlich.

## Zentrale Interaktionen

### PDF lesen und festhalten

1. PDF in der Dateiliste auswählen.
2. Leertaste öffnet die Schnellvorschau; Enter öffnet einen dauerhaften Tab.
3. Text markieren und „Als Zitat in Notiz“ wählen.
4. Atlas erzeugt Markdown mit Dokumenttitel, Seite und stabilem Dateilink.
5. Ein Klick auf das Zitat öffnet später dieselbe Stelle im PDF.

### Zwei Dokumente vergleichen

1. Erstes PDF öffnen.
2. Zweites PDF auf den rechten Rand ziehen.
3. Atlas erzeugt einen Split und kann Zoom oder Scrollposition optional koppeln.
4. Unterschiede und Kommentare landen in einer zugeordneten Vergleichsnotiz.

### Arbeit fortsetzen

Beim nächsten Start wird der letzte Arbeitsbereich mit Ordnern, Tabs, Splits und Lesepositionen wiederhergestellt. Eine Startseite zeigt zusätzlich zuletzt verwendete Projekte und angeheftete Dokumente.

### Material für eine Aufgabe sammeln (Zukunfts-Backlog)

1. Dateien aus verschiedenen Unterordnern zum Arbeitskorb hinzufügen.
2. Relevante PDFs über einen temporären Split prüfen.
3. Markierungen als Zitate übernehmen oder in der Zitatbibliothek sammeln.
4. Beim Beenden bleiben Arbeitskorb, Tabs und Lesepositionen Bestandteil der Sitzung.

## Bedienkonzept

- **Ein Klick:** auswählen und Vorschau aktualisieren
- **Doppelklick/Enter:** dauerhaft im Tab öffnen
- **Leertaste:** große temporäre Schnellvorschau
- **Strg/Cmd + P:** Dateien und Aktionen per Command Palette finden
- **Strg/Cmd + Shift + F:** projektweite Inhaltssuche
- **Drag-and-drop:** organisieren, öffnen, splitten oder verknüpfen

Die Oberfläche soll ruhig, dicht und desktoptypisch sein. Informationen erscheinen kontextabhängig; permanente Symbolleisten werden auf häufige Aktionen begrenzt.

## Entwicklungsphasen

### Phase 1: Funktionsfähiger Kern

- genau einen Projektordner auswählen, dauerhaft merken und als Explorer-Baum anzeigen
- lokale Dateioperationen mit Linux-Papierkorb
- versteckte Dateien, Unix-Berechtigungen, Besitzer und Gruppe
- Dateiliste mit Vorschau
- Tabs und zwei Split-Bereiche
- integrierter PDF-Reader mit Suche und gespeicherter Leseposition; Annotationen folgen in Phase 2
- Markdown-Editor für frei im Projektbaum verteilte Dateien, YAML-Tags und projektwurzelrelative Links
- Autosave und Absturz-Recovery für Markdown- und Textdateien
- lokale Volltextsuche und projektweite Entwicklersuche mit `.gitignore`-Unterstützung
- Wiederherstellung der letzten Sitzung
- mehrere benannte Sessions mit dauerhaft sichtbarem Dropdown in der Titelleiste
- universelle Arbeitsleiste für Dateien und Aktionen
- temporäre Splits
- integriertes Terminal im Projektordner
- technische Markdown-Darstellung
- lesbare Codevorschau mit Übergabe an einen externen Editor

### Phase 2: Vertiefte Arbeitsabläufe

- PDF-Markierungen, Kommentare, Freihand, Lesezeichen und Export
- nicht destruktive Annotationen unter `.atlas/annotations/` und stabile Links auf PDF-Seiten
- Projektkalender mit sofortiger bidirektionaler Nextcloud-CalDAV-Synchronisierung und konfigurierbarem Standardkalender
- benutzerdefinierte, im Terminal ausgeführte Aktionen
- deklaratives Erweiterungssystem für eigene Aktionen und Dateiverarbeitung
- erweiterte Backlink-Filter, Wiki-Links und lokaler Kontextgraph auf Basis des Phase-1-Indexes

### Zukunfts-Backlog

- projektweite Zitatbibliothek
- sitzungsbezogener Arbeitskorb
- Git-Integration
- automatisch aus Projektdateien erkannte Aufgaben und Befehle
- Cloud-Synchronisierung und Teamfunktionen
- Wissensgraph
- komplexes Aufgabenmanagement
- Office-Bearbeitung
- KI-Assistent und automatische Zusammenfassungen – niedrige Priorität, vorerst ohne Architekturvorleistungen
- umfassender Plugin-Marktplatz
- Aktivitäts- und Änderungschronik
- Beziehungen zwischen beliebigen Dateien
- universelle Diff-Ansicht
- SSH-, Container-, Dev-Container-, Nix- und andere Remote-Entwicklungsumgebungen

### Derzeit nicht vorgesehen

- PDF-Inbox mit Lesestatus
- kurze Relevanzbeschreibung für jede Datei

## Entscheidungsprotokoll der Ideen

| Idee | Entscheidung | Einordnung |
|---|---|---|
| Universelle Arbeitsleiste | Behalten | Phase 1; Konzept oben konkretisiert |
| Automatische Sitzungsspeicherung | Behalten | Phase 1 |
| Mehrere benannte Sessions | Behalten | Phase 1; Auswahl über Dropdown in der Titelleiste |
| Aktivitätsansicht | Zurückstellen | eventuell später erneut bewerten |
| PDF-Inbox | Nicht priorisieren | derzeit kein relevantes Kernproblem |
| Zitatbibliothek | Zurückstellen | Zukunfts-Backlog |
| Temporäre Splits | Behalten | Phase 1 |
| „Warum ist diese Datei relevant?“ | Nicht übernehmen | aktuell kein Bedarf |
| Arbeitskorb | Zurückstellen | Zukunfts-Backlog |
| Beziehungen zwischen Dateien | Zurückstellen | mögliche spätere Ausbaustufe |
| KI-Assistent | Niedrige Priorität | vorerst unwichtig; erst nach Phase 2 erneut bewerten |
| Terminal-Integration | Behalten | fester Bestandteil für Linux-/Informatik-Projekte |
| Technisches Markdown | Behalten | Codeblöcke, Diagramme, Formeln und sichere Befehlsübernahme |
| Codevorschau | Behalten | gute Lese- und Kleinfunktionen; komplexe Arbeit im externen Editor |
| Universelle Diff-Ansicht | Zurückstellen | sehr interessant, aber für eine spätere Version |
| Linux-Dateirechte, Besitzer und Gruppe | Behalten | Phase 1 |
| Papierkorb und versteckte Dateien | Behalten | Phase 1 |
| Entwicklersuche | Behalten | reguläre Ausdrücke, Dateifilter und `.gitignore` |
| Benutzerdefinierte Aktionen | Behalten | Phase 2; projektbezogene Befehle im Terminal |
| Remote- und Container-Umgebungen | Zurückstellen | mögliche spätere Ausbaustufe |
| Git-Integration | Zurückstellen | Zukunfts-Backlog |
| Automatisch erkannte Projektaufgaben | Zurückstellen | Zukunfts-Backlog |
| Projektmodell | Festgelegt | Nutzer wählt genau einen festen Projektordner aus |
| PDF-Annotationen | Behalten | Phase 2 |
| Erweiterungssystem | Behalten | Phase 2; zunächst deklarativ und aktionsorientiert |

## Technischer Rahmen

- Linux-first mit nativer Wayland-Unterstützung; Hyprland ist ein primäres Zielsystem
- NixOS ist die erste Entwicklungs- und Zielplattform; Arch Linux folgt zum Ende der Stabilisierung
- XWayland darf kein notwendiger Bestandteil des normalen Betriebs sein
- Qt 6/QML für die Oberfläche und Rust für den System- und Anwendungskern
- reproduzierbare Nix-Entwicklungsumgebung und Nix-Paket beziehungsweise Flake
- Arch-Paketierung gegen Ende der Stabilisierung über ein `PKGBUILD`, später optional AUR-Veröffentlichung
- PDF-Engine: reproduzierbarer Vergleich von Poppler und PDFium; performanteste ausreichend korrekte Engine verwenden
- CXX-Qt als bevorzugte Brücke zwischen QML und Rust
- Projektsuche über `ripgrep`; ein zusätzlicher Inhaltsindex wird erst bei nachgewiesenem Bedarf eingeführt
- Notizen als frei platzierbare `.md`-Dateien ohne zentralen Notizordner; Tags im YAML-Frontmatter
- Projektkonfiguration, Sitzungen, Suchzustand und UI-Metadaten im versteckten Projektordner `.atlas/` speichern
- dauerhafte Metadaten von löschbaren Index- und Cache-Daten innerhalb von `.atlas/` trennen
- Dateiidentität zunächst über Pfad, Geräte-ID und Inode; leichter Fingerabdruck als Reparaturhilfe
- Open-Source-Entwicklung unter GPL-3.0-or-later

## Erfolgskriterien

- Ein lokales PDF ist in höchstens zwei Interaktionen geöffnet.
- Nach einem Neustart kann die Arbeit ohne erneutes Einrichten fortgesetzt werden.
- Ab Phase 2 lässt sich ein markiertes PDF-Zitat in unter zehn Sekunden dauerhaft festhalten.
- Der Nutzer muss für den Kernablauf kein Konto und keine separate Bibliothek anlegen.
- Eine bestehende Ordnerstruktur kann unverändert genutzt und wieder verlassen werden.

## Offene Entscheidungen

- Produktname und visuelle Identität
- endgültige PDF-Engine ergibt sich aus dem Phase-0-Benchmark
- genaue Aufteilung zwischen versionierbaren, gerätespezifischen und rekonstruierbaren Dateien in `.atlas/`
- Annotationen in Phase 2: PDF, Begleitdatei oder wählbares Modell
- globale Tags: Betriebssystem-Metadaten, eigene Datenbank oder Sidecar-Dateien
- technische Umsetzung von Besitzerwechseln und privilegierten Dateioperationen

