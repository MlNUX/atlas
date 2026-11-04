// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import CoreGraphics
import Foundation
import ImageIO
import PDFKit
import UniformTypeIdentifiers

/// RGBA8 pixels, row 0 at the top - the same layout for PDFKit renders and
/// the Linux reference PNGs, so both can be measured the same way.
struct Bitmap: Sendable {
    let width: Int
    let height: Int
    var pixels: [UInt8]

    init(width: Int, height: Int, pixels: [UInt8]) {
        self.width = width
        self.height = height
        self.pixels = pixels
    }

    init(_ image: CGImage) {
        width = image.width
        height = image.height
        var data = [UInt8](repeating: 255, count: width * height * 4)
        data.withUnsafeMutableBytes { buf in
            let ctx = CGContext(
                data: buf.baseAddress, width: image.width, height: image.height, bitsPerComponent: 8,
                bytesPerRow: image.width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
            ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
            ctx.fill(CGRect(x: 0, y: 0, width: image.width, height: image.height))
            ctx.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        }
        pixels = data
    }

    init?(png url: URL) {
        guard let src = CGImageSourceCreateWithURL(url as CFURL, nil),
            let image = CGImageSourceCreateImageAtIndex(src, 0, nil)
        else { return nil }
        self.init(image)
    }

    func rgb(_ x: Int, _ y: Int) -> (r: Int, g: Int, b: Int) {
        let i = (y * width + x) * 4
        return (Int(pixels[i]), Int(pixels[i + 1]), Int(pixels[i + 2]))
    }

    /// Coloured (not white/grey/black): highlights are, text is not.
    /// 12 catches the pale green PDFKit draws and still ignores antialiased text.
    func isTinted(_ x: Int, _ y: Int) -> Bool {
        let c = rgb(x, y)
        return max(c.r, c.g, c.b) - min(c.r, c.g, c.b) >= 12
    }

    var cgImage: CGImage {
        let provider = CGDataProvider(data: Data(pixels) as CFData)!
        return CGImage(
            width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
            provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)!
    }

    func writePNG(to url: URL) throws {
        guard let dest = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)
        else { throw CocoaError(.fileWriteUnknown) }
        CGImageDestinationAddImage(dest, cgImage, nil)
        guard CGImageDestinationFinalize(dest) else { throw CocoaError(.fileWriteUnknown) }
    }
}

enum PageRenderer {
    /// Renders `page` with its annotations as displayed (crop box, rotation)
    /// at `scale` pixels per point - the same output as the Linux reference
    /// (poppler/cairo at the same scale).
    static func render(_ page: PDFPage, scale: CGFloat) -> Bitmap {
        let size = PageGeometry(page: page).displaySize
        let w = Int((size.width * scale).rounded()), h = Int((size.height * scale).rounded())
        var data = [UInt8](repeating: 255, count: w * h * 4)
        data.withUnsafeMutableBytes { buf in
            let ctx = CGContext(
                data: buf.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                space: CGColorSpace(name: CGColorSpace.sRGB)!,
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
            ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
            ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
            ctx.scaleBy(x: scale, y: scale)
            // draw(with:to:) applies crop box origin and /Rotate itself;
            // concatenating page.transform(for:) as well would apply them twice.
            page.draw(with: .cropBox, to: ctx)
        }
        return Bitmap(width: w, height: h, pixels: data)
    }

    /// Linux | macOS | overlay (macOS at full strength, Linux at 50 % on top).
    static func comparison(linux: Bitmap, mac: Bitmap) -> Bitmap {
        let gap = 16
        let w = max(linux.width, mac.width), h = max(linux.height, mac.height)
        var out = Bitmap(width: w * 3 + gap * 2, height: h, pixels: [UInt8](repeating: 230, count: (w * 3 + gap * 2) * h * 4))
        func put(_ b: Bitmap, at x0: Int) {
            for y in 0..<b.height {
                for x in 0..<b.width {
                    let s = (y * b.width + x) * 4, d = (y * out.width + x0 + x) * 4
                    out.pixels[d..<d + 4] = b.pixels[s..<s + 4]
                }
            }
        }
        put(linux, at: 0)
        put(mac, at: w + gap)
        put(mac, at: 2 * (w + gap))
        let x0 = 2 * (w + gap)
        for y in 0..<min(linux.height, h) {
            for x in 0..<min(linux.width, w) {
                let s = (y * linux.width + x) * 4, d = (y * out.width + x0 + x) * 4
                for c in 0..<3 { out.pixels[d + c] = UInt8((Int(out.pixels[d + c]) + Int(linux.pixels[s + c])) / 2) }
            }
        }
        return out
    }
}

/// Where the highlight colour of a mark really is in a rendered page,
/// in points (top left origin). Measured the same way on both renders.
struct MeasuredExtent: Sendable, CustomStringConvertible {
    var x: Double, y: Double, width: Double, height: Double

    var description: String { String(format: "%.1f, %.1f, %.1f × %.1f", x, y, width, height) }

    /// Searches a window around the expected Atlas rect `r` for tinted pixels:
    /// columns inside the rect's rows give the horizontal extent, rows inside
    /// the rect's columns the vertical one.
    static func measure(_ bmp: Bitmap, around r: [Double], scale: Double) -> MeasuredExtent? {
        let x0 = r[0] * scale, y0 = r[1] * scale, w = r[2] * scale, h = r[3] * scale
        func clampX(_ v: Double) -> Int { min(max(Int(v), 0), bmp.width - 1) }
        func clampY(_ v: Double) -> Int { min(max(Int(v), 0), bmp.height - 1) }
        // Inner band (avoid the edges) used as the cross section.
        let innerRows = clampY(y0 + h * 0.3)...clampY(y0 + h * 0.7)
        let innerCols = clampX(x0 + w * 0.3)...clampX(x0 + w * 0.7)
        let searchCols = clampX(x0 - max(w, h) * 0.6)...clampX(x0 + w + max(w, h) * 0.6)
        let searchRows = clampY(y0 - max(w, h) * 0.6)...clampY(y0 + h + max(w, h) * 0.6)

        func fraction(col: Int, rows: ClosedRange<Int>) -> Double {
            Double(rows.filter { bmp.isTinted(col, $0) }.count) / Double(rows.count)
        }
        func fraction(row: Int, cols: ClosedRange<Int>) -> Double {
            Double(cols.filter { bmp.isTinted($0, row) }.count) / Double(cols.count)
        }
        let cols = searchCols.filter { fraction(col: $0, rows: innerRows) > 0.3 }
        let rows = searchRows.filter { fraction(row: $0, cols: innerCols) > 0.3 }
        guard let c0 = cols.min(), let c1 = cols.max(), let r0 = rows.min(), let r1 = rows.max() else { return nil }
        return MeasuredExtent(
            x: Double(c0) / scale, y: Double(r0) / scale,
            width: Double(c1 - c0 + 1) / scale, height: Double(r1 - r0 + 1) / scale)
    }
}
