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

## In der CI

`.github/workflows/macos-setup.yml` ist ein wiederverwendbarer Workflow (`workflow_call`).
Jede Mac-Aufgabe hat eine eigene `mac-<ID>.yml`, die ihn mit dem auszuführenden Befehl
aufruft (Beispiel: `mac-P0.3.yml`). Alles, was in `$ATLAS_CI_OUT` landet, wird als Artefakt
hochgeladen; eine `summary.md` darin erscheint auf der Übersichtsseite des Laufs.

Installationen gehören in `Brewfile` bzw. `bootstrap.sh` (Integrator), nicht in die
Aufgaben-Workflows.

## Auf einem echten Mac

```sh
xcode-select -p            # muss auf Xcode 26 zeigen
apps/macos/scripts/bootstrap.sh
apps/macos/scripts/ci-smoke/run.sh   # optional: dieselbe Prüfung wie in der CI
```
