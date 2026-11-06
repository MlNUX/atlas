// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import CoreGraphics
import PDFKit

/// Draws a page of an Atlas notebook (`.atlasnote`, Atlas Tablet) with
/// CoreGraphics, as `atlas_core::atlasnote` draws it with cairo on Linux:
/// paper (colour, pattern, PDF or picture), strokes (pressure, translucent
/// brushes), text and placed pictures. Note units, origin top left.
enum NoteRenderer {
    static func image(of page: NotePage, pageWidth: Double, widthPx: Int) -> CGImage? {
        guard widthPx > 0, pageWidth > 0, page.height > 0 else { return nil }
        let scale = Double(widthPx) / pageWidth
        let heightPx = max(1, Int((page.height * scale).rounded(.up)))
        guard let space = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: widthPx, height: heightPx, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return nil }
        // Top left origin in note units.
        ctx.translateBy(x: 0, y: CGFloat(heightPx))
        ctx.scaleBy(x: 1, y: -1)
        ctx.scaleBy(x: scale, y: scale)
        let previous = NSGraphicsContext.current
        NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: true)
        defer { NSGraphicsContext.current = previous }
        drawPaper(page, width: pageWidth, in: ctx)
        for mark in page.marks {
            ctx.saveGState()
            switch mark {
            case let .stroke(_, color, width, pressure, points):
                drawStroke(color: color, width: width, pressure: pressure, points: points, in: ctx)
            case let .text(color, size, x, y, rotation, lines):
                drawText(color: color, size: size, x: x, y: y, rotation: rotation, lines: lines, in: ctx)
            case let .image(path, topLeft, topRight, bottomLeft):
                drawImage(path: path, a: topLeft, b: topRight, c: bottomLeft, in: ctx)
            }
            ctx.restoreGState()
        }
        return ctx.makeImage()
    }

    static func cgColor(_ c: NoteColor, opaque: Bool = false) -> CGColor {
        CGColor(srgbRed: c.r, green: c.g, blue: c.b, alpha: opaque ? 1 : c.a)
    }

    // MARK: paper

    static func drawPaper(_ page: NotePage, width: Double, in ctx: CGContext) {
        let all = CGRect(x: 0, y: 0, width: width, height: page.height)
        ctx.setFillColor(cgColor(page.background))
        ctx.fill(all)
        if let pdf = page.pdf {
            ctx.setFillColor(CGColor(gray: 1, alpha: 1))
            ctx.fill(all)
            if let doc = PDFDocument(url: URL(filePath: pdf.path)), let p = doc.page(at: Int(pdf.page)) {
                // The page stretched over the note page (its own rotation included).
                let box = p.bounds(for: .cropBox).applying(p.transform(for: .cropBox)).standardized
                ctx.saveGState()
                ctx.translateBy(x: 0, y: page.height)
                ctx.scaleBy(x: width / box.width, y: -page.height / box.height)
                ctx.translateBy(x: -box.minX, y: -box.minY)
                p.draw(with: .cropBox, to: ctx)
                ctx.restoreGState()
            }
            return
        }
        if let path = page.image {
            NSImage(contentsOfFile: path)?.draw(in: all, from: .zero, operation: .sourceOver, fraction: 1,
                                                respectFlipped: true, hints: nil)
            return
        }
        guard let pattern = page.pattern else { return }
        ctx.setStrokeColor(cgColor(pattern.color))
        ctx.setFillColor(cgColor(pattern.color))
        ctx.setLineWidth(pattern.lineWidth)
        switch pattern.kind {
        case .lines:
            var y = pattern.start
            while y < page.height {
                ctx.move(to: CGPoint(x: pattern.margin, y: y))
                ctx.addLine(to: CGPoint(x: width - pattern.margin, y: y))
                y += pattern.step
            }
            ctx.strokePath()
        case .grid:
            var x = pattern.start
            while x < width {
                ctx.move(to: CGPoint(x: x, y: 0))
                ctx.addLine(to: CGPoint(x: x, y: page.height))
                x += pattern.step
            }
            var y = pattern.start
            while y < page.height {
                ctx.move(to: CGPoint(x: 0, y: y))
                ctx.addLine(to: CGPoint(x: width, y: y))
                y += pattern.step
            }
            ctx.strokePath()
        case .dots:
            var y = pattern.start
            while y < page.height {
                var x = pattern.start
                while x <= width - pattern.start {
                    let r = pattern.dotRadius
                    ctx.addEllipse(in: CGRect(x: x - r, y: y - r, width: 2 * r, height: 2 * r))
                    x += pattern.step
                }
                y += pattern.step
            }
            ctx.fillPath()
        }
    }

    // MARK: marks

    /// Segment widths of a pressure stroke: `width × (0.4 + 0.8 × mean pressure)`.
    static func segmentWidth(_ width: Double, _ a: NotePoint, _ b: NotePoint) -> Double {
        width * (0.4 + 0.8 * (max(0, a.pressure) + max(0, b.pressure)) / 2)
    }

    static func drawStroke(color: NoteColor, width: Double, pressure: Bool, points: [NotePoint], in ctx: CGContext) {
        guard let first = points.first else { return }
        // Translucent brushes: drawn opaque, faded as a whole (overlaps do not darken).
        let translucent = color.a < 0.999
        if translucent {
            ctx.setAlpha(color.a)
            ctx.beginTransparencyLayer(auxiliaryInfo: nil)
        }
        let ink = cgColor(color, opaque: translucent)
        ctx.setStrokeColor(ink)
        ctx.setFillColor(ink)
        ctx.setLineCap(.round)
        ctx.setLineJoin(.round)
        if points.count == 1 {
            ctx.fillEllipse(in: CGRect(x: first.x - width / 2, y: first.y - width / 2, width: width, height: width))
        } else if pressure, points.contains(where: { $0.pressure >= 0 }) {
            for (a, b) in zip(points, points.dropFirst()) {
                ctx.setLineWidth(segmentWidth(width, a, b))
                ctx.move(to: CGPoint(x: a.x, y: a.y))
                ctx.addLine(to: CGPoint(x: b.x, y: b.y))
                ctx.strokePath()
            }
        } else {
            ctx.setLineWidth(width)
            ctx.move(to: CGPoint(x: first.x, y: first.y))
            for p in points.dropFirst() { ctx.addLine(to: CGPoint(x: p.x, y: p.y)) }
            ctx.strokePath()
        }
        if translucent { ctx.endTransparencyLayer() }
    }

    static func drawText(color: NoteColor, size: Double, x: Double, y: Double, rotation: Double, lines: [String],
                         in ctx: CGContext) {
        ctx.translateBy(x: x, y: y)
        ctx.rotate(by: rotation * .pi / 180)
        let font = NSFont.systemFont(ofSize: size)
        let attributes: [NSAttributedString.Key: Any] = [
            .font: font,
            .foregroundColor: NSColor(srgbRed: color.r, green: color.g, blue: color.b, alpha: color.a),
        ]
        for (i, line) in lines.enumerated() {
            // Baseline at `size × (1 + 1.3 i)` below the point; draw(at:) takes the top of the line.
            let baseline = size * (1 + 1.3 * Double(i))
            (line as NSString).draw(at: CGPoint(x: 0, y: baseline - font.ascender), withAttributes: attributes)
        }
    }

    static func drawImage(path: String, a: NotePoint, b: NotePoint, c: NotePoint, in ctx: CGContext) {
        guard let image = NSImage(contentsOfFile: path),
              let cg = image.cgImage(forProposedRect: nil, context: nil, hints: nil)
        else { return }
        let w = Double(cg.width), h = Double(cg.height)
        // The picture's pixels onto the three corners (move, scale and rotation in one).
        ctx.concatenate(CGAffineTransform(a: (b.x - a.x) / w, b: (b.y - a.y) / w, c: (c.x - a.x) / h, d: (c.y - a.y) / h,
                                          tx: a.x, ty: a.y))
        image.draw(in: CGRect(x: 0, y: 0, width: w, height: h), from: .zero, operation: .sourceOver, fraction: 1,
                   respectFlipped: true, hints: nil)
    }
}
