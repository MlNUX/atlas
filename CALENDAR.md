# Atlas Workspace – Konzept der Kalenderansicht

## Ziel

Der Atlas-Kalender verbindet die Planung innerhalb des geöffneten Projektordners mit bestehenden Nextcloud-Kalendern. Sein Schwerpunkt liegt auf der Verbindung zwischen Zeit, Dateien, Aufgaben und Sessions; E-Mail und Kontakte bleiben außerhalb von Atlas.

Typische Fragen:

- Woran arbeite ich diese Woche?
- Welche Datei oder Notiz gehört zu einem Termin?
- Welche Fristen stehen im Projekt an?
- Welche Aufgaben sind noch nicht eingeplant?
- Welche Atlas-Session soll zu einem Termin geöffnet werden?

## Position in der Oberfläche

„Kalender“ erscheint als eigener Hauptmodus neben „Dateien“ und „Notizen“:

```text
Dateien | Kalender | Notizen | Suche
```

Die linke Leiste mit Orten und Projektordner bleibt unverändert. Dadurch lassen sich Dateien auf Termine ziehen oder aus dem Kalender direkt öffnen.

## Ansichten

### Woche

Die Standardansicht zeigt Montag bis Sonntag, eine Ganztagszeile, Stundenraster, aktuelle Uhrzeit und ungeplante Aufgaben. Sie eignet sich für konkrete Arbeitsblöcke.

### Monat

Die Monatsansicht priorisiert Fristen, ganztägige Termine und Auslastung. Detailinformationen erscheinen erst bei Auswahl in der rechten Kontextleiste.

### Agenda

Eine chronologische Liste für schnelle Navigation und kleine Fenster. Sie gruppiert Einträge nach Heute, Morgen, Diese Woche und Später.

## Kalendereinträge

Ein Eintrag kann enthalten:

- Titel
- Beginn und Ende oder ganztägiges Datum
- Typ: Termin, Arbeitsblock oder Frist
- Status: geplant, begonnen, erledigt oder verschoben
- Tags
- Beschreibung
- Checkliste
- verknüpfte Projektdateien
- verknüpfte Atlas-Session
- optionale Erinnerung
- optionale Wiederholung

Dateiverknüpfungen verwenden projektwurzelrelative Pfade, beispielsweise:

```text
/src/parser.rs
/recherche/literatur.pdf
/docs/kapitel-3.md
```

## Verbindung mit Sessions

Ein Termin kann mit einer benannten Session verknüpft werden. Die Aktion „In Session wechseln“ speichert die aktuelle Session und öffnet anschließend die verknüpfte Arbeitsanordnung.

Die Session wird nicht automatisch zu einer bestimmten Uhrzeit gestartet. Kalender und Session bleiben bewusst unter Kontrolle des Nutzers.

## Aufgaben ohne Termin

Der Bereich „Noch nicht eingeplant“ enthält Aufgaben mit Status und optionalem Aufwand, aber ohne Startzeit. Eine Aufgabe kann auf einen Zeitraum im Kalender gezogen werden und wird dadurch zum Arbeitsblock.

Eine Aufgabe kann aus folgenden Quellen entstehen:

- direkt im Kalender
- Checkliste in einer Markdown-Datei
- Kontextaktion einer Datei: „Als Aufgabe einplanen“
- später: benutzerdefinierte Aktion oder Erweiterung

## Rechte Kontextleiste

Bei Auswahl eines Eintrags zeigt sie:

- Datum, Uhrzeit und Dauer
- Status und Tags
- verknüpfte Dateien
- verknüpfte Session
- Checkliste
- „Datei öffnen“
- „In Session wechseln“
- Zusammenfassung der aktuellen Woche

## Interaktionen

- Klick wählt einen Termin und öffnet seine Details.
- Doppelklick öffnet den kompakten Editor.
- Drag-and-drop verschiebt Termin oder Aufgabe.
- Ziehen an den Rändern verändert die Dauer.
- Datei aus dem Projektbaum auf Termin ziehen verknüpft sie.
- Datei auf freien Zeitraum ziehen erzeugt einen Arbeitsblock.
- Aufgabe aus „Noch nicht eingeplant“ auf die Woche ziehen plant sie ein.
- `Strg + P` findet Termine und Kalenderaktionen.

## Speicherung

Kalenderdaten bleiben projektlokal:

```text
.atlas/
└ calendar/
   ├ events.ics
   └ state.json
```

- `events.ics` verwendet das offene iCalendar-Format.
- Atlas-spezifische Verknüpfungen können als gültige `X-ATLAS-*`-Felder gespeichert werden.
- `state.json` enthält ausschließlich Darstellungszustand wie aktive Ansicht und sichtbaren Zeitraum.
- Projektdateien bleiben außerhalb von `.atlas/`.

## Nextcloud-Verbindung

Die Nextcloud-Anbindung ist ein fester Bestandteil der geplanten Kalenderfunktion. Atlas verwendet CalDAV und benötigt keine Atlas-spezifische App auf dem Nextcloud-Server.

### Einrichtung

Der Einrichtungsdialog fragt nach:

- Nextcloud-Serveradresse, beispielsweise `https://cloud.example.com`
- Benutzername
- separates App-Passwort
- zu synchronisierende Kalender

Atlas ermittelt aus der Serveradresse den DAV-Endpunkt und entdeckt die verfügbaren Kalender. Eine manuelle CalDAV-URL kann in einem erweiterten Bereich eingetragen werden, falls automatische Erkennung nicht funktioniert.

Atlas verwendet keinen Linux-Schlüsselbund. Das App-Passwort wird pro Atlas-Start einmal abgefragt, ausschließlich im Arbeitsspeicher gehalten und beim Beenden verworfen. Es wird niemals in `project.toml`, `events.ics`, einer Session-Datei oder unter `.atlas/` gespeichert.

### Kalenderauswahl

Nach erfolgreicher Verbindung zeigt Atlas alle verfügbaren Nextcloud-Kalender mit Farbe und Zugriffsmodus. Pro Kalender kann festgelegt werden:

- sichtbar oder ausgeblendet
- schreibgeschützt oder für neue Einträge verwendbar
- Standardkalender für neue Termine
- optional ausschließlich in bestimmten Atlas-Projekten sichtbar

Projektbezogene Atlas-Einträge können entweder in einem vorhandenen Nextcloud-Kalender oder in einem eigens dafür angelegten Kalender wie „Atlas – Projektarbeit“ gespeichert werden.

Atlas darf einen eigenen Kalender wie „Atlas – Projektarbeit“ anlegen, fragt davor jedoch ausdrücklich nach Bestätigung.

In den Einstellungen wählt der Nutzer verbindlich einen Standardkalender für neue Termine. Diese Auswahl kann global für Atlas oder abweichend pro Projekt gespeichert werden. Beim Erstellen eines Termins bleibt der Zielkalender sichtbar und kann für diesen einzelnen Termin geändert werden.

### Synchronisierung

- bidirektionale CalDAV-Synchronisierung
- neue und geänderte lokale Termine bei bestehender Verbindung sofort synchronisieren
- manuelles Aktualisieren und automatische Hintergrundsynchronisierung
- Offline-Verwendung über einen lokalen Cache
- Offline-Änderungen in einer lokalen Warteschlange speichern und nach Wiederherstellung der Verbindung automatisch übertragen
- serverseitige Kalenderfarben und Zugriffsrechte respektieren
- gelöschte oder schreibgeschützte Kalender nicht lokal überschreiben
- letzter erfolgreicher Abgleich und Fehlerstatus in der Oberfläche sichtbar

Der lokale Kalendercache liegt projektbezogen unter `.atlas/calendar/cache/`. Das App-Passwort existiert nur während der laufenden Atlas-Sitzung im Arbeitsspeicher.

### Konflikte

Atlas nutzt CalDAV-Änderungsinformationen und Versionskennungen, um parallele Änderungen zu erkennen. Bei einem echten Konflikt wird nichts still überschrieben. Der Nutzer kann zwischen Nextcloud-Version, lokaler Version oder einer Duplikation als neuer Termin wählen.

```text
Dieser Termin wurde in Nextcloud und Atlas geändert.

[Nextcloud-Version] [Lokale Version] [Beide behalten]
```

### Verknüpfung mit Atlas-Dateien

Projektwurzelrelative Dateilinks und Session-Namen sind Atlas-spezifische Zusatzinformationen. Sie dürfen den normalen Termin in anderen CalDAV-Clients nicht unlesbar machen. Wenn der Server unbekannte Erweiterungsfelder nicht zuverlässig erhält, bewahrt Atlas diese Verknüpfungen getrennt in `.atlas/calendar/links.json` und ordnet sie über die stabile Termin-UID zu.

### Verbindung verwalten

In den Kalendereinstellungen stehen zur Verfügung:

- Verbindung testen
- Kalenderauswahl ändern
- jetzt synchronisieren
- App-Passwort für die laufende Atlas-Sitzung neu eingeben
- lokale Cache-Daten leeren
- Konto trennen

„Konto trennen“ entfernt das lokale Zugangstoken und den Cache, löscht aber keine Termine auf dem Nextcloud-Server.

Beispiel eines Eintrags:

```ics
BEGIN:VEVENT
UID:018f-example@atlas
DTSTART:20261017T100000
DTEND:20261017T150000
SUMMARY:Kapitel 3 fertigstellen
X-ATLAS-FILE:/docs/kapitel-3.md
X-ATLAS-SESSION:Dokumentation
END:VEVENT
```

## Markdown-Aufgaben

Eine Markdown-Datei kann optionale planbare Metadaten im YAML-Frontmatter enthalten:

```yaml
---
tags: [dokumentation]
due: 2026-10-18
status: planned
estimate: 2h
---
```

Atlas darf diese Angaben im Kalender darstellen. Markdown-Checklisten und Frontmatter-Daten werden nur über eine sichtbare Nutzeraktion in Kalenderaufgaben übernommen.

## Erinnerungen

Projektinterne Erinnerungen sind optional. In einer ersten Version erscheinen sie nur, wenn Atlas läuft. Eine spätere native Desktop-Benachrichtigung kann über die Linux-Desktop-Schnittstellen ergänzt werden.

## Nicht enthalten

- E-Mail und Kontakte
- Einladungen und Teilnehmerverwaltung
- automatische Verfügbarkeitsplanung
- Nextcloud-Konto als Voraussetzung für die rein lokale Kalendernutzung
- ungefragte Aktivierung weiterer Nextcloud-Dienste
- automatische Ausführung von Sessions oder Terminalbefehlen

## Mögliche spätere Erweiterungen

- Import und Export weiterer `.ics`-Dateien
- wiederkehrende Termine
- native Desktop-Benachrichtigungen
- Zeiterfassung pro Arbeitsblock
- Vergleich von Planung und tatsächlicher Arbeitszeit

## Offene Entscheidungen

- ob Erinnerungen bereits in der ersten Kalender-Version enthalten sind

