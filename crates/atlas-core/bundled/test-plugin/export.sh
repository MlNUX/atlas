#!/bin/sh
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
# Schreibt die Datei in Großbuchstaben nach $2.
tr '[:lower:]' '[:upper:]' < "$1" > "$2"
