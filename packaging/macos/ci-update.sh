#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# macOS CI of the updates (P4.4): build version 1 and 2 with a throwaway
# EdDSA key, serve an appcast with version 2 on localhost, install version 1,
# let it look for updates without asking, quit it – Sparkle replaces it –
# and check that version 2 is installed.
set -euo pipefail
root="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
here="$root/packaging/macos"
out="${ATLAS_CI_OUT:-$root/target/ci-out}"
mkdir -p "$out"
say() { printf '\n==> %s\n' "$*"; }
port=8765

say "Throwaway EdDSA key (sparkle-keygen.sh, as for the real one)"
keyfile="$(mktemp -u)"
public="$("$here/sparkle-keygen.sh" "$keyfile")"
private="$(cat "$keyfile")"
rm -f "$keyfile"
echo "public key: $public"

export ATLAS_SPARKLE_PUBLIC_KEY="$public"
export ATLAS_FEED_URL="http://127.0.0.1:$port/appcast.xml"

say "Version 1"
ATLAS_BUILD_NUMBER=1 "$here/bundle.sh" "$out/v1" >"$out/bundle-v1.log" 2>&1 || { tail -n 40 "$out/bundle-v1.log"; exit 1; }
say "Version 2"
ATLAS_SKIP_CORE=1 ATLAS_BUILD_NUMBER=2 "$here/bundle.sh" "$out/v2" >"$out/bundle-v2.log" 2>&1 || { tail -n 40 "$out/bundle-v2.log"; exit 1; }

say "Appcast"
feed="$out/feed"
mkdir -p "$feed"
cp "$(ls "$out"/v2/Atlas-*.dmg)" "$feed/Atlas-2.dmg"
ATLAS_SPARKLE_PRIVATE_KEY="$private" "$here/appcast.sh" "$feed" "http://127.0.0.1:$port/"
python3 -m http.server "$port" --bind 127.0.0.1 --directory "$feed" >"$out/http.log" 2>&1 &
server=$!
trap 'kill $server 2>/dev/null || true' EXIT
sleep 1

say "Install version 1"
mnt="$(mktemp -d)"
hdiutil attach -nobrowse -readonly -mountpoint "$mnt" "$(ls "$out"/v1/Atlas-*.dmg)" >/dev/null
apps="${RUNNER_TEMP:-/tmp}/Programme"
rm -rf "$apps" && mkdir -p "$apps"
ditto "$mnt/Atlas.app" "$apps/Atlas.app"
hdiutil detach "$mnt" >/dev/null
app="$apps/Atlas.app"
build() { /usr/libexec/PlistBuddy -c "Print :CFBundleVersion" "$app/Contents/Info.plist"; }
echo "installed: $(build)"
[[ "$(build)" == 1 ]]
# Updates on, downloaded and installed without asking (as after „Automatisch installieren“).
defaults write de.minux.atlas SUEnableAutomaticChecks -bool true
defaults write de.minux.atlas SUAutomaticallyUpdate -bool true
defaults write de.minux.atlas SUHasLaunchedBefore -bool true

say "Look for updates"
open -n "$app" --args -AtlasCheckUpdates YES
for _ in $(seq 1 60); do
  grep -q "GET /Atlas-2.dmg" "$out/http.log" && break
  sleep 2
done
cat "$out/http.log"
grep -q "GET /Atlas-2.dmg" "$out/http.log" || { echo "::error::version 1 did not download the update"; exit 1; }
sleep 15
screencapture -x "$out/atlas-update.png" || true
osascript -e 'tell application "Atlas" to quit' 2>/dev/null || pkill -x Atlas || true

say "Installed on quit"
for _ in $(seq 1 60); do
  [[ "$(build 2>/dev/null || echo 0)" == 2 ]] && break
  sleep 2
done
log show --last 5m --style compact --predicate 'subsystem BEGINSWITH "org.sparkle-project"' 2>/dev/null | tail -n 40 || true
echo "installed: $(build)"
[[ "$(build)" == 2 ]] || { echo "::error::update not installed"; exit 1; }
codesign --verify --strict "$app"

{
  echo "# Sparkle-Update (P4.4)"
  echo
  echo "Version 1 hat das Update aus dem Test-Appcast geladen und beim Beenden installiert: CFBundleVersion $(build)."
} >"$out/summary.md"
