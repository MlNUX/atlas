#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
# Same output in both terminal variants of the spike (P0.5).
e=$'\e'
printf '%s[1mAtlas Terminal-Spike%s[0m  %s\n\n' "$e" "$e" "$(uname -sm)"
printf 'Stile:  %s[1mfett%s[0m %s[3mkursiv%s[0m %s[4munterstrichen%s[0m %s[9mdurchgestrichen%s[0m %s[7minvers%s[0m %s[2mgedimmt%s[0m\n' \
  "$e" "$e" "$e" "$e" "$e" "$e" "$e" "$e" "$e" "$e" "$e" "$e"
printf '16 Farben: '
for c in 30 31 32 33 34 35 36 37 90 91 92 93 94 95 96 97; do printf '%s[%sm■%s[0m' "$e" "$c" "$e"; done
printf '\n256 Farben: '
for c in $(seq 16 6 231); do printf '%s[48;5;%sm %s[0m' "$e" "$c" "$e"; done
printf '\nTruecolor:  '
for i in $(seq 0 4 255); do printf '%s[48;2;%s;%s;%sm %s[0m' "$e" "$i" "$((255 - i))" 128 "$e"; done
printf '\n\nUnicode: äöüß ÄÖÜ €  日本語 한국어  🙂👍  ∑∫√  e\xcc\x81 (kombiniert)\n'
printf 'Rahmen:  ┌──────┬──────┐\n         │ links│rechts│\n         └──────┴──────┘\n'
printf 'Fortschritt: '
for i in 1 2 3 4 5 6 7 8 9 10; do printf '\rFortschritt: [%-10s] %3d%%' "$(printf '#%.0s' $(seq 1 $i))" $((i * 10)); done
printf '\n\n'
ls -la /usr | head -n 8
printf '\n%s[32m$%s[0m ' "$e" "$e"
