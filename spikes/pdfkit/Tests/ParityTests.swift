// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import PDFKit
import XCTest

@testable import PDFKitSpike

/// Compares PDFKit with the Linux version (poppler) on spike.pdf:
///  1. selections made in PDFKit give the rects poppler stored,
///  2. the Atlas annotations drawn by PDFKit sit where the Linux viewer
///     draws them (measured on both renders).
/// Writes tables (parity-*.md) and comparison images to $ATLAS_CI_OUT.
final class ParityTests: XCTestCase {
    static let scale = 2.0

    private var bundle: Bundle { Bundle(for: Self.self) }

    private func fixtures() throws -> (PDFDocument, AtlasAnnotationFile) {
        let pdf = try XCTUnwrap(bundle.url(forResource: "spike", withExtension: "pdf"))
        let json = try XCTUnwrap(bundle.url(forResource: "spike.atlas", withExtension: "json"))
        return (try XCTUnwrap(PDFDocument(url: pdf)), try AtlasAnnotationFile.load(json))
    }

    private var outDir: URL {
        let env = ProcessInfo.processInfo.environment["ATLAS_CI_OUT"]
        let url = env.map { URL(fileURLWithPath: $0) }
            ?? FileManager.default.temporaryDirectory.appendingPathComponent("pdfkit-spike")
        try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    private func fmt(_ r: [Double]) -> String { r.map { String(format: "%.2f", $0) }.joined(separator: ", ") }

    // MARK: 1. Selection -> Atlas rects

    @MainActor
    func testSelectionGivesPopplerRects() throws {
        let (doc, file) = try fixtures()
        var rows = ["| S. | Text | poppler (Linux) | PDFKit | Δx | Δy-Mitte | Δb | Δh | ok |", "|---|---|---|---|---|---|---|---|---|"]
        var failures: [String] = []
        for a in file.annotations where a.kind.isTextMarkup {
            let text = try XCTUnwrap(a.text), expected = try XCTUnwrap(a.rects?.first)
            let page = try XCTUnwrap(doc.page(at: a.page - 1))
            guard let sel = doc.findString(text, withOptions: []).first(where: { $0.pages.contains(page) }) else {
                failures.append("S. \(a.page) \(text): nicht gefunden")
                rows.append("| \(a.page) | \(text) | \(fmt(expected)) | – | | | | | ✗ |")
                continue
            }
            let got = AtlasPDFKit.atlasRects(of: sel, on: page)
            // One line each in the fixture; a rotated page reports the line
            // "vertically" - compare the union either way.
            let r = got.dropFirst().reduce(got.first ?? [0, 0, 0, 0]) { u, s in
                let x0 = min(u[0], s[0]), y0 = min(u[1], s[1])
                return [x0, y0, max(u[0] + u[2], s[0] + s[2]) - x0, max(u[1] + u[3], s[1] + s[3]) - y0]
            }
            // Along the text: start and length must match closely; across the
            // text the two engines use different glyph boxes, so compare the
            // centre line tightly and the thickness loosely.
            let along = a.page == 3 ? (1, 3) : (0, 2), across = a.page == 3 ? (0, 2) : (1, 3)
            let dStart = r[along.0] - expected[along.0], dLen = r[along.1] - expected[along.1]
            let dMid = (r[across.0] + r[across.1] / 2) - (expected[across.0] + expected[across.1] / 2)
            let dThick = r[across.1] - expected[across.1]
            let ok = abs(dStart) <= 1.5 && abs(dLen) <= 2.0 && abs(dMid) <= 2.0 && abs(dThick) <= 4.0
            if !ok { failures.append("S. \(a.page) \(text): poppler [\(fmt(expected))] PDFKit [\(fmt(r))]") }
            rows.append(
                "| \(a.page) | \(text) | \(fmt(expected)) | \(fmt(r)) | "
                    + [dStart, dMid, dLen, dThick].map { String(format: "%+.2f", $0) }.joined(separator: " | ")
                    + " | \(ok ? "✓" : "✗") |")
        }
        let md = "## Auswahl in PDFKit → Atlas-Rechtecke (Vergleich mit poppler)\n\n"
            + "Δ entlang der Zeile (Anfang, Länge) und quer dazu (Mitte, Dicke), in Punkten. "
            + "Grenzen: Anfang ±1,5, Länge ±2, Mitte ±2, Dicke ±4.\n\n" + rows.joined(separator: "\n") + "\n"
        try md.write(to: outDir.appendingPathComponent("parity-selection.md"), atomically: true, encoding: .utf8)
        XCTAssertTrue(failures.isEmpty, failures.joined(separator: "\n"))
    }

    // MARK: 2. Render with annotations vs. Linux reference

    @MainActor
    func testAnnotationsRenderWhereLinuxDrawsThem() throws {
        let (doc, file) = try fixtures()
        AtlasPDFKit.apply(file, to: doc)
        let renders = outDir.appendingPathComponent("renders")
        try FileManager.default.createDirectory(at: renders, withIntermediateDirectories: true)

        var rows = ["| S. | Text | gespeichert (Atlas) | Linux gemessen | macOS gemessen | max. Abweichung macOS↔Linux | ok |", "|---|---|---|---|---|---|---|"]
        var failures: [String] = []
        for index in 0..<doc.pageCount {
            let n = index + 1
            let page = try XCTUnwrap(doc.page(at: index))
            let mac = PageRenderer.render(page, scale: Self.scale)
            let refURL = try XCTUnwrap(bundle.url(forResource: "ref-page-\(n)", withExtension: "png"))
            let linux = try XCTUnwrap(Bitmap(png: refURL))
            XCTAssertEqual(mac.width, linux.width, "Breite S. \(n)")
            XCTAssertEqual(mac.height, linux.height, "Höhe S. \(n)")
            try mac.writePNG(to: renders.appendingPathComponent("mac-page-\(n).png"))
            let cmp = PageRenderer.comparison(linux: linux, mac: mac)
            try cmp.writePNG(to: outDir.appendingPathComponent("compare-page-\(n).png"))
            add(XCTAttachment(image: NSImage(cgImage: cmp.cgImage, size: .zero)))

            for a in file.annotations where a.page == n && a.kind == .highlight {
                let r = try XCTUnwrap(a.rects?.first)
                let l = MeasuredExtent.measure(linux, around: r, scale: Self.scale)
                let m = MeasuredExtent.measure(mac, around: r, scale: Self.scale)
                var dev = Double.infinity
                if let l, let m {
                    dev = [m.x - l.x, m.y - l.y, (m.x + m.width) - (l.x + l.width), (m.y + m.height) - (l.y + l.height)]
                        .map(abs).max() ?? .infinity
                }
                let ok = dev <= 1.5
                if !ok { failures.append("S. \(n) \(a.text ?? ""): Linux \(l.map(\.description) ?? "–"), macOS \(m.map(\.description) ?? "–")") }
                rows.append(
                    "| \(n) | \(a.text ?? "") | \(fmt(r)) | \(l.map(\.description) ?? "–") | \(m.map(\.description) ?? "–") | "
                        + (dev.isFinite ? String(format: "%.2f pt", dev) : "–") + " | \(ok ? "✓" : "✗") |")
            }
        }
        let md = "## Gezeichnete Markierungen: macOS (PDFKit) gegen Linux (poppler + Overlay wie PdfViewer.qml)\n\n"
            + "Ausdehnung der Markierungsfarbe, auf beiden Bildern gleich gemessen (2 px je Punkt), "
            + "in Punkten (x, y, Breite × Höhe). Grenze: 1,5 pt je Kante.\n\n" + rows.joined(separator: "\n") + "\n"
        try md.write(to: outDir.appendingPathComponent("parity-render.md"), atomically: true, encoding: .utf8)
        XCTAssertTrue(failures.isEmpty, failures.joined(separator: "\n"))
    }
}
