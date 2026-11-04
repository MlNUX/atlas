#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# macOS CI for spike P0.2 (.github/workflows/mac-P0.2.yml).
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
out="${ATLAS_CI_OUT:-$here/target/ci-out}"
mkdir -p "$out"
cd "$here"

say() { printf '\n==> %s\n' "$*"; }

say "Rust tests"
cargo test --locked 2>&1 | tee "$out/cargo-test.log"

say "Build XCFramework + bindings"
./build.sh 2>&1 | tee "$out/build.log"
wc -c target/gen/atlas_spike.swift | tee -a "$out/build.log"

say "swift test (SpikeKit, Swift 6 language mode)"
(cd SpikeKit && swift test 2>&1) | tee "$out/swift-test.log"

say "SwiftUI app"
(cd SpikeApp && xcodegen generate --quiet)
derived="${RUNNER_TEMP:-$here/target}/spike-app"
xcodebuild -project SpikeApp/SpikeApp.xcodeproj -scheme SpikeApp \
  -destination 'platform=macOS,arch=arm64' -derivedDataPath "$derived" \
  build >"$out/xcodebuild.log" 2>&1 || { tail -n 80 "$out/xcodebuild.log"; exit 1; }
grep -E "error|warning:|BUILD" "$out/xcodebuild.log" | grep -v "^ld: warning" | tail -n 40 || true
app="$derived/Build/Products/Debug/SpikeApp.app"
[[ -d "$app" ]] || { echo "app not built"; exit 1; }

say "Launch + screenshot"
open "$app"
sleep 5
screencapture -x "$out/spike-uniffi.png" || echo "screencapture failed (non-fatal)"
pkill -x SpikeApp || true

{
  echo "# Spike P0.2 – UniFFI"
  echo
  echo "- $(rustc --version), uniffi $(grep -A1 'name = "uniffi"' Cargo.lock | sed -n 's/version = //p' | tr -d '\"')"
  echo "- $(swift --version 2>&1 | head -1)"
  echo "- Größe libatlas_spike.a: $(du -h target/aarch64-apple-darwin/release/libatlas_spike.a | cut -f1)"
  echo
  echo '```'
  grep -E "✔|✘|Test run" "$out/swift-test.log" || true
  echo '```'
} >"$out/summary.md"
