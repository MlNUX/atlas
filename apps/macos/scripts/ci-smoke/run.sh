#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# macOS CI check for task P0.3 (run by .github/workflows/mac-P0.3.yml):
#   1. generate and build/test an empty SwiftUI app (fatal on failure),
#      launch it and take a screenshot (best effort),
#   2. try the Rust crates on macOS and write a report of what breaks
#      (informational: the core still needs poppler until P1.1).
# Results land in $ATLAS_CI_OUT (uploaded as artifact by macos-setup.yml).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(git -C "$here" rev-parse --show-toplevel)"
out="${ATLAS_CI_OUT:-$root/target/ci-out}"
derived="${RUNNER_TEMP:-$root/target}/xcode-smoke"
mkdir -p "$out"

say() { printf '\n==> %s\n' "$*"; }

# --- 1. Xcode ---------------------------------------------------------------
say "XcodeGen"
(cd "$here" && xcodegen generate --quiet)

macos_major="$(sw_vers -productVersion | cut -d. -f1)"
if (( macos_major >= 26 )); then
  action=test
else
  # Deployment target is macOS 26: older runners can build but not run it.
  action=build
  echo "macOS $(sw_vers -productVersion) < 26: build only, no tests/screenshot" | tee "$out/xcode-note.txt"
fi

say "xcodebuild $action"
rm -rf "$out/AtlasSmoke.xcresult"
xcodebuild \
  -project "$here/AtlasSmoke.xcodeproj" \
  -scheme AtlasSmoke \
  -destination 'platform=macOS,arch=arm64' \
  -derivedDataPath "$derived" \
  -resultBundlePath "$out/AtlasSmoke.xcresult" \
  "$action" 2>&1 | tee "$out/xcodebuild.log"

if [[ "$action" == test ]]; then
  say "Screenshot"
  app="$derived/Build/Products/Debug/AtlasSmoke.app"
  if open "$app"; then
    sleep 6
    screencapture -x "$out/atlas-smoke.png" || echo "screencapture failed (non-fatal)"
    pkill -x AtlasSmoke || true
  else
    echo "could not launch $app (non-fatal)"
  fi
fi

# --- 2. Rust ----------------------------------------------------------------
report="$out/rust-report.md"
{
  echo "# Rust auf macOS ($(sw_vers -productVersion), $(uname -m))"
  echo
  echo "$(rustc --version) · $(cargo --version)"
  echo
  echo "Hinweis: \`atlas-core\` hat noch keine Features; \`--no-default-features\` baut"
  echo "deshalb dasselbe wie der Standard, inklusive poppler/cairo/glib (P1.1 ändert das)."
} >"$report"

# Runs one cargo command, logs it and appends a section with the first errors.
rust_check() {
  local name="$1"; shift
  local log="$out/cargo-$name.log"
  say "cargo $*"
  local rc=0
  cargo "$@" >"$log" 2>&1 || rc=$?
  tail -n 40 "$log"
  {
    echo
    echo "## \`cargo $*\`"
    echo
    if (( rc == 0 )); then
      echo "**Ergebnis: grün**"
    else
      echo "**Ergebnis: rot** (Exit $rc, vollständiges Log: \`cargo-$name.log\`)"
      echo
      echo '```'
      # error lines plus their source location, and failing build scripts / tests
      awk '/^error/ {print; getline; if ($0 ~ /-->/) print; next}
           /failed to run custom build command/ ||
           /pkg-config|Package .* was not found/ ||
           /^test .* FAILED$/ || /^failures:$/' "$log" | head -n 80
      echo '```'
    fi
  } >>"$report"
}

rust_check core-no-default test --locked -p atlas-core --no-default-features
rust_check terminal-spell test --locked -p atlas-terminal -p atlas-spell

say "Report"
cat "$report"
cp "$report" "$out/summary.md"
