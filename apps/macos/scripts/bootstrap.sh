#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Prepares a Mac (Apple Silicon, macOS 26+, Xcode 26+) for Atlas development.
# The CI setup workflow (.github/workflows/macos-setup.yml) runs this same script,
# so a real Mac and the GitHub runners stay identical.
#
# Usage: apps/macos/scripts/bootstrap.sh
#
# Expects Xcode 26 to be selected (xcode-select) and rustup to be installed.
# Exports ATLAS_RG_BIN and ORT_DYLIB_PATH into $GITHUB_ENV when run in Actions,
# otherwise prints them for your shell profile.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
min_xcode=26
min_macos=26

say() { printf '==> %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

[[ "$(uname -s)" == Darwin ]] || die "this script runs on macOS only"
[[ "$(uname -m)" == arm64 ]] || die "Atlas for macOS supports Apple Silicon (arm64) only"

macos="$(sw_vers -productVersion)"
say "macOS $macos ($(uname -m))"
if (( ${macos%%.*} < min_macos )); then
  warn "macOS $macos is older than $min_macos: the app builds, but cannot run here"
fi

command -v xcodebuild >/dev/null || die "Xcode not found; install Xcode $min_xcode+"
xcode="$(xcodebuild -version | awk 'NR==1 {print $2}')"
say "Xcode $xcode at $(xcode-select -p)"
(( ${xcode%%.*} >= min_xcode )) || die "Xcode $xcode is too old, need $min_xcode+ (select it with xcode-select -s)"

command -v brew >/dev/null || die "Homebrew not found, see https://brew.sh"
say "Homebrew packages (Brewfile)"
HOMEBREW_NO_AUTO_UPDATE="${HOMEBREW_NO_AUTO_UPDATE:-1}" brew bundle install --file="$here/Brewfile"

command -v rustup >/dev/null || die "rustup not found, see https://rustup.rs"
say "Rust target aarch64-apple-darwin"
rustup target add aarch64-apple-darwin
say "$(rustc --version)"

rg_bin="$(brew --prefix ripgrep)/bin/rg"
ort_dylib="$(brew --prefix onnxruntime)/lib/libonnxruntime.dylib"
[[ -x "$rg_bin" ]] || die "rg missing at $rg_bin"
[[ -f "$ort_dylib" ]] || warn "libonnxruntime.dylib missing at $ort_dylib"

if [[ -n "${GITHUB_ENV:-}" ]]; then
  {
    echo "ATLAS_RG_BIN=$rg_bin"
    echo "ORT_DYLIB_PATH=$ort_dylib"
  } >>"$GITHUB_ENV"
else
  say "Add to your shell profile:"
  echo "  export ATLAS_RG_BIN=\"$rg_bin\""
  echo "  export ORT_DYLIB_PATH=\"$ort_dylib\""
fi
say "done"
