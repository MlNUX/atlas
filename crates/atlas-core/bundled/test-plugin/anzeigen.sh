#!/bin/sh
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
# Zeigt eine *.testplugin-Datei als Markdown, mit den Einstellungen.
echo "# $(basename "$1")"
echo
echo "| Einstellung | Wert |"
echo "|---|---|"
echo "| Begrüßung | ${ATLAS_SETTING_GRUSS} |"
echo "| In Großbuchstaben | ${ATLAS_SETTING_GROSS} |"
echo "| Wiederholungen | ${ATLAS_SETTING_WIEDERHOLUNGEN} |"
echo "| Zählen nach | ${ATLAS_SETTING_ZAEHLEN} |"
if [ -n "$ATLAS_SETTING_TOKEN" ]; then token=gesetzt; else token="nicht gesetzt"; fi
echo "| Token | $token |"
echo
echo '```'
cat "$1"
echo '```'
