#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Writes appcast.xml for the DMGs in a folder, signed with the EdDSA key of
# the updates (P4.4). The app reads it from the newest GitHub release
# (ATLAS_FEED_URL in apps/macos/project.yml).
#
#   ATLAS_SPARKLE_PRIVATE_KEY=… packaging/macos/appcast.sh <folder with Atlas-*.dmg> <download URL prefix>
#
# The prefix is where the DMGs are downloaded from, e.g.
# https://github.com/MlNUX/atlas/releases/download/v1.1.5/
# ATLAS_SPARKLE_PRIVATE_KEY is the key as `generate_keys -x` exports it.
set -euo pipefail
dir="${1:?usage: appcast.sh <folder> <download URL prefix>}"
prefix="${2:?usage: appcast.sh <folder> <download URL prefix>}"
[[ -n "${ATLAS_SPARKLE_PRIVATE_KEY:-}" ]] || { echo "appcast.sh: ATLAS_SPARKLE_PRIVATE_KEY missing" >&2; exit 1; }
tools="$("$(dirname "${BASH_SOURCE[0]}")/sparkle-tools.sh")"
printf '%s' "$ATLAS_SPARKLE_PRIVATE_KEY" | "$tools/generate_appcast" --ed-key-file - \
  --download-url-prefix "$prefix" ${ATLAS_RELEASE_NOTES_URL:+--full-release-notes-url "$ATLAS_RELEASE_NOTES_URL"} \
  -o "$dir/appcast.xml" "$dir"
cat "$dir/appcast.xml"
