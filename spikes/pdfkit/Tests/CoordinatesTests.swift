// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import PDFKit
import XCTest

@testable import PDFKitSpike

final class CoordinatesTests: XCTestCase {
    private let crop = CGRect(x: 36, y: 48, width: 540, height: 696)

    func testRoundTripAllRotations() {
        for rotation in [0, 90, 180, 270, -90, 450] {
            let geo = PageGeometry(cropBox: crop, rotation: rotation)
            for r in [[0.0, 0, 10, 20], [100.5, 33.25, 42, 12.5], [500, 600, 7, 9]] {
                let back = geo.toAtlas(geo.toPage(r))
                for i in 0..<4 { XCTAssertEqual(back[i], r[i], accuracy: 1e-9, "rotation \(rotation) rect \(r)") }
            }
        }
    }

    func testCornersWithoutRotation() {
        let geo = PageGeometry(cropBox: crop, rotation: 0)
        // Atlas origin = top left of the crop box.
        XCTAssertEqual(geo.toPage(CGPoint(x: 0, y: 0)), CGPoint(x: 36, y: 744))
        XCTAssertEqual(geo.toPage(CGPoint(x: 540, y: 696)), CGPoint(x: 576, y: 48))
        XCTAssertEqual(geo.displaySize, CGSize(width: 540, height: 696))
    }

    func testCornersRotated90() {
        // /Rotate 90 turns the page clockwise: the unrotated top left corner
        // ends up top right, the display is as wide as the crop box is high.
        let geo = PageGeometry(cropBox: crop, rotation: 90)
        XCTAssertEqual(geo.displaySize, CGSize(width: 696, height: 540))
        XCTAssertEqual(geo.toAtlas(CGPoint(x: 36, y: 744)), CGPoint(x: 696, y: 0))
        XCTAssertEqual(geo.toAtlas(CGPoint(x: 576, y: 744)), CGPoint(x: 696, y: 540))
        XCTAssertEqual(geo.toAtlas(CGPoint(x: 36, y: 48)), CGPoint(x: 0, y: 0))
    }

    func testCornersRotated180And270() {
        let g180 = PageGeometry(cropBox: crop, rotation: 180)
        XCTAssertEqual(g180.toAtlas(CGPoint(x: 36, y: 744)), CGPoint(x: 540, y: 696))
        let g270 = PageGeometry(cropBox: crop, rotation: 270)
        XCTAssertEqual(g270.displaySize, CGSize(width: 696, height: 540))
        XCTAssertEqual(g270.toAtlas(CGPoint(x: 36, y: 744)), CGPoint(x: 0, y: 540))
    }

    /// Page 1 and 3 of spike.pdf carry the same content, page 3 with
    /// /Rotate 90. poppler's rects (in the fixture) differ, but must land on
    /// the same spot in page space.
    func testFixtureRotatedPageMatchesPlainPage() throws {
        let bundle = Bundle(for: Self.self)
        let doc = try XCTUnwrap(PDFDocument(url: try XCTUnwrap(bundle.url(forResource: "spike", withExtension: "pdf"))))
        let file = try AtlasAnnotationFile.load(
            try XCTUnwrap(bundle.url(forResource: "spike.atlas", withExtension: "json")))
        let p1 = try XCTUnwrap(doc.page(at: 0)), p3 = try XCTUnwrap(doc.page(at: 2))
        XCTAssertEqual(p3.rotation, 90)
        for text in ["alpha", "bravo", "charlie delta"] {
            let a1 = try XCTUnwrap(file.annotations.first { $0.page == 1 && $0.text == text }?.rects?.first)
            let a3 = try XCTUnwrap(file.annotations.first { $0.page == 3 && $0.text == text }?.rects?.first)
            let r1 = PageGeometry(page: p1).toPage(a1), r3 = PageGeometry(page: p3).toPage(a3)
            XCTAssertEqual(r1.minX, r3.minX, accuracy: 0.01, text)
            XCTAssertEqual(r1.minY, r3.minY, accuracy: 0.01, text)
            XCTAssertEqual(r1.width, r3.width, accuracy: 0.01, text)
            XCTAssertEqual(r1.height, r3.height, accuracy: 0.01, text)
        }
    }
}
