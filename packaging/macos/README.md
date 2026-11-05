<!-- SPDX-FileCopyrightText: 2026 Minux -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# macOS-Paket (P4.3)

`bundle.sh` baut `Atlas.app` und `Atlas-<Version>.dmg` (Version aus
`MARKETING_VERSION` in `apps/macos/project.yml`):

1. Kern (`AtlasKit`) und App im Release-Modus, ohne Xcode-Signatur.
2. Mitgeliefert werden **ripgrep** (`Contents/Helpers/rg`) und **ONNX Runtime**
   (`Contents/Frameworks/libonnxruntime.dylib`) aus den offiziellen Releases,
   mit fester Version und SHA-256 im Skript, ihre Lizenzen in
   `Contents/Resources/Licenses`. Die App setzt beim Start `ATLAS_RG_BIN` und
   `ORT_DYLIB_PATH` auf diese Kopien (vom Finder gestartet fehlt Homebrew im
   `PATH`).
3. Signatur von innen nach außen mit Hardened Runtime, ohne Sandbox
   (Terminal, beliebige Projektordner).
4. DMG mit Verknüpfung auf `/Applications`.

```sh
nix develop   # oder apps/macos/scripts/bootstrap.sh
packaging/macos/bundle.sh                        # ad hoc → target/macos-dist/
```

## Signatur und Notarisierung

Solange der Apple-Developer-Account fehlt (E5), signiert `bundle.sh` **ad hoc**
(`Atlas-adhoc.entitlements`: ohne Team-ID muss die Bibliotheksprüfung für ONNX
Runtime aus sein). Gatekeeper lehnt solche Builds ab: nach dem Kopieren in den
Programme-Ordner einmal per Rechtsklick → „Öffnen“ starten oder
`xattr -dr com.apple.quarantine /Applications/Atlas.app`.

Mit Account fehlen nur das Zertifikat und die Zugangsdaten:

| Schritt | Lokal | CI (Secrets) |
|---|---|---|
| Zertifikat „Developer ID Application“ | im Schlüsselbund | `ATLAS_CERT_P12_BASE64`, `ATLAS_CERT_PASSWORD` → `import-certificate.sh >>"$GITHUB_ENV"` |
| Signieren | `ATLAS_SIGN_IDENTITY="Developer ID Application: … (TEAMID)" bundle.sh` | dasselbe, `ATLAS_KEYCHAIN` setzt `import-certificate.sh` |
| Notarisieren und Ticket anheften | `notarize.sh target/macos-dist/Atlas-<Version>.dmg` | API-Schlüssel: `ATLAS_NOTARY_KEY` (Pfad der `.p8`), `ATLAS_NOTARY_KEY_ID`, `ATLAS_NOTARY_ISSUER` – oder Apple-ID: `ATLAS_APPLE_ID`, `ATLAS_TEAM_ID`, `ATLAS_APP_PASSWORD` |

Mit Developer ID gilt `Atlas.entitlements`: Bibliotheksprüfung an, weil die
Helfer mit derselben Identität signiert sind.

## Prüfen

`ci-package.sh` (Workflow `mac-P4.3`): `bundle.sh`, dann die App aus dem DMG
kopieren, Signatur, `rg` und ONNX Runtime prüfen und die Kopie per `open`
starten. Die Projektsuche läuft dabei ohne Homebrew im `PATH`, beweist also das
mitgelieferte `rg`. Das DMG liegt im Artefakt unter `dist/`.

## Symbol

`AppIcon.svg` ist das Logo aus `data/atlas.svg` im macOS-Raster (824 px Fläche in
1024 px). `render-icon.js` erzeugt daraus `Assets.xcassets/AppIcon.appiconset`.

## Updates (Sparkle, P4.4)

Die App sucht mit [Sparkle](https://sparkle-project.org) 2.10.1 nach Updates
(Menü Atlas → „Nach Updates suchen …“, sonst automatisch täglich). Sie liest
`appcast.xml` der neuesten GitHub-Release
(`releases/latest/download/appcast.xml`, `ATLAS_FEED_URL` in
`apps/macos/project.yml`). Jedes Update ist mit dem EdDSA-Schlüssel des
Projekts signiert; ohne öffentlichen Schlüssel (`ATLAS_SPARKLE_PUBLIC_KEY`, leer
in lokalen Builds) hat die App keinen Updater.

Einmalig (Mensch), auf einem Mac:

```sh
bin="$(packaging/macos/sparkle-tools.sh)"
"$bin/generate_keys"                 # legt den Schlüssel im Schlüsselbund an, zeigt den öffentlichen
"$bin/generate_keys" -x sparkle.key  # privater Schlüssel → Secret ATLAS_SPARKLE_PRIVATE_KEY, Datei danach löschen
```

Den öffentlichen Schlüssel als Repository-Variable `ATLAS_SPARKLE_PUBLIC_KEY`
eintragen. **Der private Schlüssel darf nicht verloren gehen:** ohne ihn
findet keine installierte Version mehr ein Update.

`.github/workflows/macos-release.yml` läuft bei jeder veröffentlichten Release:

1. DMG bauen
2. mit der Developer ID signieren und notarisieren
3. `appcast.sh` schreibt `appcast.xml` mit der Download-Adresse der Release
4. DMG, Prüfsumme und Appcast an die Release hängen

Ohne Zertifikat entsteht nur ein ad hoc signiertes DMG im Artefakt (von Hand:
`attach-adhoc`).

`ci-update.sh` (Workflow `mac-P4.4`) prüft den Weg mit einem Wegwerf-Schlüssel:

1. Version 1 und 2 bauen.
2. Version 2 als Appcast auf `localhost` bereitstellen.
3. Version 1 installieren und suchen lassen.
4. Beim Beenden ersetzt Sparkle die App; danach muss Version 2 installiert sein.
