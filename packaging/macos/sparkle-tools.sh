#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Prints the folder with Sparkle's tools (generate_keys, sign_update,
# generate_appcast) of the version the app links (apps/macos/project.yml),
# fetched once from the release and checked (P4.4).
set -euo pipefail
SPARKLE_VERSION=2.10.1
SPARKLE_SHA256=1a856d2a9edb8e628de88121d7be04cf0e2167f8641650b78fca00b14244cf27
cache="${ATLAS_PACKAGING_CACHE:-$HOME/Library/Caches/atlas-packaging}/sparkle-$SPARKLE_VERSION"
if [[ ! -x "$cache/bin/generate_appcast" ]]; then
  mkdir -p "$cache"
  archive="$cache.tar.xz"
  curl -fsSL --retry 3 -o "$archive" \
    "https://github.com/sparkle-project/Sparkle/releases/download/$SPARKLE_VERSION/Sparkle-$SPARKLE_VERSION.tar.xz"
  echo "$SPARKLE_SHA256  $archive" | shasum -a 256 -c --status || { echo "checksum of Sparkle $SPARKLE_VERSION does not match" >&2; exit 1; }
  tar -xJf "$archive" -C "$cache" ./bin
  rm -f "$archive"
fi
echo "$cache/bin"
