#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
"""Linux side of the PDFKit spike (task P0.4).

Reads spike.pdf with poppler-glib (the library the Linux app uses) and writes
  spike.atlas.json   annotations in the Atlas format (.atlas/annotations/*.json)
                     with the text-markup rectangles poppler reports,
  ref-page-N.png     each page rendered by poppler/cairo at 2x with the
                     annotations drawn the way PdfViewer.qml draws them.
The macOS tests load the same JSON into PDFKit and compare against both.

Needs poppler-glib introspection and pycairo, e.g. on Ubuntu:
  apt install gir1.2-poppler-0.18 python3-gi-cairo
and the system python those belong to (here /usr/bin/python3.12).
"""

import json
import os
import sys
from pathlib import Path

import gi

gi.require_version("Poppler", "0.18")
gi.require_foreign("cairo")
import cairo  # noqa: E402
from gi.repository import Poppler  # noqa: E402

HERE = Path(__file__).parent
PDF = HERE / "spike.pdf"
SCALE = 2.0
STAMP = 1792355296

# (page, kind, words, colour) - text markup found via poppler's text layout.
MARKUP = [
    (1, "highlight", "alpha", "#ffd400"),
    (1, "underline", "bravo", "#4c8dff"),
    (1, "highlight", "charlie delta", "#7bd88f"),
    (1, "strikeout", "foxtrot", "#ff6b6b"),
    (2, "highlight", "alpha", "#ffd400"),
    (2, "underline", "bravo", "#4c8dff"),
    (2, "highlight", "charlie delta", "#7bd88f"),
    (3, "highlight", "alpha", "#ffd400"),
    (3, "underline", "bravo", "#4c8dff"),
    (3, "highlight", "charlie delta", "#7bd88f"),
]

# Ink strokes and notes are given in unrotated PDF space (origin bottom left,
# media box), as drawn by make_pdf.py, and converted below like the viewer
# would store them: display space of the page, origin top left of the crop box.
BOX = {1: (72, 200), 2: (90, 200), 3: (72, 200)}  # lower left of the 200x120 box


def page_geometry(doc, n):
    """Crop box (x0, y0, x1, y1) and /Rotate of 1-based page n via pypdf-free
    parsing: poppler-glib does not expose the crop box, so make_pdf.py's values
    are mirrored here and checked against poppler's page size."""
    crop = {1: (0, 0, 595.2756, 841.8898), 2: (36, 48, 576, 744), 3: (0, 0, 595.2756, 841.8898)}[n]
    rot = {1: 0, 2: 0, 3: 90}[n]
    w, h = doc.get_page(n - 1).get_size()
    cw, ch = crop[2] - crop[0], crop[3] - crop[1]
    exp = (ch, cw) if rot in (90, 270) else (cw, ch)
    assert abs(w - exp[0]) < 0.01 and abs(h - exp[1]) < 0.01, (n, w, h, exp)
    return crop, rot


def to_display(crop, rot, x, y):
    """PDF page space (bottom-left, media origin) -> Atlas display space."""
    u, v = x - crop[0], crop[3] - y  # unrotated, top left of crop box
    w, h = crop[2] - crop[0], crop[3] - crop[1]
    return {0: (u, v), 90: (h - v, u), 180: (w - u, h - v), 270: (v, w - u)}[rot]


def word_rect(page, words):
    ok, rects = page.get_text_layout()
    text = page.get_text()
    k = text.find(words)
    assert ok and k >= 0, words
    rs = rects[k : k + len(words)]
    x1, y1 = min(r.x1 for r in rs), min(r.y1 for r in rs)
    x2, y2 = max(r.x2 for r in rs), max(r.y2 for r in rs)
    return [round(x1, 3), round(y1, 3), round(x2 - x1, 3), round(y2 - y1, 3)]


def build(doc):
    out, i = [], 0
    for n, kind, words, color in MARKUP:
        i += 1
        out.append({
            "id": f"{i:012x}", "kind": kind, "page": n,
            "rects": [word_rect(doc.get_page(n - 1), words)], "text": words,
            "color": color, "created": STAMP, "modified": STAMP,
        })
    for n in (1, 2, 3):
        crop, rot = page_geometry(doc, n)
        bx, by = BOX[n]
        stroke = [to_display(crop, rot, bx + 20 + t * 16, by + 60 + (30 if t % 2 else -30)) for t in range(11)]
        i += 1
        out.append({
            "id": f"{i:012x}", "kind": "ink", "page": n,
            "ink": [[[round(p[0], 3), round(p[1], 3)] for p in stroke]],
            "width": 3.0, "color": "#ff6b6b", "created": STAMP, "modified": STAMP,
        })
        nx, ny = to_display(crop, rot, bx + 230, by + 120)  # note: top left of icon
        i += 1
        out.append({
            "id": f"{i:012x}", "kind": "note", "page": n,
            "rects": [[round(nx, 3), round(ny, 3), 18.0, 18.0]],
            "comment": f"Notiz auf Seite {n}", "color": "#ffd400",
            "created": STAMP, "modified": STAMP,
        })
    return out


def rgba(color, alpha=1.0):
    c = color.lstrip("#")
    return (int(c[0:2], 16) / 255, int(c[2:4], 16) / 255, int(c[4:6], 16) / 255, alpha)


def render(doc, n, annots, path):
    page = doc.get_page(n - 1)
    w, h = page.get_size()
    surf = cairo.ImageSurface(cairo.FORMAT_ARGB32, round(w * SCALE), round(h * SCALE))
    cr = cairo.Context(surf)
    cr.set_source_rgb(1, 1, 1)
    cr.paint()
    cr.scale(SCALE, SCALE)
    page.render(cr)
    # Overlay like PdfViewer.qml: highlight = colour at 38 %, underline /
    # strikeout = 1.2 pt line, note = 18 pt square, ink = round stroke.
    for a in annots:
        if a["page"] != n:
            continue
        if a["kind"] in ("highlight", "underline", "strikeout"):
            for x, y, rw, rh in a["rects"]:
                if a["kind"] == "highlight":
                    cr.set_source_rgba(*rgba(a["color"], 0.38))
                    cr.rectangle(x, y, rw, rh)
                else:
                    lh = 1.2
                    ly = y + rh - lh if a["kind"] == "underline" else y + (rh - lh) / 2
                    cr.set_source_rgba(*rgba(a["color"]))
                    cr.rectangle(x, ly, rw, lh)
                cr.fill()
        elif a["kind"] == "note":
            x, y, rw, rh = a["rects"][0]
            cr.set_source_rgba(*rgba(a["color"]))
            cr.rectangle(x, y, rw, rh)
            cr.fill_preserve()
            cr.set_source_rgba(0, 0, 0, 0.5)
            cr.set_line_width(1)
            cr.stroke()
        elif a["kind"] == "ink":
            cr.set_source_rgba(*rgba(a["color"]))
            cr.set_line_width(a.get("width", 2))
            cr.set_line_cap(cairo.LINE_CAP_ROUND)
            cr.set_line_join(cairo.LINE_JOIN_ROUND)
            for stroke in a["ink"]:
                cr.move_to(*stroke[0])
                for p in stroke[1:]:
                    cr.line_to(*p)
                cr.stroke()
    surf.write_to_png(str(path))


def main():
    doc = Poppler.Document.new_from_file(PDF.resolve().as_uri(), None)
    annots = build(doc)
    data = {
        "version": 1,
        "document": {"path": "spike.pdf", "blake3": "", "size": os.path.getsize(PDF), "pages": doc.get_n_pages()},
        "annotations": annots,
    }
    (HERE / "spike.atlas.json").write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
    for n in range(1, doc.get_n_pages() + 1):
        render(doc, n, annots, HERE / f"ref-page-{n}.png")
    print(f"{len(annots)} annotations, {doc.get_n_pages()} reference pages (poppler {Poppler.get_version()})")


if __name__ == "__main__":
    sys.exit(main())
