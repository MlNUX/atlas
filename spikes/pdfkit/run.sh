#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# macOS CI run of the PDFKit spike (task P0.4, .github/workflows/mac-P0.4.yml);
# also works on a real Mac after apps/macos/scripts/bootstrap.sh.
#   1. generate the project, build and run the tests (coordinate maths,
#      selection vs. poppler, render vs. Linux reference),
#   2. launch the app and take a screenshot.
# Results (tables, comparison images, screenshot, .xcresult) go to $ATLAS_CI_OUT.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(git -C "$here" rev-parse --show-toplevel)"
out="${ATLAS_CI_OUT:-$root/target/pdfkit-spike}"
derived="${RUNNER_TEMP:-$root/target}/xcode-pdfkit-spike"
mkdir -p "$out"

say() { printf '\n==> %s\n' "$*"; }

say "XcodeGen"
(cd "$here" && xcodegen generate --quiet)

say "xcodebuild test"
rm -rf "$out/PDFKitSpike.xcresult"
rc=0
TEST_RUNNER_ATLAS_CI_OUT="$out" xcodebuild \
  -project "$here/PDFKitSpike.xcodeproj" \
  -scheme PDFKitSpike \
  -destination 'platform=macOS,arch=arm64' \
  -derivedDataPath "$derived" \
  -resultBundlePath "$out/PDFKitSpike.xcresult" \
  test 2>&1 | tee "$out/xcodebuild.log" || rc=$?

say "Screenshot"
app="$derived/Build/Products/Debug/PDFKitSpike.app"
if [[ -d "$app" ]] && open "$app"; then
  sleep 8
  screencapture -x "$out/app-window.png" || echo "screencapture failed (non-fatal)"
  pkill -x PDFKitSpike || true
else
  echo "app not built or not launchable (non-fatal)"
fi

say "Summary"
{
  echo "# P0.4 Spike PDFKit ($(sw_vers -productVersion), $(xcodebuild -version | head -n 1))"
  echo
  if (( rc == 0 )); then echo "**Tests: grün**"; else echo "**Tests: rot** (xcodebuild Exit $rc)"; fi
  echo
  grep -E "Test (Case|Suite) .*(passed|failed)|error:|XCTAssert" "$out/xcodebuild.log" | grep -v "Suite 'All tests'" | head -n 40 | sed 's/^/    /' || true
  echo
  for f in "$out"/parity-selection.md "$out"/parity-render.md; do
    [[ -f "$f" ]] && { cat "$f"; echo; }
  done
} >"$out/summary.md"
cat "$out/summary.md"
exit "$rc"
