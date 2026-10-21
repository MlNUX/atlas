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
# A hidden note: not part of the graph.
printf '# Versteckt\n' > "$project/.versteckt.md"
# A second project for sessions in another folder.
mkdir -p "$work/zweites/texte"
printf '# Zweites Projekt\n' > "$work/zweites/texte/start.md"

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
printf -- '---\ndue: 2026-10-23\nstatus: planned\n---\n# Aufgaben\n\n- [ ] Literatur sichten\n- [x] Gliederung\n- [ ] Kapitel 3 schreiben\n' > "$project/notes/aufgaben.md"
printf -- '---\ntags: [wiki]\n---\n# Wiki\n\nSiehe [[/notes/plan|Plan]] und [[/notes/idee.md#Einleitung|die Idee]].\n' > "$project/notes/wiki.md"
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
# A two page PDF with known text positions (Helvetica 20 pt at y = 700
# and 670 from the bottom), written with a correct cross-reference table.
pdf() {
  local out="$1" objs=() offsets=() i body
  local page1="BT /F1 20 Tf 72 700 Td (Zitat aus der Quelle) Tj 0 -30 Td (Zweite Zeile hier) Tj ET"
  local page2="BT /F1 20 Tf 72 700 Td (Seite zwei Text) Tj ET"
  objs+=("<< /Type /Catalog /Pages 2 0 R >>")
  objs+=("<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>")
  objs+=("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 5 0 R /Resources << /Font << /F1 7 0 R >> >> >>")
  objs+=("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 6 0 R /Resources << /Font << /F1 7 0 R >> >> >>")
  objs+=("<< /Length ${#page1} >>
stream
$page1
endstream")
  objs+=("<< /Length ${#page2} >>
stream
$page2
endstream")
  objs+=("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
  body="%PDF-1.4"$'\n'
  for i in "${!objs[@]}"; do
    offsets+=("$(printf '%s' "$body" | wc -c)")
    body+="$((i + 1)) 0 obj"$'\n'"${objs[$i]}"$'\n'"endobj"$'\n'
  done
  local xref
  xref=$(printf '%s' "$body" | wc -c)
  body+="xref"$'\n'"0 $((${#objs[@]} + 1))"$'\n'"0000000000 65535 f "$'\n'
  for i in "${offsets[@]}"; do body+="$(printf '%010d' "$i") 00000 n "$'\n'; done
  body+="trailer"$'\n'"<< /Size $((${#objs[@]} + 1)) /Root 1 0 R >>"$'\n'"startxref"$'\n'"$xref"$'\n'"%%EOF"$'\n'
  printf '%s' "$body" > "$out"
}
pdf "$project/docs/quelle.pdf"
pdf "$project/docs/zweite.pdf"

mkdir -p "$project/.atlas"
cat > "$project/.atlas/actions.yaml" <<'YAML'
version: 1
actions:
  - name: Zeilen zählen
    applies_to: "*.md"
    command: [wc, -l, "{file}"]
  - name: Liste schreiben
    when: project
    shell: true
    command: "ls notes > aktion-ergebnis.txt"
  - name: Ordner zeigen
    when: dir
    command: [ls, "{dir}"]
YAML

# Extensions: one installed by the user, one coming with the project.
mkdir -p "$work/config/extensions/demo" "$project/.atlas/extensions/proj"
cat > "$work/config/extensions/demo/extension.yaml" <<'YAML'
atlas_extension: 1
name: demo
title: Demo
version: "1.0"
actions:
  - {name: Hallo, when: project, command: [echo, hallo]}
processors:
  - name: Großbuchstaben
    applies_to: "*.dat"
    output: markdown
    shell: true
    command: "echo '# Daten'; tr a-z A-Z < {file}"
exporters:
  - {name: Kopie als Text, applies_to: "*.md", target: txt, command: [cp, "{file}", "{output}"]}
YAML
cat > "$project/.atlas/extensions/proj/extension.yaml" <<'YAML'
atlas_extension: 1
name: proj
actions:
  - {name: Projekt-Hallo, when: project, command: [echo, projekt]}
YAML
printf 'hallo\000welt\n' > "$project/daten.dat"

cat > "$project/docs/seite.html" <<'HTML'
<!doctype html>
<html><head><meta charset="utf-8"><title>Testseite</title><link rel="stylesheet" href="seite.css"></head>
<body><h1 id="kopf">Hallo HTML</h1><p id="js">ohne Skript</p>
<script>document.getElementById("js").textContent = "mit Skript"</script></body></html>
HTML
printf 'h1 { color: rgb(1, 2, 3); }\n' > "$project/docs/seite.css"
printf '<!doctype html><title>Web</title><script src="https://cdn.example.invalid/sql-wasm.js"></script><p>x</p>\n' > "$project/docs/web.html"
cat > "$project/docs/dyn.html" <<'HTML'
<!doctype html><title>Dyn</title><script>
const s = document.createElement("script"); s.src = "https://cdn.example.invalid/lib.js"; document.head.appendChild(s)
fetch("https://cdn.example.invalid/data.json").catch(e => console.error("fetch: " + e))
</script><script src="fehlt.js"></script><script>initSqlJs()</script>
HTML
printf 'echo "lief in $(basename "$PWD")" > gelaufen.txt\n' > "$project/src/hallo.sh"
echo "// TODO: EINZIGARTIG" > "$project/src/lib.rs"
echo "EINZIGARTIG" > "$project/target/out.txt"
echo "EINZIGARTIG" > "$project/node_modules/x/a.js"

export ATLAS_STATE_DIR="$work/state" ATLAS_CONFIG_DIR="$work/config"
# Own XDG data/config: file manager places (and the trash) never touch the
# user's.
export XDG_DATA_HOME="$work/xdg-data" XDG_CONFIG_HOME="$work/xdg-config"
mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME/gtk-3.0"
cat > "$XDG_DATA_HOME/user-places.xbel" <<XBEL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE xbel>
<xbel xmlns:bookmark="http://www.freedesktop.org/standards/desktop-bookmarks">
 <bookmark href="file://$project/docs"><title>Belege</title></bookmark>
 <bookmark href="file://$project/src"><title>Versteckt</title>
  <info><metadata owner="http://www.kde.org"><IsHidden>true</IsHidden></metadata></info></bookmark>
 <bookmark href="sftp://server/x"><title>Server</title></bookmark>
</xbel>
XBEL
echo "file://$project/notes Notizen" > "$XDG_CONFIG_HOME/gtk-3.0/bookmarks"
export QT_QPA_PLATFORM=offscreen QT_FORCE_STDERR_LOGGING=1
# Software rendering for the Markdown preview without a GPU.
export QTWEBENGINE_CHROMIUM_FLAGS="--disable-gpu"

status=0
# One step every 1.5 s; generous for slow machines.
timeout 300 "$bin" "$project" --self-test 2> "$work/log" || status=$?
grep -E "selftest:" "$work/log" | sed 's/^qml: //' || true
if [ "$status" -ne 0 ]; then
  echo "--- Fehler (Exit $status); letzte Ausgabe:" >&2
  grep -vE "selftest:" "$work/log" | tail -20 >&2
fi
exit "$status"
