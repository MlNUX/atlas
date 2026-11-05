<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# macOS-Skripte

Gehört zum macOS-Port (`docs/porting/macos-swiftui.md`, Aufgabe P0.3).

| Datei | Zweck |
|---|---|
| `Brewfile` | Homebrew-Pakete (rg, onnxruntime, xcodegen) – einzige Quelle für CI und echten Mac |
| `bootstrap.sh` | richtet einen Mac ein (prüft arm64, macOS 26, Xcode 26; Brewfile; Rust-Target); läuft auch in der CI |
| `ci-screenshots.sh` | holt die Bildschirmfotos aus einem Job-Log (für Agents, die keine Artefakte laden können) |
| `core-without-poppler.sh` | Bericht P0.6: was im Kern ohne poppler/cairo/glib bricht (verändert die Arbeitskopie, nur CI) |
| `ci-smoke/` | leere SwiftUI-App (XcodeGen) + `run.sh`: Xcode-Build/-Test, Bildschirmfoto, Rust-Bericht |
| `build-core.sh` | baut `atlas-ffi` (arm64) und packt Bibliothek und Swift-Bindungen nach `AtlasKit` (P2.11) |
| `ci-app.sh` | Kern, Xcode-Projekt (XcodeGen), `xcodebuild test`, Start mit Beispielprojekt, Bildschirmfotos (P3.0) |
| `ci-macos.sh` | dauerhafte Mac-CI (P4.2): `ci-app.sh`, dann `swift test` in `AtlasKit`, Zusammenfassung |

## In der CI

`.github/workflows/macos-setup.yml` ist ein wiederverwendbarer Workflow (`workflow_call`).

- **`macos.yml`** (P4.2) läuft dauerhaft für PRs auf den Integrationszweig und für Pushes
  auf ihn und `main`, sobald sich Mac-App, Kern, `atlas-api`/`atlas-ffi` oder die Web-Seiten
  ändern: `ci-macos.sh` (Kern, App-Tests, Start mit Bildschirmfotos, AtlasKit-Tests).
- Eine Mac-Aufgabe, die *vor* ihrem PR auf dem Mac prüfen will, legt zusätzlich eine eigene
  `mac-<ID>.yml` an (Push auf `port/macos/<ID>-*`, Beispiel: `mac-P3.2.yml`); sie wird
  entfernt, sobald die Aufgabe erledigt ist. Wer einen alten Lauf wiederholen will (etwa die
  Spikes P0.2, P0.4, P0.5), holt die Datei aus der Git-Geschichte zurück:
  `git show <commit>^:.github/workflows/mac-P0.4.yml` (Commit von P4.2). Alles, was in `$ATLAS_CI_OUT` landet, wird als Artefakt
hochgeladen; eine `summary.md` darin erscheint auf der Übersichtsseite des Laufs.

Installationen gehören in `Brewfile` bzw. `bootstrap.sh` (Integrator), nicht in die
Aufgaben-Workflows.

## Auf einem echten Mac

```sh
xcode-select -p            # muss auf Xcode 26 zeigen
apps/macos/scripts/bootstrap.sh
apps/macos/scripts/ci-smoke/run.sh   # optional: dieselbe Prüfung wie in der CI
```
