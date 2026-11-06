// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import CoreGraphics
import Foundation
import Testing

@testable import Atlas

@MainActor
struct ViewersTests {
    /// RGBA of pixel (x, y), origin top left.
    func pixel(_ image: CGImage, _ x: Int, _ y: Int) -> [UInt8] {
        let ctx = CGContext(data: nil, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
                            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.draw(image, in: CGRect(x: -x, y: -(image.height - 1 - y), width: image.width, height: image.height))
        let p = ctx.data!.assumingMemoryBound(to: UInt8.self)
        return [p[0], p[1], p[2], p[3]]
    }

    @Test func notePageWithPaperStrokeAndText() throws {
        let paper = NoteColor(r: 1, g: 0.992, b: 0.969, a: 1)
        let line = NoteColor(r: 0.85, g: 0.87, b: 0.92, a: 1)
        let blue = NoteColor(r: 0, g: 0, b: 1, a: 1)
        let page = NotePage(
            height: 500, dark: false, background: paper,
            pattern: PaperPattern(kind: .lines, step: 32, start: 48, margin: 32, lineWidth: 0.65, dotRadius: 1.1, color: line),
            pdf: nil, image: nil,
            marks: [
                .stroke(tool: "PEN", color: blue, width: 10, pressure: true,
                        points: [NotePoint(x: 100, y: 300, pressure: 1), NotePoint(x: 600, y: 300, pressure: 1)]),
                .text(color: blue, size: 30, x: 100, y: 380, rotation: 0, lines: ["Hallo"]),
            ])
        // 1000 note units wide drawn at 500 px: scale 0.5.
        let image = try #require(NoteRenderer.image(of: page, pageWidth: 1000, widthPx: 500))
        #expect(image.width == 500 && image.height == 250)
        // Paper away from the marks; the stroke in blue at (350, 300) → (175, 150).
        let background = pixel(image, 10, 5)
        #expect(background[0] > 240 && background[2] > 230)
        let stroke = pixel(image, 175, 150)
        #expect(stroke[2] > 200 && stroke[0] < 40)
        // The width with full pressure: 10 × (0.4 + 0.8).
        #expect(abs(NoteRenderer.segmentWidth(10, NotePoint(x: 0, y: 0, pressure: 1), NotePoint(x: 1, y: 0, pressure: 1)) - 12) < 1e-9)
        #expect(abs(NoteRenderer.segmentWidth(10, NotePoint(x: 0, y: 0, pressure: -1), NotePoint(x: 1, y: 0, pressure: -1)) - 4) < 1e-9)
    }

    @Test func remoteContentIsRecognised() {
        #expect(HtmlPage.refersToRemote(#"<script src="https://cdn.example.com/x.js"></script>"#))
        #expect(HtmlPage.refersToRemote(#"<img src=//example.com/a.png>"#))
        #expect(HtmlPage.refersToRemote(#"@import url("https://fonts.example.com/a.css");"#))
        #expect(!HtmlPage.refersToRemote(#"<img src="bild.png"><a href="https://example.com">Link</a>"#))
    }
}
