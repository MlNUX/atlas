#!/bin/sh
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Richtet Jupyter im Projekt $1 ein: eine virtuelle Umgebung .venv (mit dem
# Python $2) mit jupyter_server und ipykernel, danach startet server.sh den
# Server aus ihr ($3 = Port, weitere Argumente gehen an Jupyter).
set -eu
project=$1
python=${2:-python3}
port=${3:-8899}
[ $# -ge 3 ] && shift 3 || shift $#

fail() {
    printf '\n%s\n\nEnter schließt diesen Tab.' "$*" >&2
    read -r _ || true
    exit 1
}

venv="$project/.venv"
command -v "$python" >/dev/null 2>&1 || fail "„$python“ fehlt – Python 3 installieren oder in den Einstellungen der Erweiterung angeben."
if [ ! -x "$venv/bin/python" ]; then
    echo "Lege $venv an …"
    "$python" -m venv "$venv" || fail "Die Umgebung lässt sich nicht anlegen (fehlt das Modul venv? Unter Debian/Ubuntu: python3-venv)."
fi
echo "Installiere jupyter_server und ipykernel in $venv …"
"$venv/bin/python" -m pip install --disable-pip-version-check jupyter_server ipykernel \
    || fail "Die Installation ist fehlgeschlagen (Meldungen oben)."
echo
exec "$(dirname "$0")/server.sh" "$project" "$port" "$venv/bin/jupyter" "$@"
