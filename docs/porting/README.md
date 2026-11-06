<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# macOS-Port – Dokumente

Atlas für den Mac (`apps/macos`, SwiftUI) auf demselben Rust-Kern wie die Linux-Version.
Für Benutzer: Installation im [README](../../README.md#macos), Tastenkürzel mit Mac-Spalte
in [`../shortcuts.md`](../shortcuts.md).

## Aktuell

| Dokument | Inhalt |
|---|---|
| [`macos-swiftui.md`](macos-swiftui.md) | Plan des Ports: Entscheidungen (E1–E16), Phasen P0–P4, Aufteilung auf zwei Modelle, Rollen |
| [`paritaet.md`](paritaet.md) | jede Linux-Funktion mit API-Dienst, Swift-Ordner und Stand auf dem Mac; was noch fehlt |
| [`api.md`](api.md) | Vertrag zwischen Kern und Oberfläche (`crates/atlas-api`): Grundsätze, Dienste, Aufgaben |
| [`testkatalog.md`](testkatalog.md) | Szenarien des Linux-Selbsttests als Vorlage der UI-Tests (XCUITest, P4.1) |

Weitere Anleitungen: [`apps/macos/scripts/README.md`](../../apps/macos/scripts/README.md)
(Mac einrichten, bauen, CI) und [`packaging/macos/README.md`](../../packaging/macos/README.md)
(Paket, Signatur, Notarisierung, Updates mit Sparkle).

## Berichte aus Phase 0 (abgeschlossen)

Entscheidungsgrundlagen; der Stand ist der vom 04.11.2026 und wird nicht nachgeführt.

| Dokument | Ergebnis |
|---|---|
| [`spike-uniffi.md`](spike-uniffi.md) | P0.2: UniFFI im Proc-Macro-Modus für `atlas-api`/`atlas-ffi` |
| [`spike-pdfkit.md`](spike-pdfkit.md) | P0.4: PDFKit statt poppler; Koordinaten-Umrechnung der Anmerkungen |
| [`spike-terminal.md`](spike-terminal.md) | P0.5: SwiftTerm statt eigenem Raster |
| [`kern-ohne-poppler.md`](kern-ohne-poppler.md) | P0.6: was im Kern ohne poppler/cairo/glib bricht (umgesetzt in P1.1) |
