#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
"""Writes spike.pdf, the test document of the PDFKit spike (task P0.4).

Three pages cover the cases where coordinate conversion can go wrong:
  1. plain A4,
  2. US Letter with a crop box that does not start at (0, 0),
  3. A4 with /Rotate 90.
reportlab draws the content; pypdf sets crop box and rotation afterwards
(reportlab's own setPageRotation also swaps the media box).
Needs: python3 -m pip install reportlab pypdf
"""

import io
import sys
from pathlib import Path

from pypdf import PdfReader, PdfWriter
from pypdf.generic import RectangleObject
from reportlab.lib.pagesizes import A4, letter
from reportlab.pdfgen import canvas

OUT = Path(__file__).with_name("spike.pdf")

LINES = [
    "Atlas PDFKit spike: highlight target alpha in this first line.",
    "The second line holds the word bravo and a longer phrase charlie delta.",
    "Third line: echo foxtrot golf, then some filler text to the margin.",
]

CROP_P2 = (36, 48, 576, 744)


def page(c, size, title, x, top):
    c.setPageSize(size)
    c.setFont("Helvetica-Bold", 20)
    c.drawString(x, top, title)
    c.setFont("Helvetica", 14)
    for i, line in enumerate(LINES):
        c.drawString(x, top - 60 - i * 26, line)
    c.rect(x, 200, 200, 120)  # a box the ink stroke goes into
    c.showPage()


def main():
    buf = io.BytesIO()
    c = canvas.Canvas(buf, pagesize=A4)
    c.setTitle("Atlas PDFKit spike")
    page(c, A4, "Seite 1 - A4", 72, A4[1] - 90)
    page(c, letter, "Seite 2 - CropBox", 90, 680)
    page(c, A4, "Seite 3 - Rotate 90", 72, A4[1] - 90)
    c.save()

    reader = PdfReader(io.BytesIO(buf.getvalue()))
    writer = PdfWriter()
    for p in reader.pages:
        writer.add_page(p)
    writer.pages[1].cropbox = RectangleObject(CROP_P2)
    writer.pages[2].rotation = 90
    writer.add_metadata({"/Title": "Atlas PDFKit spike"})
    with OUT.open("wb") as f:
        writer.write(f)
    print(OUT)


if __name__ == "__main__":
    sys.exit(main())
