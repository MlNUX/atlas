#!/bin/sh
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
# Gibt die Begrüßung aus den Einstellungen aus.
text=${ATLAS_SETTING_GRUSS:-Hallo}
if [ "$ATLAS_SETTING_GROSS" = true ]; then
    text=$(printf '%s' "$text" | tr '[:lower:]' '[:upper:]')
fi
n=${ATLAS_SETTING_WIEDERHOLUNGEN:-1}
n=${n%%.*}
case $n in '' | *[!0-9]*) n=1 ;; esac
[ "$n" -gt 20 ] && n=20
i=0
while [ "$i" -lt "$n" ]; do
    echo "$text aus dem Test-Plugin!"
    i=$((i + 1))
done
echo "Projekt: ${1:-–}"
