#!/bin/sh
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Startet einen Jupyter-Server für das Projekt $1 auf 127.0.0.1:$2 mit dem
# Programm $3; weitere Argumente gehen an Jupyter. Steht dort nur „jupyter“
# und hat das Projekt eine .venv mit Jupyter, läuft der Server aus ihr (ihr
# Python ist dann der Kernel „Python 3“). Das Token liegt in
# .atlas/cache/jupyter/token (nur für den Benutzer lesbar); die
# Notebook-Ansicht liest es von dort.
set -eu
project=$1
port=${2:-8899}
jupyter=${3:-jupyter}
[ $# -ge 3 ] && shift 3 || shift $#
port=${port%.*}

# Der Tab schließt sich mit dem Programm: Fehler bleiben bis Enter stehen.
fail() {
    printf '\n%s\n\nEnter schließt diesen Tab.' "$*" >&2
    read -r _ || true
    exit 1
}

if [ "$jupyter" = jupyter ] && [ -x "$project/.venv/bin/jupyter" ]; then
    jupyter="$project/.venv/bin/jupyter"
fi
command -v "$jupyter" >/dev/null 2>&1 || fail "„$jupyter“ fehlt. In der Notebook-Ansicht „Jupyter im Projekt einrichten“ wählen, \
mit „pip install jupyter_server ipykernel“ installieren oder in den Einstellungen der Erweiterung das Programm angeben."

dir="$project/.atlas/cache/jupyter"
umask 077
mkdir -p "$dir"
if [ ! -s "$dir/token" ]; then
    od -An -N24 -tx1 /dev/urandom | tr -d ' \n' > "$dir/token"
fi
token=$(cat "$dir/token")
umask 022
echo "Jupyter-Server für $project auf http://127.0.0.1:$port ($jupyter; Strg+C beendet ihn)"
"$jupyter" server --no-browser --ip=127.0.0.1 --port="$port" --ServerApp.port_retries=0 \
    --ServerApp.root_dir="$project" --ServerApp.allow_origin='*' --ServerApp.disable_check_xsrf=True \
    --IdentityProvider.token="$token" "$@" || fail "Der Jupyter-Server wurde mit Fehler beendet (Meldungen oben)."
