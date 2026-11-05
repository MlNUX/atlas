#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# macOS CI of the package (P4.3): bundle.sh, then install from the DMG as a
# user would and check it – signature, helpers, start from the copy (as
# from Finder: no Homebrew in PATH, so the search proves the bundled rg),
# screenshot.
set -euo pipefail
root="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
out="${ATLAS_CI_OUT:-$root/target/ci-out}"
mkdir -p "$out"
say() { printf '\n==> %s\n' "$*"; }

"$root/packaging/macos/bundle.sh" "$out/dist" 2>&1 | tee "$out/bundle.log"
dmg="$(ls "$out"/dist/Atlas-*.dmg)"

say "Install from the DMG"
mnt="$(mktemp -d)"
hdiutil attach -nobrowse -readonly -mountpoint "$mnt" "$dmg" >/dev/null
ls -la "$mnt"
apps="${RUNNER_TEMP:-/tmp}/Programme"
rm -rf "$apps" && mkdir -p "$apps"
ditto "$mnt/Atlas.app" "$apps/Atlas.app"
hdiutil detach "$mnt" >/dev/null
app="$apps/Atlas.app"

say "Checks"
codesign --verify --strict --verbose=2 "$app"
codesign --display --verbose=2 "$app" 2>&1 | grep -E "flags|Signature|Identifier"
"$app/Contents/Helpers/rg" --version | head -n 1
otool -L "$app/Contents/Helpers/rg" | tail -n +2
otool -L "$app/Contents/Frameworks/libonnxruntime.dylib" | tail -n +2
python3 - "$app/Contents/Frameworks/libonnxruntime.dylib" <<'PY'
import ctypes, sys
lib = ctypes.CDLL(sys.argv[1])
lib.OrtGetApiBase.restype = ctypes.c_void_p
assert lib.OrtGetApiBase(), "OrtGetApiBase returned NULL"
print("ONNX Runtime loads:", sys.argv[1])
PY
plutil -p "$app/Contents/Info.plist" | grep -E "CFBundle(Identifier|ShortVersionString|IconName|IconFile)|LSMinimumSystemVersion" || true
ls "$app/Contents/Resources/Licenses"
# Gatekeeper refuses ad hoc builds; only notarised ones pass.
spctl --assess --type execute --verbose=2 "$app" 2>&1 || echo "(Gatekeeper: not notarised – expected for ad hoc builds)"

say "Start from the copy"
sample="${RUNNER_TEMP:-/tmp}/Paketprobe"
rm -rf "$sample" && mkdir -p "$sample/Notizen"
printf '# Lineare Algebra\n\nEigenwerte und Algebra …\n' >"$sample/Notizen/Lineare Algebra.md"
printf '# Analysis\n\nKeine Algebra hier? Doch: Algebra.\n' >"$sample/Notizen/Analysis.md"
open -n "$app" --args -AtlasOpenProject "$sample" -AtlasOpenFile "Notizen/Analysis.md" -AtlasSearch Algebra
sleep 10
pgrep -x Atlas >/dev/null || { echo "::error::Atlas from the DMG is not running"; log show --last 2m --predicate 'process == "Atlas"' 2>/dev/null | tail -n 40; exit 1; }
screencapture -x "$out/atlas-dmg.png" || echo "screencapture failed (non-fatal)"
osascript -e 'tell application "Atlas" to quit' 2>/dev/null || pkill -x Atlas || true

{
  echo "# Atlas-Paket (P4.3)"
  echo
  echo "- DMG: \`$(basename "$dmg")\`, $(du -h "$dmg" | cut -f1), SHA-256 \`$(cut -d' ' -f1 "$dmg.sha256")\`"
  echo "- Signatur: $(codesign --display --verbose=2 "$app" 2>&1 | grep -E '^Signature|^Authority' | head -n 1)"
} >"$out/summary.md"
