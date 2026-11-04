<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Atlas – Hinweise für Claude

**Lies zuerst [`AGENTS.md`](AGENTS.md).** Die Regeln dort (Branches, Datei-Besitz,
„Fertig heißt“, macOS über CI, Sprache) gelten auch für Claude. Plan des macOS-Ports:
[`docs/porting/macos-swiftui.md`](docs/porting/macos-swiftui.md).

## Repo in Kürze

- `crates/atlas-core` – Kern (Rust, ohne Qt): Projekte, Notizen, Suche, Kalender, PDF,
  Aktionen, Erweiterungen, Einstellungen.
- `crates/atlas-terminal`, `crates/atlas-spell`, `crates/atlas-ilias` – Terminal,
  Rechtschreibung, ILIAS (ohne Qt).
- `crates/atlas-app` – Linux-Oberfläche: cxx-qt-Brücke (`src/`), QML (`qml/`),
  Vorschau-Seite (`web/`).
- `docs/` – Benutzer- und Entwicklerdoku, `docs/porting/` für den macOS-Port.

## Bauen und testen

```sh
nix develop                                                        # alle Werkzeuge
cargo build -p atlas-app                                           # Linux-App (braucht Qt 6)
cargo test -p atlas-core -p atlas-terminal -p atlas-spell -p atlas-ilias   # Kern-Tests
cargo fmt --all                                                    # Format (rustfmt.toml)
scripts/smoke-test.sh                                              # Oberflächentest (offscreen)
```

Ohne Nix: `QMAKE=qmake6 cargo build --release -p atlas-app`. CI: `nix flake check -L`
(`.github/workflows/ci.yml`).

## Konventionen

- Neue Dateien bekommen den SPDX-Kopf (`GPL-3.0-or-later`, `2026 Minux`) wie die übrigen.
- Oberfläche und Doku Deutsch, Code-Kommentare Englisch, Commit-Nachrichten Deutsch.
