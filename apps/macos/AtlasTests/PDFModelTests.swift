// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Foundation
import PDFKit
import Testing

@testable import Atlas

/// P3.2 against the real core: annotations made in the Mac viewer land in
/// `.atlas/annotations/` (shared with Linux, E12) and come back.
@MainActor
struct PDFModelTests {
    func makeProject() throws -> (ProjectModel, String) {
        let dir = FileManager.default.temporaryDirectory.appending(path: "atlas-pdf-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let pdf = PDFDocument()
        for i in 0..<2 {
            let image = NSImage(size: NSSize(width: 595, height: 842))
            image.lockFocus()
            NSColor.white.setFill()
            NSRect(x: 0, y: 0, width: 595, height: 842).fill()
            ("Seite \(i + 1)" as NSString).draw(at: NSPoint(x: 72, y: 760), withAttributes: [.font: NSFont.systemFont(ofSize: 24)])
            image.unlockFocus()
            pdf.insert(PDFPage(image: image)!, at: i)
        }
        let path = dir.appending(path: "Skript.pdf").path
        #expect(pdf.write(toFile: path))
        let project = try ProjectModel(app: AppModel(), root: dir.path)
        return (project, path)
    }

    @Test func inkNotesBookmarksAndExport() throws {
        let (project, path) = try makeProject()
        let model = PDFModel(project: project, path: path)
        #expect(model.error == nil)
        #expect(model.document?.pageCount == 2)

        model.addInk(page: 0, strokes: [[CGPoint(x: 100, y: 700), CGPoint(x: 200, y: 650), CGPoint(x: 300, y: 700)]])
        let note = try #require(model.addNote(page: 1, at: CGPoint(x: 300, y: 400)))
        model.setComment("Prüfungsrelevant", of: note)
        model.bookmark(page: 1, title: "Kapitel 2")

        let all = model.annotations
        #expect(all.contains { $0.kind == .ink && $0.page == 1 && $0.ink.first?.points.count == 3 })
        #expect(all.contains { $0.kind == .note && $0.page == 2 && $0.comment == "Prüfungsrelevant" })
        #expect(model.bookmarks.map(\.title) == ["Kapitel 2"])
        // Atlas coordinates: origin top left, so a point near the top has a small y.
        let ink = try #require(all.first { $0.kind == .ink })
        #expect(ink.ink[0].points[0].y < 200)

        // Shown in PDFKit (ink on page 1, note on page 2).
        #expect(model.document?.page(at: 0)?.annotations.contains { AtlasPDFKit.atlasID(of: $0) == ink.id } == true)

        // A second model (another window) reads the same data from `.atlas/`.
        let again = PDFModel(project: project, path: path)
        #expect(again.annotations.count == all.count)

        model.selectedAnnotation = note
        model.removeSelectedAnnotation()
        #expect(!model.annotations.contains { $0.id == note })

        // Export: a copy, never the original.
        let out = URL(fileURLWithPath: path).deletingLastPathComponent().appending(path: "Kopie.pdf")
        #expect(model.export(to: out))
        #expect(PDFDocument(url: out)?.page(at: 0)?.annotations.isEmpty == false)
        #expect(!model.export(to: URL(fileURLWithPath: path)))
    }
}
