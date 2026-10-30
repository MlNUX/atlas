#!/bin/sh
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
# Baut das Programm und installiert die Erweiterung für alle Projekte.
# Aufruf im Ordner dieses Beispiels: sh install.sh
set -eu
cargo build --release
target="${XDG_CONFIG_HOME:-$HOME/.config}/atlas/extensions/tabelle"
mkdir -p "$target/bin"
cp extension.yaml "$target/"
cp target/release/atlas-tabelle "$target/bin/"
echo "Installiert nach $target"
