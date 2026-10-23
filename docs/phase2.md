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
  Löschen).
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

### Nachtrag – App-Passwort einmal eingeben

„Auf diesem Rechner merken“ (Einstellungen und Passwort-Abfrage, vorbelegt)
legt das App-Passwort einmal ab, ohne Schlüsselbund-Dienst
(`atlas_core::secret`): verschlüsselt mit `systemd-creds --user`, an den
Rechner gebunden, sonst als Datei mit Rechten 600 (wird angezeigt). Beim
Start lädt Atlas es im Hintergrund, vor einem Passwort-Befehl; danach wird
nicht mehr gefragt. Abwählen, Konto trennen oder Kontowechsel löschen es.
Gefunden und behoben: ein „Erneut versuchen“ während eines laufenden
Abrufs ließ dessen Ergebnis verfallen und die Anzeige auf „Hole
App-Passwort …“ stehen. Geprüft: Kern-Tests (verschlüsselt mit Ersatz für
systemd-creds, Rückfall auf Datei, Rechte 700/600, für andere lesbare
Datei abgelehnt, kaputtes systemd-creds) und Rauchtest (merken, Neustart
ohne Eingabe, abwählen, wieder merken, Konto trennen).

### Nachtrag – Terminal-Knopf

Der Terminal-Knopf oben rechts klappte das Terminal manchmal erst beim
zweiten Klick ein: Er nutzte die Logik von Strg+`, die ein sichtbares
Terminal ohne Fokus zuerst fokussiert. Knopf und Befehlspalette blenden
jetzt direkt ein/aus; nur Strg+` fokussiert zuerst. Rauchtest mit echten
Klicks bei Fokus im Editor.

### Nachtrag – Orte aus Dolphin

Mit „Orte wie in Dolphin anzeigen“ (Einstellungen, `[places]
file_manager`, standardmäßig an) zeigt die Seitenleiste **genau Dolphins
Orte** aus `~/.local/share/user-places.xbel`, in Dolphins Reihenfolge und
statt der festen Orte (`atlas_core::places`): Was in Dolphin entfernt oder
ausgeblendet ist, fehlt auch in Atlas. KDE-Systemorte stehen dort mit
englischen Titeln und werden wie in KDE übersetzt („Home“ → „Persönlicher
Ordner“), `trash:/` ist der Papierkorb von Atlas. Einträge nur für eine
andere App, Netzwerk-Orte (`sftp://`, `smb://`, `remote:/`) und nicht
vorhandene Ordner fallen weg; Laufwerke stehen nicht in der Datei (Dolphin
fragt UDisks2) und fehlen daher. Ohne Dolphin-Datei zeigt Atlas die festen
Orte und darunter, abgesetzt als „Dateimanager“, die Lesezeichen von
GTK-Dateimanagern (`~/.config/gtk-3.0/bookmarks`); ausgeschaltet nur die
festen Orte. Änderungen erscheinen sofort (Dateiüberwachung); die Orte
gelten auch in Ordnerauswahl und Pfadleiste. Neu ist der Dialog
**Einstellungen** (Menü, Befehlspalette) mit diesem Schalter und Wegen zu
Editor-, Kalender- und Erweiterungs-Einstellungen. Der Rauchtest nutzt
eigene `XDG_DATA_HOME`/`XDG_CONFIG_HOME` und fasst die echten Orte nie an.
Geprüft: Kern-Tests (Dolphin ersetzt die festen Orte, alles entfernt =
keine Orte, kaputte oder fehlende Datei, XBEL mit `<!DOCTYPE>`,
Systemorte übersetzt, Papierkorb, Prozentkodierung, GTK-Lesezeichen,
Duplikate) und Rauchtest (wie in Dolphin, aus- und wieder einschalten).

### Nachtrag – Strg+Mausrad unter Wayland

Unter Wayland (Hyprland, sway) zoomte Strg+Mausrad nicht: Qt meldet dort
selbst Mausräder als Gerät „touchpad“ mit Zeigertyp „Finger“, und die
WheelHandler (PDF-Zoom, Symbol-Zoom der Dateiansicht, Scrollen der
Kalenderwoche) nahmen diesen Typ nicht an. Sie akzeptieren jetzt alle
Geräte- und Zeigertypen; die Kalenderwoche wertet außerdem reine
Pixel-Deltas aus. Gefunden mit echten Wayland-Ereignissen (sway headless,
virtuelle Maus über das wlroots-Protokoll, Strg über eine virtuelle
Tastatur); der Rauchtest schickt jetzt auch Rad-Ereignisse eines solchen
„Touchpad“-Geräts und Rasten in Scroll-Phasen.

### Nachtrag – Kalender-Knopf, Menüs, Zitate im Projekt

- **Kalender-Knopf** schaltet: Ist der Kalender der aktive Tab, schließt
  ein Klick ihn; sonst öffnet bzw. zeigt er ihn.
- **Menüs** messen Breite und Höhe an ihren Einträgen: Texte wurden
  abgeschnitten („Projektordner öffne…“), und die geschätzte Höhe ließ
  einen Eintrag („Einstellungen …“) wegfallen. Kürzel und gesperrte
  Einträge sind besser lesbar.
- **Verknüpfungen: „Zitate im Projekt“** neben „Dieses Dokument“: alle
  markierten Textstellen aller PDFs des Projekts (nach PDF und Seite, mit
  Farbe und Kommentar) und wo Notizen sie zitieren („zitiert in …, Z. 10“
  bzw. „noch in keiner Notiz zitiert“); eine gelöschte Markierung
  verschwindet, auch wenn eine Notiz sie noch zitiert. Klick springt zur Markierung in der PDF, „Notiz ↗“
  zur zitierenden Notiz; Textfilter. Die Liste folgt dem Anmerkungsspeicher
  (Dateiüberwachung, `annotationsRevision`), eine neue Markierung erscheint
  also gleich – auch nach dem Schließen der PDF. Zitate in der
  Dokument-Ansicht zeigen jetzt den Zitattext.
- Behoben: Ohne Tags und Art-Filter nahm die leere Filterzeile der Liste
  den Platz weg (Layouts dehnen sich standardmäßig) – die Zitate waren
  vorhanden, aber nicht zu sehen.
- Behoben: Wurde das App-Passwort kurz hintereinander zweimal gemerkt,
  schrieben zwei Threads dieselbe temporäre Datei („cannot replace …“),
  oder ein älteres Passwort gewann. Speichern und Vergessen laufen jetzt
  nacheinander, ein neuerer Auftrag verdrängt einen wartenden älteren.
- Selbsttest: Schritte mit Hintergrundarbeit warten jetzt bis zu acht
  Takte auf ihr Ergebnis (`waitFor`), statt nach festen 1,5 s zu prüfen.
- Rauchtest: Zeitlimit des Skripts auf 300 s (bei 120 s brach der
  gewachsene Selbsttest kurz vor dem Ende ab).

### Nachtrag – aufgeräumtes Kontextmenü

Das Rechtsklick-Menü für Dateien und Ordner zeigt nur noch, was passt:
keine ausgegrauten Einträge mehr (Ausschneiden/Kopieren/Umbenennen/
Papierkorb nur mit Auswahl, Einfügen nur bei Dateien in der Zwischenablage,
„Anzeigen mit“/„Exportieren als“/„Aktionen“ nur mit Einträgen), „Im
Editor öffnen“ nur bei Text und Markdown. Entfernt: „Terminal als Tab
öffnen“ (doppelt zu „Terminal hier öffnen“), „Externen Editor festlegen
…“ sowie „Projekt-/Globale Aktionen bearbeiten …“ und „Erweiterungen …“
aus dem Aktionen-Untermenü – diese stehen jetzt im Dialog Einstellungen.
Rauchtest prüft die Einträge für PDF, Notiz und leere Fläche.

### Nachtrag – HTML als Webseite anzeigen

Eingebaute Aktion **„Als Webseite anzeigen“** für `.html`/`.htm`/`.xhtml`:
im Rechtsklick-Menü, unter „Aktionen“ in der Kontextleiste und in der
Befehlspalette. Die Datei erscheint gerendert (QtWebEngine, `HtmlView`) –
im selben Tab, wenn ihr Quelltext schon offen ist; „Quelltext anzeigen“
schaltet zurück. Leiste mit Zoom, Neu laden (F5), Quelltext und „Im
Browser öffnen“; gespeicherte Änderungen (auch aus anderen Editoren)
laden die Seite neu. Die Ansicht steht in der Session. Seiten dürfen
Skripte ausführen und Dateien neben sich laden (CSS, Bilder), aber nicht
ins Netz; Links auf Webseiten öffnen den System-Browser, lokale Links
bleiben in der Ansicht. Rauchtest: Rendern mit Skript und CSS, Umschalten
im selben Tab, Session, Neuladen nach Änderung.

- Nachtrag: Lädt eine Seite Skripte, Stylesheets oder Bilder aus dem
  Internet (z. B. sql.js von einem CDN), zeigt die Ansicht einen Hinweis
  mit „Trotzdem laden“ (Netzzugriff nur für diese Datei, in der Session
  gemerkt) und „Im Browser öffnen“ – vorher fehlte das stillschweigend
  („initSqlJs is not defined“).
- WebP, TIFF & Co.: `qtimageformats` gehört jetzt zu den Qt-Paketen; die
  Dateiansicht zeigt Vorschaubilder nur für Formate, die Qt lesen kann
  (`imageFormats`), statt „Error decoding … .webp“ zu melden.
- Seitenfehler (auch aus eingebetteten Rahmen wie `iframe srcdoc`) stehen
  in der Ansicht statt als `js:`-Zeilen im Terminal; der Netz-Hinweis nennt
  den Fehler, der durch die Blockade entsteht („initSqlJs is not defined“).
  Erkannt werden auch per Skript nachgeladene Inhalte (`.src = "https://…"`,
  `fetch`, `import`). Geprüft mit einem lokalen HTTP-Server: Nach „Trotzdem
  laden“ laden auch eingebettete Werkzeuge ihre Skripte.

### Nachtrag – Hinweis „PDF verändert“

Atlas merkt sich zu den Anmerkungen einer PDF deren Prüfsumme (BLAKE3 des
Inhalts). Weicht sie beim Öffnen ab, wurde die PDF inzwischen verändert
(neu erzeugt, überschrieben, bearbeitet) und Markierungen könnten nicht
mehr auf dem richtigen Text liegen. Der Hinweis nennt jetzt die Zahl der
Anmerkungen und erscheint nur, wenn es welche gibt; „Verstanden“ übernimmt
die neue Fassung.

### Nachtrag – Projektordner nicht einklappbar

Die Zeile des Projektordners im Baum klappt nichts mehr ein (kein Pfeil,
Rechtsklick-Menü bleibt): eingeklappt rutschte sie ans untere Ende der
Seitenleiste und ließ eine große Lücke. Unterordner klappen wie bisher.

### Nachtrag – Reiter der Kontextleiste

Die Reiter (Anmerkungen, Verknüpfungen, Lokaler Graph, Aktionen) werden
nicht mehr gekürzt („Anmerkung…“): Sie bleiben ausgeschrieben; reicht der
Platz nicht, scrollt die Leiste seitwärts (schmale Scrollleiste, Mausrad),
und der gewählte Reiter rückt ins Bild.

### Nachtrag – Skripte ausführen

Eingebaute Aktion **„Ausführen“** (Rechtsklick-Menü, Aktionen der
Kontextleiste, Befehlspalette) für `.sh`/`.bash` – mit bash, auch ohne
Ausführrecht – und für ausführbare Dateien mit `#!`-Zeile (z. B. ein
Python-Skript mit `chmod +x`). Das Skript läuft sichtbar im Terminal-Panel,
im Ordner des Skripts; Ausgabe und „✓ beendet“ bzw. „✗ Code n“ bleiben
stehen, bis Enter gedrückt wird (wie bei eigenen Aktionen).

- Der Knopf „Projektordner öffnen“ unten links ist entfernt; Projekte
  öffnen weiter über das Menü oben rechts, die Befehlspalette, Strg+O und
  „Neue Session“ im Sessionmenü.

- Im Sessionmenü ist „Aktuelle duplizieren“ entfernt.

### Release 1.0

- Version 1.0.0; die Flake liest sie aus `Cargo.toml` und bietet ein
  Overlay (`pkgs.atlas`).
- `packaging/arch/PKGBUILD` baut das Archiv des Tags `v$pkgver` und ist
  bereit fürs AUR (`atlas-workspace`, mit `.SRCINFO`).
- README für die Veröffentlichung neu geschrieben, mit echten Screenshots;
  die Konzept-Mockups stehen jetzt in [design.md](design.md).
- `CHANGELOG.md` und GitHub-CI (`nix flake check`).

### Nachtrag – Eine Session je Projektordner

Eine Session ist ein Projektordner: jeder Ordner hat genau eine Session
(Tabs, Splits, aufgeklappte Ordner), gespeichert wie bisher in
`.atlas/sessions/<gerät>/`. Ihr Name ist der Ordnername, außer sie wurde
umbenannt. Atlas merkt sich die geöffneten Ordner (`projects` in
`app.json`, die letzten 30); das Sessionfenster listet je Ordner einen
Eintrag mit Pfad und Umfang, ein Klick öffnet den Ordner (vorher wird die
aktuelle Session gespeichert). „Neue Session“ wählt einen Ordner und
optional einen Namen – hat der Ordner schon eine Session, wird sie
geöffnet (und ggf. umbenannt). Umbenennen geht für jede Session,
„Aus der Liste entfernen“ für andere Ordner: Tabs und Anordnung werden
vergessen, die Dateien bleiben. „In Session wechseln“ im Kalender öffnet
den Ordner der genannten Session. Ältere Projekte mit mehreren benannten
Sessions nutzen ihre zuletzt aktive.

Rauchtest: Session des Projekts benennen (kein zweiter Eintrag), Session
in einem zweiten Ordner (nach ihm benannt), Liste mit beiden, Rückkehr mit
unverändertem Layout, Wechsel über den Kalender, Entfernen.

### Nachtrag – Graph des ganzen Projekts

„Lokaler Graph“ heißt jetzt **Graph** und hat nur noch eine Ansicht statt
der Tiefen 1 und 2: alle Notizen des Projekts und alle Dateien, auf die sie
verlinken, dazu alle übrigen Dateien des Projektordners – ohne versteckte
Dateien und Ordner und ohne das, was `.gitignore` oder Build-Ordner
ausschließen (`NoteIndex::project_graph`, höchstens 500 Knoten; bei mehr
zuerst die verknüpften, dann Notizen, PDFs, Dokumente, Bilder, der Rest). Die aktuelle
Datei steht groß in der Mitte. Alle Dateien mit Links liegen nach einem
Kräfte-Layout (Gruppen, die nicht mit ihr verbunden sind, nahe daneben);
Dateien ganz ohne Links füllen den Raum darum gleichmäßig als
Sonnenblumen-Spirale – ein Ring wurde bei vielen Dateien riesig, und alles
verschmolz zu einem Fleck. Knoten, Linien und Pfeile werden beim
Herauszoomen kleiner; Namen erscheinen beim Darüberfahren, beim Vergrößern
und für direkte Nachbarn, sobald Platz ist. Strg+Mausrad zoomt (auch unter
Wayland, wo Mausräder als „Touchpad“ gemeldet werden); das Mausrad allein
scrollt die umgebende Leiste. Ziehen verschiebt,
„Ganzen Graphen zeigen“ passt ein.

### Nachtrag – Dateien verknüpfen

„Verknüpfen mit …“ verbindet zwei beliebige Dateien ohne Notiz dazwischen,
etwa zwei PDFs: im Kontextmenü einer Datei, in der Arbeitsleiste, unter
„Aktionen“ der Kontextleiste und mit „+“ in „Verknüpfungen“. Ein Dialog
sucht die zweite Datei (unscharf wie Strg+P, ohne Ordner und versteckte
Dateien). Gespeichert wird in `.atlas/relations.json`
(`atlas_core::relations`), die Dateien bleiben unverändert. Im Graph ist
eine Verknüpfung eine grüne, gestrichelte Linie ohne Pfeil; unter
„Verknüpfungen“ → „Dieses Dokument“ steht „Verknüpft mit“ mit den Dateien
(Klick öffnet, × löst). Umbenennen und Verschieben in Atlas passen die
Pfade an, Löschen entfernt die Verknüpfung.

Rauchtest: Dialog (nur Dateien), zwei PDFs verknüpfen, Kante im Graph,
Umbenennen, Lösen.

### Nachtrag – Ordner-Knoten

Rechtsklick auf einen Ordner → **„Als Ordner-Knoten verknüpfen“**: Im
Graph entsteht ein Knoten mit dem Namen des Ordners, an dem alle Dateien
hängen, die direkt in ihm liegen (ohne versteckte). Die Mitglieder werden
nicht gespeichert, nur der Ordner (`folders` in `.atlas/relations.json`):
später hinzugefügte Dateien gehören dazu, hinaus verschobene nicht mehr.
Ein Unterordner, der selbst Ordner-Knoten ist, hängt am nächsten
Ordner-Knoten darüber. Umbenennen und Verschieben in Atlas passen den
Ordner an, „Ordner-Knoten auflösen“ im selben Menü entfernt ihn. Dateien
zeigen ihren Ordner-Knoten unter „Verknüpft mit“.

Gefunden und behoben: Die schrittweise Speicherbereinigung von QML
(Qt 6.8+) verlor Werte von `var`-Eigenschaften, die während einer laufenden
Sammlung gesetzt wurden – die Wiki-Link-Vorschläge waren plötzlich
`undefined`, als der Graph nach Änderungen der Dateiliste neu rechnete.
Atlas setzt deshalb `QV4_GC_TIMELIMIT=0` (Sammlung in einem Zug), außer die
Variable ist schon gesetzt; die Vorschläge liegen zusätzlich nicht mehr auf
dem Popup.

Rauchtest: Menüeintrag, Ordner-Knoten im Graph mit allen Dateien des
Ordners (auch einer neu angelegten), Datei kennt ihren Knoten, Auflösen.

### Nachtrag – Menüs gestrafft, Einstellungen neu

- Kontextmenü: „Mit Standardprogramm öffnen“ und „Im Dateimanager zeigen“
  entfallen; „Neue Datei …“ und „Neuer Ordner …“ gibt es nur noch auf
  Ordnern und der leeren Fläche, nicht auf Dateien.
- Menü oben rechts: „Editor teilen“ und „Splits untereinander/nebeneinander“
  entfallen (Strg+\\ und die Arbeitsleiste bleiben).
- Einstellungen im Stil der neuen Dialoge: Kopf mit Speicherort, Schalter
  „Orte wie in Dolphin“ mit Herkunft der Orte, darunter eine Liste zu
  Externem Editor, Kalender und Nextcloud, Erweiterungen, Projekt- und
  globalen Aktionen – jeweils mit dem aktuellen Stand; unten die Version.

### Nachtrag – Nextcloud: 405 und HTML-Fehlerseiten

Ein Server antwortete mit „405 Not Allowed“ – die HTML-Fehlerseite von
nginx stand dann als großes Bild im Einstellungsfenster. Jetzt:

- Fehlermeldungen nennen Anfrage und Adresse
  („PROPFIND https://… – 405 Not Allowed“); aus HTML-Seiten wird nur der
  Titel übernommen, alles erscheint als reiner Text (auch in Tooltips).
- Die Suche nach der CalDAV-Adresse probiert nacheinander die Adresse unter
  der Server-Angabe (aus dem Browser kopierte Adressen werden vor `/apps/`
  bzw. `/index.php` abgeschnitten), `/remote.php/dav/` an der Wurzel und
  `/.well-known/caldav`; 400, 403, 404, 405 und 501 führen zur nächsten.

### Nachtrag – Rückmeldungen vom 22.10.

- Menü oben rechts ohne „Kalender-Einstellungen“, „Erweiterungen
  verwalten“ und „Externen Editor festlegen“ (alles in den Einstellungen).
  Sessionfenster ohne ⋮ bei „Neue Session“; „Umbenennen“ im ⋮ jeder Zeile.
  Text im Session-Knopf mittig.
- „Wörter zählen“ doppelt: Jede mit „Neue Erweiterung“ angelegte
  Erweiterung hieß „Beispiel-Erweiterung“ und brachte dieselben Aktionen
  mit. Jetzt fragt Atlas nach dem Namen und legt die Beispiele nur als
  Kommentare an; gleich benannte Aktionen zeigen ihre Quelle in Klammern.
- „Verknüpfen mit …“ auch für Ordner – Ordner-Knoten lassen sich
  untereinander verbinden.
- PDF: Strg+Mausrad hält die Stelle unter der Maus in Seitenkoordinaten
  fest; eine Seite schmaler als die Ansicht sprang vorher zur Seite und
  glitt zurück. „PDF-Kopie exportieren“ nur, wenn es Anmerkungen gibt.
- Terminal: Teilen teilt das aktive Terminal neben oder unter sich
  (verschachtelt, die Teile gleich groß), statt den ganzen Tab umzustellen;
  die Session speichert die Aufteilung als Baum.
- „Groß anzeigen“ beim Graph öffnet ein eigenes großes Fenster.
- Desktop-Symbol: das „A“ der Titelleiste.
- Markdown: Zeilenumbrüche im Text sind auch in der Vorschau Umbrüche;
  der Editor nummeriert die Zeilen (auch umbrochene, je am Zeilenanfang);
  Tab rückt Listenpunkte ein, Umschalt+Tab aus (auch mehrere markierte).

### Nachtrag – Google-Kalender

Alternativ zu Nextcloud lässt sich ein Google-Konto verbinden
(`provider = "google"`; Einrichtung: [google-kalender.md](google-kalender.md)).

- **Anmeldung** mit OAuth 2.0 für installierte Apps
  (`atlas_core::calendar::google`): der Browser öffnet Googles Anmeldeseite,
  Atlas wartet auf `127.0.0.1:<Port>` auf die Rückleitung (PKCE, `state`),
  tauscht den Code gegen Tokens und liest die E-Mail-Adresse aus dem
  ID-Token. Der Nutzer bringt einen eigenen OAuth-Client („Desktop-App“)
  mit; Client-ID und -Secret stehen in `config.toml`, das Refresh-Token
  liegt wie das Nextcloud-App-Passwort verschlüsselt im Geheimnisspeicher
  (eigener Name `google`).
- **Kalenderliste** über die Calendar API (`users/me/calendarList`, Name,
  Farbe, Schreibrecht); **Termine** über Googles CalDAV
  (`apidata.googleusercontent.com/caldav/v2/<id>/events/`) mit demselben
  Abgleich wie bei Nextcloud (Cache, Warteschlange, Konflikte). Der Client
  holt Zugriffs-Tokens selbst und versucht es bei 401 einmal mit einem
  frischen; ein abgelaufenes Refresh-Token führt zu „Bei Google neu
  anmelden“. „Atlas – Projektarbeit“ legt die Calendar API an.
- **Nur Termine:** Aufgaben bleiben bei Google im Projektkalender.
- Der Wechsel zwischen Nextcloud und Google vergisst Anmeldung, Auswahl und
  Cache des anderen Kontos.
- Gefunden dabei: Das eingebaute Terminal behandelt SIGCHLD; endete ein
  Programm, brachen Lesevorgänge des Kalenders mit „Interrupted system
  call“ ab. Unterbrochene Anfragen werden wiederholt.

Kern-Tests gegen einen nachgebildeten Google-Dienst: Token-Endpunkt,
Kalenderliste (mit Leserechten), Termine über CalDAV mit Bearer-Token,
abgelaufenes Zugriffs-Token, widerrufenes Refresh-Token, neuer Kalender;
dazu Rückleitung auf dem lokalen Port und Token-Antworten. Gegen Googles
echte Dienste konnte nicht getestet werden.

### Nachtrag – PDF-Dunkelmodus, Scrollen, Erweiterungen

- **Dunkelmodus für PDFs** (Mond in der PDF-Leiste, `[pdf] dark`): siehe
  den folgenden Nachtrag – die erste Fassung kehrte die Helligkeit pro
  Pixel in Rust um und renderte beim Umschalten alle Seiten neu.
- **Vorschaubilder:** Qts PDF-Bildmodul rendert Seiten durchsichtig; in der
  Dateiansicht liegt jetzt ein weißes Blatt darunter.
- **Mausrad:** 160 px pro Raste statt der wenigen Zeilen der Liste;
  Touchpads (Pixel-Deltas, keine ganze Raste) behalten ihre Bewegung, um
  die Hälfte verstärkt.
- **Erweiterungen „Core“:** Erweiterungen aus der Vorlage von 1.0
  (`beispiel*/`, Titel „Beispiel-Erweiterung“) werden beim Start zu
  `core/` „Core“, `core-2/` „Core 2“ …; eigene Titel bleiben unberührt,
  `[extensions] disabled` wird mitgezogen.
- **Erweiterungsfenster** mit Kopf wie die Einstellungen, Karten mit
  Aktionen rechts und einer Fußzeile: Anzahl und Neu laden links, „Neue
  Erweiterung …“ und „Schließen“ rechts.

Selbsttest: Mausrad scrollt eine volle Stufe, Dunkelmodus an/aus, alte
Beispiel-Erweiterung heißt nach dem Start „Core“. Kern-Test für die
Umbenennung.

### Nachtrag – Dunkelmodus schneller, Links in PDFs

- **Dunkelmodus ohne Neu-Rendern:** Die Umkehrung pro Pixel in Rust
  (farbtonerhaltend) kostete bei großen Seiten spürbar Zeit, und das
  Umschalten renderte alle sichtbaren Seiten neu. Jetzt kehrt der Painter
  der Seite beim Zeichnen um: `Difference` mit Weiß, dann `Lighten` mit
  `#262626` für dunkelgraues statt schwarzes Papier – Qts Raster-Routinen
  (SIMD). Umschalten zeichnet nur neu. Preis: Farbtöne werden mit
  umgekehrt (Rot → Cyan), wie in den meisten PDF-Programmen.
- **Links:** Poppler liefert die Link-Flächen jeder Seite
  (`poppler_page_get_link_mapping`); beim Öffnen liest Atlas sie für das
  ganze Dokument (`PdfDocument.linksJson`). Interne Verweise (auch benannte
  Ziele) springen zur Seite und, wenn angegeben, zur Höhe; `http`, `https`,
  `mailto` und `ftp` öffnet der Browser, andere Schemata (z. B. `file:`)
  werden nicht ausgeführt, nur gemeldet. Über einem Link zeigt der Mauszeiger
  eine Hand und die Adresse als Tooltip; Anmerkungen haben Vorrang.

Tests: Kern-Test mit einer von cairo erzeugten PDF (Web-Link und Sprung
mit Position); Selbsttest mit Link-Annotationen in `zweite.pdf` (Sprung
und URI, Fläche, Sprung zur Zielseite).

### Nachtrag – Kontextleiste mit Modulen, PDF-Export, Pfeiltasten

- **Module der Kontextleiste** (`ContextModules.qml`, oben in der
  „Übersicht“): *Lernzeit* – Pomodoro mit Endzeitpunkt statt
  Sekundenzähler (genau auch bei ruckelnder Oberfläche), vier Punkte für die
  Runden, nach der vierten eine lange Pause; *Demnächst* – Vorkommen des
  Kalenders und Fristen aus Notizen der nächsten sieben Tage, ohne
  erledigte und vergangene, Klick öffnet die Woche mit dem Eintrag;
  *Aufgaben* – noch nicht eingeplante, abhaken setzt den Status „done“,
  Eingabefeld legt neue an. Schalter und Längen unter Einstellungen →
  Kontextleiste (`[context]`); die Einstellungen scrollen jetzt, wenn das
  Fenster klein ist.
- **Markdown → PDF** (`MarkdownPdfExport.qml`): eine unsichtbare
  Vorschauseite (durchsichtig neben dem Fenster – verborgene Seiten legt
  Chromium nicht an) bekommt das HTML der Notiz, wartet auf Mermaid und
  Schriften (`atlas.busy()`) und druckt mit `printToPdf`. Druckstil: helles
  Papier, keine `#`-Marken. Qt druckt ohne Seitenränder und übergeht
  `@page`; eine Tabelle, deren leerer Kopf und Fuß sich auf jeder Seite
  wiederholen, ergibt die Ränder oben und unten. Gefunden: Chromium schreibt
  nicht in Pfade mit `/../`.
- Die alte Beispielaktion mit pandoc braucht `pdflatex`; fehlt es (z. B.
  NixOS ohne TeX Live), bricht pandoc ab – vermutlich der gemeldete Fehler.
- **TeX → PDF**: `latexmk -pdf -interaction=nonstopmode -file-line-error`,
  ohne latexmk `pdflatex`, im Ordner der Datei im Terminal; ohne beides eine
  klare Meldung.
- **Pfeiltasten in PDFs:** hoch/runter 80 px, links/rechts blättern (bei
  breiter Seite sie seitlich verschieben); die Seitenliste nimmt die Tasten
  nicht mehr selbst.
- **Dunkelmodus ohne Ruckeln:** Die Seite wird beim Rendern umgekehrt
  (`max(255 − v, 0x26)` per Tabelle, auf dem Render-Thread); der Painter
  kehrt nur noch übergangsweise um, bis nach dem Umschalten die neue Seite
  da ist. Vorher liefen bei jeder Größenänderung zwei Füll-Durchgänge über
  jede sichtbare Seite.

Selbsttest: Pfeiltasten (über `sendKey`), Export einer Notiz als PDF,
Module (Termin von morgen, neue und erledigte Aufgabe, Kalendermodul aus,
Pomodoro-Phasenwechsel). TeX → PDF nur ohne TeX-Installation geprüft (die
Meldung); latexmk/pdflatex selbst gibt es in der Testumgebung nicht.

### Nachtrag – Lernzeit unten, Kalender nur für heute

- Der Pomodoro-Timer ist eine eigene Komponente (`PomodoroTimer.qml`) am
  unteren Rand der Kontextleiste, außerhalb der Reiter, damit er beim
  Wechsel sichtbar bleibt und weiterläuft. Gestaltung: Ring mit dem
  Fortschritt der Phase (Shape, Bogen), Zeit in Monospace, Phase und
  Runden-Punkte, runder Start/Pause-Knopf in der Phasenfarbe (blau beim
  Lernen, grün in der Pause), daneben Neu beginnen und Überspringen. Der
  Hintergrund wird beim Laufen kräftiger.
- Das Kalendermodul heißt „Heute“ und zeigt nur den Tag: ganztägige
  Einträge zuerst, dann nach Uhrzeit mit Von–Bis; vorbei ist gedimmt, um
  Mitternacht wechselt der Tag.

Selbsttest: Termin von heute erscheint, der von morgen nicht; der Timer
liegt auf einem anderen Reiter an der Unterkante; abschaltbar.

### Nachtrag – Issues #2 bis #7

- **#2 Schwarze Vorschau:** Nach einem Workspace-Wechsel (Hyprland) bekam
  Qt von Chromium kein neues Bild, bis gescrollt wurde. `WebRepaint.qml`
  in jeder Web-Ansicht fordert beim Aktivieren oder Zeigen des Fensters und
  beim Wiederzeigen der Ansicht ein Bild an (kurze Deckkraft-Änderung).
- **#3 `#` vor Überschriften:** war Absicht (CSS-`::before`), störte aber;
  entfernt, h1 bekommt eine feine Linie.
- **#5 Natürliche Sortierung:** `listing::compare_names` vergleicht
  Ziffernfolgen nach Wert (beliebig lang, führende Nullen nachrangig), für
  Dateiansicht und Projektbaum; prozessweiter Schalter aus `[files]`.
  Die zuletzt gewählte Sortierung wird gespeichert und von neuen
  Ordneransichten übernommen (Tabs aus der Session behalten ihre).
- **#6 Rückgängig für PDF-Anmerkungen:** alle Änderungen laufen über
  `addAnnot`/`updateAnnot`/`removeAnnot`; jeder Schritt speichert
  Schnappschüsse (Anmerkung samt Gruppe, `PdfAnnotations.snapshot`) vor und
  nach der Änderung, `restore` setzt sie mit gleicher ID zurück.
  Markierungen über mehrere Seiten und Tippen in einem Kommentar (1,5 s)
  sind je ein Schritt; bis zu 200 Schritte je PDF-Tab.
- **#7 Split-Scrollen:** beide Seiten tauschen die oberste Zeile mit
  Bruchteil aus (zwischen zwei Blockmarken interpoliert). Die Vorschau
  gleitet pro Frame ein Stück zum Ziel; eigene Bewegungen meldet sie nicht
  zurück, und Editorbewegungen kurz nach einer Vorschau-Meldung schickt
  Atlas nicht zurück – keine Rückkopplung.
- **#4 Touchpad-Schwung:** `TouchpadFling.qml` beobachtet die
  Radereignisse (`blocking: false`, die Liste scrollt wie bisher), misst bei
  Pixel-Schritten ohne ganze Raste die Geschwindigkeit der letzten 100 ms
  und ruft nach 70 ms Ruhe `flick()`; ein neues Wischen bricht den Schwung
  ab. Im PDF meldet der eigene Rad-Handler die Schritte. Gefunden: Ein
  nicht angenommenes Ereignis eines WheelHandlers erreicht die Liste
  darunter nicht – nur `blocking: false` lässt sie weiter scrollen.

Selbsttest: neue Prüfungen für alle sechs Issues (Vorschau-Neuzeichnen,
Überschriften ohne `#`, Sortierung an/aus und gemerkt, Rückgängig per
Tastatur und Funktion, Scroll-Sync in beide Richtungen, Schwung im PDF und
im Editor, Mausrad im Editor unverändert).

### Nachtrag – Radierer, Schwung anhalten

- **Radierer** (Werkzeug `erase` neben dem Stift): löscht ganze Striche.
  Treffer: Abstand zwischen dem Weg des Radierers (von der letzten zur
  aktuellen Mausposition) und jedem Abschnitt des Strichs, mit echtem
  Schnitt-Test – fährt man quer über eine Linie, ohne einen ihrer Punkte zu
  berühren, trifft er trotzdem. Radius 6 pt plus halbe Strichbreite; ein
  Kreis zeigt ihn statt des Mauszeigers. Ein Zug ist ein Rückgängig-Schritt:
  Schritte bestehen jetzt aus mehreren Teilen (`parts`).
- **Schwung anhalten:** Qt meldet Touchpad-Scrollen nur, solange sich die
  Finger bewegen; Auflegen ohne Bewegung kam nie an. libinput erkennt das
  als Haltegeste, die Compositoren über `zwp_pointer_gestures_v1` Version 3
  weitergeben – Qt 6.11 bindet davon nur Wischen und Zoomen.
  `cpp/touchpad_hold.cpp` holt sich über
  `QNativeInterface::QWaylandApplication` Display und Zeiger, bindet das
  Protokoll auf einer eigenen Warteschlange (die Schnittstellen sind von
  Hand beschrieben, ohne wayland-scanner) und meldet `hold begin` als
  Signal `touchpadHeld`; jede Schwung-Ebene hält dann an. Unter X11 oder
  ohne Version 3 bleibt es aus. Neue Abhängigkeit: libwayland-client
  (ohnehin über Qt Wayland vorhanden). Gegen Sway (headless) geprüft: die
  Geste wird angebunden; eine echte Fingerberührung gab es dort nicht.
- **Schneller:** Schwung mit 1,6-facher Wischgeschwindigkeit; die
  Höchstgeschwindigkeit wird nur für diesen Schwung angehoben, Mausräder
  bleiben unverändert.

### Nachtrag – Split-Scrollen nachgebessert (#7)

- **Ruckeln:** Die Vorschau zog dem Editor mit einer eigenen Glättung nach;
  der Editor scrollt aber schon animiert – doppelt geglättet hing sie
  hinterher und holte ruckartig auf. Jetzt setzt sie ihre Position in jedem
  Frame direkt. Dazu kamen zwei Rückkopplungen: Beim Neurendern stellte die
  Vorschau ihre Scrollposition wieder her, und Chromiums Scroll Anchoring
  verschob sie bei wachsendem Inhalt – beides galt als Scrollen durch den
  Nutzer und zog den Editor mit. Das Wiederherstellen zählt jetzt als
  eigene Bewegung, Scroll Anchoring ist aus (`overflow-anchor: none`).
- **Gleiche Höhe:** Gemessen wurde an den Zeilenmarken vor jedem Block,
  also über dessen Außenabstand, und mit 8 px Versatz – der Text stand
  rechts tiefer. Jetzt zählt die Oberkante des Blocks selbst. Ausgerichtet
  wird in der Mitte der Ansicht, wo man liest: Die Vorschau setzt größer
  als der Editor, ganz gleich hoch über die ganze Höhe geht nicht; so
  verteilt sich der Unterschied nach oben und unten. Neu abgeglichen wird
  auch nach dem Öffnen im geteilten Modus, nach jedem Rendern und nach
  Größenänderungen.

### Nachtrag – ILIAS-Ansicht

- **Was:** „ILIAS“ unter „Orte“ (Tab `ilias:`) zeigt die Kurse und Gruppen
  des Dashboards als Ordner, darin Ordner, Gruppen, Sitzungen und Dateien
  (nur die Liste, nichts wird vorab geladen). Ein Klick auf eine Datei lädt
  sie nach `<Download-Ordner>/<Kurs>/<Ordner …>/<Dateiname>` und öffnet sie.
  ILIAS' Angaben zur Datei (Typ, Größe, Version, Datum) werden mit dem
  Download gemerkt; ändern sie sich beim nächsten Einlesen, steht „neue
  Version“ daneben, und der Klick lädt sie neu (die alte wird erst ersetzt,
  wenn die neue vollständig ist).
- **Anmeldung:** Die KIT-Anmeldung läuft über Shibboleth und braucht einen
  Browser. Atlas zeigt die Seite der Hochschule in einem eigenen Fenster
  mit einem dauerhaften Web-Profil (`atlas-ilias`); angemeldet ist man,
  sobald eine ILIAS-Seite auf die Abmeldung verweist (gleich in welcher
  Schreibweise – ab ILIAS 8 heißt sie `cmd=doLogout`) oder keine
  Anmeldeseite ist (weder `login.php`/Shibboleth noch Passwortfeld noch
  Anmelde-Link). Erkennt Atlas das nicht, schließt „Angemeldet – Kurse
  zeigen“ das Fenster von Hand. Nimmt ILIAS die Sitzung danach nicht an,
  bleibt das Fenster offen, statt sich im Kreis zu drehen. Die Abrufe
  aus Rust senden die Browserkennung des Profils mit. Weiterleitungen
  folgt der Client selbst: mit den Cookies auf jedem Schritt (ureq ließ
  den von Hand gesetzten `Cookie`-Kopf beim Weiterleiten fallen), nur an
  den ILIAS-Rechner, und neue Cookies von ILIAS gelten für die weiteren
  Abrufe. Nimmt ILIAS die Sitzung direkt nach der Anmeldung nicht an,
  steht das als Fehler da, statt das Fenster erneut zu öffnen. Atlas sieht und speichert das
  Passwort nie, nur die Cookies der Sitzung liegen im Web-Profil.
  „Abmelden“ löscht sie.
- **Einlesen und Laden in Rust** (`atlas_core::ilias`): Die Cookies des
  Profils folgt `cpp/helpers.cpp` über `QWebEngineCookieStore` und gibt sie
  als `Cookie`-Kopf für die ILIAS-Adresse weiter. Damit holt ein Thread die
  Dashboard-Ansichten (ILIAS 7–9 heißen sie verschieden), dann mit vier
  Seiten gleichzeitig jeden Container bis Tiefe 10, jeden nur einmal;
  Kurse innerhalb von Kursen werden nicht verfolgt. Objekte erkennt
  `objects` an Permalinks (`target=fold_12`, `goto_…_fold_12.html`,
  `/go/fold/12`) oder – bei `ref_id`-Links wie Sitzungen – am Typ-Symbol
  des Listeneintrags; Links in Navigation, Reitern und Brotkrumen zählen
  nicht. Landet eine Seite auf einem anderen Rechner oder fehlt der
  Abmelde-Link, ist die Sitzung abgelaufen: das Anmeldefenster öffnet sich,
  danach geht es weiter. Downloads nehmen den Namen aus
  `Content-Disposition` (auch `filename*`), sonst den Titel; jeder Name
  wird zu einem sicheren Pfadteil (keine Schrägstriche, keine führenden
  Punkte), der Pfad bleibt im Download-Ordner.
- **Fehlersuche:** Der Knopf mit dem Dokument in der ILIAS-Leiste öffnet
  `debug.log` (siehe [data-format.md](data-format.md)): was das
  Anmeldefenster lud und wie es geprüft wurde, welche Cookies das Profil
  hat (nur Namen), jede Seite mit Ziel, Titel und Anmelde-Urteil samt
  Grund, was darauf gefunden wurde, und Downloads. Seiten, auf denen etwas
  schiefging, liegen als HTML daneben.
- **Tests:** `atlas-core` prüft Erkennung, Einlesen und Laden gegen einen
  kleinen ILIAS im Test; der Smoke-Test meldet sich bei
  `scripts/ilias-mock.py` an, liest ein, öffnet Ordner, lädt Dateien und
  erkennt eine neue Version.

