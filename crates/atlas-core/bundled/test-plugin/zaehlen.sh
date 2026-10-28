#!/bin/sh
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
# Zählt Wörter, Zeilen oder Zeichen der Datei (Einstellung „Zählen nach“).
case $ATLAS_SETTING_ZAEHLEN in
    Zeilen) flag=-l ;;
    Zeichen) flag=-m ;;
    *) flag=-w ;;
esac
echo "$(wc $flag < "$1" | tr -d ' ') ${ATLAS_SETTING_ZAEHLEN:-Wörter} in $(basename "$1")"
