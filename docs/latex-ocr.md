<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Formeln aus PDFs als LaTeX

Text, den man in einer PDF markiert, enthält bei Formeln meist nur
Zeichensalat: Die PDF kennt den LaTeX-Code nicht mehr. Atlas erkennt die
Formel deshalb am Bild – mit [pix2tex](https://github.com/lukas-blecher/LaTeX-OCR),
das auf deinem Rechner läuft. Nichts verlässt ihn.

## Benutzen

- **∑ in der PDF-Leiste**, dann einen Rahmen um die Formel ziehen. Atlas
  erkennt sie, kopiert den LaTeX-Code und zeigt ihn zum Prüfen und Ändern –
  mit Knöpfen für `$…$` und `$$…$$` (für Markdown-Notizen). Das Werkzeug
  bleibt aktiv für die nächste Formel; Esc beendet es.
- Oder Text markieren und im Menü der Auswahl **∑** wählen.

Die erste Erkennung nach dem Start von Atlas dauert ein paar Sekunden (das
Modell wird geladen), danach etwa eine Sekunde. Beim allerersten Mal lädt
pix2tex sein Modell herunter (etwa 100 MB).

pix2tex ist auf einzelne Formeln trainiert; bei Absätzen aus Text und
Formeln gemischt besser nur die Formel einrahmen.

## Einrichten

pix2tex ist ein Python-Programm mit PyTorch und gehört in eine eigene
Umgebung. Atlas sucht es dort:

```sh
python3 -m venv ~/.local/share/atlas/pix2tex
# PyTorch ohne Grafikkarten-Unterstützung: gut 1 GB statt mehrerer
~/.local/share/atlas/pix2tex/bin/pip install torch --index-url https://download.pytorch.org/whl/cpu
~/.local/share/atlas/pix2tex/bin/pip install pix2tex
```

Ein anderes Python mit pix2tex trägst du in `~/.config/atlas/config.toml`
ein:

```toml
[pdf]
latex_ocr_python = "/pfad/zu/python"
```

**NixOS:** pix2tex ist nicht in nixpkgs. Die pip-Pakete von PyTorch
brauchen die üblichen Systembibliotheken; mit
`programs.nix-ld.enable = true;` in der Systemkonfiguration funktioniert die
venv wie oben.

## Fehler

Steht im Fenster „pix2tex lässt sich nicht laden“ oder „lässt sich nicht
starten“, fehlt pix2tex in dem Python, das Atlas nimmt (Reihenfolge:
`latex_ocr_python`, dann `~/.local/share/atlas/pix2tex`, dann `python3`).
Prüfen lässt sich das mit:

```sh
~/.local/share/atlas/pix2tex/bin/python -c "from pix2tex.cli import LatexOCR"
```
