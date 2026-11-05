<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Testkatalog für die UI-Tests (XCUITest)

Vorlage für P4.1: jedes Szenario des Linux-Selbsttests
[`crates/atlas-app/qml/SelfTest.qml`](../../crates/atlas-app/qml/SelfTest.qml)
(`atlas --self-test`, `scripts/smoke-test.sh`) mit seinen Prüfungen, dem Bereich der
Mac-App und der Art des Tests dort. Das Testprojekt ist dasselbe (`scripts/smoke-test.sh`
legt es an).

- **UI:** XCUITest in der App (`AtlasUITests`), gleiche Schritte und Prüfnamen, damit
  Linux- und Mac-Ausgabe vergleichbar bleiben.
- **AtlasKit:** reine Logik ohne Oberfläche; als `swift test` in `AtlasKit` gegen
  `atlas-api` (oder schon durch die Rust-Tests in `crates/atlas-api/tests/` abgedeckt).
- **entfällt:** prüft eine Eigenheit von Qt/Wayland/QtWebEngine.

Der Katalog ist aus `SelfTest.qml` erzeugt (Stand: 210 Schritte, 450
Prüfungen). Kommen dort Schritte hinzu, wird er nachgetragen; „Zeile“ verweist auf den
Schritt in `SelfTest.qml`. Dazu kommt die Hilfsprüfung „Tab N sichtbar“ aus `dragTabTo`, die bei jedem Ziehen in S12 mitläuft.

## Übersicht

| Nr. | Szenario | Schritte | Prüfungen | Bereich (Aufgabe) | Test auf dem Mac |
|---|---|---|---|---|---|
| S1 | [Markdown öffnen, bearbeiten, automatisch speichern](#s1) | 1–3 | 9 | Markdown (P3.3) | UI |
| S2 | [Arbeitsleiste und Symbole](#s2) | 4–5 | 2 | App (P3.0), Code (P3.4) | UI |
| S3 | [Projektsuche: Optionen, Typfilter, PDF-Text, Treffer markiert](#s3) | 6–14 | 14 | Suche (P3.8) | UI |
| S4 | [Terminal-Tab, Ausgabe in Notiz, Sitzung](#s4) | 15–17 | 6 | Terminal (P3.5) | UI |
| S5 | [PDF: Anmerkungen, Lesezeichen, Freihand, Zitat, Export](#s5) | 18–22 | 13 | PDF (P3.2) | UI |
| S6 | [Links folgen Umbenennungen; Graph](#s6) | 23–26 | 11 | Kontextleiste (P3.6), Dateien (P3.1) | UI |
| S7 | [Markdown-Editor: Wiki-Links, Listen, Tabellen, Checkboxen](#s7) | 27 | 33 | Markdown (P3.3) | UI |
| S8 | [Markdown-Vorschau: Codeblöcke, Ordnerbaum, Tags, Einklappen, Bildfenster](#s8) | 28–29 | 8 | Markdown (P3.3), Ansichten (P3.12) | UI |
| S9 | [Aktionen und Erweiterungen (Freigabe, Exporter, bnki)](#s9) | 30–34 | 39 | Erweiterungen (P3.10) | UI |
| S10 | [Jupyter-Notebook-Ansicht](#s10) | 35–37 | 4 | Erweiterungen (P3.10) | UI |
| S11 | [Kalender: Termin, Datei-Link, Frist, Woche, Monat, Agenda](#s11) | 38–41 | 21 | Kalender (P3.7) | UI |
| S12 | [Tabs und Splits ziehen](#s12) | 42–58 | 20 | App (P3.0) | UI |
| S13 | [Seitenleiste und Splits](#s13) | 59–61 | 3 | App (P3.0) | UI |
| S14 | [PDF-Zoom mit Mausrad und Touchpad](#s14) | 62–67 | 17 | PDF (P3.2) | UI (Gesten statt Wayland-Ereignisse) |
| S15 | [Vorschau: Checkboxen, Neuzeichnen](#s15) | 68–72 | 8 | Markdown (P3.3) | UI |
| S16 | [Bleibende Tabs, leeres Fenster](#s16) | 73–74 | 6 | App (P3.0) | UI |
| S17 | [Neue Session, Ordnerauswahl, Pfadleiste](#s17) | 75–80 | 7 | App (P3.0), Dateien (P3.1) | UI |
| S18 | [Projektbaum und Schnellansicht](#s18) | 81–85 | 6 | Dateien (P3.1) | UI (QuickLook) |
| S19 | [Ordneransicht: Vorschau, Ansichten, Auswahl, Zoom](#s19) | 86–92 | 10 | Dateien (P3.1) | UI |
| S20 | [Kalender-Passwort: Befehl, merken, vergessen](#s20) | 93–99 | 10 | Kalender (P3.7) | AtlasKit + UI |
| S21 | [Terminal-Panel ein/aus und teilen](#s21) | 100–104 | 6 | Terminal (P3.5) | UI |
| S22 | [Orte, Graph-Fenster, Einstellungen, Farbschema](#s22) | 105 | 9 | Dateien (P3.1), Einstellungen (P3.9) | UI |
| S23 | [Zitate im Projekt](#s23) | 106–110 | 6 | Kontextleiste (P3.6) | UI |
| S24 | [Kalender-Knopf](#s24) | 111–112 | 2 | Kalender (P3.7) | UI |
| S25 | [Kontextmenüs der Dateien](#s25) | 113–114 | 7 | Dateien (P3.1) | UI |
| S26 | [HTML-Ansicht](#s26) | 115–121 | 12 | Ansichten (P3.12) | UI |
| S27 | [Geänderte PDF, PDF-Links](#s27) | 122–128 | 6 | PDF (P3.2) | UI |
| S28 | [Stift, Radierer, Formel als LaTeX](#s28) | 129–137 | 6 | PDF (P3.2) | UI |
| S29 | [Präsentationsmodus](#s29) | 138–141 | 12 | PDF (P3.2) | UI |
| S30 | [Fokus beim Tab-Wechsel](#s30) | 142–145 | 2 | App (P3.0) | UI |
| S31 | [Zitat mit Formel und Bild, PDF-Tastatur, Rückgängig](#s31) | 146–149 | 14 | PDF (P3.2) | UI |
| S32 | [Touchpad-Schwung, Markdown als PDF, Kontextleisten-Module, Pomodoro](#s32) | 150–151 | 11 | Markdown (P3.3), Kontextleiste (P3.6) | UI |
| S33 | [Sortierung](#s33) | 152–154 | 3 | Dateien (P3.1) | UI |
| S34 | [Geteilte Ansicht: Scroll-Sync, Mausrad, Schwung](#s34) | 155–163 | 7 | Markdown (P3.3) | UI |
| S35 | [Gelöschte Datei schließt ihren Tab](#s35) | 164–166 | 3 | App (P3.0) | UI |
| S36 | [ILIAS: Anmeldung, Favoriten, Download, neue Version](#s36) | 167–182 | 20 | ILIAS (P3.11) | UI mit Testserver |
| S37 | [Projektordner-Zeile im Baum](#s37) | 183–185 | 2 | Dateien (P3.1) | UI |
| S38 | [Kontextleiste: Reiter, Bausteine, Profile](#s38) | 186–192 | 50 | Kontextleiste (P3.6) | UI |
| S39 | [Skripte ausführen](#s39) | 193–195 | 2 | Code (P3.4), Terminal (P3.5) | UI |
| S40 | [Dateien verknüpfen, Ordner-Knoten](#s40) | 196–201 | 10 | Kontextleiste (P3.6) | UI |
| S41 | [Sessions in anderen Ordnern](#s41) | 202–205 | 6 | App (P3.0) | UI |
| S42 | [Ressourcen: PDF-Dokumente und Vorschauen freigeben](#s42) | 206–210 | 7 | PDF (P3.2), Markdown (P3.3) | entfällt bzw. Instruments (QtWebEngine-spezifisch) |

## S1

**Markdown öffnen, bearbeiten, automatisch speichern** · Markdown (P3.3) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 1 | 139 | Projekt geöffnet |
| 2 | 143 | Markdown-Ansicht |
|  |  | Markdown-Hervorhebung mit Codeblöcken (Atlas Markdown) |
|  |  | Bearbeitung erkannt |
| 3 | 152 | Autosave geschrieben |
|  |  | keine Recovery nach Speichern |
|  |  | Rückverweis auf plan.md |
|  |  | Tags hierarchisch |
|  |  | defekter Link markiert |

## S2

**Arbeitsleiste und Symbole** · App (P3.0), Code (P3.4) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 4 | 162 | Arbeitsleiste findet src/main.rs |
| 5 | 167 | Symbole erkannt |

## S3

**Projektsuche: Optionen, Typfilter, PDF-Text, Treffer markiert** · Suche (P3.8) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 6 | 173 | Suche ohne Build-Ordner |
|  |  | Suche im eigenen Fenster vor Atlas |
|  |  | Suchtreffer markiert |
| 7 | 180 | Suche ignoriert Groß-/Kleinschreibung |
| 8 | 184 | Suche beachtet Groß-/Kleinschreibung auf Wunsch |
| 9 | 188 | Suche nach ganzen Wörtern |
| 10 | 192 | Suche: Typfilter Markdown |
| 11 | 196 | Suche findet Text in PDFs |
|  |  | Suche: Vorschau der PDF-Seite |
| 12 | 204 | Suchtreffer öffnet als fester Tab im aktiven Split |
|  |  | PDF-Treffer öffnet die Seite und markiert das Wort |
| 13 | 213 | Suche: Treffer in plan.md |
| 14 | 219 | Markdown-Treffer im Editor ausgewählt |
|  |  | Markdown-Treffer in der Vorschau markiert |

## S4

**Terminal-Tab, Ausgabe in Notiz, Sitzung** · Terminal (P3.5) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 15 | 228 | Terminal-Tab gestartet |
|  |  | Terminal im gewählten Ordner |
| 16 | 234 | Terminal-Ausgabe lesbar |
|  |  | Ausgabe in Notiz |
|  |  | Rechte setzen |
| 17 | 242 | Session enthält Tabs |

## S5

**PDF: Anmerkungen, Lesezeichen, Freihand, Zitat, Export** · PDF (P3.2) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 18 | 252 | PDF-Ansicht |
|  |  | Klick in den PDF-Split: Pfeiltasten ohne Klick auf die Seite |
|  |  | Anmerkungen geladen |
| 19 | 267 | Text ausgewählt |
|  |  | Markierung angelegt |
|  |  | Lesezeichen und Freihand |
|  |  | Zitat mit Quelle und Seite |
|  |  | Zitat in Notiz |
|  |  | @-Suche findet Markierung |
|  |  | Zitat-Link mit Annotation |
| 20 | 287 | (Vorbereitung, keine Prüfung) |
| 21 | 291 | Zitat öffnet die Stelle |
| 22 | 296 | Export als Kopie |
|  |  | Original-PDF unverändert |

## S6

**Links folgen Umbenennungen; Graph** · Kontextleiste (P3.6), Dateien (P3.1) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 23 | 301 | Zitat-Link folgt Umbenennung |
|  |  | Anmerkungen folgen der PDF |
|  |  | Wiki-Link mit Alias |
|  |  | Wiki-Link auf Überschrift |
|  |  | Rückverweis kennt Art, Kontext und Tags |
|  |  | Graph der PDF zeigt zitierende Notiz |
|  |  | Graph zeigt das ganze Projekt |
| 24 | 329 | Wiki-Link folgt Umbenennung |
|  |  | Kontextpanel mit Graph |
| 25 | 340 | Graph: Mausrad ohne Strg zoomt nicht |
| 26 | 348 | Graph: Strg+Mausrad zoomt |

## S7

**Markdown-Editor: Wiki-Links, Listen, Tabellen, Checkboxen** · Markdown (P3.3) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 27 | 353 | Wiki-Link-Vorschläge |
|  |  | Wiki-Link vervollständigt |
|  |  | Markdown mit Zeilennummern |
|  |  | Tab rückt Listenpunkte ein und aus, nicht normalen Text |
|  |  | Enter: neuer Spiegelstrich, leerer Punkt beendet die Liste |
|  |  | Enter nach Checkbox: neue offene Checkbox |
|  |  | Enter in nummerierter Liste zählt weiter |
|  |  | Enter nach normalem Text: nur Zeilenumbruch |
|  |  | Tabelle einfügen (ausgerichtet) |
|  |  | Tabelle: Tab richtet die Spalten aus (mit Ausrichtung) |
|  |  | Tabelle: Tab markiert die nächste Zelle |
|  |  | Tabelle: Tab springt über die Trennzeile |
|  |  | Tabelle: Umschalt+Tab geht zurück |
|  |  | Tabelle: Tab in der letzten Zelle fügt eine Zeile an |
|  |  | Tabelle: Enter am Zeilenende fügt eine Zeile an |
|  |  | Tabelle: Enter in leerer letzter Zeile beendet die Tabelle |
|  |  | Tabellen ausrichten: ganze Notiz |
|  |  | Diagramm-Vorlage einfügen |
|  |  | Ordnerbaum einfügen |
|  |  | Datei aus dem Projekt einfügen: Link relativ zur Notiz |
|  |  | Strg+V mit Text: normales Einfügen |
|  |  | # schlägt Tags des Projekts vor |
|  |  | Tag vervollständigt |
|  |  | Rechtschreibprüfung: Tippfehler unterstrichen, richtige Wörter nicht |
|  |  | Rechtschreibprüfung: Vorschlag |
|  |  | Rechtschreibprüfung: eigenes Wörterbuch |
|  |  | Rechtschreibprüfung: alte Stellen hinter dem Textende ignoriert |
|  |  | Aktionsdatei fehlerfrei |
|  |  | Aktion passt zu Markdown |
|  |  | Aktion passt zu Ordnern |
|  |  | Projektaktion |
|  |  | Programm ohne Shell, Arbeitsordner Projekt |
|  |  | Umleitung verlangt Bestätigung |

## S8

**Markdown-Vorschau: Codeblöcke, Ordnerbaum, Tags, Einklappen, Bildfenster** · Markdown (P3.3), Ansichten (P3.12) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 28 | 485 | Vorschau: Kopieren-Knopf an Codeblöcken, Ausführen bei bash |
|  |  | Vorschau: Ordnerbaum mit Linien |
|  |  | Vorschau: #tag im Text |
|  |  | Vorschau: Zitat mit schließendem Anführungszeichen |
|  |  | Vorschau: Überschrift klappt ihren Abschnitt ein und aus |
|  |  | Vorschau: Diagramm auf dem Hintergrund der Seite |
|  |  | Vorschau: Bild mit Vergrößern-Knopf |
| 29 | 509 | Bild vergrößern öffnet eigenes Fenster mit dem Bild |

## S9

**Aktionen und Erweiterungen (Freigabe, Exporter, bnki)** · Erweiterungen (P3.10) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 30 | 516 | Aktion im Projektordner ausgeführt |
|  |  | Aktion sichtbar mit Status |
|  |  | Erweiterungen gefunden |
|  |  | Alte Beispiel-Erweiterung heißt jetzt Core |
|  |  | Projekt-Erweiterung wartet auf Freigabe |
|  |  | Nicht freigegeben: keine Aktionen |
|  |  | Freigabe |
|  |  | Freigegeben: Aktionen da |
|  |  | Ausgeschaltet: keine Aktionen |
|  |  | API 2: Rechte gelesen |
|  |  | API 2: nicht angemeldetes Programm abgelehnt |
|  |  | API 2: Befehl nur in der Arbeitsleiste, mit Kürzel |
|  |  | API 2: Kürzel angemeldet |
|  |  | Einstellung: Standardwert als Platzhalter und Variable |
|  |  | Einstellung ändern |
|  |  | Einstellung: falscher Typ abgelehnt |
|  |  | Einstellung zurückgesetzt |
|  |  | Exporter für Markdown |
| 31 | 555 | Unbekannter Typ über Erweiterung angezeigt |
|  |  | Export geöffnet |
|  |  | Export geschrieben |
|  |  | bnki-Karten installiert |
|  |  | bnki: Ansicht mit Strg+Alt+B |
|  |  | Einstellungsdialog zeigt die Einstellungen von bnki-Karten |
| 32 | 570 | bnki: Tab mit der Seite der Erweiterung |
|  |  | bnki: Ordner ohne Projekt-Ordner bleibt unangetastet |
| 33 | 580 | bnki: Stapel als CSV-Datei im Projekt gespeichert |
|  |  | bnki: Dateien außerhalb des Projekts gesperrt |
| 34 | 590 | bnki: KaTeX von Atlas, Formel in der Liste gesetzt |
|  |  | bnki: Farben von Atlas |
|  |  | bnki: CSV-Datei mit „Anzeigen mit“ als eigener Tab |
|  |  | bnki deinstalliert |
|  |  | Deinstalliert: nicht mehr in der Liste, Kürzel frei |
|  |  | Deinstalliert: wieder installierbar |
|  |  | Deinstalliert: offener Tab sagt es |
|  |  | Deinstalliert: Stapel bleiben |
|  |  | Neu installiert: Einstellungen wie neu |
|  |  | Jupyter installiert |
|  |  | Jupyter: Notebook über die Brücke geschrieben |

## S10

**Jupyter-Notebook-Ansicht** · Erweiterungen (P3.10) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 35 | 617 | Jupyter: .ipynb öffnet in der Notebook-Ansicht |
|  |  | Jupyter: Ansicht nicht als Zustand gespeichert |
| 36 | 627 | Jupyter: Zellen, Formel und Ausgabe gezeigt |
| 37 | 633 | Jupyter: „Datei selbst“ zeigt das JSON |

## S11

**Kalender: Termin, Datei-Link, Frist, Woche, Monat, Agenda** · Kalender (P3.7) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 38 | 639 | Kalender als Tab |
|  |  | Ohne Nextcloud: Projektkalender |
|  |  | Termin angelegt |
|  |  | events.ics im iCalendar-Format |
|  |  | Datei verknüpft |
|  |  | links.json über die UID |
|  |  | Woche zeigt den Termin |
|  |  | Notiz-Frist aus dem Frontmatter |
|  |  | Termin verschoben |
|  |  | Checkliste erkannt |
|  |  | Aufgaben übernommen |
|  |  | Markdown unverändert |
|  |  | Noch nicht eingeplant |
|  |  | Aufgabe eingeplant |
|  |  | Als Aufgabe einplanen |
|  |  | Arbeitsleiste findet Termine |
| 39 | 672 | Dateilink folgt Umbenennung |
|  |  | Kalender-Tab in der Session |
|  |  | Löschen |
| 40 | 683 | Monatsansicht |
| 41 | 688 | Agenda |

## S12

**Tabs und Splits ziehen** · App (P3.0) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 42 | 698 | Ausgangslage: zwei Splits |
| 43 | 704 | Tab in anderen Split: im alten verschwunden |
|  |  | Tab im neuen Split, aktiv |
|  |  | Terminal läuft nach dem Ziehen weiter |
|  |  | Ziehzustand zurückgesetzt |
|  |  | Sichtbar: verschobener Tab im Ziel, nächster im alten Split |
| 44 | 716 | An den Rand: neuer Split rechts |
|  |  | … und im alten Split verschwunden |
| 45 | 723 | Letzter Tab verschoben: leerer Split geschlossen |
| 46 | 731 | Auf den eigenen Split: unverändert |
| 47 | 741 | Im Tab-Streifen umsortiert |
| 48 | 748 | Einziger Tab an den eigenen Rand: kein leerer Split |
| 49 | 755 | Abgebrochenes Ziehen: unverändert |
| 50 | 762 | Gleiche Datei im Ziel: nur ein Tab |
| 51 | 771 | Vier Splits |
| 52 | 777 | Bei vier Splits: in den Split statt neuem |
| 53 | 784 | Kalender im Split |
| 54 | 790 | Tab auf Split mit Kalender verschoben |
|  |  | Session: jeder Tab genau einmal, in seinem Split |
| 55 | 800 | (Vorbereitung, keine Prüfung) |
| 56 | 806 | (Vorbereitung, keine Prüfung) |
| 57 | 812 | An den linken Rand: neuer Split links |
| 58 | 821 | (Vorbereitung, keine Prüfung) |

## S13

**Seitenleiste und Splits** · App (P3.0) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 59 | 825 | Seitenleiste zu: Splits füllen den Platz |
| 60 | 830 | Breitere Seitenleiste: Splits passen sich an |
| 61 | 834 | Schmalere Seitenleiste: Splits wachsen mit |

## S14

**PDF-Zoom mit Mausrad und Touchpad** · PDF (P3.2) · Test: UI (Gesten statt Wayland-Ereignisse)

| Schritt | Zeile | Prüfung |
|---|---|---|
| 62 | 839 | Wayland: Strg+Mausrad vom „Touchpad“-Gerät zoomt |
|  |  | Strg+Mausrad ohne Gestenbeginn zoomt |
|  |  | Strg+Mausrad vergrößert die PDF |
|  |  | Strg+Mausrad zurück verkleinert |
|  |  | Mausrad allein zoomt nicht |
|  |  | Mausrad: Stufe wird weich gescrollt |
|  |  | Mausrad scrollt eine große Stufe |
|  |  | Touchpad unter Wayland: Ansicht folgt den Fingern genau |
|  |  | PDF-Dunkelmodus: Seiten invertiert, Papier dunkel |
|  |  | PDF-Dunkelmodus aus |
|  |  | Strg+Mausrad: schmale Seite bleibt mittig |
|  |  | Strg+Touchpad zoomt die PDF |
|  |  | Wayland: Strg+Mausrad mitten in einer Scroll-Geste zoomt |
|  |  | „In Notiz übernehmen“ öffnet |
|  |  | Sessionliste mit Umfang |
|  |  | Sessionfenster öffnet |
| 63 | 920 | (Vorbereitung, keine Prüfung) |
| 64 | 927 | (Vorbereitung, keine Prüfung) |
| 65 | 935 | Strg+Mausrad auf Seite 2: die Stelle unter der Maus bleibt (kein Wegscrollen) |
| 66 | 946 | (Vorbereitung, keine Prüfung) |
| 67 | 952 | (Vorbereitung, keine Prüfung) |

## S15

**Vorschau: Checkboxen, Neuzeichnen** · Markdown (P3.3) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 68 | 956 | Checkbox in der Vorschau hakt ab |
| 69 | 961 | … und wieder ab |
|  |  | Überschriften in der Vorschau ohne # |
|  |  | Abhaken gespeichert |
| 70 | 975 | Vorschau zeichnet nach dem Wiederzeigen neu |
| 71 | 981 | Vorschau: Momentaufnahme beim Verlassen |
|  |  | Zurück: Momentaufnahme sofort sichtbar statt Schwarz |
| 72 | 988 | … und danach wieder die echte Vorschau |

## S16

**Bleibende Tabs, leeres Fenster** · App (P3.0) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 73 | 992 | Einzelklick öffnet bleibende Tabs (keine Vorschau-Tabs) |
| 74 | 1003 | Leeres Fenster ohne Schließen-Knopf |
|  |  | Öffnen im Split nutzt das leere Fenster |
|  |  | Fenster mit Dokument hat Schließen-Knopf |
|  |  | Split neben leerem Fenster: kein leerer Split bleibt |
|  |  | Temporärer Split ersetzt das leere Fenster |

## S17

**Neue Session, Ordnerauswahl, Pfadleiste** · App (P3.0), Dateien (P3.1) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 75 | 1017 | Neue Session fragt nach dem Projektordner |
| 76 | 1024 | Ordnerauswahl: nur Ordner |
|  |  | Pfadleiste endet im Ordner |
| 77 | 1035 | Pfadleiste: Klick daneben zum Eintippen |
| 78 | 1040 | Ordnerauswahl listet Unterordner |
| 79 | 1045 | Eine Session je Projektordner |
|  |  | Projektordner öffnen: eigener Dialog |
| 80 | 1053 | (Vorbereitung, keine Prüfung) |

## S18

**Projektbaum und Schnellansicht** · Dateien (P3.1) · Test: UI (QuickLook)

| Schritt | Zeile | Prüfung |
|---|---|---|
| 81 | 1058 | PDF im Baum sichtbar |
| 82 | 1065 | Strg+Klick: Schnellansicht offen |
|  |  | … ohne neuen Tab |
| 83 | 1070 | Schnellansicht: PDF ganzseitig, ohne eigene Leiste |
|  |  | Schnellansicht: Seiten und Infos |
| 84 | 1075 | Schnellansicht → im Split öffnen |
| 85 | 1079 | (Vorbereitung, keine Prüfung) |

## S19

**Ordneransicht: Vorschau, Ansichten, Auswahl, Zoom** · Dateien (P3.1) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 86 | 1084 | Ordner: PDF-Vorschau mit der ersten Seite |
| 87 | 1091 | (Vorbereitung, keine Prüfung) |
| 88 | 1095 | Ordner: Symbolansicht |
|  |  | Ordner: Statusleiste zählt |
|  |  | Ordner: nichts vorausgewählt |
| 89 | 1108 | Ordner: Klick wählt aus |
|  |  | Ordner: Bild am Mauszeiger fürs Ziehen bereit |
|  |  | Ordner: Strg+Mausrad zoomt |
| 90 | 1117 | Ordner: Detailansicht behält Auswahl |
|  |  | Ordner: Ansicht im Sitzungszustand |
| 91 | 1134 | Ordner: Markierungsrechteck wählt die Zeilen darin |
| 92 | 1141 | (Vorbereitung, keine Prüfung) |

## S20

**Kalender-Passwort: Befehl, merken, vergessen** · Kalender (P3.7) · Test: AtlasKit + UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 93 | 1146 | Passwort-Befehl: Fehler sichtbar |
| 94 | 1152 | Passwort-Befehl liefert das App-Passwort |
|  |  | Passwort-Befehl steht im Konto, das Passwort nicht |
| 95 | 1161 | Passwort gemerkt |
|  |  | Gemerktes Passwort nicht in den Einstellungen |
|  |  | Ohne Passwort im Speicher gesperrt |
| 96 | 1172 | Nach Neustart ohne Eingabe entsperrt |
|  |  | Nicht mehr merken: gelöscht |
| 97 | 1180 | Wieder merken mit bekanntem Passwort |
|  |  | Konto getrennt: Passwort vergessen |
| 98 | 1187 | (Vorbereitung, keine Prüfung) |
| 99 | 1193 | (Vorbereitung, keine Prüfung) |

## S21

**Terminal-Panel ein/aus und teilen** · Terminal (P3.5) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 100 | 1200 | Terminal-Knopf: ein Klick blendet aus |
| 101 | 1208 | Terminal-Knopf: ein Klick blendet ein |
| 102 | 1215 | Terminal nebeneinander geteilt, gleich breit |
| 103 | 1224 | Terminal untereinander: nur das aktive geteilt |
| 104 | 1236 | Terminal-Splits in der Session als Baum |
|  |  | Geteilte Terminals wieder geschlossen |

## S22

**Orte, Graph-Fenster, Einstellungen, Farbschema** · Dateien (P3.1), Einstellungen (P3.9) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 105 | 1245 | Orte wie in Dolphin |
|  |  | Graph im eigenen Fenster |
|  |  | Einstellungen öffnen |
|  |  | Helles Farbschema |
|  |  | Unbekanntes Farbschema wird dunkel |
|  |  | Einstellungen: ILIAS-Adresse einstellbar (ohne https:// ergänzt, Ungültiges abgelehnt) |
|  |  | Einstellungen: Kalender für neue Termine wählbar |
|  |  | Ausgeschaltet: feste Orte |
|  |  | Wieder an: Dolphins Orte |

## S23

**Zitate im Projekt** · Kontextleiste (P3.6) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 106 | 1283 | Zitate im Projekt: Markierung mit Notiz |
|  |  | Verknüpfungen: Zitate nach PDF gruppiert |
| 107 | 1301 | (Vorbereitung, keine Prüfung) |
| 108 | 1307 | Neue Markierung erscheint nach dem Schließen |
|  |  | Zitatliste hat Platz |
| 109 | 1322 | Zitat öffnet die PDF-Stelle |
| 110 | 1333 | Gelöschte Markierungen verschwinden |

## S24

**Kalender-Knopf** · Kalender (P3.7) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 111 | 1346 | Kalender-Knopf öffnet |
| 112 | 1353 | Kalender-Knopf schließt |

## S25

**Kontextmenüs der Dateien** · Dateien (P3.1) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 113 | 1357 | Kontextmenü PDF: nur Passendes |
|  |  | Kontextmenü Notiz: externer Editor |
|  |  | Kontextmenü: „Checkliste als Aufgaben“ nur bei einer Checkliste |
|  |  | Kontextmenü leere Fläche: keine Datei-Befehle |
|  |  | Kontextmenü Datei: ohne Startprogramm, Dateimanager, Neu |
|  |  | Kontextmenü Ordner: Neu, ohne Startprogramm und Dateimanager |
| 114 | 1393 | HTML-Aktion im Kontextmenü |

## S26

**HTML-Ansicht** · Ansichten (P3.12) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 115 | 1403 | HTML gerendert |
| 116 | 1411 | HTML: Skript und CSS wirken |
| 117 | 1416 | HTML: Quelltext im selben Tab |
| 118 | 1422 | HTML: wieder gerendert, kein zweiter Tab |
|  |  | HTML: Ansicht in der Sitzung |
|  |  | HTML geändert |
| 119 | 1429 | HTML lädt beim Speichern neu |
|  |  | Eigene Seite ohne Netz-Hinweis |
| 120 | 1437 | Seite mit Web-Inhalten: Hinweis |
|  |  | „Trotzdem laden“ gilt für die Datei |
| 121 | 1445 | Seitenfehler in der Ansicht |
|  |  | Bildformate von Qt |

## S27

**Geänderte PDF, PDF-Links** · PDF (P3.2) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 122 | 1453 | (Vorbereitung, keine Prüfung) |
| 123 | 1457 | PDF verändert |
| 124 | 1464 | (Vorbereitung, keine Prüfung) |
| 125 | 1467 | Geänderte PDF mit Anmerkung: Hinweis |
| 126 | 1475 | (Vorbereitung, keine Prüfung) |
| 127 | 1478 | Geänderte PDF ohne Anmerkungen: kein Hinweis |
| 128 | 1484 | PDF-Links gelesen |
|  |  | Kein Link neben den Links |
|  |  | Link springt zur Zielseite |

## S28

**Stift, Radierer, Formel als LaTeX** · PDF (P3.2) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 129 | 1497 | (Vorbereitung, keine Prüfung) |
| 130 | 1506 | (Vorbereitung, keine Prüfung) |
| 131 | 1513 | Radierer löscht die berührten Striche ganz |
|  |  | Radier-Zug ist ein Rückgängig-Schritt |
|  |  | Strg+Z holt beide Striche zurück |
| 132 | 1525 | (Vorbereitung, keine Prüfung) |
| 133 | 1539 | Formel als LaTeX: Rahmen aufziehen erkennt die Formel (eingebautes Modell) |
|  |  | Formel als LaTeX: Werkzeug bleibt aktiv, kein Rahmen übrig |
| 134 | 1547 | (Vorbereitung, keine Prüfung) |
| 135 | 1553 | Text der Seite: Formel als LaTeX eingesetzt, übriger Text bleibt |
| 136 | 1561 | (Vorbereitung, keine Prüfung) |
| 137 | 1568 | (Vorbereitung, keine Prüfung) |

## S29

**Präsentationsmodus** · PDF (P3.2) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 138 | 1576 | Präsentation (F5): eigenes Vollbildfenster, von Seite 1, nur diese Seite zu sehen |
|  |  | Präsentation: keine Leiste, keine Statuszeile |
|  |  | Präsentation: Größenänderung im Tab erreicht sie nicht |
| 139 | 1593 | Präsentation: Pfeil rechts zeigt die nächste Seite allein |
|  |  | Präsentation: nach der letzten Seite bleibt sie stehen |
|  |  | Präsentation: Pfeil links zurück |
| 140 | 1605 | Präsentation: Klick zeigt die nächste Seite |
|  |  | Präsentation: Mausrad hoch zeigt die vorige Seite |
|  |  | Präsentation: Zahl + Enter springt zur Seite |
|  |  | Präsentation: B macht den Bildschirm schwarz |
|  |  | Präsentation: nächste Taste hebt Schwarz auf, ohne zu blättern |
| 141 | 1621 | Esc beendet die Präsentation: zurück im Tab, vorheriger Zoom, zuletzt gezeigte Seite |

## S30

**Fokus beim Tab-Wechsel** · App (P3.0) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 142 | 1629 | (Vorbereitung, keine Prüfung) |
| 143 | 1636 | Tab-Wechsel: der neue Tab (Ordner) bekommt den Fokus |
| 144 | 1644 | Tab-Wechsel zurück: die PDF hat den Fokus, nicht mehr der Ordner |
| 145 | 1652 | (Vorbereitung, keine Prüfung) |

## S31

**Zitat mit Formel und Bild, PDF-Tastatur, Rückgängig** · PDF (P3.2) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 146 | 1659 | Zitat aus PDF: Formel als LaTeX im Zitat |
|  |  | Bild aus PDF als Zitat: PNG in assets/, Bildunterschrift mit Link auf die Seite |
| 147 | 1672 | Pfeil runter scrollt die PDF |
|  |  | Pfeil rechts blättert weiter |
|  |  | Pfeil links blättert zurück |
|  |  | Strich gezeichnet |
|  |  | Strg+Z nimmt den Strich zurück |
|  |  | Strg+Umschalt+Z stellt ihn wieder her (gleiche ID) |
|  |  | Rückgängig: Farbe wieder wie vorher |
|  |  | Rückgängig: Löschen zurückgenommen |
|  |  | Wiederholen: wieder gelöscht |
|  |  | Touchpad: Schwung setzt beim Loslassen sofort ein |
|  |  | Touchpad: Finger abheben stoppt den Schwung nicht |
|  |  | Markdown-PDF-Export startet |
| 148 | 1722 | (Vorbereitung, keine Prüfung) |
| 149 | 1728 | (Vorbereitung, keine Prüfung) |

## S32

**Touchpad-Schwung, Markdown als PDF, Kontextleisten-Module, Pomodoro** · Markdown (P3.3), Kontextleiste (P3.6) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 150 | 1729 | Touchpad-Wischen läuft mit Schwung weiter |
|  |  | Markdown als PDF exportiert |
|  |  | Module der Kontextleiste eingeschaltet |
|  |  | Heute zeigt den Termin von heute, nicht den von morgen |
|  |  | Aufgaben zeigen die neue Aufgabe |
|  |  | Erledigte Aufgabe verschwindet |
|  |  | Kalendermodul abschaltbar |
|  |  | Pomodoro unten fixiert, auch auf anderen Reitern |
|  |  | Pomodoro abschaltbar |
|  |  | Pomodoro läuft |
| 151 | 1771 | Pomodoro: nach dem Lernen kommt die Pause |

## S33

**Sortierung** · Dateien (P3.1) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 152 | 1780 | Natürliche Sortierung: 2 vor 10 |
| 153 | 1787 | Ohne natürliche Sortierung: nach Zeichen |
|  |  | Gewählte Sortierung gemerkt |
| 154 | 1800 | (Vorbereitung, keine Prüfung) |

## S34

**Geteilte Ansicht: Scroll-Sync, Mausrad, Schwung** · Markdown (P3.3) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 155 | 1805 | Split: nach dem Öffnen gleich ausgerichtet |
| 156 | 1814 | Split: Vorschau folgt dem Editor, Mitte auf gleicher Höhe |
| 157 | 1823 | Split: Editor folgt der Vorschau, Mitte auf gleicher Höhe |
| 158 | 1830 | (Vorbereitung, keine Prüfung) |
| 159 | 1836 | Split: Neurendern der Vorschau bewegt den Editor nicht |
| 160 | 1841 | (Vorbereitung, keine Prüfung) |
| 161 | 1851 | Editor: Mausrad scrollt weiter, ohne Schwung |
| 162 | 1860 | Editor: Touchpad-Schwung |
|  |  | Finger auflegen stoppt den Schwung |
| 163 | 1872 | (Vorbereitung, keine Prüfung) |

## S35

**Gelöschte Datei schließt ihren Tab** · App (P3.0) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 164 | 1877 | Datei zum Löschen geöffnet |
| 165 | 1883 | In den Papierkorb: ihr Tab schließt sich |
| 166 | 1892 | Außerhalb gelöscht: Tab schließt sich (ohne ungespeicherte Änderungen) |

## S36

**ILIAS: Anmeldung, Favoriten, Download, neue Version** · ILIAS (P3.11) · Test: UI mit Testserver

| Schritt | Zeile | Prüfung |
|---|---|---|
| 167 | 1898 | ILIAS ist der letzte Ort |
|  |  | ILIAS-Tab offen |
| 168 | 1907 | ILIAS: Anmeldung nur verschlüsselt gespeichert |
| 169 | 1916 | ILIAS: ohne Sitzung wird nach der Anmeldung gefragt |
|  |  | ILIAS: fragt einmal, ob die Anmeldung auf dieser Seite eingesetzt werden darf |
| 170 | 1923 | (Vorbereitung, keine Prüfung) |
| 171 | 1924 | ILIAS: nach der Anmeldung nur die Favoriten eingelesen (nicht „Meine Kurse“) |
|  |  | ILIAS: Ordner, Datei und Sitzung im Kurs (ohne Navigation) |
|  |  | ILIAS: Sitzung am Symbol erkannt, mit Datei |
|  |  | ILIAS: Protokoll mit Anmeldung, Seiten und Kursen, ohne Cookie-Werte |
|  |  | ILIAS: Cookie-Datenbank des Web-Profils lesbar (für die Sitzung nach einem Neustart) |
|  |  | ILIAS: Ordner zuerst, dann Dateien |
|  |  | ILIAS: Dateien natürlich sortiert |
| 172 | 1952 | ILIAS: Download fragt nach dem Zielordner (zuletzt gewählter vorgeschlagen) |
| 173 | 1962 | (Vorbereitung, keine Prüfung) |
| 174 | 1963 | ILIAS: Datei landet direkt im gewählten Ordner und öffnet sich |
|  |  | ILIAS: geladene Datei öffnet ohne Nachfrage |
| 175 | 1983 | (Vorbereitung, keine Prüfung) |
| 176 | 1989 | ILIAS: zweite Datei im anderen Ordner |
| 177 | 1999 | (Vorbereitung, keine Prüfung) |
| 178 | 2000 | ILIAS: neue Version erkannt |
| 179 | 2010 | ILIAS: neue Version bietet den Ordner der alten an |
| 180 | 2018 | ILIAS: neue Version geladen und ersetzt die alte |
| 181 | 2025 | (Vorbereitung, keine Prüfung) |
| 182 | 2031 | ILIAS: nach dem Abmelden ohne Zutun wieder angemeldet (gespeicherte Anmeldung) |

## S37

**Projektordner-Zeile im Baum** · Dateien (P3.1) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 183 | 2038 | Projektordner-Zeile ohne Pfeil |
| 184 | 2047 | Projektordner bleibt offen |
| 185 | 2051 | (Vorbereitung, keine Prüfung) |

## S38

**Kontextleiste: Reiter, Bausteine, Profile** · Kontextleiste (P3.6) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 186 | 2056 | Kontextleiste schmal: Reiter als Symbole, kein Seitwärtsscrollen |
| 187 | 2063 | Kontextleiste breit: Reiter ausgeschrieben |
|  |  | Kontextleiste: Standard wie bisher (ohne Profile) |
|  |  | Baustein einklappen: gespeichert in „Allgemein“ |
|  |  | Baustein ausklappen |
|  |  | Baustein nach oben (Alt+↑) |
|  |  | Baustein ausblenden, mit Rückgängig |
|  |  | Ausblenden rückgängig |
|  |  | Graph in der Übersicht ausblenden: Reiter bleibt |
|  |  | Reiter ausblenden: Baustein bleibt in der Übersicht |
|  |  | Reiter ausblenden rückgängig |
|  |  | Reiter Aktionen aus Bausteinen |
|  |  | Baustein in einen Reiter verschieben |
|  |  | … und zurück in die Übersicht |
|  |  | Eigener Reiter angelegt |
|  |  | Eigener Reiter: umbenannt, mit Baustein |
|  |  | Eigener Reiter entfernt (samt Seite) |
|  |  | Baustein als eigener Reiter |
| 188 | 2118 | Als Reiter angezeigt |
|  |  | Baustein unten anheften |
| 189 | 2128 | Markdown ohne eigenes Profil: Allgemein |
|  |  | „Gilt für: nur Markdown“ legt das Profil an |
|  |  | Änderung nur im Markdown-Profil, als Abweichung gespeichert |
|  |  | „Alle Dateitypen“ ändert Allgemein und das Markdown-Profil |
|  |  | Rückgängig stellt alle Profile wieder her |
|  |  | Gliederung der Notiz |
| 190 | 2158 | Tags der Notiz |
|  |  | Dateiinfo |
| 191 | 2163 | Code: kein eigenes Profil, also Allgemein |
|  |  | Leerer Baustein klappt zu |
|  |  | Leere Bausteine ausblenden |
|  |  | Bearbeitungsmodus öffnet mit dem Profil der Datei |
|  |  | Profil noch nicht eingestellt: Vorschlag |
|  |  | Vorschlag übernommen: Code-Profil gilt |
|  |  | Baustein hinzufügen (Galerie) |
|  |  | Baustein ziehen: an den Anfang |
|  |  | Ziehen nach „Unten angeheftet“ |
|  |  | Auge blendet aus, Platz bleibt |
|  |  | Bearbeitungsmodus: Baustein auf den Reiter „Aktionen“ |
|  |  | Bearbeitungsmodus: auf dem Reiter ordnen |
|  |  | Bearbeitungsmodus: vom Reiter entfernen |
|  |  | Bearbeitungsmodus: neuer Reiter gewählt |
|  |  | Bearbeitungsmodus: Reiter benannt |
|  |  | Bearbeitungsmodus: Reiter entfernt, Übersicht gewählt |
|  |  | Reiter entfernen |
|  |  | Zurücksetzen: Profil gelöscht, es gilt Allgemein |
|  |  | Fertig beendet den Bearbeitungsmodus |
| 192 | 2214 | Profile nach Dateityp aus: überall Allgemein |
|  |  | Profile nach Dateityp an: Markdown-Profil |
|  |  | Alle Profile zurückgesetzt: Leiste wie ausgeliefert |

## S39

**Skripte ausführen** · Code (P3.4), Terminal (P3.5) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 193 | 2223 | „Ausführen“ nur bei Skripten |
| 194 | 2234 | Skript lief im Terminal, im eigenen Ordner |
| 195 | 2239 | (Vorbereitung, keine Prüfung) |

## S40

**Dateien verknüpfen, Ordner-Knoten** · Kontextleiste (P3.6) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 196 | 2243 | Verknüpfen: Dialog mit Dateien und Ordnern, ohne versteckte |
| 197 | 2251 | Zwei PDFs verknüpft, im Graph |
| 198 | 2260 | Verknüpfung folgt Umbenennung |
|  |  | Verknüpfung gelöst |
| 199 | 2267 | Ordner: „Als Ordner-Knoten verknüpfen“ im Menü |
|  |  | Ordner-Knoten angelegt |
| 200 | 2278 | Ordner-Knoten im Graph, mit allen Dateien des Ordners |
|  |  | Datei kennt ihren Ordner-Knoten |
|  |  | Ordner-Knoten untereinander verknüpft |
|  |  | Ordner-Knoten auflösen |
| 201 | 2304 | (Vorbereitung, keine Prüfung) |

## S41

**Sessions in anderen Ordnern** · App (P3.0) · Test: UI

| Schritt | Zeile | Prüfung |
|---|---|---|
| 202 | 2315 | Neue Session im anderen Ordner, nach ihm benannt |
|  |  | Sessionliste: ein Eintrag je Ordner |
| 203 | 2328 | Erste Session unverändert |
| 204 | 2336 | Kalender: „In Session wechseln“ öffnet ihren Ordner |
|  |  | Offene Session lässt sich nicht entfernen |
|  |  | Andere Session aus der Liste entfernt |
| 205 | 2343 | (Vorbereitung, keine Prüfung) |

## S42

**Ressourcen: PDF-Dokumente und Vorschauen freigeben** · PDF (P3.2), Markdown (P3.3) · Test: entfällt bzw. Instruments (QtWebEngine-spezifisch)

| Schritt | Zeile | Prüfung |
|---|---|---|
| 206 | 2350 | PDF-Tab hält sein Dokument offen |
| 207 | 2354 | Geschlossener PDF-Tab gibt das Dokument frei |
| 208 | 2360 | Markdown im Bearbeiten-Modus hat keine Vorschau (kein Browser) |
| 209 | 2366 | Vorschau entsteht beim ersten Anzeigen |
|  |  | Verdeckte Vorschau wird verworfen |
| 210 | 2374 | Verworfene Vorschau entsteht beim Zeigen neu |
|  |  | Schritt  |
