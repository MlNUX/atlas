#!/bin/sh
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Führt das Skript aus, das die Notebook-Ansicht nach
# $1/.atlas/cache/jupyter/$2.sh geschrieben hat (Umgebungen suchen, bauen,
# Pakete installieren). Läuft als Hintergrundbefehl von Atlas; Ausgabe und
# Exit-Code liest die Ansicht über atlas.commandStatus.
project=$1
name=${2:-task}
script="$project/.atlas/cache/jupyter/$name.sh"
[ -f "$script" ] || { echo "Kein Skript $script" >&2; exit 2; }
exec sh "$script" </dev/null
