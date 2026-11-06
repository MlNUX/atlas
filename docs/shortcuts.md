<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Tastenkürzel

Spalte „macOS“: Auf dem Mac gelten die Konventionen von macOS (E10 in
[`porting/macos-swiftui.md`](porting/macos-swiftui.md)): ⌘ statt Strg, wo es geht; ist das
Kürzel dort schon belegt, steht ein Ersatz da. „–“ heißt: auf dem Mac (noch) ohne Kürzel
oder ohne die Funktion (siehe [Paritätstabelle](porting/paritaet.md#offen-auf-dem-mac)).
Kürzel mit „(PR)“ kommen mit einem offenen Pull Request.

## Allgemein

| Linux | macOS | Aktion |
|---|---|---|
| Strg+P | ⌘P | Arbeitsleiste: Dateien, Ordner, Notizen, Aktionen (`>` Aktionen, `#` Tags, `@` Zitate und PDF-Anmerkungen, `!` Termine, `:` Zeile, `?` Volltext, `pfad:42`); auf dem Mac bisher Dateien und `pfad:42` |
| Strg+Umschalt+P | ⇧⌘P | Arbeitsleiste mit Aktionen |
| Strg+O | ⌘O | Projektordner öffnen |
| Strg+Umschalt+F | ⇧⌘F | Projektweite Suche (Fenster; ↑/↓ wählen, Enter öffnen, Strg+Enter im Split, Esc schließt) |
| Strg+Umschalt+E | – | Explorer |
| Strg+B | ⌃⌘S | Seitenleiste ein-/ausblenden (macOS-Standard) |
| – | ⌥⌘I | Kontextleiste ein-/ausblenden |
| – | ⇧⌘K | Kalender |
| – | ⇧⌘I | ILIAS |
| Strg+` | ⌃` (PR) | Terminal-Panel ein-/ausblenden |
| Strg+Umschalt+` | ⌃⇧` (PR) | Neues Terminal |

Strg+P und Strg+` funktionieren auch, wenn ein Terminal den Fokus hat;
alle übrigen Tasten gehen dort an das Programm im Terminal. Auf dem Mac gehen
⌘-Kürzel immer an das Menü.

## Tabs und Splits

| Linux | macOS | Aktion |
|---|---|---|
| Strg+\\ | ⌘\\ | Editor teilen (Tab in neuen Split) |
| – | ⌥⌘\\ | Splits drehen (nebeneinander / untereinander) |
| Strg+W | ⌘W | Tab schließen |
| Strg+Tab / Strg+Umschalt+Tab | – | Nächster / vorheriger Tab |
| Alt+← / Alt+→ | ⌘[ (Ordneransicht, PR) | Zurück / vor (Ordner, gefolgte Links) |
| Strg+Klick, Mittelklick | – | In neuem Split öffnen (Mac: Kontextmenü „In neuem Split öffnen“) |
| Doppelklick | – | Vorschau-Tab fest anheften |
| Tab ziehen | – | Im Tab-Streifen umsortieren; auf einen anderen Split verschieben (Mitte) oder an dessen linken/rechten Rand für einen neuen Split – der Tab behält seinen Zustand, Terminals laufen weiter |

## Explorer und Dateilisten

| Linux | macOS | Aktion |
|---|---|---|
| Leertaste | Leertaste, ⌘Y (PR) | Schnellansicht (Mac: Quick Look) |
| Enter | Doppelklick | Öffnen (Ordner im Baum auf-/zuklappen) |
| Alt+Enter | ⌘I (PR) | Eigenschaften (Rechte, Besitzer, Gruppe) |
| F2 | – (Menü „Umbenennen …“, PR) | Umbenennen |
| Entf | ⌘⌫ (PR) | In den Papierkorb |
| – | ⌥⌘N / ⇧⌘N (PR) | Neue Datei / neuer Ordner |
| Strg+C / Strg+X / Strg+V | – (Kontextmenü, PR) | Kopieren / Ausschneiden / Einfügen (kompatibel mit anderen Dateimanagern bzw. dem Finder) |
| Alt+↑, Rücktaste | ⌘↑ (PR) | Übergeordneter Ordner (Dateiliste) |
| Strg+Klick (Datei im Baum) | – | Schnellansicht |
| Strg+L | – | Pfad eintippen (Dateiliste, Ordnerauswahl) |
| Strg+Mausrad, Strg+Plus/Minus | – | Symbole größer/kleiner (Dateiliste) |
| Ziehen auf freier Fläche | – | Markierungsrechteck: wählt die Einträge darin (Strg: umschalten, Umschalt: hinzufügen; am Rand scrollt die Liste mit) |

## Texte, Code und Markdown

| Linux | macOS | Aktion |
|---|---|---|
| Strg+S | ⌘S | Sofort speichern (sonst automatisch nach 1 s) |
| Strg+F | ⌘F | Suchen im Dokument (Enter weiter, Umschalt+Enter zurück); Mac: Suchleiste des Systems, auch Ersetzen |
| Strg+G | ⌘L | Gehe zu Zeile (⌘G ist auf dem Mac „Weitersuchen“) |
| Esc | Esc | Suche schließen |
| – | ⌥⌘1 / ⌥⌘2 / ⌥⌘3 | Markdown: Bearbeiten / Geteilt / Vorschau |
| – | ⇧⌘O | Code: Symbole ein-/ausblenden |
| – | ⌘R | Skript ausführen |
| – | ⌥⌘U | HTML: als Webseite / Quelltext anzeigen |
| `[[` (Markdown) | `[[` | Wiki-Link: Vorschläge aus dem Projekt, ↑/↓ wählen, Enter/Tab übernehmen |
| ⇠ / ◎ (Markdown, PDF) | Kontextleiste | Rückverweise mit Filtern / Graph des Projekts |

## PDF

| Linux | macOS | Aktion |
|---|---|---|
| Strg+F | – (Suchfeld in der Leiste) | Volltextsuche im Dokument; F3 / Umschalt+F3 nächster / vorheriger Treffer |
| Bild↑ / Bild↓, Strg+Pos1 / Strg+Ende | Bild↑ / Bild↓ (PDFKit) | Seiten blättern, Anfang / Ende |
| Strg++ / Strg+− / Strg+Mausrad | Zusammenziehen/Aufziehen auf dem Touchpad | Zoom |
| Strg+0 | – | Seitenbreite |
| – | ⌥⌘T | Miniaturen ein-/ausblenden |
| Ziehen / Doppelklick | Ziehen / Doppelklick | Text auswählen (Wort); Umschalt+Klick erweitert die Auswahl |
| Strg+C | ⌘C | Ausgewählten Text kopieren |
| Strg+H | ⇧⌘H | Auswahl markieren (zuletzt gewählte Farbe; ⌘H blendet auf dem Mac die App aus) |
| – | ⇧⌘U / ⇧⌘X | Auswahl unterstreichen / durchstreichen |
| Strg+Q | – | Auswahl markieren und als Zitat in eine Notiz übernehmen (Mac: Menü; ⌘Q beendet) |
| Strg+D | – | Lesezeichen für die aktuelle Seite setzen / entfernen |
| Klick / Rechtsklick auf eine Anmerkung | Klick / Rechtsklick | Kommentar, Farbe, Zitat, Link kopieren, Löschen |
| Strg+Enter (im Kommentar) | – | Kommentar übernehmen |
| Esc | Esc | Auswahl aufheben, zurück zum Auswahlwerkzeug |

## Bilder

| Linux | macOS | Aktion |
|---|---|---|
| – | ⌘+ / ⌘− | Vergrößern / verkleinern |
| – | ⌘0 / ⌘1 | Einpassen / Originalgröße |

## Kalender

| Linux | macOS | Aktion |
|---|---|---|
| T | ⌥⌘T | Heute |
| Alt+← / Alt+→ | – (Knöpfe) | Vorige / nächste Woche bzw. Monat |
| Enter | Doppelklick | Ausgewählten Eintrag bearbeiten |
| Entf | – (Dialog) | Ausgewählten Eintrag löschen |
| Klick / Doppelklick | Doppelklick | Auswählen (Details rechts) / bearbeiten |
| Doppelklick auf freie Zeit, Ziehen über freie Zeit | Doppelklick | Neuer Termin (eine Stunde / gezogener Zeitraum) |
| Eintrag ziehen, unteren Rand ziehen | Eintrag ziehen | Verschieben (15-Minuten-Raster) / Dauer ändern |
| Datei aus dem Explorer auf einen Eintrag / auf freie Zeit | – | Verknüpfen / Arbeitsblock anlegen |
| Datei auf „Noch nicht eingeplant“ | – | Als Aufgabe einplanen |
| Aufgabe in die Woche oder den Monat ziehen | – (Kontextmenü „Nächste volle Stunde einplanen“ …) | Als Arbeitsblock einplanen |

## Terminal

| Linux | macOS | Aktion |
|---|---|---|
| Strg+Umschalt+C / Strg+Umschalt+V | ⌘C / ⌘V | Kopieren / Einfügen |
| Umschalt+Einfg | – | Einfügen |
| Rechtsklick | Rechtsklick (PR) | Menü: Kopieren, Einfügen, Ausgabe in Notiz übernehmen |
