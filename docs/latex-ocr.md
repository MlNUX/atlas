<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Formeln aus PDFs als LaTeX

Text, den man in einer PDF markiert, enthält bei Formeln meist nur
Zeichensalat: Die PDF kennt den LaTeX-Code nicht mehr. Atlas erkennt die
Formel deshalb am Bild – mit dem Modell von
[pix2tex/LaTeX-OCR](https://github.com/lukas-blecher/LaTeX-OCR), eingebaut
und auf deinem Rechner. Nichts verlässt ihn, und es muss nichts extra
installiert werden.

## Benutzen

- **∑ in der PDF-Leiste**, dann einen Rahmen um die Formel ziehen. Atlas
  erkennt sie, kopiert den LaTeX-Code und zeigt ihn zum Prüfen und Ändern –
  mit Knöpfen für `$…$` und `$$…$$` (für Markdown-Notizen). Das Werkzeug
  bleibt aktiv für die nächste Formel; Esc beendet es.
- Oder Text markieren und im Menü der Auswahl **∑** wählen.

**Beim ersten Mal** lädt Atlas das Modell herunter (etwa 100 MB, einmalig,
nach `~/.local/share/atlas/latex-ocr/`); das Fenster zeigt den Fortschritt.
Danach geht es ohne Netz. Die erste Formel nach dem Start von Atlas dauert
ein paar Sekunden (das Modell wird geladen), weitere etwa eine Sekunde.

Das Modell ist auf einzelne Formeln trainiert; bei Absätzen aus Text und
Formeln gemischt besser nur die Formel einrahmen.

## Wie es funktioniert

- Das Modell kommt im ONNX-Format von
  [RapidLaTeXOCR](https://github.com/RapidAI/RapidLaTeXOCR) (Release
  `v0.0.0`: `image_resizer.onnx`, `encoder.onnx`, `decoder.onnx`,
  `tokenizer.json`).
- Es läuft mit ONNX Runtime, die mit Atlas kommt (Nix-Paket; Arch:
  Abhängigkeit `onnxruntime`). Eine andere Bibliothek lässt sich mit
  `ORT_DYLIB_PATH=/pfad/libonnxruntime.so` wählen.
- Atlas rendert den Rahmen selbst in etwa 290 dpi, schneidet auf die Formel
  zu, lässt das Modell die passende Breite wählen, erkennt Zeichen für
  Zeichen und räumt überflüssige Leerzeichen im LaTeX weg – wie
  RapidLaTeXOCR, nur in Rust (`atlas_core::latex_ocr`).

## Ohne Internet

Die vier Dateien lassen sich auch von Hand in `~/.local/share/atlas/latex-ocr/`
legen (von der Release-Seite oben). Fehlt eine, lädt Atlas nur diese nach.
