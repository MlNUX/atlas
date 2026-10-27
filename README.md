# Atlas Workspace

**Ein dateizentrierter Arbeitsplatz für Linux:** Projektordner, PDFs,
Markdown-Notizen, Code und Terminal in einem Fenster – für Studium,
Forschung und alle, die mit vielen Dateien in einem Ordner arbeiten.

*A file-centric workspace for Linux (Qt 6, Wayland-first): project folder,
PDF annotation, linked Markdown notes and a terminal. The interface is
German for now.*

![Atlas mit PDF, Markdown-Notiz und Terminal](docs/screenshots/arbeitsbereich.png)

Deine Dateien bleiben gewöhnliche Dateien in deinem Ordner. Atlas legt nur
einen versteckten Unterordner `.atlas/` an (Sessions, Anmerkungen, Index) und
verändert weder PDFs noch Notizen ungefragt.

## Funktionen

- **Projektordner** mit Verzeichnisbaum, Dateiansicht wie in Dolphin,
  Schnellansicht (Leertaste), Tabs und Splits. Jeder Ordner ist eine
  **Session**: Tabs, Splits und Baum werden automatisch gespeichert.
- **PDFs** markieren, kommentieren und zitieren – die PDF bleibt unverändert.
  Formeln als LaTeX erkennen (lokal, [latex-ocr.md](docs/latex-ocr.md)),
  Abbildungen als Bild zitieren, Präsentationsmodus (F5), Dunkelmodus,
  Export als Kopie mit Anmerkungen.
- **Markdown** mit Live-Vorschau, Formeln (KaTeX), Diagrammen (Mermaid,
  mit Vorlagen), Tabellen, Bildern, Ordnerbäumen, Checklisten, `#Tags`,
  Wiki-Links, Backlinks und Graph; Rechtschreibprüfung Deutsch und Englisch.
- **Suche** (`Strg+Umschalt+F`) in Notizen, Code und dem Text aller PDFs,
  mit Typfiltern; der Treffer ist im geöffneten Dokument markiert.
  Die **Arbeitsleiste** (`Strg+P`) findet Dateien, Aktionen, Tags, Zitate
  und Termine.
- **Terminal** im Fenster, **eigene Aktionen** und **Erweiterungen**.
- **Kalender** mit Fristen aus Notizen, optional mit Nextcloud (CalDAV) oder
  Google ([Einrichtung](docs/google-kalender.md)).
- **ILIAS** (z. B. KIT): Favoriten als Ordner, Dateien mit einem Klick laden.

![Schnellansicht einer PDF](docs/screenshots/schnellansicht.png)

## Installation

Atlas läuft unter Linux (x86_64 und aarch64), entwickelt unter Wayland
(Hyprland); X11 geht auch.

**Nix** – ausprobieren ohne Installation:

```sh
nix run github:MlNUX/atlas -- ~/Pfad/zum/Projekt
```

Fest in einer NixOS-Konfiguration mit Flakes:

```nix
# flake.nix des Systems
{
  inputs.atlas.url = "github:MlNUX/atlas/v1.1.2";
  inputs.atlas.inputs.nixpkgs.follows = "nixpkgs";

  outputs = { nixpkgs, atlas, ... }: {
    nixosConfigurations.meinrechner = nixpkgs.lib.nixosSystem {
      modules = [
        ./configuration.nix
        { nixpkgs.overlays = [ atlas.overlays.default ]; }
      ];
    };
  };
}
```

Dann in `configuration.nix` (oder `home.packages` in Home Manager):
`environment.systemPackages = [ pkgs.atlas ];`

Oder `nix profile install github:MlNUX/atlas`.

**Arch Linux** – aus dem AUR: `yay -S atlas-workspace` (oder
`packaging/arch/PKGBUILD` mit `makepkg -si` bauen).

**Aus dem Quellcode** – benötigt Rust ≥ 1.85, einen C++-Compiler, clang,
pkg-config, Qt 6 (Base, Declarative, Wayland, SVG, Image Formats,
WebEngine, WebChannel, Positioning), KF6 Syntax-Highlighting, poppler-glib,
cairo, libwayland-client und ripgrep:

```sh
QMAKE=qmake6 cargo build --release -p atlas-app
./target/release/atlas ~/Pfad/zum/Projekt
```

**Rechtschreibprüfung:** Sie braucht die Hunspell-Wörterbücher `de_DE` und
`en_US`. Das Nix-Paket und `nix develop` bringen sie mit; sonst z. B.
`sudo pacman -S hunspell-de hunspell-en_us` (Arch) oder
`sudo apt install hunspell-de-de hunspell-en-us` (Debian/Ubuntu). Ohne sie
meldet Atlas beim Start „Keine Wörterbücher gefunden“ und prüft nicht.
Eigener Ordner: `ATLAS_HUNSPELL_DIRS=/pfad atlas`.

## Erste Schritte

```sh
atlas ~/Projekte/Bachelorarbeit             # Projektordner öffnen
atlas ~/Projekt --open notizen/plan.md      # mit einer Datei
atlas --help                                # alle Optionen
```

Ohne Argument öffnet Atlas das zuletzt benutzte Projekt.

| Kürzel | Aktion |
|---|---|
| `Strg+P` | Arbeitsleiste: Dateien, `>` Aktionen, `#` Tags, `@` Zitate, `?` Volltext |
| `Strg+Umschalt+F` | Suche im Projekt |
| `Strg+O` | Projektordner öffnen |
| `Leertaste` | Schnellansicht |
| `Strg+\` | Editor teilen |
| `` Strg+` `` | Terminal ein-/ausblenden |
| `F5` | PDF präsentieren |

Alle Kürzel: [docs/shortcuts.md](docs/shortcuts.md).

## Konfiguration und Daten

- Einstellungen im Dialog (Menü oben rechts) oder in
  `~/.config/atlas/config.toml`.
- Eigene Aktionen: [docs/actions.md](docs/actions.md), Erweiterungen:
  [docs/extensions.md](docs/extensions.md).
- Projektdaten liegen in `.atlas/` im Projekt, gerätebezogene unter
  `~/.local/state/atlas`; alle Dateien und Optionen:
  [docs/data-format.md](docs/data-format.md).
- Passwörter stehen nie in Konfiguration oder Projekt: Atlas speichert sie
  verschlüsselt mit `systemd-creds` oder holt sie von einem Befehl wie
  `pass` (das Nextcloud-Passwort ohne systemd-creds als Datei, die nur du
  lesen kannst; das ILIAS-Passwort dann gar nicht).

## Entwicklung

```sh
nix develop                           # alle Werkzeuge
cargo run -p atlas-app -- ~/Projekt   # starten
cargo test -p atlas-core              # Kern-Tests
scripts/smoke-test.sh                 # Oberflächentest (offscreen)
nix flake check                       # Paket, Tests, Formatierung
```

`crates/atlas-core` ist der Rust-Kern ohne Qt (Dateien, Suche, PDF,
Anmerkungen, Notizen, Kalender, ILIAS), `crates/atlas-app` die
Qt-6/QML-Oberfläche über [cxx-qt](https://github.com/KDAB/cxx-qt).
Konzept und Entwicklungsberichte: [docs/README.md](docs/README.md),
Änderungen: [CHANGELOG.md](CHANGELOG.md).

Fehler und Wünsche gerne als [Issue](https://github.com/MlNUX/atlas/issues).

## Lizenz

Copyright © 2026 Minux. Atlas ist freie Software unter der
[GNU General Public License v3.0 oder neuer](LICENSE).
Mitgelieferte Bibliotheken (KaTeX, Mermaid, highlight.js) stehen unter
eigenen Lizenzen, siehe [crates/atlas-app/web/vendor](crates/atlas-app/web/vendor/README.md).
