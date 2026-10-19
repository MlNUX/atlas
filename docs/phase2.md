<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Phase 2 – Vertiefte Arbeitsabläufe: Fortschritt

Umsetzung nach [PHASES.md](../PHASES.md) und [CALENDAR.md](../CALENDAR.md);
nach jedem Schritt ein Commit.

## Entscheidungen zu Beginn (2026-10-19)

- Phase 2 beginnt auf Wunsch vor dem stabilen Phase-1-Release; die offenen
  Punkte aus [phase1.md](phase1.md) (Durchgang auf echtem Hyprland, Arch-Paket)
  bleiben offen.
- **Annotationen** liegen als JSON-Begleitdatei in `.atlas/annotations/`;
  die Original-PDF wird nie verändert, ein Export erzeugt immer eine Kopie.
- **Aktionen und Erweiterungen** werden in YAML beschrieben (wie im
  Konzept), mit Formatversion.
- **Kalender** als eigener Tab-Typ (Seitenleiste mit Projektbaum bleibt
  sichtbar, Dateien lassen sich auf Termine ziehen); lokale Daten im
  iCalendar-Format.

## Schritte

| # | Schritt | Stand |
|---|---|---|
| 1 | PDF-Annotationen und Zitate | ✅ |
| 2 | Notizverknüpfungen: Wiki-Links, Backlink-Filter, Kontextgraph | ✅ |
| 3 | Benutzerdefinierte Aktionen | ✅ |
| 4 | Kleines Erweiterungssystem | ✅ |
| 5 | Projektkalender (lokal) | ✅ |
| 6 | Nextcloud/CalDAV-Synchronisierung | ✅ |
| 7 | Tests, Dokumentation und Abnahme | ✅ (Hyprland/Nextcloud-Durchgang offen) |

### Schritt 1 – PDF-Annotationen und Zitate

Kern: `atlas_core::annotations` (Datenmodell, Ablage, Zuordnung, Zitate) und
`atlas_core::pdf` (Textauswahl, PDF-ID, Export) – mit Tests.

- **Werkzeuge im PDF-Viewer**: *Text auswählen* (Ziehen, auch über
  Seitengrenzen und mit automatischem Scrollen; Doppelklick wählt ein Wort,
  Umschalt+Klick erweitert), *Freihand* (Farbe und Strichstärke wählbar),
  *Notiz anheften* und *Lesezeichen* je Seite (Strg+D).
- Nach einer Auswahl erscheint eine kleine Leiste: **Markieren** in fünf
  Farben (Strg+H), Unterstreichen, Durchstreichen, *Markieren und
  kommentieren*, **❝ Zitat** (Strg+Q) und Kopieren (Strg+C).
- Klick auf eine Anmerkung öffnet Kommentar, Farbe, *Zitat in Notiz*,
  *Link kopieren* und *Löschen*. Kommentare erscheinen als 💬 an der
  Markierung.
- Seitenleiste **Anmerkungen**: alle Anmerkungen nach Seite, Klick springt
  hin; Lesezeichen auch in den Seitenminiaturen.
- **Nicht destruktiv**: alles liegt in `.atlas/annotations/` (Format in
  [data-format.md](data-format.md)); die PDF bleibt unverändert.
- **Zuordnung** über Projektpfad, Fingerabdruck (BLAKE3, Größe, Seitenzahl,
  permanente PDF-ID) und Seite: Verschieben/Umbenennen in Atlas passt den
  Pfad an, externes Verschieben wird über den Fingerabdruck erkannt, eine
  veränderte PDF am selben Ort erzeugt einen Hinweis statt stiller
  Verschiebungen.
- **Export** „Als PDF-Kopie exportieren …“ schreibt eine Kopie mit echten
  PDF-Annotationen (Markierungen, Kommentare, Freihand); das Original kann
  nicht als Ziel gewählt werden.
- **Zitat in Markdown**: Blockzitat mit Quelle und Seite,
  `— [paper.pdf, S. 8](/docs/paper.pdf#page=8&annot=<id>)`, optional mit
  Kommentar. Ziel ist eine offene Notiz (am Ende eingefügt), eine beliebige
  Notiz oder eine neue; die zuletzt gewählte steht oben, so dass ein Zitat
  mit Auswahl → Strg+Q → Enter festgehalten ist.
- **Rücksprung**: Der Link öffnet die PDF auf der Seite und hebt die
  Markierung hervor; fehlt die Anmerkung, gilt die Seite. Links folgen
  Umbenennungen in Atlas (Linkaktualisierung aus Phase 1).
- Arbeitsleiste: `@text` sucht in Zitaten, Kommentaren und Lesezeichen
  aller PDFs; längere Suchbegriffe zeigen passende Zitate auch ohne `@`.

Gefunden und behoben: `poppler_path_new_from_array` übernimmt den
Punktepuffer und gibt ihn selbst frei – mit einem Rust-Vektor führte der
Export von Freihandstrichen zu Speicherfehlern. Die Punkte werden jetzt mit
`g_malloc` übergeben, Freihand-Annotationen erst auf die Seite gelegt und
dann mit Strichen gefüllt.

Geprüft: Kern-Tests (Auswahl über eine und zwei Seiten, Export aller Arten,
Original unverändert, Zuordnung nach Verschieben/Kopie/Änderung) und neue
Rauchtest-Schritte: Auswahl → Markierung → Kommentar → Lesezeichen →
Freihand → Zitat mit Quelle und Seite → `@`-Suche → Rücksprung zur Stelle →
Export als Kopie → Umbenennen der PDF (Zitat-Link und Anmerkungen folgen).

### Schritt 2 – Notizverknüpfungen

Kern: `atlas_core::markdown` (Wiki-Links, Bilder und Referenzdefinitionen
als Links, Umschreiben) und `atlas_core::notes` (Linkart, Kontextzeile,
lokaler Graph) – mit Tests.

- **Wiki-Links** `[[/docs/architektur.md]]`, mit Überschrift und Alias
  `[[/docs/architektur.md#Parser|Architektur]]`; `/` ist die Projektwurzel,
  sonst relativ zur Notiz; ohne Endung ist eine Markdown-Datei gemeint.
  In der Vorschau gestrichelt unterstrichen, ohne Alias mit dem Notiznamen
  („architektur › Parser“); defekte Wiki-Links sind wie andere markiert und
  reparierbar.
- **Vervollständigung**: `[[` im Markdown-Editor schlägt Notizen und
  Dateien des Projekts vor (unscharfe Suche wie Strg+P) und setzt den
  wurzelrelativen Pfad samt `]]` ein.
- **Rückverweise mit Filtern** (⇠ in Markdown und PDF): nach Notiz
  gruppiert, je Link Zeile, Kontext (die Zeile des Links) und Ziel
  (Überschrift, Codezeile, Seite, Zitat); Filter nach Freitext, Art
  (Wiki-Links, Markdown-Links, Zitate, mit Seite, auf Überschrift,
  Bilder), Tag (hierarchisch) und Ordner der Quelle.
- **Lokaler Kontextgraph** (◎) für die aktuelle Markdown-Datei oder PDF:
  verlinkte und verlinkende Notizen, PDFs und andere Dateien, wahlweise mit
  Nachbarn der Nachbarn; Farben nach Dateiart, fehlende Ziele
  durchgestrichen, Zitate aus PDFs als gelbe Kanten, Pfeile in
  Linkrichtung. Klick öffnet, Strg-/Mittelklick im Split. Panel, Tab und
  Tiefe werden je Tab in der Session gespeichert.
- **PDFs im Netz**: Notizen, die eine PDF zitieren oder verlinken,
  erscheinen in deren Rückverweisen und Graph.
- **Robuste Linkaktualisierung** beim Verschieben/Umbenennen in Atlas:
  zusätzlich zu Phase 1 auch Wiki-Links (Schreibweise ohne Endung bleibt),
  Bilder `![](…)` und Referenzdefinitionen `[x]: pfad`; Fragmente wie
  `#page=3&annot=…` bleiben erhalten. Die Dateiliste der Arbeitsleiste
  folgt Dateioperationen in Atlas sofort.

Geprüft: Kern-Tests (Wiki-Links, Bilder, Definitionen inkl. Umschreiben,
Graph um Notiz und PDF, Rückverweise mit Art/Kontext) und Rauchtest
(Alias, Überschriftenziel, Rückverweis mit Tags, Graph der PDF mit
Zitatkante, Tiefe 2, Wiki-Link folgt Umbenennung, Kontextpanel in der
Session, Vervollständigung).

### Schritt 3 – Benutzerdefinierte Aktionen

Kern: `atlas_core::actions` (Format, Dateimuster, Platzhalter, Vorbereitung
des Befehls, Heuristik für destruktive Befehle) mit Tests; Format in
[actions.md](actions.md).

- **Deklarativ in YAML** (`version: 1`): `.atlas/actions.yaml` für das
  Projekt, `~/.config/atlas/actions.yaml` für alle Projekte; Projektaktionen
  ersetzen globale mit gleicher `id`. Beim ersten Bearbeiten legt Atlas eine
  kommentierte Vorlage an. Fehler in einzelnen Einträgen werden gemeldet,
  der Rest bleibt nutzbar.
- **Ohne Shell**: Befehle sind Programm + Argumentliste (als Liste oder als
  Text, der nur in Wörter zerlegt wird). `shell: true` schaltet `sh -c`
  ausdrücklich ein; Platzhalter werden dann shell-sicher gequotet.
- **Platzhalter** `{file}` `{files}` `{name}` `{basename}` `{stem}` `{ext}`
  `{dir}` `{relative}` `{project}` `{line}` – nach dem Zerlegen eingesetzt,
  Dateinamen bleiben ein Argument; unbekannte Platzhalter sind ein Fehler.
- **Kontext**: `applies_to` (Muster für Dateiname oder Projektpfad, auch
  mehrere) und `when: file | dir | project`; bei Mehrfachauswahl müssen
  alle Dateien passen.
- **Anzeige** im Kontextmenü (Untermenü „Aktionen“, auch auf freier Fläche
  für Projektaktionen, mit „… bearbeiten“) und in der Arbeitsleiste
  („Aktion: …“ für das aktive Dokument und das Projekt).
- **Sichtbare Ausführung** im Terminal-Panel oder als Terminal-Tab
  (`terminal: tab`): ein fester Wrapper reicht die Argumente unverändert
  durch, lässt die Ausgabe nach dem Ende stehen und zeigt ✓/✗ mit Exit-Code
  in Zeile und Tab-Titel. Aktions-Tabs werden nicht in der Session
  wiederhergestellt.
- **Reproduzierbar**: Arbeitsordner ist standardmäßig der Projektordner,
  abweichend über `cwd` (mit Platzhaltern).
- **Bestätigung** mit dem genauen Befehl und Arbeitsordner bei
  `confirm: true` und immer bei destruktiv wirkenden Befehlen (`rm`, `mv`,
  `dd`, `git clean/reset/push --force`, Umleitungen `>` …).

Geprüft: Kern-Tests (Parsen und Fehler, Muster für Dateien/Ordner/Projekt,
Platzhalter mit `$(…)` und Leerzeichen im Dateinamen, Shell-Quoting,
Heuristik, Wrapper mit Exit-Code, Vorrang der Projektaktionen, Vorlage) und
Rauchtest (Aktionen je Kontext, Programm ohne Shell im Projektordner,
Bestätigungspflicht bei Umleitung, Ausführung im Terminal mit Ergebnis und
Statuszeile).

### Schritt 4 – Kleines Erweiterungssystem

Kern: `atlas_core::extensions` (Manifest, Freigabe, Dateiverarbeitung mit
Cache, Exporter, Ausführung mit Zeitlimit) mit Tests; Format in
[extensions.md](extensions.md).

- **Klar versioniertes Format**: Ordner mit `extension.yaml`,
  `atlas_extension: 1`; neuere Formate werden abgelehnt statt falsch
  gelesen. Global in `~/.config/atlas/extensions/`, pro Projekt in
  `.atlas/extensions/`. Vorlage über „Neue Erweiterung …“.
- **Eigene Aktionen** im Format von Schritt 3 (`{extension}` verweist auf
  Skripte im Erweiterungsordner); sie erscheinen wie andere Aktionen.
- **Eigene Dateiverarbeitung**: Dateien ohne eigene Ansicht werden über
  einen Befehl als Markdown, Text oder PDF angezeigt (nur lesend,
  automatisch oder über „Anzeigen mit“); Ergebnisse im Cache je
  Dateiinhalt, Zeitlimit, verständliche Fehlermeldung mit stderr.
- **Exporter** („Exportieren als“): Ergebnis neben die Datei, Rückfrage vor
  dem Ersetzen, atomar über eine temporäre Datei, Quelle nie überschrieben,
  Ergebnis danach geöffnet.
- **Keine UI-Erweiterung**: Erweiterungen tragen nur Befehle und
  Beschreibungen bei.
- **Sicherheit**: Projekt-Erweiterungen sind erst nach Prüfung aktiv (Hinweis
  unter der Titelleiste, Freigabedialog mit allen Befehlen); die Freigabe
  hängt am Inhalt des Manifests und erlischt bei Änderungen. Ein- und
  Ausschalten in `config.toml` (`[extensions] disabled`).
- Verwaltung „Erweiterungen verwalten …“ mit Status (aktiv, ausgeschaltet,
  nicht freigegeben, fehlerhaft), Beiträgen, Befehlen und Fehlern je
  Erweiterung.

Gefunden und behoben: Beim Öffnen startete die Anzeige die Umwandlung bis zu
dreimal gleichzeitig mit derselben temporären Datei – der zweite Lauf fand
sie nicht mehr. Temporäre Namen sind jetzt eindeutig, ein schon vorhandenes
Ergebnis gilt als Erfolg, und die Ansicht bündelt Startanlässe.

Geprüft: Kern-Tests (Manifeste mit Fehlern, Vorrang Projekt vor global,
Freigabe an das Manifest gebunden, Umwandlung mit Cache und parallel,
Fehler mit stderr, Zeitlimit, atomarer Export) und Rauchtest (Status,
Freigabe, Ein-/Ausschalten, Anzeige einer Binärdatei über eine Erweiterung,
Export mit Öffnen des Ergebnisses).

### Schritt 5 – Projektkalender (lokal)

Kern: `atlas_core::calendar` (Einträge, iCalendar-Lesen/-Schreiben mit
Erhalt fremder Daten, Wiederholungen, Vorkommen in einem Zeitraum,
Erinnerungen, Markdown-Checklisten) mit Tests; Format in
[data-format.md](data-format.md#kalender-calendar).

- **Eigener Tab** „Kalender“ (📅 in der Titelleiste, Strg+P → „Kalender
  öffnen“, `atlas PROJEKT --open calendar:`); Explorer und Projektbaum
  bleiben daneben, Ansicht, Datum und Auswahl stehen in der Session.
- **Woche** (Standard) mit Ganztagszeile, Stundenraster, aktueller Uhrzeit
  und Überlappungen nebeneinander; **Monat** mit Fristen und ganztägigen
  Einträgen zuerst und einem Auslastungsbalken je Tag; **Agenda** nach
  Überfällig, Heute, Morgen, Diese Woche, Später.
- **Einträge**: Termin, Arbeitsblock, Frist, Aufgabe ohne Zeit; Status,
  Tags, Beschreibung, Checkliste, verknüpfte Dateien und Session, Aufwand,
  Erinnerung, einfache Wiederholung (täglich bis jährlich, Intervall, bis
  Datum oder Anzahl). Regeln, die Atlas nicht vollständig bearbeiten kann,
  bleiben unverändert erhalten.
- **Bedienung**: Klick wählt (Details rechts), Doppelklick bearbeitet,
  Ziehen verschiebt (auch ganze Serien, über Tage und im Monat), der untere
  Rand ändert die Dauer; Ziehen oder Doppelklick auf freie Zeit legt einen
  Termin an.
- **Dateien**: aus dem Explorer auf einen Eintrag ziehen verknüpft, auf
  freie Zeit ziehen legt einen Arbeitsblock an, auf „Noch nicht eingeplant“
  eine Aufgabe; Kontextmenü „Als Aufgabe einplanen“. Links sind
  wurzelrelativ (`/docs/kapitel-3.md`), öffnen per Klick (Strg: Split) und
  folgen Umbenennungen in Atlas.
- **Noch nicht eingeplant**: Aufgaben mit Aufwand; in die Woche oder den
  Monat gezogen werden sie zum Arbeitsblock (Dauer = Aufwand), „Zurück zu
  ‚Noch nicht eingeplant‘“ macht das rückgängig.
- **Sessions**: „In Session wechseln“ speichert die aktuelle Session und
  öffnet die verknüpfte (fehlt sie, wird sie leer angelegt) – nie
  automatisch zu einer Uhrzeit.
- **Markdown**: Notizen mit `due:` im Frontmatter erscheinen als Frist
  (nur lesend, Doppelklick öffnet die Notiz); „Checkliste als Aufgaben
  übernehmen …“ übernimmt gewählte Punkte nur auf ausdrücklichen Wunsch,
  die Datei bleibt unverändert.
- **Erinnerungen** erscheinen, solange Atlas läuft, als Hinweis unter der
  Titelleiste („Zeigen“ springt zum Eintrag).
- **Arbeitsleiste**: Termine werden mit `!` (und bei längeren Suchbegriffen
  auch ohne) gefunden; dazu Kalenderaktionen (neuer Termin, Arbeitsblock,
  Frist, Aufgabe, Ansichten, Einstellungen).
- Rechte Leiste ohne Auswahl: „Diese Woche“ mit verplanten Stunden,
  Anzahl Termine/Blöcke/Fristen, offenen Aufgaben und den Fristen.

Gefunden und behoben: Die Wochen- und Monatsansicht bekamen ihren Kalender
über `cal: cal` – die Bindung zeigte auf die eigene Eigenschaft und blieb
leer; außerdem fing die Drop-Fläche des Editor-Splits Dateien ab, bevor der
Kalender sie verknüpfen konnte.

### Schritt 6 – Nextcloud/CalDAV-Synchronisierung

Kern: `atlas_core::calendar::caldav` (Client: Erkennung über
`/remote.php/dav/` bzw. `/.well-known/caldav`, Principal und
`calendar-home-set`, Kalender mit Farbe, Rechten und Komponenten, ETags,
`calendar-multiget`, bedingtes PUT/DELETE, MKCALENDAR) und
`atlas_core::calendar::sync` (Cache, Warteschlange, Konflikte, Abgleich)
mit Tests gegen einen kleinen CalDAV-Testserver.

- **Einrichtung** in den Kalender-Einstellungen (⚙ im Kalender, Strg+P):
  Serveradresse, Benutzer, App-Passwort, im erweiterten Bereich eine
  CalDAV-URL von Hand. „Verbindung testen“ zeigt die gefundenen Kalender
  (schreibgeschützte markiert); keine Nextcloud-App nötig.
- **App-Passwort** nur im Arbeitsspeicher, einmal pro Atlas-Start: ein
  Hinweis unter der Titelleiste fragt danach (oder „Später“); kein
  Schlüsselbund, nichts in `config.toml`, `.atlas/` oder Sessions.
- **Kalenderauswahl**: je Kalender sichtbar/ausgeblendet (ausgeblendete
  werden auch nicht abgeglichen), „nur hier“ (nur in diesem Projekt),
  Farbe und Zugriff vom Server. **Standardkalender** für neue Einträge
  global oder nur für dieses Projekt; im Eintragsdialog lässt sich der
  Zielkalender für einen einzelnen Eintrag ändern (auch nachträglich –
  der Eintrag zieht dann um).
- **Sofort synchronisieren**: neue und geänderte Einträge in
  Nextcloud-Kalendern gehen bei bekanntem Passwort direkt zum Server;
  zusätzlich alle 5 Minuten im Hintergrund und über ⟳ von Hand.
- **Offline**: Änderungen landen in der Warteschlange in
  `.atlas/calendar/cache/sync.json` (je Eintrag eine, spätere ersetzen
  frühere), die Einträge bleiben sichtbar; ist der Server nicht
  erreichbar, versucht Atlas es jede Minute erneut und überträgt sie,
  sobald die Verbindung wieder steht. Neue Objekte heißen nach ihrer UID und
  werden mit `If-None-Match: *` angelegt – auch eine wiederholte
  Übertragung nach verlorener Antwort erzeugt **keine Duplikate**.
- **Konflikte** (geändert in Nextcloud und Atlas, gelöscht auf einer Seite
  oder vom Server abgelehnt): nichts wird still überschrieben (`If-Match`
  mit der ETag der Ausgangsfassung). Ein Hinweis führt zum Dialog mit
  beiden Fassungen und **[Nextcloud-Version] [Lokale Version] [Beide
  behalten]** („Beide behalten“ legt die lokale Fassung als neuen Eintrag
  mit eigenen Verknüpfungen an).
- **Schreibgeschützte und verschwundene Kalender** werden nie
  überschrieben: Einträge darin sind nicht bearbeitbar; ein Kalender, den
  es auf dem Server nicht mehr gibt, bleibt im Cache und wird markiert,
  seine wartenden Änderungen bleiben liegen.
- **Atlas-Verknüpfungen** reisen als `X-ATLAS-*` im Termin mit und stehen
  zusätzlich über die UID in `links.json`; andere CalDAV-Clients sehen
  einen normalen Termin. **Erinnerungen** (`VALARM`) aus Nextcloud werden
  übernommen, fremde Alarmarten bleiben erhalten.
- **Status** im Kalenderkopf (⟳ mit Zahl wartender Änderungen, 🔑 ohne
  Passwort, ⚠ bei Fehler) und in den Einstellungen: letzter erfolgreicher
  Abgleich bzw. Fehlermeldung.
- **Verwaltung**: Verbindung testen, Kalenderauswahl ändern, jetzt
  synchronisieren, App-Passwort neu eingeben, „Atlas – Projektarbeit“ nach
  Bestätigung in Nextcloud anlegen, lokalen Cache leeren (mit Warnung bei
  wartenden Änderungen), Konto trennen (löscht nichts auf dem Server).

Netzwerk: HTTP über `ureq` mit rustls und den Zertifikaten des Systems
(auch eigene Zertifizierungsstellen), `HTTPS_PROXY` wird beachtet; XML über
`roxmltree`. Anfragen laufen im Hintergrund, die Oberfläche bleibt
bedienbar; der Cache wird nur in kurzen Transaktionen geschrieben.

### Schritt 7 – Tests, Dokumentation und Abnahme

- Kern-Tests (109): u. a. CalDAV-Erkennung und Antworten von Nextcloud,
  bedingte Schreibzugriffe, Anlegen eines Kalenders, nicht erreichbarer
  Server; Abgleich „lokal angelegt → erscheint im gewählten Kalender →
  Änderung auf dem Server kommt zurück → Löschen“; Offline-Änderungen
  werden nach der Rückkehr genau einmal übertragen, auch nach verlorener
  Antwort; Konflikte mit allen drei Auflösungen; verschwundene und
  schreibgeschützte Kalender; Konto-Einstellungen ohne Passwort.
- Rauchtest (`scripts/smoke-test.sh`) um den Kalender erweitert: Tab,
  Anlegen und `events.ics`, Datei verknüpfen und `links.json`, Wochen-,
  Monats- und Agenda-Ansicht, Notiz-Frist aus dem Frontmatter,
  Verschieben, Checkliste → Aufgaben (Markdown unverändert), Einplanen,
  „Als Aufgabe einplanen“, Suche, Dateilink folgt Umbenennung, Tab in der
  Session, Löschen.
- Dokumentiert: Dateiformate und `[calendar]` in
  [data-format.md](data-format.md), Tastenkürzel in
  [shortcuts.md](shortcuts.md).

#### Abnahmekriterien

| Kriterium | Nachweis |
|---|---|
| PDF-Markierung mit Quelle und Seite in eine Notiz | Schritt 1, Rauchtest „Zitat mit Quelle und Seite“, „Zitat in Notiz“ |
| Zitat öffnet zuverlässig die PDF-Seite | Rauchtest „Zitat öffnet die Stelle“, „Zitat-Link folgt Umbenennung“ |
| Original-PDF ohne Export unverändert | Rauchtest „Original-PDF unverändert“, Kern-Tests Export |
| Lokaler Graph der Markdown- oder PDF-Datei | Rauchtest „Graph der PDF zeigt zitierende Notiz“, „Graph mit Tiefe 2“ |
| Eigene Aktionen im richtigen Arbeitsordner | Rauchtest „Aktion im Projektordner ausgeführt“ |
| Markdown und PDF ohne Atlas nutzbar | Zitate/Wiki-Links sind normales Markdown, Annotationen und Kalender liegen in `.atlas/`, PDF unverändert |
| Lokal erstellter Termin erscheint im gewählten Nextcloud-Kalender | Kern-Test `local_entry_appears_on_server_and_comes_back` |
| Offline erstellte Termine ohne Duplikate übertragen | Kern-Test `offline_changes_are_sent_once_after_reconnecting` |

Offen: ein Durchgang gegen einen echten Nextcloud-Server und auf echtem
Hyprland (die Tests laufen gegen einen nachgebauten CalDAV-Server bzw.
offscreen), dazu weiterhin die offenen Punkte aus [phase1.md](phase1.md).
Komplexe Serienausnahmen (einzelne Vorkommen getrennt bearbeiten) und
native Desktop-Benachrichtigungen bleiben wie geplant für später.

### Nachtrag – Design nach dem Gesamt-Mockup

Oberfläche an das dunkle „Navy“-Mockup angeglichen (Screenshot:
[previews/14-atlas-design-navy.png](../previews/14-atlas-design-navy.png)):

- **Farben und Maße** zentral in `qml/Theme.qml`; Splits, Terminal und
  Kontextleiste als abgerundete Karten mit kleinem Abstand.
- **Linien-Icons** (`qml/Icon.qml`, SVG, beliebig einfärbbar) statt Emoji –
  in Orten, Projektbaum (Dateiart farbig: PDF rot, Markdown blau, Tabellen
  grün), Tabs und Werkzeugleisten.
- **Kopfzeile**: Logo, Suchfeld in der Mitte (öffnet die Arbeitsleiste),
  „Kalender“, „Sitzung: …“, Terminal, Kontextleiste und Menü (Projekt
  öffnen, Splits, Seitenleiste, Einstellungen).
- **Seitenleiste**: „Orte“ (jetzt auch Bilder, Musik, Videos), Projektordner
  mit hervorgehobener Projektzeile, freier Speicherplatz des Projekt-
  Dateisystems, „Projektordner öffnen“. Die Projektsuche öffnet sich an
  gleicher Stelle (Strg+Umschalt+F) mit Rückweg zum Explorer.
- **Kontextleiste** rechts (neu, `qml/Inspector.qml`) für das aktive
  Dokument: *Anmerkungen* (Karten der zuletzt aktiven PDF mit Farbe,
  Kommentar, Seite und Zeit, Klick springt hin; darunter lokaler Graph und
  eigene Aktionen), *Verknüpfungen* (Rückverweise mit Filtern), *Lokaler
  Graph*, *Aktionen* (eingebaute wie „Zitat übernehmen“, „PDF-Kopie
  exportieren“, „Als Aufgabe einplanen“ und die eigenen Aktionen).
  Sichtbarkeit, Reiter und Breite stehen in der Session.
- **Markdown-Vorschau**: Überschriften mit `#`/`##`, farbige Tag-Chips,
  Zitate als Karte mit goldenem Rand und Quellen-Chip, eigene Checkboxen.
- Terminal in den Farben des Designs.

#### Fenster und Dialoge

- Eigener Qt-Quick-Controls-Stil `qml/AtlasStyle` (wird beim Start gesetzt,
  `QT_QUICK_CONTROLS_STYLE` überschreibt ihn; nicht gestaltete Controls
  kommen aus Fusion): Dialoge als Karten mit Titel auf abgedunkeltem
  Fenster, abgerundete Eingabefelder, Auswahllisten, Checkboxen und
  Knöpfe (Bestätigen in Akzentfarbe, Standardknöpfe auf Deutsch), Menüs mit
  runder Hervorhebung und rechtsbündigen Kürzeln, schmale Scrollbalken,
  Reiter mit Akzentlinie, Tooltips. Damit folgen alle Fenster dem Design –
  Einstellungen, Eigenschaften, Erweiterungen, Kalender, Konflikte,
  Umbenennen, Bestätigungen, Kontextmenüs.
- **Sessionfenster** nach dem Mockup
  ([previews/15-atlas-sessions-navy.png](../previews/15-atlas-sessions-navy.png)):
  Suche, alle Sessions mit Symbol nach Inhalt (PDF, Notizen, Code, leer),
  „3 Tabs · 2 Splits“, aktive Session markiert („Automatisch gespeichert“),
  sonst Zeit der letzten Speicherung; ⋮ je Session zum Umbenennen und
  Löschen; unten „Neue Session“ (mit ⋮: Umbenennen, Session speichern,
  Löschen) und „Aktuelle duplizieren“.
- Arbeitsleiste mit Suchsymbol und farbigen Dateisymbolen.
- `--screenshot-dialog NAME` öffnet einen Dialog für Design-Screenshots.

Gefunden und behoben: „In Notiz übernehmen“ (Terminal-Ausgabe, PDF-Zitat)
öffnete sich nicht – eine lokale Variable `open` verdeckte `open()`.

### Nachtrag – Rückmeldungen aus dem Test

Behoben:

- **Splits füllen den Editorbereich immer**: Beim Vergrößern eines Splits
  gegen die Seitenleiste, beim Ein-/Ausblenden und beim Ändern der Breite
  der Seitenleiste blieben Lücken. Die Splits merken sich ihre Anteile und
  werden bei jeder Größenänderung neu verteilt; Dokumente gehören nicht mehr
  zur SplitView (dort zählten sie als zusätzliche Splits).
- **Strg+Mausrad zoomt PDFs** um den Punkt unter der Maus – auch mit
  Touchpad und feinem Scrollen unter Wayland: Diese Ereignisse tragen nur
  ein Pixel-Delta (`angleDelta` 0) und liefen vorher ins Scrollen. Strg+Rad
  scrollt nie mehr; gleiches gilt für den Symbol-Zoom der Dateiansicht.
- **Projektwechsel** öffnete Dateien des vorherigen Projekts (z. B.
  `paper.pdf`, `README.md`): Dateien von der Kommandozeile gelten nur beim
  Start und nur, wenn sie existieren.
- **Checkboxen in der Markdown-Vorschau** sind anklickbar und haken die
  Zeile im Quelltext ab (wird wie jede Änderung gespeichert). Sie ersetzen
  den Aufzählungspunkt, sitzen auf der Textlinie (Häkchen als SVG statt
  Zeichen, das die Grundlinie verschob), erledigte Aufgaben erscheinen
  gedämpft und durchgestrichen.
- **Leeres Fenster**: ohne „Split teilen“/„Schließen“-Knopf; wer etwas im
  Split oder temporären Split öffnet, während nur das leere Fenster da ist,
  bekommt es dort – ein leeres Fenster bleibt nie neben einem Dokument
  stehen.

Neu gestaltet:

- **Schnellansicht** nach
  [previews/03-schnellansicht-dark.png](../previews/03-schnellansicht-dark.png):
  Kopfzeile „Schnellansicht – Name“ mit Seitenwechsel, Zoom und „Ganze
  Seite“, das Dokument ohne eigene Leisten (PDFs ganzseitig), rechts eine
  Infospalte (Art, Größe, Änderung, Ordner; bei PDFs Seiten, Abmessungen und
  Anmerkungen, bei Notizen Überschriften und Schlagwörter) mit „Im Tab
  öffnen“ / „Im Split öffnen“, darunter die Tastenhinweise. **Strg+Klick auf
  eine Datei im Projektbaum** öffnet sie in der Schnellansicht (Mittelklick
  weiterhin im Split, Strg+Klick auf Ordner als Dateiliste im Split).
- **Dateiansicht wie Dolphin**: Pfadleiste mit anklickbaren Abschnitten
  (beginnt beim Projekt bzw. Ort, Klick auf die freie Fläche oder Strg+L zum
  Eintippen), Symbolansicht mit großen Symbolen und Bildvorschau oder
  Detailansicht mit Spalten, Zoom-Regler und Strg+Mausrad, Statusleiste mit
  „5 Ordner, 3 Dateien (1,2 MB)“ bzw. der Auswahl und dem freien
  Speicherplatz. Ansicht und Symbolgröße stehen in der Session.
- **Ordnerauswahl** („Projektordner öffnen“, Strg+O) als eigenes Fenster im
  Design statt des Systemdialogs: Projekt und Orte links, Pfadleiste,
  Unterordner, Ordner per Doppelklick betreten, „Ordner öffnen“ übernimmt
  den gezeigten oder markierten Ordner.
- **Neue Session** (Sessionmenü oben rechts, Befehlspalette) fragt immer
  nach dem Projektordner und dem Namen; liegt der Ordner außerhalb des
  aktuellen Projekts, wird dieses Projekt geöffnet und die Session dort
  angelegt.

Geprüft im Rauchtest: echte Mausereignisse für Strg+Klick im Baum, Klick und
Strg+Mausrad in der Dateiansicht, Wechsel Symbol-/Detailansicht mit
erhaltener Auswahl, Ordnerauswahl und Anlegen einer Session, leeres Fenster
und Splits, Checkbox-Klick in der Vorschau, PDF-Zoom.

### Nachtrag – App-Passwort aus einem Passwort-Befehl

Statt eines Schlüsselbunds (kein Secret-Service-Dienst nötig) kann in den
Kalender-Einstellungen ein **Passwort-Befehl** stehen
(`[calendar] password_command`), etwa `pass show nextcloud/atlas`,
`rbw get nextcloud` oder `systemd-creds --user decrypt …`. Atlas führt ihn
einmal pro Start im Hintergrund aus (ohne Shell, ohne Eingabe, 90 s
Zeitlimit für pinentry), nimmt die erste Zeile als App-Passwort und hält es
wie bisher nur im Arbeitsspeicher. Während er läuft, fragt Atlas nicht
nach; scheitert er, nennt der Hinweis Code und Fehlerzeile und bietet
„Erneut versuchen“, „Eingeben …“ und „Später“. „Verbindung testen“ nutzt
den Befehl, wenn kein Passwort eingetippt ist. Ein geänderter Befehl wird
sofort neu ausgeführt; „Konto trennen“ vergisst Passwort und Ergebnis.
Geprüft: Kern-Tests (erste Zeile, Fehler mit Code und stderr ohne
Passwort, leere Ausgabe, fehlendes Programm, Zeitlimit) und Rauchtest
(Fehlerhinweis, erfolgreicher Befehl entsperrt, Passwort nicht im Konto).
