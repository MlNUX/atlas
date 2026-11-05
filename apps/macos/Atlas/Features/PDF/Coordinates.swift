// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import CoreGraphics
import PDFKit

/// Converts between Atlas annotation coordinates and PDFKit page space.
/// From spike P0.4 (`docs/porting/spike-pdfkit.md`).
///
/// **Atlas** (what poppler reports and the Linux app stores): PDF points,
/// origin at the top left of the *crop box*, axes of the page *as displayed*,
/// i.e. after applying `/Rotate`. Width and height are those of the rotated
/// crop box. (`docs/data-format.md` says "unrotated page"; poppler-glib and
/// therefore every stored file use the rotated page - see the spike report.)
///
/// **PDFKit page space**: unrotated, origin at the bottom left of the media
/// box, y up. `PDFPage.bounds(for: .cropBox)` lives in this space.
struct PageGeometry: Sendable, Equatable {
    /// Crop box in page space.
    var cropBox: CGRect
    /// Clockwise rotation like `/Rotate`: 0, 90, 180 or 270.
    var rotation: Int

    init(cropBox: CGRect, rotation: Int) {
        self.cropBox = cropBox.standardized
        self.rotation = ((rotation % 360) + 360) % 360
    }

    init(page: PDFPage) {
        self.init(cropBox: page.bounds(for: .cropBox), rotation: page.rotation)
    }

    /// Size of the page as displayed (and as Atlas sees it).
    var displaySize: CGSize {
        rotation == 90 || rotation == 270
            ? CGSize(width: cropBox.height, height: cropBox.width)
            : cropBox.size
    }

    /// Atlas point -> PDFKit page space.
    func toPage(_ p: CGPoint) -> CGPoint {
        let w = cropBox.width, h = cropBox.height
        // (u, v): unrotated, origin top left of the crop box, y down.
        let uv: (u: CGFloat, v: CGFloat) =
            switch rotation {
            case 90: (p.y, h - p.x)
            case 180: (w - p.x, h - p.y)
            case 270: (w - p.y, p.x)
            default: (p.x, p.y)
            }
        return CGPoint(x: cropBox.minX + uv.u, y: cropBox.maxY - uv.v)
    }

    /// PDFKit page space -> Atlas point.
    func toAtlas(_ p: CGPoint) -> CGPoint {
        let w = cropBox.width, h = cropBox.height
        let u = p.x - cropBox.minX, v = cropBox.maxY - p.y
        switch rotation {
        case 90: return CGPoint(x: h - v, y: u)
        case 180: return CGPoint(x: w - u, y: h - v)
        case 270: return CGPoint(x: v, y: w - u)
        default: return CGPoint(x: u, y: v)
        }
    }

    /// Atlas `[x, y, w, h]` -> rectangle in page space.
    func toPage(_ r: [Double]) -> CGRect {
        precondition(r.count == 4, "Atlas rect needs four numbers")
        let a = toPage(CGPoint(x: r[0], y: r[1]))
        let b = toPage(CGPoint(x: r[0] + r[2], y: r[1] + r[3]))
        return CGRect(x: min(a.x, b.x), y: min(a.y, b.y), width: abs(a.x - b.x), height: abs(a.y - b.y))
    }

    /// Rectangle in page space -> Atlas `[x, y, w, h]`.
    func toAtlas(_ r: CGRect) -> [Double] {
        let a = toAtlas(CGPoint(x: r.minX, y: r.minY))
        let b = toAtlas(CGPoint(x: r.maxX, y: r.maxY))
        return [min(a.x, b.x), min(a.y, b.y), abs(a.x - b.x), abs(a.y - b.y)].map(Double.init)
    }
}

extension PageGeometry {
    /// Atlas rectangle (AtlasKit) -> page space.
    func toPage(_ r: Rect) -> CGRect { toPage([r.x, r.y, r.width, r.height]) }

    /// Page space -> Atlas rectangle (AtlasKit).
    func toAtlasRect(_ r: CGRect) -> Rect {
        let a = toAtlas(r)
        return Rect(x: a[0], y: a[1], width: a[2], height: a[3])
    }
}
