#!/bin/sh
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds the index of the docs search: index.sh <cache> <project> <python> <packages>
#
# Every package is read with the first Python that has it, in this order:
# the setting, environments in the project (.venv, venv, env, also in
# subfolders), the Pythons of registered Jupyter kernels, environments
# built by the Jupyter view (~/.local/share/atlas/envs), Conda and pyenv
# environments, python3 in PATH and at the usual places. So the docs match
# the versions the notebooks work with, even when Atlas runs without a
# Python in its PATH (started from the menu, NixOS).
set -u
cache=$1
project=${2:-}
python=${3:-}
packages=${4:-}
here=$(cd "$(dirname "$0")" && pwd)
# Package names only (letters, digits, _ and .), separated by spaces.
packages=$(printf '%s' "$packages" | tr ',;' '  ' | tr -cd 'A-Za-z0-9_. ')
[ -n "$(printf '%s' "$packages" | tr -d ' ')" ] || packages="numpy pandas matplotlib"

list=$(mktemp) || exit 2
trap 'rm -f "$list"' EXIT
add() {
    for p in "$@"; do
        case $p in
            "") continue ;;
            /*) ;;
            */*) p="$PWD/$p" ;;
            *) p=$(command -v "$p" 2>/dev/null) || continue ;;
        esac
        [ -x "$p" ] && [ ! -d "$p" ] || continue
        # The folder with links resolved (/bin → /usr/bin), not the file:
        # the python of a venv is a link to another one but not the same.
        dir=$(cd "$(dirname "$p")" 2>/dev/null && pwd -P) && p="$dir/$(basename "$p")"
        printf '%s\n' "$p" >> "$list"
    done
}
envs_in() { # <folder>: <folder>/*/bin/python
    for e in "$1"/*/bin/python; do add "$e"; done
}

if [ -n "$python" ]; then
    add "$python"
    grep -q . "$list" || echo "Python aus den Einstellungen nicht gefunden: $python – suche selbst." >&2
fi
if [ -n "$project" ] && [ -d "$project" ]; then
    for env in .venv venv env; do add "$project/$env/bin/python"; done
    # Environments in subfolders (a notebook folder with its own .venv).
    find "$project" -maxdepth 6 \( -name node_modules -o -name .git -o -name target -o -name __pycache__ \) -prune -o \
        -path '*/bin/python' -print 2>/dev/null | grep -E '/(\.venv|venv|env|\.env)/bin/python$' | head -n 20 |
        while IFS= read -r p; do add "$p"; done
fi
# Jupyter kernels (ipykernel install --user, the Jupyter view of Atlas).
for d in "${JUPYTER_DATA_DIR:-}" "${XDG_DATA_HOME:-$HOME/.local/share}/jupyter" "$HOME/.local/share/jupyter" \
         "$HOME/Library/Jupyter" /usr/local/share/jupyter /usr/share/jupyter; do
    [ -n "$d" ] || continue
    for spec in "$d"/kernels/*/kernel.json; do
        [ -f "$spec" ] || continue
        # The first entry of "argv" is the Python of the kernel.
        add "$(tr -d '\n' < "$spec" | sed -n 's/.*"argv"[[:space:]]*:[[:space:]]*\[[[:space:]]*"\([^"]*\)".*/\1/p')"
    done
done
envs_in "$HOME/.local/share/atlas/envs"
add "${CONDA_PREFIX:-}/bin/python" "${VIRTUAL_ENV:-}/bin/python"
if [ -f "$HOME/.conda/environments.txt" ]; then
    while IFS= read -r e; do add "$e/bin/python"; done < "$HOME/.conda/environments.txt"
fi
for base in "$HOME/miniforge3" "$HOME/mambaforge" "$HOME/miniconda3" "$HOME/anaconda3" "$HOME/.miniconda3" \
            "$HOME/micromamba" "$HOME/.local/share/atlas/micromamba" /opt/miniforge3 /opt/miniconda3 /opt/anaconda3 /opt/conda; do
    [ -d "$base" ] || continue
    add "$base/bin/python"
    envs_in "$base/envs"
done
envs_in "$HOME/.pyenv/versions"
add python3 python "$HOME/.nix-profile/bin/python3" /run/current-system/sw/bin/python3 \
    /usr/local/bin/python3 /usr/bin/python3 /opt/homebrew/bin/python3

# Each Python once (links to the same one count once), in this order.
candidates=$(awk '!seen[$0]++' "$list")
if [ -z "$candidates" ]; then
    echo "Kein Python gefunden. In den Einstellungen der Doku-Suche bei „Python“ den Pfad zum Python der Umgebung mit den Paketen eintragen (z. B. …/.venv/bin/python)." >&2
    exit 2
fi

# Which package each Python has (one start per Python, from / so that
# nothing in the project shadows a package; not -I: packages from
# `pip install --user` and PYTHONPATH count).
plan=$(mktemp) || exit 2
trap 'rm -f "$list" "$plan"' EXIT
left=$packages
echo "Gesucht in:"
printf '%s\n' "$candidates" | while IFS= read -r candidate; do
    [ -n "$(printf '%s' "$left" | tr -d ' ')" ] || break
    # shellcheck disable=SC2086 # one argument per package
    have=$(cd / && "$candidate" -c 'import importlib.util, sys
print(" ".join(p for p in sys.argv[1:] if importlib.util.find_spec(p) is not None))' $left 2>/dev/null) || {
        echo "  $candidate (startet nicht)"
        continue
    }
    echo "  $candidate: ${have:-–}"
    [ -n "$have" ] || continue
    printf '%s\t%s\n' "$candidate" "$have" >> "$plan"
    for p in $have; do left=$(printf ' %s ' "$left" | sed "s/ $p / /g"); done
done

missing=$packages
while IFS="$(printf '\t')" read -r _ have; do
    for p in $have; do missing=$(printf ' %s ' "$missing" | sed "s/ $p / /g"); done
done < "$plan"
missing=$(printf '%s' "$missing" | tr -s ' ' | sed 's/^ //; s/ $//')

if ! grep -q . "$plan"; then
    echo "Keins der Pakete ($packages) ist in einem gefundenen Python installiert." >&2
    echo "In den Einstellungen der Doku-Suche bei „Python“ den Pfad zum Python der Umgebung mit den Paketen eintragen (z. B. …/.venv/bin/python)." >&2
    first=$(printf '%s\n' "$candidates" | head -n 1)
    # Records the missing packages in the index (shown in the view).
    # shellcheck disable=SC2086
    (cd / && "$first" "$here/index.py" "$cache" $packages)
    exit 2
fi

cd / || exit 2
status=0
while IFS="$(printf '\t')" read -r candidate have; do
    # shellcheck disable=SC2086 # one argument per package
    "$candidate" "$here/index.py" "$cache" $have || status=$?
done < "$plan"
if [ -n "$missing" ]; then
    echo "Nirgends installiert: $missing"
    # shellcheck disable=SC2086
    "$(head -n 1 "$plan" | cut -f1)" "$here/index.py" "$cache" $missing >/dev/null 2>&1
fi
exit "$status"
