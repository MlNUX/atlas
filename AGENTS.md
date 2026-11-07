<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Regeln für Agents (macOS-Port)

Gilt für alle Agents, die am macOS-Port von Atlas arbeiten. Der vollständige Plan mit
Entscheidungen (E1–E16), Architektur, Phasen und Wellen steht in
[`docs/porting/macos-swiftui.md`](docs/porting/macos-swiftui.md). Die Entscheidungen dort
sind festgelegt und werden nicht neu verhandelt. Offene Fragen entscheidest du im Geist
dieser Liste, dokumentierst sie im PR und arbeitest weiter (E16: im Zweifel die Variante,
die schneller fertig ist).

Die Aufgaben stehen als GitHub-Issues mit dem Label `macos-port` im Repo, je mit ID,
Branch, Dateien und Abnahme.

## 1. Branches

- Integrationszweig ist der aktuelle Release-Branch, jetzt **`release-1.1.5`** (nach
  dessen Veröffentlichung der nächste, z. B. `release-1.1.6` – der Integrator sagt Bescheid).
- Jede Aufgabe auf einem eigenen Branch **`port/macos/<ID>-<kurz>`**
  (z. B. `port/macos/P2.3-notizen`), abgezweigt vom aktuellen Integrationszweig.
- Pull-Request zurück in den Integrationszweig, nach dem Merge den Branch löschen.
- Keine Betriebssystem-Branches, kein Arbeiten direkt auf dem Integrationszweig.
- Auf dem Integrationszweig landet auch andere Arbeit: **vor jedem PR auf den aktuellen
  Stand rebasen.**

## 2. Datei-Besitz

- Jede Aufgabe nennt die Dateien, die sie ändern darf. Nur diese ändern.
- Gemeinsame Dateien ändert nur der **Integrator (LLM1)**:
  - Workspace-`Cargo.toml`, `crates/atlas-core/Cargo.toml`, `crates/atlas-core/src/lib.rs`
  - `crates/atlas-api/src/lib.rs`
  - `crates/atlas-app/src/app.rs` (Aufzählung der Qt-Funktionen)
  - `.github/workflows/ci.yml`, `.github/workflows/macos-setup.yml`
  - `CHANGELOG.md`
  - Xcode-Projekt (`apps/macos/Atlas.xcodeproj`)
  - `AGENTS.md`
- Wer eine Änderung an einer fremden oder gemeinsamen Datei braucht (Abhängigkeit,
  `pub mod`-Zeile, Menüeintrag …), schreibt sie wörtlich in die PR-Beschreibung; der
  Integrator trägt sie beim Zusammenführen ein.
- Gerüste mit Platzhaltern (P2.1 für `atlas-api`, P3.0 für die Feature-Ordner) legen alle
  Dateien vorab an. Spätere Aufgaben füllen nur ihre eigenen Dateien und ändern Router,
  Menüleiste und Projektdatei nicht.
- Xcode: synchronisierte Ordner bzw. ein lokales Swift-Paket je Funktion, damit neue
  Swift-Dateien die `project.pbxproj` nicht ändern.

## 3. Vertrag zuerst

- Die API (`docs/porting/api.md` + Rust-Signaturen in `crates/atlas-api`) ändert nur die
  Aufgabe P2.1, danach nur der Integrator.
- Swift-Aufgaben bauen gegen die generierten Bindings (`AtlasKit`), nie an der API vorbei.
- Fachlogik gehört in `atlas-core`/`atlas-api`, nicht in Swift oder QML. Die Fassade ruft
  nur `atlas-core` auf und doppelt keine Logik.
- Projekte sind zwischen Linux und Mac austauschbar (E12, **bindend**): Das
  `.atlas/`-Format (Anmerkungen, Sitzungen, Index, Kontextleiste) bleibt gleich.

## 4. Fertig heißt

- Code formatiert: `cargo fmt`, `swift-format`.
- Tests grün: `cargo test`, Xcode-/`swift test`-Tests.
- Wurde der Kern berührt: Linux-Selbsttest `scripts/smoke-test.sh` weiter **BESTANDEN**;
  die Qt-Version bleibt lauffähig.
- Abnahmekriterium der Aufgabe (siehe Issue) erfüllt.
- PR mit kurzer Notiz: **was**, **wie geprüft**, **was offen** – und ggf. benötigte
  Änderungen an gemeinsamen Dateien (Regel 2).

## 5. macOS über CI

- Mac-Builds und -Tests laufen auf den macOS-Runnern von GitHub Actions (arm64).
- Jede Mac-Aufgabe hat eine eigene Workflow-Datei **`.github/workflows/mac-<ID>.yml`**,
  die nur auf ihren Branches (`port/macos/<ID>-*`) läuft und
  `.github/workflows/macos-setup.yml` (`workflow_call`) aufruft.
- Installationen nur im Einrichtungs-Workflow (gehört dem Integrator), nie von Hand im
  eigenen Workflow.
- Ergebnisse (Logs, `screencapture`-Bilder, `.xcresult`) landen in `$ATLAS_CI_OUT` und
  werden als Artefakt hochgeladen; eine `summary.md` darin erscheint auf der Seite des Laufs.
  Lassen sich Artefakte aus der eigenen Umgebung nicht laden: Bildschirmfotos (`*.png`)
  stehen zusätzlich im Job-Log, `apps/macos/scripts/ci-screenshots.sh <log>` holt sie heraus
  (Log mit `gh run view --job <id> --log`).
- Ein Push startet den eigenen Workflow nur, wenn er Dateien unter `paths:` ändert (ein
  reiner Merge aus dem Integrationszweig oft nicht).
- Caches für Cargo und Homebrew nutzen; Läufe nur bei Änderungen in den eigenen Pfaden
  (`paths:`). macOS-Minuten sind knapp.
- Große Rust-Builds (Kern, XCFramework) macht nur der Integrator, einmal pro
  Zusammenführung, und legt sie als Artefakt ab; andere laden sie herunter.

## 6. Sprache

- Oberfläche: Deutsch (wie Linux); String Catalogs, Deutsch als Ausgangssprache (E11).
- Code-Kommentare: Englisch.
- Commits, PRs, Issues, Doku: Deutsch.

## Rollen

- **LLM1 – Integrator:** führt PRs am Ende jeder Welle in den Integrationszweig zusammen,
  besitzt die gemeinsamen Dateien, lässt nach jedem Merge Linux-Selbsttest und Mac-Tests
  laufen, pflegt die Issues.
- **LLM2:** bearbeitet die LLM2-Aufgaben der jeweiligen Welle und prüft den API-Entwurf
  (P2.1).
- Beide starten die nächste Welle vom neuen Stand des Integrationszweigs.
