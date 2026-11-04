#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# macOS CI run of the terminal spike (task P0.5, .github/workflows/mac-P0.5.yml);
# also works on a real Mac after apps/macos/scripts/bootstrap.sh.
#   1. build variant B's Rust staticlib (atlas-terminal behind a C ABI),
#   2. generate the project, build, run the tests (function + measurements),
#   3. launch the app (both variants side by side) and take a screenshot.
# Results go to $ATLAS_CI_OUT.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(git -C "$here" rev-parse --show-toplevel)"
out="${ATLAS_CI_OUT:-$root/target/terminal-spike}"
derived="${RUNNER_TEMP:-$root/target}/xcode-terminal-spike"
mkdir -p "$out" "$here/lib"

say() { printf '\n==> %s\n' "$*"; }

say "cargo build (variant B)"
export MACOSX_DEPLOYMENT_TARGET=26.0
cargo build --release --target aarch64-apple-darwin --manifest-path "$here/core/Cargo.toml" 2>&1 | tail -n 5
cp "$here/core/target/aarch64-apple-darwin/release/libatlas_term_spike.a" "$here/lib/"
ls -lh "$here/lib/libatlas_term_spike.a" | tee "$out/staticlib-size.txt"

say "XcodeGen"
(cd "$here" && xcodegen generate --quiet)

say "xcodebuild test"
rm -rf "$out/TerminalSpike.xcresult"
rc=0
TEST_RUNNER_ATLAS_CI_OUT="$out" xcodebuild \
  -project "$here/TerminalSpike.xcodeproj" \
  -scheme TerminalSpike \
  -configuration Release \
  -destination 'platform=macOS,arch=arm64' \
  -derivedDataPath "$derived" \
  -resultBundlePath "$out/TerminalSpike.xcresult" \
  -skipPackagePluginValidation -skipMacroValidation \
  test >"$out/xcodebuild.log" 2>&1 || rc=$?
grep -E "Test Case .*(passed|failed)|Executed|\*\* " "$out/xcodebuild.log" || true
if (( rc != 0 )); then
  # Artifacts cannot be downloaded from every agent environment: put the
  # errors into the job log.
  echo "--- errors (xcodebuild exit $rc)"
  grep -E "error[: ]|failed|Could not|cannot|unresolved" "$out/xcodebuild.log" | grep -v "^note:" | sort -u | head -n 80 || true
  echo "--- last lines"
  tail -n 40 "$out/xcodebuild.log"
fi

say "Screenshot"
app="$derived/Build/Products/Release/TerminalSpike.app"
if [[ -d "$app" ]] && open "$app"; then
  sleep 10
  screencapture -x "$out/app-window.png" || echo "screencapture failed (non-fatal)"
  pkill -x TerminalSpike || true
else
  echo "app not built or not launchable (non-fatal)"
fi

say "Summary"
{
  echo "# P0.5 Spike Terminal ($(sw_vers -productVersion), $(xcodebuild -version | sed -n 1p))"
  echo
  if (( rc == 0 )); then echo "**Tests: grün**"; else echo "**Tests: rot**"; fi
  echo
  grep -E "Test Case .*(passed|failed)|error:" "$out/xcodebuild.log" | head -n 40 | sed 's/^/    /' || true
  echo
  echo "Staticlib: $(cat "$out/staticlib-size.txt")"
  echo
  [[ -f "$out/perf.md" ]] && cat "$out/perf.md"
} >"$out/summary.md"
cat "$out/summary.md"
exit "$rc"
