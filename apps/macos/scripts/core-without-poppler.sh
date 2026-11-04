#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Task P0.6 (macOS port): lists every place in atlas-core that breaks when
# poppler, cairo and glib are not available (as on macOS).
#
# Works on a throw-away checkout: it removes the four dependencies from
# crates/atlas-core/Cargo.toml in place, runs cargo check and writes a report
# grouped by file to $ATLAS_CI_OUT/poppler-report.md (also summary.md).
# Do not run it in a working copy you care about (use --force to confirm).
set -euo pipefail

root="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
out="${ATLAS_CI_OUT:-$root/target/ci-out}"
mkdir -p "$out"
cd "$root"

if [[ -z "${CI:-}" && "${1:-}" != --force ]]; then
  echo "modifies crates/atlas-core/Cargo.toml in place; run in CI or pass --force" >&2
  exit 2
fi

manifest=crates/atlas-core/Cargo.toml
removed=(poppler-rs poppler-sys-rs glib cairo-rs)
for dep in "${removed[@]}"; do
  sed -i.bak -E "/^${dep} *=/d" "$manifest"
done
rm -f "$manifest.bak"
git diff --stat -- "$manifest"

log="$out/cargo-check-no-poppler.log"
rc=0
CARGO_TERM_COLOR=never cargo check -p atlas-core --all-targets --message-format short \
  >"$log" 2>&1 || rc=$?
tail -n 20 "$log"

# "path:line:col: error[E0433]: message" -> grouped by path
errors="$(grep -E '^[^ ]+\.rs:[0-9]+:[0-9]+: error' "$log" | sort -u -t: -k1,1 -k2,2n -k3,3n || true)"
count="$(grep -c . <<<"$errors" || true)"

{
  echo "# atlas-core ohne poppler/cairo/glib ($(uname -s) $(uname -m))"
  echo
  echo "Entfernt aus \`$manifest\`: ${removed[*]}."
  echo "Befehl: \`cargo check -p atlas-core --all-targets\` · Exit $rc · $count Fehlerstellen"
  echo
  echo "rustc bricht nach der Namensauflösung ab: Typfehler dahinter (und Stellen in"
  echo "\`examples/\`) zeigen sich erst, wenn diese Stellen hinter \`cfg(feature)\` stehen (P1.1)."
  echo
  if [[ -n "$errors" ]]; then
    echo "| Datei | Stellen |"
    echo "|---|---|"
    cut -d: -f1 <<<"$errors" | sort | uniq -c | sort -rn |
      awk '{printf "| `%s` | %s |\n", $2, $1}'
    echo
    echo "## Alle Fehler"
    echo
    echo '```'
    cat <<<"$errors"
    echo '```'
  fi
  other="$(grep -E '^error' "$log" | grep -vE 'could not compile|aborting due to' || true)"
  if [[ -n "$other" ]]; then
    echo
    echo "## Sonstige Fehler (Build-Skripte, Abhängigkeiten)"
    echo
    echo '```'
    echo "$other" | head -n 40
    echo '```'
  fi

  # Other crates without Qt: build and test (they do not depend on poppler).
  echo
  echo "## atlas-terminal, atlas-spell"
  echo
  trc=0
  CARGO_TERM_COLOR=never cargo test -p atlas-terminal -p atlas-spell >"$out/cargo-test-terminal-spell.log" 2>&1 || trc=$?
  echo "\`cargo test -p atlas-terminal -p atlas-spell\` · Exit $trc"
  echo
  echo '```'
  grep -E '^test .* FAILED$|^error|panicked at|^test result' "$out/cargo-test-terminal-spell.log" | head -n 30
  echo '```'

  # Code that compiles everywhere but only works on Linux at run time.
  echo
  echo "## Linux-Annahmen zur Laufzeit (Textsuche)"
  echo
  echo '```'
  grep -rnE '"/proc|prctl|systemd-creds|xdg-open|"/usr/(share|lib)|gio |dbus|\.local/share|XDG_' \
    crates/atlas-core/src crates/atlas-terminal/src crates/atlas-spell/src crates/atlas-ilias/src 2>/dev/null |
    grep -v '^.*//' | cut -c1-160 | head -n 60 || true
  echo '```'
} >"$out/poppler-report.md"
cp "$out/poppler-report.md" "$out/summary.md"
cat "$out/poppler-report.md"
