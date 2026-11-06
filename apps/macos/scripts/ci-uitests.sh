#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# UI tests of the Mac app (P4.1): the test project of the Linux self-test
# (scripts/smoke-test.sh, run with a stand-in binary that only keeps a copy
# of it), then the XCUITests of the scheme AtlasUITests. Output: the
# `selftest:` lines like on Linux, $ATLAS_CI_OUT/AtlasUITests.xcresult and
# screenshots ($ATLAS_CI_OUT/ui-*.png).
set -euo pipefail
root="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
out="${ATLAS_CI_OUT:-$root/target/ci-out}"
derived="${RUNNER_TEMP:-$root/target}/atlas-app"
tmp="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/atlas-uitest"
mkdir -p "$out"

say() { printf '\n==> %s\n' "$*"; }

say "Test project (scripts/smoke-test.sh)"
rm -rf "$tmp"
mkdir -p "$tmp/bin"
# macOS has no `timeout`; the stand-in for Atlas copies the generated folder.
printf '#!/bin/sh\nshift\nexec "$@"\n' >"$tmp/bin/timeout"
cat >"$tmp/bin/atlas-copy" <<SH
#!/bin/sh
cp -R "\$(dirname "\$1")" "$tmp/vorlage"
SH
chmod +x "$tmp/bin/timeout" "$tmp/bin/atlas-copy"
(cd "$root" && PATH="$tmp/bin:$PATH" ATLAS_BIN="$tmp/bin/atlas-copy" scripts/smoke-test.sh) >"$out/test-project.log" 2>&1 \
  || { tail -n 30 "$out/test-project.log"; exit 1; }
test -f "$tmp/vorlage/projekt/notes/idee.md" || { echo "test project missing"; cat "$out/test-project.log"; exit 1; }
find "$tmp/vorlage" -maxdepth 3 | sed "s|$tmp/vorlage|.|" | sort | head -n 60

say "Core (AtlasKit)"
"$root/apps/macos/scripts/build-core.sh" release >"$out/build-core.log" 2>&1 || { tail -n 60 "$out/build-core.log"; exit 1; }

say "Xcode project"
cd "$root/apps/macos"
xcodegen generate --quiet
for attempt in 1 2 3 4; do
  xcodebuild -resolvePackageDependencies -project Atlas.xcodeproj -scheme AtlasUITests >"$out/resolve.log" 2>&1 && break
  [[ $attempt == 4 ]] && { tail -n 20 "$out/resolve.log"; exit 1; }
  echo "package download failed, retry $attempt"; sleep $((attempt * 15))
done

say "xcodebuild test (AtlasUITests)"
status=0
TEST_RUNNER_ATLAS_UITEST_TEMPLATE="$tmp/vorlage" TEST_RUNNER_ATLAS_CI_OUT="$out" \
  xcodebuild -project Atlas.xcodeproj -scheme AtlasUITests -destination 'platform=macOS,arch=arm64' \
  -derivedDataPath "$derived" -resultBundlePath "$out/AtlasUITests.xcresult" -skipPackagePluginValidation \
  test >"$out/uitests.log" 2>&1 || status=$?
grep -aE "error:" "$out/uitests.log" | head -n 40 || true
# The checks like on Linux, then the test results.
grep -aE "selftest: " "$out/uitests.log" | sed -E 's/^.*selftest: /selftest: /' || true
grep -aE "Test Case .*(passed|failed)|Executed [0-9]+ test" "$out/uitests.log" | tail -n 40 || true
{
  echo "# UI-Tests (P4.1)"
  echo
  echo '```'
  grep -aE "selftest: " "$out/uitests.log" | sed -E 's/^.*selftest: /selftest: /' || true
  grep -aE "Executed [0-9]+ test" "$out/uitests.log" | tail -n 1 || true
  echo '```'
} >"$out/summary.md"
exit "$status"
