#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# A new EdDSA (Ed25519) key for the updates (P4.4), on Linux or macOS, with
# openssl – the same format as Sparkle's `generate_keys` (both base64: the
# 32-byte private seed and the 32-byte public key).
#
#   packaging/macos/sparkle-keygen.sh <private key file>
#
# Writes the private key into the file (mode 600) and prints the public key.
# Private key → secret ATLAS_SPARKLE_PRIVATE_KEY, public key → repository
# variable ATLAS_SPARKLE_PUBLIC_KEY; then delete the file (keep a copy in a
# password manager: without it no installed version finds an update).
set -euo pipefail
file="${1:?usage: sparkle-keygen.sh <private key file>}"
[[ ! -e "$file" ]] || { echo "sparkle-keygen.sh: $file exists – not overwritten" >&2; exit 1; }
der="$(mktemp)"
trap 'rm -f "$der"' EXIT
openssl genpkey -algorithm ed25519 -outform DER -out "$der"
(umask 077; tail -c 32 "$der" | base64 | tr -d '\n' >"$file")
openssl pkey -inform DER -in "$der" -pubout -outform DER | tail -c 32 | base64 | tr -d '\n'
echo
