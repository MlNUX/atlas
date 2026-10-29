#!/bin/sh
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Startet einen Jupyter-Server für das Projekt $1 auf 127.0.0.1:$2 mit dem
# Programm $3; weitere Argumente gehen an Jupyter. Das Token liegt in .atlas/cache/jupyter/token (nur für den
# Benutzer lesbar); die Notebook-Ansicht liest es von dort.
set -eu
project=$1
port=${2:-8899}
jupyter=${3:-jupyter}
[ $# -ge 3 ] && shift 3 || shift $#
port=${port%.*}
dir="$project/.atlas/cache/jupyter"
umask 077
mkdir -p "$dir"
if [ ! -s "$dir/token" ]; then
    od -An -N24 -tx1 /dev/urandom | tr -d ' \n' > "$dir/token"
fi
token=$(cat "$dir/token")
if ! command -v "$jupyter" >/dev/null 2>&1; then
    echo "„$jupyter“ fehlt. Installieren mit:  pip install jupyter_server ipykernel" >&2
    echo "oder in den Einstellungen der Erweiterung das Programm angeben." >&2
    exit 1
fi
echo "Jupyter-Server für $project auf http://127.0.0.1:$port (Strg+C beendet ihn)"
exec "$jupyter" server --no-browser --ip=127.0.0.1 --port="$port" --ServerApp.port_retries=0 \
    --ServerApp.root_dir="$project" --ServerApp.allow_origin='*' --ServerApp.disable_check_xsrf=True \
    --IdentityProvider.token="$token" "$@"
