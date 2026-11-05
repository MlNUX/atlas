#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds Atlas.app and Atlas-<version>.dmg (P4.3):
#   - core (AtlasKit), Release build of the app without Xcode signing
#   - ripgrep (Contents/Helpers/rg) and ONNX Runtime
#     (Contents/Frameworks/libonnxruntime.dylib) from their official
#     releases, pinned and checked; their licences in Resources/Licenses
#   - signature inside out with Hardened Runtime, then the DMG
#
#   packaging/macos/bundle.sh [out-dir]          (default: target/macos-dist)
#
# Environment:
#   ATLAS_SIGN_IDENTITY  "Developer ID Application: … (TEAMID)"; unset or "-"
#                        signs ad hoc (test builds until the account exists, E5)
#   ATLAS_KEYCHAIN       keychain holding the identity (import-certificate.sh)
#   ATLAS_SKIP_CORE=1    use the AtlasKit already built (it must not have
#                        ATLAS_RG_BIN/ATLAS_ORT_DYLIB baked in)
# Notarisation of the DMG: packaging/macos/notarize.sh.
set -euo pipefail
root="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
here="$root/packaging/macos"
out="$(mkdir -p "${1:-$root/target/macos-dist}" && cd "${1:-$root/target/macos-dist}" && pwd)"
identity="${ATLAS_SIGN_IDENTITY:--}"
cache="${ATLAS_PACKAGING_CACHE:-$HOME/Library/Caches/atlas-packaging}"
derived="${RUNNER_TEMP:-$root/target}/atlas-release"

# Pinned helpers: version, URL, SHA-256 of the archive.
RG_VERSION=14.1.1
RG_URL="https://github.com/BurntSushi/ripgrep/releases/download/$RG_VERSION/ripgrep-$RG_VERSION-aarch64-apple-darwin.tar.gz"
RG_SHA256=24ad76777745fbff131c8fbc466742b011f925bfa4fffa2ded6def23b5b937be
# ort 2.0.0-rc.15 with `api-22` (crates/atlas-core/Cargo.toml) needs ONNX Runtime 1.22.
ORT_VERSION=1.22.0
ORT_URL="https://github.com/microsoft/onnxruntime/releases/download/v$ORT_VERSION/onnxruntime-osx-arm64-$ORT_VERSION.tgz"
ORT_SHA256=cab6dcbd77e7ec775390e7b73a8939d45fec3379b017c7cb74f5b204c1a1cc07

say() { printf '\n==> %s\n' "$*"; }

fetch() { # url sha256 -> path of the checked archive in the cache
  local url="$1" sum="$2" file="$cache/$(basename "$1")"
  mkdir -p "$cache"
  if [[ ! -f "$file" ]] || ! echo "$sum  $file" | shasum -a 256 -c --status; then
    curl -fsSL --retry 3 -o "$file.part" "$url"
    echo "$sum  $file.part" | shasum -a 256 -c --status || { echo "checksum of $url does not match" >&2; exit 1; }
    mv "$file.part" "$file"
  fi
  echo "$file"
}

version="$(sed -n 's/^ *MARKETING_VERSION: *"\{0,1\}\([^"]*\)"\{0,1\}$/\1/p' "$root/apps/macos/project.yml" | head -n 1)"
[[ -n "$version" ]] || { echo "MARKETING_VERSION not found in apps/macos/project.yml" >&2; exit 1; }

# The core bakes ATLAS_RG_BIN/ATLAS_ORT_DYLIB in when set at build time
# (Nix); the app must use its own helpers, not those of the build machine.
unset ATLAS_RG_BIN ATLAS_ORT_DYLIB ORT_DYLIB_PATH

if [[ "${ATLAS_SKIP_CORE:-}" != 1 ]]; then
  say "Core (AtlasKit)"
  "$root/apps/macos/scripts/build-core.sh" release
fi

say "App (Release $version)"
cd "$root/apps/macos"
xcodegen generate --quiet
xcodebuild -project Atlas.xcodeproj -scheme Atlas -configuration Release \
  -destination 'platform=macOS,arch=arm64' -derivedDataPath "$derived" -skipPackagePluginValidation \
  CODE_SIGNING_ALLOWED=NO build >"$out/xcodebuild-release.log" 2>&1 \
  || { grep -E "error:" "$out/xcodebuild-release.log" | head -n 40; exit 1; }
app="$out/Atlas.app"
rm -rf "$app"
ditto "$derived/Build/Products/Release/Atlas.app" "$app"

say "Helpers: ripgrep $RG_VERSION, ONNX Runtime $ORT_VERSION"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
tar -xzf "$(fetch "$RG_URL" "$RG_SHA256")" -C "$work"
tar -xzf "$(fetch "$ORT_URL" "$ORT_SHA256")" -C "$work"
rg_dir="$work/ripgrep-$RG_VERSION-aarch64-apple-darwin"
ort_dir="$work/onnxruntime-osx-arm64-$ORT_VERSION"
mkdir -p "$app/Contents/Helpers" "$app/Contents/Frameworks" "$app/Contents/Resources/Licenses"
install -m 755 "$rg_dir/rg" "$app/Contents/Helpers/rg"
install -m 644 "$ort_dir/lib/libonnxruntime.$ORT_VERSION.dylib" "$app/Contents/Frameworks/libonnxruntime.dylib"
install_name_tool -id @rpath/libonnxruntime.dylib "$app/Contents/Frameworks/libonnxruntime.dylib"
cp "$rg_dir/UNLICENSE" "$app/Contents/Resources/Licenses/ripgrep-UNLICENSE"
cp "$rg_dir/LICENSE-MIT" "$app/Contents/Resources/Licenses/ripgrep-LICENSE-MIT"
cp "$ort_dir/LICENSE" "$app/Contents/Resources/Licenses/onnxruntime-LICENSE"
[[ -f "$ort_dir/ThirdPartyNotices.txt" ]] && cp "$ort_dir/ThirdPartyNotices.txt" "$app/Contents/Resources/Licenses/onnxruntime-ThirdPartyNotices.txt"
cp "$root/LICENSE" "$app/Contents/Resources/Licenses/Atlas-LICENSE" 2>/dev/null || true

say "Signature (${identity/#-/ad hoc})"
sign=(codesign --force --options runtime --sign "$identity")
entitlements="$here/Atlas.entitlements"
if [[ "$identity" == "-" ]]; then
  entitlements="$here/Atlas-adhoc.entitlements"
else
  sign+=(--timestamp)
fi
[[ -n "${ATLAS_KEYCHAIN:-}" ]] && sign+=(--keychain "$ATLAS_KEYCHAIN")
# Inside out: helpers first, the app last (no --deep).
"${sign[@]}" "$app/Contents/Helpers/rg"
"${sign[@]}" "$app/Contents/Frameworks/libonnxruntime.dylib"
"${sign[@]}" --entitlements "$entitlements" "$app"
codesign --verify --strict --verbose=2 "$app"
codesign --display --entitlements - --verbose=2 "$app" 2>&1 | grep -E "Identifier|Authority|Signature|flags|TeamIdentifier|key" || true

say "DMG"
dmg="$out/Atlas-$version.dmg"
stage="$work/dmg"
mkdir -p "$stage"
ditto "$app" "$stage/Atlas.app"
ln -s /Applications "$stage/Applications"
rm -f "$dmg"
hdiutil create -quiet -volname "Atlas $version" -srcfolder "$stage" -fs HFS+ -format UDZO -ov "$dmg"
if [[ "$identity" != "-" ]]; then
  codesign --force --timestamp --sign "$identity" ${ATLAS_KEYCHAIN:+--keychain "$ATLAS_KEYCHAIN"} "$dmg"
fi
shasum -a 256 "$dmg" | tee "$dmg.sha256"
du -h "$dmg"
echo "$dmg"
