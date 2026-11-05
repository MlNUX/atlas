#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# macOS CI of the app (P3.0 onwards): build the core (AtlasKit), generate
# the Xcode project, run the unit tests, start the app on a sample project
# and take screenshots ($ATLAS_CI_OUT/*.png, also printed into the log).
set -euo pipefail
root="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
out="${ATLAS_CI_OUT:-$root/target/ci-out}"
derived="${RUNNER_TEMP:-$root/target}/atlas-app"
mkdir -p "$out"
cd "$root/apps/macos"

say() { printf '\n==> %s\n' "$*"; }

say "Core (AtlasKit)"
"$root/apps/macos/scripts/build-core.sh" release >"$out/build-core.log" 2>&1 || { tail -n 60 "$out/build-core.log"; exit 1; }

say "Xcode project"
xcodegen generate --quiet

say "xcodebuild test"
# Swift packages (Sparkle) come from GitHub: retry when a download fails (504 …).
for attempt in 1 2 3 4; do
  xcodebuild -resolvePackageDependencies -project Atlas.xcodeproj -scheme Atlas >"$out/resolve.log" 2>&1 && break
  [[ $attempt == 4 ]] && { tail -n 20 "$out/resolve.log"; exit 1; }
  echo "package download failed, retry $attempt"; sleep $((attempt * 15))
done
xcodebuild -project Atlas.xcodeproj -scheme Atlas -destination 'platform=macOS,arch=arm64' \
  -derivedDataPath "$derived" -resultBundlePath "$out/Atlas.xcresult" -skipPackagePluginValidation \
  test >"$out/xcodebuild.log" 2>&1 || { grep -E "error:|✘|failed" "$out/xcodebuild.log" | head -n 60; exit 1; }
grep -E "✔|✘|Test run|TEST SUCCEEDED" "$out/xcodebuild.log" | tail -n 30

say "Sample project"
sample="${RUNNER_TEMP:-/tmp}/Beispielprojekt"
rm -rf "$sample"
mkdir -p "$sample/Notizen" "$sample/Skripte"
printf '# Analysis\n\n- [ ] Blatt 3\n- [x] Blatt 2\n\n[[Lineare Algebra]] #lernen\n' >"$sample/Notizen/Analysis.md"
printf '# Lineare Algebra\n\nEigenwerte …\n' >"$sample/Notizen/Lineare Algebra.md"
printf 'fn main() {\n    println!("Hallo");\n}\n' >"$sample/Skripte/main.rs"
cp "$root/spikes/pdfkit/fixtures/spike.pdf" "$sample/Skript.pdf" 2>/dev/null || true

say "Launch + screenshots"
app="$derived/Build/Products/Debug/Atlas.app"
# `open` brings the app to the front (a directly started binary stays behind Finder).
open -n "$app" --args -AtlasOpenProject "$sample" -AtlasOpenFile "${ATLAS_CI_OPEN_FILE:-Notizen/Analysis.md}" ${ATLAS_CI_SEARCH:+-AtlasSearch "$ATLAS_CI_SEARCH"} ${ATLAS_CI_EXTENSION:+-AtlasExtension "$ATLAS_CI_EXTENSION"}
sleep 8
screencapture -x "$out/atlas-window.png" || echo "screencapture failed (non-fatal)"
osascript -e 'tell application "Atlas" to activate' \
  -e 'tell application "System Events" to keystroke "p" using command down' 2>/dev/null || echo "no accessibility for keystrokes (non-fatal)"
sleep 2
osascript -e 'tell application "System Events" to keystroke "lin"' 2>/dev/null || true
sleep 2
screencapture -x "$out/atlas-quickopen.png" || true
osascript -e 'tell application "Atlas" to quit' 2>/dev/null || pkill -x Atlas || true
sleep 2
say "Session written for Linux (E12)"
session="$(find "$sample/.atlas" -name '*.json' -path '*session*' 2>/dev/null | head -n 1 || true)"
if [[ -n "$session" ]]; then
  echo "$session"
  head -c 2000 "$session" | tee "$out/session.json"
  echo
else
  echo "no session file found (non-fatal)"
  find "$sample/.atlas" -maxdepth 3 2>/dev/null | head -n 30 || true
fi

{
  echo "# Atlas.app (P3.0)"
  echo
  echo '```'
  grep -E "✔|✘|Test run" "$out/xcodebuild.log" | tail -n 20 || true
  echo '```'
} >"$out/summary.md"
