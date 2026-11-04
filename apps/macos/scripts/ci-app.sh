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
"$app/Contents/MacOS/Atlas" -AtlasOpenProject "$sample" -AtlasOpenFile "Notizen/Analysis.md" \
  >"$out/app.log" 2>&1 &
pid=$!
sleep 8
screencapture -x "$out/atlas-window.png" || echo "screencapture failed (non-fatal)"
osascript -e 'tell application "System Events" to keystroke "p" using command down' 2>/dev/null || true
sleep 2
osascript -e 'tell application "System Events" to keystroke "lin"' 2>/dev/null || true
sleep 2
screencapture -x "$out/atlas-quickopen.png" || true
kill "$pid" 2>/dev/null || true
sleep 1
say "Session written for Linux (E12)"
ls "$sample/.atlas/sessions" 2>/dev/null && cat "$sample/.atlas/sessions/"*.json 2>/dev/null | head -c 2000 | tee "$out/session.json"
echo

{
  echo "# Atlas.app (P3.0)"
  echo
  echo '```'
  grep -E "✔|✘|Test run" "$out/xcodebuild.log" | tail -n 20 || true
  echo '```'
} >"$out/summary.md"
