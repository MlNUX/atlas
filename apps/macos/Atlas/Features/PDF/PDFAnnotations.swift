// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import PDFKit

/// Atlas annotations (the core's data, source of truth) <-> PDFKit
/// annotations (display only). Based on spike P0.4.
enum AtlasPDFKit {
    /// Opacity of highlights, as in the Linux viewer (`PdfViewer.qml`).
    static let highlightAlpha: CGFloat = 0.38
    /// `userName` prefix of PDFKit annotations that show Atlas ones.
    static let prefix = "atlas:"

    static func atlasID(of annotation: PDFAnnotation) -> String? {
        guard let name = annotation.userName, name.hasPrefix(prefix) else { return nil }
        return String(name.dropFirst(prefix.count))
    }

    /// Replaces the shown Atlas annotations of `document` with `annotations`.
    static func show(_ annotations: [Annotation], in document: PDFDocument) {
        for i in 0..<document.pageCount {
            guard let page = document.page(at: i) else { continue }
            for a in page.annotations where atlasID(of: a) != nil { page.removeAnnotation(a) }
        }
        for a in annotations {
            guard a.page >= 1, Int(a.page) <= document.pageCount, let page = document.page(at: Int(a.page) - 1)
            else { continue }
            for shown in make(a, on: page) { page.addAnnotation(shown) }
        }
    }

    static func make(_ a: Annotation, on page: PDFPage) -> [PDFAnnotation] {
        let geo = PageGeometry(page: page)
        let color = NSColor(atlasHex: a.color)
        let tag = prefix + a.id
        switch a.kind {
        case .highlight, .underline, .strikeout:
            let rects = a.rects.map(geo.toPage)
            guard let first = rects.first else { return [] }
            let bounds = rects.dropFirst().reduce(first) { $0.union($1) }
            let type: PDFAnnotationSubtype =
                a.kind == .highlight ? .highlight : a.kind == .underline ? .underline : .strikeOut
            let shown = PDFAnnotation(bounds: bounds, forType: type, withProperties: nil)
            shown.color = a.kind == .highlight ? color.withAlphaComponent(highlightAlpha) : color
            // Quad points relative to the annotation origin, per rect:
            // top left, top right, bottom left, bottom right (page space).
            let o = bounds.origin
            shown.quadrilateralPoints = rects.flatMap { r in
                [
                    NSPoint(x: r.minX - o.x, y: r.maxY - o.y), NSPoint(x: r.maxX - o.x, y: r.maxY - o.y),
                    NSPoint(x: r.minX - o.x, y: r.minY - o.y), NSPoint(x: r.maxX - o.x, y: r.minY - o.y),
                ].map { NSValue(point: $0) }
            }
            shown.contents = a.comment
            shown.userName = tag
            return [shown]

        case .note:
            guard let r = a.rects.first else { return [] }
            let shown = PDFAnnotation(bounds: geo.toPage(r), forType: .text, withProperties: nil)
            shown.iconType = .comment
            shown.color = color
            shown.contents = a.comment
            shown.userName = tag
            return [shown]

        case .ink:
            let strokes = a.ink.map { $0.points.map { geo.toPage(CGPoint(x: $0.x, y: $0.y)) } }.filter { !$0.isEmpty }
            let points = strokes.flatMap { $0 }
            guard let p0 = points.first else { return [] }
            let lineWidth = CGFloat(a.width ?? 2)
            var bounds = CGRect(origin: p0, size: .zero)
            for p in points { bounds = bounds.union(CGRect(origin: p, size: .zero)) }
            bounds = bounds.insetBy(dx: -lineWidth, dy: -lineWidth)
            let shown = PDFAnnotation(bounds: bounds, forType: .ink, withProperties: nil)
            let border = PDFBorder()
            border.lineWidth = lineWidth
            shown.border = border
            shown.color = color
            for stroke in strokes {
                let path = NSBezierPath()
                path.lineCapStyle = .round
                path.lineJoinStyle = .round
                path.lineWidth = lineWidth
                path.move(to: NSPoint(x: stroke[0].x - bounds.minX, y: stroke[0].y - bounds.minY))
                for p in stroke.dropFirst() { path.line(to: NSPoint(x: p.x - bounds.minX, y: p.y - bounds.minY)) }
                shown.add(path)
            }
            shown.userName = tag
            return [shown]

        case .bookmark:
            // No PDF equivalent (as in the Linux export); shown in the sidebar.
            return []
        }
    }

    /// One Atlas rect per line of `selection` on `page`, as the Linux viewer
    /// stores a text selection.
    static func atlasRects(of selection: PDFSelection, on page: PDFPage) -> [Rect] {
        let geo = PageGeometry(page: page)
        return selection.selectionsByLine().compactMap { line in
            let b = line.bounds(for: page)
            return b.isEmpty ? nil : geo.toAtlasRect(b)
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
