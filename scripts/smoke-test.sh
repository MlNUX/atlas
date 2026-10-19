#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Runs Atlas' built-in smoke test (--self-test) offscreen against a freshly
# generated project. Usage:
#
#   scripts/smoke-test.sh                 # target/debug/atlas (in `nix develop`)
#   ATLAS_BIN=result/bin/atlas scripts/smoke-test.sh
#
# Nothing outside a temporary folder is touched (own state/config folders).
set -euo pipefail

bin="${ATLAS_BIN:-target/debug/atlas}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
project="$work/projekt"
mkdir -p "$project"/{notes,src,docs,target,node_modules/x}

cat > "$project/notes/idee.md" <<'MD'
---
title: Idee
tags: [Architektur/Backend, rust]
---
# Einleitung

Siehe [main](/src/main.rs#L2) und [kaputt](/fehlt.pdf#page=3).

$E = mc^2$

```rust
fn main() {}
```
MD
printf '# Plan\n\n## Ziele\n\nZurück zur [Idee](idee.md).\n' > "$project/notes/plan.md"
cat > "$project/src/main.rs" <<'RS'
pub struct Parser {
    pos: usize,
}

impl Parser {
    pub fn new() -> Self {
        Parser { pos: 0 }
    }
}

fn main() {}
RS
echo "// TODO: EINZIGARTIG" > "$project/src/lib.rs"
echo "EINZIGARTIG" > "$project/target/out.txt"
echo "EINZIGARTIG" > "$project/node_modules/x/a.js"

export ATLAS_STATE_DIR="$work/state" ATLAS_CONFIG_DIR="$work/config"
export QT_QPA_PLATFORM=offscreen QT_FORCE_STDERR_LOGGING=1
# Software rendering for the Markdown preview without a GPU.
export QTWEBENGINE_CHROMIUM_FLAGS="--disable-gpu"

status=0
timeout 120 "$bin" "$project" --self-test 2> "$work/log" || status=$?
grep -E "selftest:" "$work/log" | sed 's/^qml: //' || true
if [ "$status" -ne 0 ]; then
  echo "--- Fehler (Exit $status); letzte Ausgabe:" >&2
  grep -vE "selftest:" "$work/log" | tail -20 >&2
fi
exit "$status"
