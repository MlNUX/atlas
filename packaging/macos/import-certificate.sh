#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# CI: imports the Developer ID certificate into a temporary keychain and
# prints the lines for $GITHUB_ENV (ATLAS_KEYCHAIN, ATLAS_SIGN_IDENTITY).
#
#   ATLAS_CERT_P12_BASE64=… ATLAS_CERT_PASSWORD=… packaging/macos/import-certificate.sh >>"$GITHUB_ENV"
#
# Without ATLAS_CERT_P12_BASE64 it prints nothing (ad hoc builds).
set -euo pipefail
[[ -n "${ATLAS_CERT_P12_BASE64:-}" ]] || { echo "import-certificate.sh: no certificate – ad hoc" >&2; exit 0; }
keychain="${RUNNER_TEMP:-$(mktemp -d)}/atlas-signing.keychain-db"
password="$(uuidgen)"
p12="$(mktemp)"
trap 'rm -f "$p12"' EXIT
printf '%s' "$ATLAS_CERT_P12_BASE64" | base64 --decode >"$p12"
security create-keychain -p "$password" "$keychain"
security set-keychain-settings -lut 3600 "$keychain"
security unlock-keychain -p "$password" "$keychain"
security import "$p12" -k "$keychain" -P "${ATLAS_CERT_PASSWORD:-}" -T /usr/bin/codesign -T /usr/bin/security
security set-key-partition-list -S apple-tool:,apple: -s -k "$password" "$keychain" >/dev/null
security list-keychains -d user -s "$keychain" $(security list-keychains -d user | tr -d '"')
identity="$(security find-identity -v -p codesigning "$keychain" | sed -n 's/.*"\(Developer ID Application: .*\)"/\1/p' | head -n 1)"
[[ -n "$identity" ]] || { echo "import-certificate.sh: no Developer ID Application identity in the certificate" >&2; exit 1; }
echo "ATLAS_KEYCHAIN=$keychain"
echo "ATLAS_SIGN_IDENTITY=$identity"
