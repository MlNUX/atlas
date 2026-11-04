#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds the Rust core for the macOS app (task P2.11): atlas-ffi as a
# static library for arm64 (E2), the Swift bindings, and packs both into
# apps/macos/AtlasKit (AtlasFFI.xcframework + Sources/AtlasKit/atlas.swift).
# Same steps as spikes/uniffi/build.sh.
#
# Usage: apps/macos/scripts/build-core.sh [debug|release]   (default release)
set -euo pipefail

root="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
cd "$root"
profile="${1:-release}"
target=aarch64-apple-darwin
kit=apps/macos/AtlasKit
gen=target/atlas-ffi-gen
flag=$([[ "$profile" == release ]] && echo --release || true)
out="target/$target/$profile"

# Only atlas-ffi: no poppler (feature pdf-poppler stays off, P1.1).
cargo build --locked $flag --target "$target" -p atlas-ffi --lib
cargo run --locked $flag -p atlas-ffi --features cli --bin uniffi-bindgen -- \
  generate --library "$out/libatlas_ffi.dylib" --language swift --out-dir "$gen"

rm -rf "$gen/headers" "$kit/AtlasFFI.xcframework"
mkdir -p "$gen/headers"
cp "$gen/atlasFFI.h" "$gen/headers/"
# SwiftPM/Xcode only pick up a module map named module.modulemap.
cp "$gen/atlasFFI.modulemap" "$gen/headers/module.modulemap"

xcodebuild -create-xcframework \
  -library "$out/libatlas_ffi.a" -headers "$gen/headers" \
  -output "$kit/AtlasFFI.xcframework"
cp "$gen/atlas.swift" "$kit/Sources/AtlasKit/atlas.swift"

ls -la "$out/libatlas_ffi.a"
echo "AtlasKit ready: $kit"
