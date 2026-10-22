<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Von Atlas erzeugte Daten

Atlas verändert Projektdateien nur auf ausdrückliche Aktion (Bearbeiten,
Dateioperationen, Linkaktualisierung nach Verschieben in Atlas, bestätigte
Linkreparatur). Alles Übrige liegt in den unten genannten Orten. Pfade in
Projektdaten sind immer **relativ zur Projektwurzel** und verwenden `/`.

## Im Projekt: `.atlas/`

| Datei | Inhalt | Wiederherstellbar? |
|---|---|---|
| `project.toml` | `format_version`, Projekt-`id` (UUID), `created_unix` | wird bei Fehlen neu angelegt |
| `sessions/<device-id>/<session>.json` | die Session des Ordners auf diesem Gerät (eine je Projektordner; ältere Projekte können mehrere haben, genutzt wird die aktive) | nein (Nutzerzustand) |
| `sessions/<device-id>/active.json` | `{"session": "<id>"}` – aktive Session | ja (Standard-Session) |
| `metadata.sqlite` | Dokumentidentitäten und Anzeigezustand | teilweise (Positionen gehen verloren) |
| `index.sqlite` | Dateiliste mit Gerät/Inode | ja, wird bei jedem Start neu aufgebaut |
| `recovery/<hash>.json` | ungespeicherte Bearbeitungen | nein, bis gespeichert oder verworfen |
| `annotations/<name>-<8 hex>.json` | PDF-Annotationen, eine Datei je PDF | nein (Nutzerdaten) |
| `relations.json` | von Hand verknüpfte Dateien („Verknüpfen mit …“): `{"version": 1, "relations": [{"a": "docs/a.pdf", "b": "docs/b.pdf"}], "folders": ["literatur"]}`, ohne Richtung; `folders` sind Ordner-Knoten (alle Dateien direkt im Ordner hängen daran); folgt Umbenennen/Verschieben in Atlas, Einträge gelöschter Dateien entfallen | nein (Nutzerdaten) |
| `actions.yaml` | eigene Aktionen des Projekts ([actions.md](actions.md)) | nein (Nutzerdaten) |
| `extensions/<name>/extension.yaml` | Erweiterungen des Projekts ([extensions.md](extensions.md)) | nein (Nutzerdaten) |
| `cache/processed/` | Ergebnisse der Dateiverarbeitung durch Erweiterungen | ja |
| `calendar/events.ics` | Einträge des Projektkalenders (iCalendar) | nein (Nutzerdaten) |
| `calendar/links.json` | Dateien und Session je Termin-UID | nein (Nutzerdaten) |
| `calendar/state.json` | Ansicht des Kalenders (Woche/Monat/Agenda, Leisten) | ja |
| `calendar/settings.json` | Standardkalender dieses Projekts | ja (dann gilt der globale) |
| `calendar/cache/sync.json` | Nextcloud-Cache, Offline-Warteschlange, Konflikte | teilweise (siehe unten) |

Alle JSON-/TOML-Dateien werden atomar geschrieben (temporäre Datei, fsync,
rename, fsync des Ordners). `.atlas/` kann in `.gitignore` stehen; Sessions
anderer Geräte liegen in eigenen Unterordnern und stören sich nicht.

### Session (`<session>.json`)

```json
{
  "version": 1,
  "name": "Standard",
  "expanded": ["src", "docs/notes"],
  "documents": ["docs/paper.pdf", "README.md"],
  "layout": { "version": 2, "editor": { "groups": [ … ] }, "terminal": { … }, … }
}
```

`layout` gehört der Oberfläche (Splits, Tabs mit Viewer-Zustand wie
PDF-Seite und Zoom, Markdown-Modus, Seitenleiste, Suche, Terminals mit
Arbeitsordnern). Terminal-Tabs stehen als `terminal:<ordner>#<n>` darin.
Ältere Layouts (Phase 0: `panes`) werden beim Laden übernommen.

### `metadata.sqlite`

`PRAGMA user_version` = Schemaversion (aktuell 2, Migration von 1).

```sql
CREATE TABLE documents (           -- wiederfinden nach externem Verschieben
    path TEXT PRIMARY KEY, dev INTEGER, ino INTEGER,
    size INTEGER, head TEXT,       -- BLAKE3 der ersten 64 KiB
    seen_unix INTEGER);
CREATE TABLE document_state (      -- zuletzt gelesene Position je Dokument
    path TEXT PRIMARY KEY, state TEXT,   -- JSON des Viewers, z. B. {"page":3,"zoom":0}
    updated_unix INTEGER);
```

### `index.sqlite`

```sql
CREATE TABLE files (path TEXT PRIMARY KEY, dev INTEGER, ino INTEGER,
                    size INTEGER, mtime_ns INTEGER, is_dir INTEGER);
```

Beachtet `.gitignore`; wird in eine temporäre Datei gebaut und atomar
ersetzt. Ein beschädigter oder veralteter Index wird ohne Rückfrage neu
gebaut.

### Recovery (`recovery/<hash>.json`)

```json
{ "version": 1, "path": "/abs/pfad/notiz.md", "base_hash": "<blake3>",
  "saved_at": 1792355296, "text": "…" }
```

`<hash>` = die ersten 24 Hex-Zeichen von BLAKE3 des absoluten Pfads.
`base_hash` ist die Fassung auf der Platte, auf der die Bearbeitung beruht;
weicht sie beim Wiederherstellen ab, entsteht ein Konflikt statt eines
stillen Überschreibens.

### PDF-Annotationen (`annotations/*.json`)

Die PDF selbst wird nie verändert. Je annotierter PDF gibt es eine Datei;
sie wird mit der ersten Anmerkung angelegt und mit der letzten entfernt.

```json
{
  "version": 1,
  "document": {
    "path": "docs/paper.pdf",
    "blake3": "<BLAKE3 der ganzen Datei>", "size": 482113, "pages": 12,
    "pdf_id": "<permanente ID aus dem PDF-Trailer, falls vorhanden>"
  },
  "annotations": [
    { "id": "1a2b3c4d5e6f", "kind": "highlight", "page": 3,
      "rects": [[72.0, 140.5, 420.25, 16.0]], "text": "markierter Text",
      "comment": "", "color": "#ffd400", "group": "",
      "created": 1792355296, "modified": 1792355296 },
    { "id": "…", "kind": "ink", "page": 4, "ink": [[[80, 300], [150, 340]]],
      "width": 2.0, "color": "#ff6b6b", "created": …, "modified": … },
    { "id": "…", "kind": "bookmark", "page": 7, "title": "Methodik", … }
  ]
}
```

- `kind`: `highlight`, `underline`, `strikeout` (Textmarkierungen mit
  `rects` je Zeile und `text`), `note` (angehefteter Kommentar, `rects`
  = Symbol), `ink` (Freihandstriche), `bookmark` (Lesezeichen einer Seite).
- `page` ist 1-basiert wie in Links; Koordinaten sind PDF-Punkte, Ursprung
  oben links auf der unrotierten Seite.
- Markierungen über mehrere Seiten sind je Seite ein Eintrag mit gemeinsamer
  `group`.
- **Zuordnung:** zuerst über `path`; ist die PDF dort nicht mehr, über
  `blake3` oder `pdf_id` + `pages` (externes Verschieben – `path` wird dann
  angepasst). Verschieben und Umbenennen in Atlas aktualisiert `path`
  direkt. Weicht `blake3` am gleichen Pfad ab, zeigt Atlas einen Hinweis;
  mit „Verstanden“ wird der Fingerabdruck übernommen.
- **Export** schreibt eine Kopie mit echten PDF-Annotationen
  (Highlight/Underline/StrikeOut, Text, Ink); Lesezeichen haben dort keine
  Entsprechung. Das Original kann nicht als Ziel gewählt werden.
- PDFs außerhalb eines Projekts: gleiche Dateien unter
  `$XDG_STATE_HOME/atlas/annotations/`, `path` ist dann absolut.

### Kalender (`calendar/`)

**`events.ics`** ist gültiges iCalendar (RFC 5545) und mit jedem
Kalenderprogramm lesbar. Termine, Arbeitsblöcke und Fristen sind `VEVENT`,
Aufgaben ohne Zeit `VTODO`. Atlas ändert nur die Eigenschaften, die es
verwaltet; unbekannte Eigenschaften, fremde Erinnerungen und Komponenten
bleiben erhalten. Zeiten neuer Einträge stehen mit `TZID` der eigenen
Zeitzone, ganztägige mit `VALUE=DATE` (Ende exklusiv).

```ics
BEGIN:VEVENT
UID:0b6c…@atlas
DTSTART;TZID=Europe/Berlin:20261017T100000
DTEND;TZID=Europe/Berlin:20261017T150000
SUMMARY:Kapitel 3 fertigstellen
CATEGORIES:schreiben
RRULE:FREQ=WEEKLY;COUNT=4
X-ATLAS-KIND:block                  # event | block | deadline | task
X-ATLAS-STATUS:started              # planned | started | done | postponed
X-ATLAS-FILE:/docs/kapitel-3.md     # beliebig oft, wurzelrelativ
X-ATLAS-SESSION:Dokumentation
X-ATLAS-CHECK:x;Gliederung          # Checkliste: "x;" erledigt, " ;" offen
X-ATLAS-ESTIMATE:120                # Aufwand in Minuten
BEGIN:VALARM                        # Erinnerung (15 min vorher)
ACTION:DISPLAY
TRIGGER:-PT15M
END:VALARM
END:VEVENT
```

**`links.json`** hält Dateien und Session zusätzlich je UID – für
Nextcloud-Termine, falls ein Server die `X-ATLAS-*`-Felder nicht bewahrt:

```json
{ "0b6c…@atlas": { "files": ["/docs/kapitel-3.md"], "session": "Dokumentation" } }
```

Verschieben und Umbenennen in Atlas passt die Pfade in beiden Dateien an.
Notizen mit `due:` im Frontmatter erscheinen im Kalender (nur lesend);
Checklisten werden nur über „Checkliste als Aufgaben übernehmen …“ zu
Aufgaben, die Markdown-Datei bleibt dabei unverändert.

**`settings.json`**: `{"default_calendar": "<URL>" | "local"}` – der
Standardkalender für neue Einträge in diesem Projekt; ohne Eintrag gilt
`[calendar] default` aus `config.toml`, sonst der Projektkalender.

**`cache/sync.json`** (Nextcloud, siehe [Phase-2-Bericht](phase2.md)):

```json
{
  "version": 1,
  "home": "https://cloud.example.com/remote.php/dav/calendars/minux/",
  "calendars": {
    "<Kalender-URL>": {
      "url": "…", "name": "Persönlich", "color": "#0082c9", "writable": true,
      "components": ["VEVENT", "VTODO"], "missing": false,
      "objects": { "<href>": { "uid": "…", "etag": "\"abc\"", "data": "BEGIN:VCALENDAR…" } }
    }
  },
  "queue": [ { "id": 7, "kind": "put", "calendar": "…", "href": "…", "uid": "…",
               "base": "\"abc\"", "data": "BEGIN:VCALENDAR…" } ],
  "conflicts": [ { "id": 9, "calendar": "…", "href": "…", "uid": "…", "title": "…",
                   "local": "…" , "remote": "…", "remote_etag": "\"def\"", "message": "" } ],
  "last_sync": 1792355296, "error": "", "next_id": 9
}
```

- `objects` ist der Stand des Servers mit bereits eingetragenen lokalen
  Änderungen; `etag` ist die Serverfassung, auf der der Eintrag beruht
  (`""` = noch nicht auf dem Server).
- `queue` hält je Objekt höchstens eine Änderung (`put` oder `delete`);
  `base` ist die ETag, die überschrieben werden darf (`null` = neu). Neue
  Objekte heißen nach ihrer UID (`<uid>.ics`, nur sichere Zeichen) und
  werden mit `If-None-Match: *` angelegt – eine wiederholte Übertragung
  erzeugt so nie ein Duplikat.
- `conflicts` sind Einträge, die auf beiden Seiten geändert wurden oder die
  der Server abgelehnt hat (`message`); `null` steht für „gelöscht“.
- Das App-Passwort steht nie in dieser Datei, in `config.toml` oder im
  Projekt (zum Merken siehe „Pro Benutzer“).
- „Lokalen Cache leeren“ löscht die Datei (mit der Warteschlange); der
  nächste Abgleich lädt alles neu.

## Pro Benutzer

| Ort | Inhalt |
|---|---|
| `$XDG_STATE_HOME/atlas/app.json` | `{"version": 1, "last_project": "/pfad", "projects": ["/pfad", …]}` – zuletzt geöffnete Projekte (höchstens 30) für die Sessionliste |
| `$XDG_STATE_HOME/atlas/device-id` | `<hostname>-<8 hex>`, einmal erzeugt |
| `$XDG_STATE_HOME/atlas/recovery/` | Recovery für Dateien außerhalb von Projekten |
| `$XDG_CONFIG_HOME/atlas/config.toml` | Einstellungen (siehe unten) |
| `$XDG_CONFIG_HOME/atlas/actions.yaml` | eigene Aktionen für alle Projekte ([actions.md](actions.md)) |
| `$XDG_CONFIG_HOME/atlas/extensions/` | installierte Erweiterungen ([extensions.md](extensions.md)) |
| `$XDG_STATE_HOME/atlas/trusted-extensions.json` | freigegebene Projekt-Erweiterungen: Ordner → BLAKE3 des Manifests |

`ATLAS_STATE_DIR` und `ATLAS_CONFIG_DIR` ersetzen die beiden Ordner (Tests).

### `config.toml`

```toml
[editor]
command = "code --goto {file}:{line}:{column}"   # {file} {line} {column} {dir}
terminal = false                                 # im Atlas-Terminal ausführen

[terminal]
shell = "/run/current-system/sw/bin/fish"        # Standard: Login-Shell

[extensions]
disabled = ["beispiel"]                          # ausgeschaltete Erweiterungen

[places]
file_manager = true                              # Orte wie in Dolphin (ohne KDE: feste + GTK-Lesezeichen); Standard

[calendar]                                       # Nextcloud/CalDAV oder Google – ohne Passwort
provider = ""                                    # "" = Nextcloud/CalDAV, "google" = Google (siehe google-kalender.md)
google_client_id = ""                            # Google: eigener OAuth-Client (Desktop-App)
google_client_secret = ""                        # für Desktop-Apps nicht geheim; das Token steht nie hier
server = "https://cloud.example.com"
user = "minux"
dav_url = ""                                     # optional: CalDAV-URL von Hand
password_command = "pass show nextcloud/atlas"   # optional: gibt das App-Passwort aus
remember_password = true                         # Passwort auf diesem Rechner merken (nur die Wahl steht hier)
default = "https://cloud.example.com/remote.php/dav/calendars/minux/personal/"

[[calendar.calendars]]                           # gewählte Kalender des Kontos
url = "https://cloud.example.com/remote.php/dav/calendars/minux/personal/"
name = "Persönlich"
hidden = false                                   # ausgeblendet: weder angezeigt noch abgeglichen
projects = []                                    # nicht leer: nur in diesen Projektordnern
```

Mit `remember_password` (Häkchen „Auf diesem Rechner merken“) wird das
App-Passwort einmal eingegeben und unter `$XDG_STATE_HOME/atlas/secrets/`
abgelegt (Ordner 700) – ohne Schlüsselbund-Dienst:

- `nextcloud.cred`: verschlüsselt mit `systemd-creds --user` (systemd ≥ 256)
  und an diesen Rechner gebunden (Host-Schlüssel bzw. TPM); eine kopierte
  Datei ist anderswo wertlos. Atlas prüft beim Speichern, dass sie sich
  wieder entschlüsseln lässt.
- `nextcloud.secret`: nur wenn das nicht geht – Klartext mit Rechten 600;
  die Einstellungen zeigen das an. Ist die Datei für andere lesbar, nutzt
  Atlas sie nicht.

Häkchen entfernen oder „Konto trennen“ löscht die Datei; ein anderes Konto
ersetzt sie. Ohne Häkchen wird das Passwort einmal pro Atlas-Start
abgefragt und nur im Arbeitsspeicher gehalten. Ist
`password_command` gesetzt, führt Atlas diesen Befehl stattdessen einmal pro
Start aus (wie ein Shell-Befehl zerlegt, aber ohne Shell, ohne Eingabe,
höchstens 90 s – genug für eine Passphrase im pinentry) und nimmt die erste
Zeile der Ausgabe. Beispiele:

| Werkzeug | Befehl |
|---|---|
| pass / gopass (GPG) | `pass show nextcloud/atlas` |
| rbw (Bitwarden/Vaultwarden) | `rbw get nextcloud-atlas` |
| KeePassXC-Datenbank | `keepassxc-cli show -sa password /pfad/db.kdbx Nextcloud` – fragt das Master-Passwort per Terminal ab, daher nur mit Schlüsseldatei (`-k … --no-password`) |
| age | `age -d -i /pfad/schluessel.txt /pfad/nextcloud.age` |
| systemd-creds | `systemd-creds --user decrypt /pfad/nextcloud.cred` |

Scheitert der Befehl, zeigt der Hinweis unter der Titelleiste Code und
letzte Fehlerzeile (nie die Ausgabe) mit „Erneut versuchen“ und „Eingeben
…“.

Unbekannte Schlüssel bleiben beim Speichern aus Atlas erhalten.

## In Notizen

Atlas speichert nichts Verstecktes in Markdown-Dateien. Es liest Tags aus dem
YAML-Frontmatter (`tags: [a, b/c]`) und `title:`, und es schreibt Links nur in
der dokumentierten Form (`/pfad` ab Projektwurzel oder relativ, mit
`#L10-L20`, `#page=3` oder Überschriften-Anker).

**Wiki-Links** (`[[/docs/architektur.md#Parser|Architektur]]`): `/` ist die
Projektwurzel, sonst relativ zur Notiz; Pfade stehen ohne
Percent-Encoding; ohne Endung ist eine Markdown-Datei gemeint
(`[[/docs/architektur]]`). Nach `#` folgt eine Überschrift, nach `|` der
angezeigte Text. Verschieben in Atlas passt Wiki-Links, normale Links,
Bilder und Referenzdefinitionen (`[x]: pfad`) an und behält dabei die
Schreibweise (wurzelrelativ/relativ, mit/ohne Endung) bei.

PDF-Zitate sind normales Markdown; der Link führt auf Seite und Anmerkung
(`#page=8&annot=<id>`). Andere Programme öffnen damit die PDF (die Seite
verstehen viele Betrachter), Atlas springt zusätzlich zur Markierung:

```markdown
> Der markierte Text aus der PDF …
>
> — [paper.pdf, S. 8](/docs/paper.pdf#page=8&annot=1a2b3c4d5e6f)

Optionaler Kommentar
```
