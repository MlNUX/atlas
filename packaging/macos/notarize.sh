#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Notarises a Developer-ID-signed DMG (bundle.sh) and staples the ticket (P4.3).
#
#   packaging/macos/notarize.sh target/macos-dist/Atlas-<version>.dmg
#
# Credentials, one of:
#   App Store Connect API key: ATLAS_NOTARY_KEY (path of the .p8 file),
#                              ATLAS_NOTARY_KEY_ID, ATLAS_NOTARY_ISSUER
#   Apple ID:                  ATLAS_APPLE_ID, ATLAS_TEAM_ID,
#                              ATLAS_APP_PASSWORD (app-specific password)
# Without them it says so and exits 0 (ad hoc test builds).
set -euo pipefail
dmg="${1:?usage: notarize.sh <file.dmg>}"
if [[ -n "${ATLAS_NOTARY_KEY:-}" ]]; then
  auth=(--key "$ATLAS_NOTARY_KEY" --key-id "${ATLAS_NOTARY_KEY_ID:?}" --issuer "${ATLAS_NOTARY_ISSUER:?}")
elif [[ -n "${ATLAS_APPLE_ID:-}" ]]; then
  auth=(--apple-id "$ATLAS_APPLE_ID" --team-id "${ATLAS_TEAM_ID:?}" --password "${ATLAS_APP_PASSWORD:?}")
else
  echo "notarize.sh: no credentials (ATLAS_NOTARY_KEY or ATLAS_APPLE_ID) – not notarised"
  exit 0
fi
if codesign --display --verbose=2 "$dmg" 2>&1 | grep -q "Signature=adhoc" || ! codesign --display "$dmg" >/dev/null 2>&1; then
  echo "notarize.sh: $dmg is not signed with a Developer ID (bundle.sh with ATLAS_SIGN_IDENTITY)" >&2
  exit 1
fi
result="$(mktemp)"
xcrun notarytool submit "$dmg" "${auth[@]}" --wait --timeout 30m --output-format json | tee "$result"
status="$(plutil -extract status raw -o - "$result" 2>/dev/null || python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["status"])' "$result")"
if [[ "$status" != "Accepted" ]]; then
  id="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1])).get("id",""))' "$result")"
  [[ -n "$id" ]] && xcrun notarytool log "$id" "${auth[@]}" || true
  echo "notarize.sh: notarisation $status" >&2
  exit 1
fi
xcrun stapler staple "$dmg"
xcrun stapler validate "$dmg"
spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg"
