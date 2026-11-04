<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Spike P0.2 – UniFFI (Rust → Swift)

Stand 04.11.2026 · Code: `spikes/uniffi/` · CI: `.github/workflows/mac-P0.2.yml`

## Ergebnis

**UniFFI im Proc-Macro-Modus taugt für `atlas-api`/`atlas-ffi`.** Alles, was die Fassade
braucht, läuft auf macOS 26.6.2 (arm64) im ersten CI-Lauf
([41832087450](https://github.com/MlNUX/atlas/actions/runs/41832087450)) unter Swift 6
mit strikter Nebenläufigkeitsprüfung, ohne Nacharbeit am generierten Code:

| Baustein | Rust | Swift | Test |
|---|---|---|---|
| Funktionen | `core_version()`, `add(a, b)` | `coreVersion()`, `add(a:b:)` | ✔ `plainCalls` |
| Record + Fehler | `describe_project(path) -> Result<ProjectInfo, SpikeError>` | `throws`, `struct ProjectInfo: Equatable, Hashable, Sendable` | ✔ `recordAndError` |
| Objekt | `Ticker` (`#[derive(uniffi::Object)]`, Konstruktor) | `class Ticker: @unchecked Sendable` | ✔ (über Callback-Test) |
| Callback-Trait | `#[uniffi::export(with_foreign)] trait EventSink`, Aufruf aus einem Rust-Thread | `protocol EventSink: AnyObject, Sendable` | ✔ `callbackArrivesOnMainActorInOrder` (21 Ereignisse, Reihenfolge erhalten, alle auf dem Main-Thread) |
| `async` | `async fn delayed_sum` (Thread + `futures-channel::oneshot`, **keine** Rust-Laufzeit) | `func delayedSum(...) async -> Int64` | ✔ `asyncCall`, ✔ `manyConcurrentAsyncCalls` (50 parallel) |
| SwiftUI | – | `@Observable`-Modell, `.task { await model.load() }` | Bildschirmfoto aus der CI: alle Werte und „Tick 1 … Tick 5, Fertig (5)“ |

Versionen: uniffi 0.32.2, rustc 1.99.1, Xcode 26.6, Swift 6.3.3.

## Festlegungen für P2.1 und P2.11

1. **Proc-Macros, keine UDL.** `uniffi::setup_scaffolding!()` in `atlas-ffi`, Bindings im
   *Library-Modus* aus der gebauten `.dylib` (`uniffi-bindgen generate --library …`).
   `atlas-ffi` bekommt ein eigenes Binary `uniffi-bindgen` hinter dem Feature `cli`, wie
   im Spike, damit die Bindgen-Version immer zur Bibliothek passt.
2. **Crate-Typen** `["lib", "staticlib", "cdylib"]`: Die `staticlib` kommt ins
   XCFramework, die `cdylib` braucht nur der Bindgen.
3. **XCFramework:** Header plus Modulkarte, umbenannt in `module.modulemap`, sonst findet
   SwiftPM das Modul nicht. Erzeugt wird es mit `xcodebuild -create-xcframework -library
   … -headers …`. Vorlage ist `spikes/uniffi/build.sh`.
4. **Swift-Paket:** `binaryTarget` für das XCFramework, darüber ein normales Target mit der
   generierten `.swift`-Datei und handgeschriebenen Ergänzungen (`AtlasKit`).
   `swift-tools-version: 6.2`, `platforms: [.macOS(.v26)]`.
5. **Ereignisse → Main-Actor:** Der Kern ruft den `EventSink` aus beliebigen Threads auf.
   Die Swift-Seite reicht jedes Ereignis mit `DispatchQueue.main.async { MainActor.assumeIsolated { … } }`
   weiter (`MainActorEventSink.swift`). Die Main-Queue ist FIFO, so kommen Ereignisse in
   der Reihenfolge an, in der Rust sie geschickt hat. Mit `Task { @MainActor in … }`
   wäre die Reihenfolge nicht garantiert.
6. **`async` ohne Tokio:** Lange Arbeiten laufen auf eigenen Threads und melden das
   Ergebnis über einen Oneshot-Kanal. UniFFI pollt die Future vom Swift-Executor aus.
   Das passt zum Kern, der heute schon mit Threads und `crossbeam-channel` arbeitet. Das
   UniFFI-Feature `tokio` wird nicht gebraucht.
7. **Fehler:** `#[derive(uniffi::Error, thiserror::Error)]`-Enums mit benannten Feldern;
   in Swift ist das ein `Error` mit `LocalizedError`.
8. **Objekte statt globaler Funktionen** für die Dienste (`Project`, `Notes` …), wie im Plan:
   `#[derive(uniffi::Object)]` mit `#[uniffi::constructor]`. Auf der Swift-Seite sind sie
   `Sendable` und lassen sich direkt in `@Observable`-Modellen halten.

## Zahlen

- Erster Release-Build des Spike-Crates samt Bindgen (`cli`): etwa 2 Minuten auf dem
  Runner. `libatlas_spike.a` ist 21 MB groß (mit Debug-Zeilen, vor dem Strippen). Für
  den Kern wird die Bibliothek deutlich größer, deshalb baut sie laut Plan nur der
  Integrator, einmal pro Zusammenführung, und legt sie als Artefakt ab.
- Ein ganzer CI-Lauf (Rust-Tests, XCFramework, `swift test`, App, Bildschirmfoto) dauert
  rund 4 Minuten.

## Offen / Hinweise

- Der Bindgen ruft `swift-format` auf, wenn es installiert ist. Unter Linux fehlt es (nur
  eine Warnung), in Xcode 26 ist es enthalten.
- Artefakte lassen sich nicht aus jeder Agent-Umgebung laden. Bildschirmfotos stehen
  deshalb zusätzlich im Job-Log; `apps/macos/scripts/ci-screenshots.sh` holt sie heraus.
- Der Spike bleibt als Referenz im Repo, bis P2.11 steht; danach kann `spikes/uniffi/`
  weg (P4.7).
