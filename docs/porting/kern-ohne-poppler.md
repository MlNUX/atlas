<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# P0.6 – Kern ohne poppler auf macOS

Stand 04.11.2026 · Skript: `apps/macos/scripts/core-without-poppler.sh` · CI:
`.github/workflows/mac-P0.6.yml`, Lauf
[41833368412](https://github.com/MlNUX/atlas/actions/runs/41833368412) (macOS 26.6.2 arm64,
rustc 1.99.1)

Das Skript entfernt `poppler-rs`, `poppler-sys-rs`, `glib` und `cairo-rs` aus
`crates/atlas-core/Cargo.toml` und ruft `cargo check -p atlas-core --all-targets` auf.
Zusätzlich testet es `atlas-terminal` und `atlas-spell` und sucht im Code nach
Linux-Annahmen, die sich erst zur Laufzeit zeigen.

## Übersetzungsfehler (67 Stellen)

| Datei | Stellen | Ursache | Lösung |
|---|---|---|---|
| `pdf/poppler.rs` | 32 | poppler, glib, cairo | ganzes Modul hinter Feature `pdf-poppler` (P1.1) |
| `atlasnote.rs` | 17 | Seiten mit cairo zeichnen, PDF-Hintergrund mit poppler | Zeichnen hinter dem Feature; ohne Feature meldet `render` „nicht verfügbar“, Swift zeichnet selbst (P3.12) (P1.1) |
| `pdf/mod.rs` | 9 | nur Tests (cairo erzeugt Test-PDFs) | Tests hinter dem Feature, Engine-freie Tests getrennt (P1.1) |
| `fileops.rs` | 5 | `trash::os_limited` (Papierkorb auflisten, wiederherstellen, leeren) gibt es im `trash`-Crate unter macOS nicht | eigene macOS-Fassung über `~/.Trash` (P1.2) |
| `search.rs` | 2 | nur ein Test (cairo) | Test hinter dem Feature, neuer Test mit eingespeistem Text (P1.1) |
| `background.rs` | 2 | `libc::prctl(PR_SET_PDEATHSIG)` | `platform::end_with_parent()`, unter macOS wirkungslos (P1.2) |

Hinter der Namensauflösung hat `rustc` keine weiteren Fehler gemeldet. Nach P1.1/P1.2
übersetzt der Kern unter Linux ohne Feature und besteht dort alle Tests (163 grün). Den
Nachweis auf macOS liefert `mac-P1.2.yml`.

## atlas-terminal, atlas-spell

- `atlas-spell`: übersetzt, 4 Tests grün.
- `atlas-terminal`: übersetzt, 11 von 12 Tests grün. `tracks_current_directory` scheitert,
  weil das Arbeitsverzeichnis aus `/proc/<pid>/cwd` gelesen wird. Lösung in P1.2:
  `proc_pidinfo(PROC_PIDVNODEPATHINFO)`.
- `atlas-ilias` hing über die Default-Features am Kern und damit an poppler. Lösung in
  P1.2: `default-features = false`, ILIAS braucht keine PDF-Engine.

## Linux-Annahmen zur Laufzeit

| Stelle | Wirkung auf macOS | Lösung |
|---|---|---|
| `secret.rs`: `systemd-creds` | nicht vorhanden → Passwort nur als Datei | Schlüsselbund (P1.2) |
| `config.rs`: `dirs::config_dir()/atlas` | `~/Library/Application Support/atlas` (klein) | `~/Library/Application Support/Atlas` (E13, P1.2) |
| `places.rs`: XDG-Papierkorb, KDE/GTK-Lesezeichen | falscher Papierkorb, keine Lesezeichen | `~/.Trash`, macOS-Ordnernamen (P1.2); Finder-Favoriten nicht lesbar (binär, geschützt) |
| `perms.rs`: `/proc/sys/kernel/overflowgid` | Datei fehlt, Rückfall auf 65534 | keine Änderung nötig |
| `atlas-spell`: `/usr/share/hunspell`, `XDG_DATA_DIRS` | keine Wörterbücher | P2.8 entscheidet: Homebrew-Pfade/`~/Library/Spelling` oder `NSSpellChecker` |
