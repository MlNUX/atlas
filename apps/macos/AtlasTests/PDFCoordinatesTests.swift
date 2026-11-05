// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import CoreGraphics
import Testing

@testable import Atlas

/// Atlas coordinates <-> PDFKit page space (P3.2, from spike P0.4).
struct PDFCoordinatesTests {
    let crop = CGRect(x: 36, y: 48, width: 540, height: 696)

    @Test(arguments: [0, 90, 180, 270, -90, 450])
    func roundTrip(rotation: Int) {
        let geo = PageGeometry(cropBox: crop, rotation: rotation)
        for r in [Rect(x: 0, y: 0, width: 10, height: 20), Rect(x: 100.5, y: 33.25, width: 42, height: 12.5)] {
            let back = geo.toAtlasRect(geo.toPage(r))
            #expect(abs(back.x - r.x) < 1e-9 && abs(back.y - r.y) < 1e-9)
            #expect(abs(back.width - r.width) < 1e-9 && abs(back.height - r.height) < 1e-9)
        }
    }

    @Test func cornersWithoutRotation() {
        let geo = PageGeometry(cropBox: crop, rotation: 0)
        #expect(geo.toPage(CGPoint(x: 0, y: 0)) == CGPoint(x: 36, y: 744))
        #expect(geo.toPage(CGPoint(x: 540, y: 696)) == CGPoint(x: 576, y: 48))
    }

    @Test func rotated90() {
        let geo = PageGeometry(cropBox: crop, rotation: 90)
        #expect(geo.displaySize == CGSize(width: 696, height: 540))
        #expect(geo.toAtlas(CGPoint(x: 36, y: 744)) == CGPoint(x: 696, y: 0))
    }

    @Test func atlasColours() {
        #expect(NSColor(atlasHex: "#ff0000").redComponent == 1)
        #expect(NSColor(atlasHex: "kaputt").greenComponent > 0.8)
    }
}
