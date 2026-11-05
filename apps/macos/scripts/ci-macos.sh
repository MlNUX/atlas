#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The permanent macOS CI (P4.2, .github/workflows/macos.yml): build the core
# (AtlasKit), run the app's tests with the Xcode scheme (unit tests, and the
# UI tests once P4.1 adds them to the scheme), start the app on a sample
# project with screenshots (ci-app.sh), then the AtlasKit tests (swift test).
# Writes $ATLAS_CI_OUT/summary.md; fails if any step fails.
set -euo pipefail
root="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
out="${ATLAS_CI_OUT:-$root/target/ci-out}"
mkdir -p "$out"
export CARGO_TERM_COLOR=never

say() { printf '\n==> %s\n' "$*"; }
status=0
app="ok"
kit="ok"

# Core, Xcode project, `xcodebuild test`, launch and screenshots.
say "App (ci-app.sh)"
if ! "$root/apps/macos/scripts/ci-app.sh"; then
  app="fehlgeschlagen"
  status=1
fi

# AtlasKit uses the XCFramework ci-app.sh built; build it if that failed early.
say "AtlasKit (swift test)"
if [[ ! -d "$root/apps/macos/AtlasKit/AtlasFFI.xcframework" ]]; then
  "$root/apps/macos/scripts/build-core.sh" release >"$out/build-core.log" 2>&1 || true
fi
if ! (cd "$root/apps/macos/AtlasKit" && swift test) >"$out/swift-test.log" 2>&1; then
  kit="fehlgeschlagen"
  status=1
fi
grep -E "✔|✘|Test run|error:" "$out/swift-test.log" | tail -n 30 || true

# ci-app.sh wrote its own summary; this one replaces it with both parts.
{
  echo "# macOS (P4.2)"
  echo
  echo "| Schritt | Ergebnis |"
  echo "|---|---|"
  echo "| Kern, App bauen, \`xcodebuild test\`, Start mit Beispielprojekt | $app |"
  echo "| AtlasKit (\`swift test\`) | $kit |"
  echo
  echo "## App-Tests"
  echo
  echo '```'
  grep -E "✔|✘|Test run|Executed|TEST (SUCCEEDED|FAILED)" "$out/xcodebuild.log" 2>/dev/null | tail -n 30 || echo "(kein xcodebuild.log)"
  echo '```'
  echo
  echo "## AtlasKit"
  echo
  echo '```'
  grep -E "✔|✘|Test run|error:" "$out/swift-test.log" 2>/dev/null | tail -n 30 || echo "(kein swift-test.log)"
  echo '```'
} >"$out/summary.md"

exit "$status"
