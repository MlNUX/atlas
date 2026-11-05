<!-- SPDX-FileCopyrightText: 2026 Minux -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
### macOS (Atlas-*.dmg)

Die Mac-App ist noch **nicht notarisiert** (kein Apple-Developer-Account). Beim ersten Start meldet macOS, dass Apple die App nicht prüfen konnte:

1. `Atlas.app` aus dem DMG in den Programme-Ordner ziehen.
2. Atlas einmal öffnen und die Meldung schließen.
3. **Systemeinstellungen → Datenschutz & Sicherheit → „Dennoch öffnen“** bei Atlas, dann bestätigen.

Oder im Terminal: `xattr -dr com.apple.quarantine /Applications/Atlas.app`

Danach startet Atlas normal. Updates kommen über „Atlas → Nach Updates suchen …“.
