#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Spike P0.2: builds the Rust crate for arm64 macOS, generates the Swift
# bindings and packs the static library into AtlasSpikeFFI.xcframework.
# Model for apps/macos/scripts/build-core.sh (P2.11).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

target=aarch64-apple-darwin
lib=atlas_spike
rel="target/$target/release"

cargo build --locked --release --target "$target" --lib
cargo run --locked --release --features cli --bin uniffi-bindgen -- \
  generate --library "$rel/lib$lib.dylib" --language swift --out-dir target/gen

rm -rf target/headers SpikeKit/AtlasSpikeFFI.xcframework
mkdir -p target/headers
cp "target/gen/${lib}FFI.h" target/headers/
# SwiftPM/Xcode only pick up a module map named module.modulemap
cp "target/gen/${lib}FFI.modulemap" target/headers/module.modulemap

xcodebuild -create-xcframework \
  -library "$rel/lib$lib.a" -headers target/headers \
  -output SpikeKit/AtlasSpikeFFI.xcframework
cp "target/gen/$lib.swift" SpikeKit/Sources/AtlasSpike/

ls -la "$rel/lib$lib.a" SpikeKit/AtlasSpikeFFI.xcframework
