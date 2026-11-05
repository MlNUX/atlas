#!/bin/sh
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds the index of the docs search: index.sh <cache> <project> <python> <packages>
# Python: the setting if set, else the environment of the project (.venv,
# venv, env), the active Conda environment, python3. The first one that
# imports at least one of the packages is used, so the docs match the
# versions the project works with.
set -u
cache=$1
project=${2:-}
python=${3:-}
packages=${4:-}
here=$(cd "$(dirname "$0")" && pwd)
# Package names only (letters, digits, _ and .), separated by spaces.
packages=$(printf '%s' "$packages" | tr ',;' '  ' | tr -cd 'A-Za-z0-9_. ')
[ -n "$(printf '%s' "$packages" | tr -d ' ')" ] || packages="numpy pandas matplotlib"

candidates=""
if [ -n "$python" ]; then
    candidates="$python"
    command -v "$python" >/dev/null 2>&1 || echo "Python aus den Einstellungen nicht gefunden: $python – suche selbst." >&2
fi
if [ -n "$project" ]; then
    for env in .venv venv env; do
        [ -x "$project/$env/bin/python" ] && candidates="$candidates $project/$env/bin/python"
    done
fi
[ -n "${CONDA_PREFIX:-}" ] && [ -x "$CONDA_PREFIX/bin/python" ] && candidates="$candidates $CONDA_PREFIX/bin/python"
candidates="$candidates python3 python"

chosen=""
for candidate in $candidates; do
    case $candidate in
        /*) ;;
        */*) candidate="$PWD/$candidate" ;;
    esac
    command -v "$candidate" >/dev/null 2>&1 || continue
    [ -z "$chosen" ] && chosen=$candidate
    # One that has the packages wins over the first one found.
    for pkg in $packages; do
        # (From / so that nothing in the project shadows the package; not
        # -I: packages from `pip install --user` and PYTHONPATH count.)
        if (cd / && "$candidate" -c "import importlib.util, sys; sys.exit(importlib.util.find_spec('$pkg') is None)") 2>/dev/null; then
            chosen=$candidate
            break 2
        fi
    done
done
if [ -z "$chosen" ]; then
    echo "Kein Python gefunden – in den Einstellungen der Doku-Suche „Python“ setzen." >&2
    exit 2
fi
echo "Python: $chosen"
cd / || exit 2
# shellcheck disable=SC2086 # one argument per package
exec "$chosen" "$here/index.py" "$cache" $packages
