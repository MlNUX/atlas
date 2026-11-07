#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Extracts the screenshots that macos-setup.yml prints into a job log.
# For agents that cannot download artifacts.
#
# Usage: ci-screenshots.sh <job-log> [out-dir]
#   job log e.g. from: gh run view --job <id> --log > job.log
set -euo pipefail
log="${1:?usage: ci-screenshots.sh <job-log> [out-dir]}"
out="${2:-.}"
mkdir -p "$out"
# Log lines may carry "<job>\t<step>\t<timestamp> " prefixes; strip everything
# up to the marker or the base64 payload.
sed -E 's/^.*Z //' "$log" | awk -v out="$out" '
  /^ATLAS-SCREENSHOT-BEGIN / { name = $2; file = out "/" name ".b64"; printf "" > file; next }
  /^ATLAS-SCREENSHOT-END/    { close(file); print file; name = ""; next }
  name != ""                 { print >> file }
' | while read -r b64; do
  base64 -d <"$b64" >"${b64%.b64}" && rm "$b64" && echo "${b64%.b64}"
done
