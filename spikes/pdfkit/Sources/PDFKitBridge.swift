// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import PDFKit

/// Atlas annotations <-> PDFKit. Atlas data stays the source of truth; the
/// PDFKit annotations only exist in memory for display (P3.2 adds editing).
enum AtlasPDFKit {
    /// Opacity the Linux viewer uses for highlights (`PdfViewer.qml`).
    static let highlightAlpha: CGFloat = 0.38

    /// Adds all annotations of `file` to the matching pages of `document`.
    /// Returns how many PDFKit annotations were added.
    @discardableResult
    static func apply(_ file: AtlasAnnotationFile, to document: PDFDocument) -> Int {
        var added = 0
        for a in file.annotations {
            guard a.page >= 1, a.page <= document.pageCount, let page = document.page(at: a.page - 1) else { continue }
            for annotation in makeAnnotations(a, on: page) {
                page.addAnnotation(annotation)
                added += 1
            }
        }
        return added
    }

    static func makeAnnotations(_ a: AtlasAnnotation, on page: PDFPage) -> [PDFAnnotation] {
        let geo = PageGeometry(page: page)
        let color = NSColor(atlasHex: a.color ?? "#ffd400")
        switch a.kind {
        case .highlight, .underline, .strikeout:
            let rects = (a.rects ?? []).filter { $0.count == 4 }.map(geo.toPage)
            guard let first = rects.first else { return [] }
            let bounds = rects.dropFirst().reduce(first) { $0.union($1) }
            let type: PDFAnnotationSubtype =
                a.kind == .highlight ? .highlight : a.kind == .underline ? .underline : .strikeOut
            let annotation = PDFAnnotation(bounds: bounds, forType: type, withProperties: nil)
            annotation.color = a.kind == .highlight ? color.withAlphaComponent(highlightAlpha) : color
            // Quad points relative to the annotation origin, per rect:
            // top left, top right, bottom left, bottom right (page space).
            let o = bounds.origin
            annotation.quadrilateralPoints = rects.flatMap { r in
                [
                    NSPoint(x: r.minX - o.x, y: r.maxY - o.y), NSPoint(x: r.maxX - o.x, y: r.maxY - o.y),
                    NSPoint(x: r.minX - o.x, y: r.minY - o.y), NSPoint(x: r.maxX - o.x, y: r.minY - o.y),
                ].map { NSValue(point: $0) }
            }
            annotation.contents = a.comment
            annotation.userName = a.id
            return [annotation]

        case .note:
            guard let r = a.rects?.first, r.count == 4 else { return [] }
            let annotation = PDFAnnotation(bounds: geo.toPage(r), forType: .text, withProperties: nil)
            annotation.iconType = .comment
            annotation.color = color
            annotation.contents = a.comment
            annotation.userName = a.id
            return [annotation]

        case .ink:
            let strokes = (a.ink ?? []).map { $0.filter { $0.count == 2 }.map { geo.toPage(CGPoint(x: $0[0], y: $0[1])) } }
                .filter { !$0.isEmpty }
            let points = strokes.flatMap { $0 }
            guard let p0 = points.first else { return [] }
            let lineWidth = CGFloat(a.width ?? 2)
            var bounds = CGRect(origin: p0, size: .zero)
            for p in points { bounds = bounds.union(CGRect(origin: p, size: .zero)) }
            bounds = bounds.insetBy(dx: -lineWidth, dy: -lineWidth)
            let annotation = PDFAnnotation(bounds: bounds, forType: .ink, withProperties: nil)
            let border = PDFBorder()
            border.lineWidth = lineWidth
            annotation.border = border
            annotation.color = color
            for stroke in strokes {
                let path = NSBezierPath()
                path.lineCapStyle = .round
                path.lineJoinStyle = .round
                path.lineWidth = lineWidth
                path.move(to: NSPoint(x: stroke[0].x - bounds.minX, y: stroke[0].y - bounds.minY))
                for p in stroke.dropFirst() { path.line(to: NSPoint(x: p.x - bounds.minX, y: p.y - bounds.minY)) }
                annotation.add(path)
            }
            annotation.userName = a.id
            return [annotation]

        case .bookmark:
            // No PDF equivalent (same as the Linux export); shown in the sidebar in P3.2.
            return []
        }
    }

    /// One Atlas rect per line of a PDFKit selection on `page`, like the
    /// Linux viewer stores a text selection.
    static func atlasRects(of selection: PDFSelection, on page: PDFPage) -> [[Double]] {
        let geo = PageGeometry(page: page)
        return selection.selectionsByLine().compactMap { line in
            let b = line.bounds(for: page)
            return b.isEmpty ? nil : geo.toAtlas(b)
        }
    }
}

extension NSColor {
    /// `#rrggbb` (Atlas colours); anything else falls back to the Atlas yellow.
    convenience init(atlasHex hex: String) {
        let s = hex.hasPrefix("#") ? String(hex.dropFirst()) : hex
        guard s.count == 6, let v = UInt32(s, radix: 16) else {
            self.init(srgbRed: 1, green: 0.831, blue: 0, alpha: 1)
            return
        }
        self.init(
            srgbRed: CGFloat((v >> 16) & 0xff) / 255, green: CGFloat((v >> 8) & 0xff) / 255,
            blue: CGFloat(v & 0xff) / 255, alpha: 1)
    }
}
