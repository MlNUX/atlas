// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Foundation
import Observation
import PDFKit

/// What a click or drag on the page does.
enum PDFTool: String, CaseIterable, Sendable {
    /// Select text (and click annotations).
    case select
    /// Freehand strokes.
    case ink
    /// Pin a comment.
    case note
    /// Drag a box: the area as picture into a note (Linux: `figure`).
    case figure
    /// Drag a box over a formula: LaTeX (Linux: `latex`).
    case latex
}

/// A box dragged with `figure` or `latex`: page index, PDFKit page space.
struct PDFArea: Equatable {
    let id = UUID()
    let page: Int
    let rect: CGRect
    let tool: PDFTool
}

/// Entry of the outline (table of contents) of a PDF.
struct PDFOutlineEntry: Identifiable {
    let id = UUID()
    let title: String
    let page: Int
    let level: Int
}

/// One open PDF: PDFKit document for display, the core's `PdfDocument` for
/// the Atlas data (annotations in `.atlas/annotations/`, shared with Linux).
@MainActor
@Observable
final class PDFModel {
    /// Atlas' marker colours (Linux: `PdfToolbar.qml`).
    static let colors = ["#ffd400", "#7cd67c", "#5ec8f2", "#ff8fb1", "#c9a7ff"]

    let path: String
    let document: PDFDocument?
    let core: PdfDocument?
    private(set) var status: PdfStatus?
    var error: String?
    var color = PDFModel.colors[0]
    /// The Atlas annotation clicked last (for deleting, editing the comment).
    var selectedAnnotation: String?
    var sidebarVisible = true
    var tool: PDFTool = .select
    /// Dark mode: pages inverted onto dark paper, colours kept (Linux: `pdf.dark`).
    var dark = false
    /// Freehand stroke width in points.
    var inkWidth: Double = 2
    var searchText = ""
    private(set) var searchHits: [PDFSelection] = []
    /// Bumped whenever the shown annotations must be rebuilt.
    private(set) var revision = 0
    /// The box just dragged (the view takes it as picture or formula).
    var chosenArea: PDFArea?
    /// Formula recognition: running, its LaTeX or its problem.
    private(set) var latexBusy = false
    var latex: String?
    var latexError: String?
    /// Set by the view: the PDFView's current text selection.
    @ObservationIgnored var currentSelection: () -> PDFSelection? = { nil }

    init(project: ProjectModel, path: String) {
        self.path = path
        document = PDFDocument(url: URL(fileURLWithPath: path))
        do {
            core = try project.core.pdf(path: path)
        } catch {
            core = nil
            self.error = "\(error)"
        }
        guard let document else {
            error = "PDF kann nicht geöffnet werden"
            return
        }
        do {
            status = try core?.attach(pageCount: UInt32(document.pageCount), pdfId: nil)
        } catch {
            self.error = "\(error)"
        }
        showAnnotations()
        feedPageText()
    }

    /// Hands the page text to the core (project search, index, quotes; P1.1).
    /// Read from a document of its own off the main thread.
    private func feedPageText() {
        guard let core else { return }
        let url = URL(fileURLWithPath: path)
        Task.detached(priority: .utility) {
            guard let doc = PDFDocument(url: url) else { return }
            let pages = (0..<doc.pageCount).map { doc.page(at: $0)?.string ?? "" }
            core.setPageTexts(pages: pages)
        }
    }

    var annotations: [Annotation] { core?.annotations() ?? [] }

    func showAnnotations() {
        guard let document else { return }
        AtlasPDFKit.show(annotations, in: document)
        revision += 1
    }

    /// The core changed the annotations (another window, Linux via sync).
    func reload() {
        status = try? core?.reload()
        showAnnotations()
    }

    /// Marks the current text selection: one annotation per page, sharing a group.
    func mark(_ kind: AnnotationKind) {
        guard let core, let selection = currentSelection(), !selection.pages.isEmpty, let document else { return }
        let group = selection.pages.count > 1 ? UUID().uuidString.lowercased() : ""
        do {
            for page in selection.pages {
                let rects = AtlasPDFKit.atlasRects(of: selection, on: page)
                guard !rects.isEmpty else { continue }
                let text = selection.selectionsByLine().filter { $0.pages.contains(page) }
                    .compactMap(\.string).joined(separator: " ")
                _ = try core.add(annotation: Annotation(
                    id: "", kind: kind, page: UInt32(document.index(for: page) + 1), rects: rects, text: text,
                    comment: "", color: color, ink: [], width: nil, title: "", group: group, created: 0, modified: 0))
            }
            showAnnotations()
        } catch {
            self.error = "\(error)"
        }
    }

    /// Adds freehand strokes on page `index` (0-based); points in page space.
    func addInk(page index: Int, strokes: [[CGPoint]]) {
        guard let core, let document, let page = document.page(at: index), !strokes.isEmpty else { return }
        let geo = PageGeometry(page: page)
        let ink = strokes.map { stroke in
            Stroke(points: stroke.map { p in
                let a = geo.toAtlas(p)
                return Point(x: a.x, y: a.y)
            })
        }
        do {
            _ = try core.add(annotation: Annotation(
                id: "", kind: .ink, page: UInt32(index + 1), rects: [], text: "", comment: "", color: color,
                ink: ink, width: inkWidth, title: "", group: "", created: 0, modified: 0))
            showAnnotations()
        } catch {
            self.error = "\(error)"
        }
    }

    /// Pins a comment at `point` (page space) on page `index`; returns its id.
    func addNote(page index: Int, at point: CGPoint) -> String? {
        guard let core, let document, let page = document.page(at: index) else { return nil }
        let geo = PageGeometry(page: page)
        let icon = geo.toAtlasRect(CGRect(x: point.x - 10, y: point.y - 10, width: 20, height: 20))
        do {
            let id = try core.add(annotation: Annotation(
                id: "", kind: .note, page: UInt32(index + 1), rects: [icon], text: "", comment: "", color: color,
                ink: [], width: nil, title: "", group: "", created: 0, modified: 0))
            showAnnotations()
            return id
        } catch {
            self.error = "\(error)"
            return nil
        }
    }

    var bookmarks: [Annotation] { annotations.filter { $0.kind == .bookmark }.sorted { $0.page < $1.page } }

    /// The PDF's own table of contents, flattened with levels.
    var outline: [PDFOutlineEntry] {
        guard let document, let root = document.outlineRoot else { return [] }
        var out: [PDFOutlineEntry] = []
        func walk(_ item: PDFOutline, level: Int) {
            for i in 0..<item.numberOfChildren {
                guard let child = item.child(at: i) else { continue }
                let page = child.destination?.page.map { document.index(for: $0) } ?? 0
                out.append(PDFOutlineEntry(title: child.label ?? "", page: page, level: level))
                walk(child, level: level + 1)
            }
        }
        walk(root, level: 0)
        return out
    }

    /// Writes a copy with the annotations embedded (PDFKit; the original is
    /// never written). Bookmarks have no PDF equivalent and are left out.
    func export(to url: URL) -> Bool {
        guard let document else { return false }
        if url.standardizedFileURL == URL(fileURLWithPath: path).standardizedFileURL {
            error = "Der Export darf die Original-PDF nicht überschreiben."
            return false
        }
        return document.write(to: url)
    }

    var defaultExportPath: String { core?.defaultExportPath() ?? path }

    /// Bookmark for the page `index` (0-based).
    func bookmark(page index: Int, title: String) {
        do {
            _ = try core?.add(annotation: Annotation(
                id: "", kind: .bookmark, page: UInt32(index + 1), rects: [], text: "", comment: "", color: color,
                ink: [], width: nil, title: title, group: "", created: 0, modified: 0))
            revision += 1
        } catch {
            self.error = "\(error)"
        }
    }

    func setComment(_ comment: String, of id: String) {
        do {
            try core?.update(id: id, patch: AnnotationPatch(
                comment: comment, color: nil, title: nil, rects: nil, ink: nil, kind: nil))
            showAnnotations()
        } catch {
            self.error = "\(error)"
        }
    }

    func removeSelectedAnnotation() {
        guard let id = selectedAnnotation else { return }
        do {
            try core?.remove(id: id)
            selectedAnnotation = nil
            showAnnotations()
        } catch {
            self.error = "\(error)"
        }
    }

    func search() {
        guard let document, !searchText.isEmpty else {
            searchHits = []
            return
        }
        searchHits = document.findString(searchText, withOptions: [.caseInsensitive, .diacriticInsensitive])
    }

    /// Markdown quote of the selected annotation, for the clipboard.
    func quoteSelected(into note: String) -> String? {
        guard let id = selectedAnnotation else { return nil }
        return try? core?.quoteMarkdown(id: id, note: note)
    }

    // MARK: areas (picture, formula)

    /// `rect` of page `index` (PDFKit page space) drawn at `scale` pixels per
    /// point on white; grey (one byte per pixel) or RGB.
    func render(page index: Int, rect: CGRect, scale: CGFloat, gray: Bool) -> CGImage? {
        guard let page = document?.page(at: index), rect.width > 1, rect.height > 1 else { return nil }
        // Page space → the drawing space of `draw(with:to:)` (rotation, crop box).
        let box = rect.applying(page.transform(for: .cropBox)).standardized
        let width = Int((box.width * scale).rounded(.up)), height = Int((box.height * scale).rounded(.up))
        let context = gray
            ? CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width,
                        space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue)
            : CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
                        space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)
        guard let context else { return nil }
        context.setFillColor(CGColor(gray: 1, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        context.scaleBy(x: scale, y: scale)
        context.translateBy(x: -box.minX, y: -box.minY)
        page.draw(with: .cropBox, to: context)
        return context.makeImage()
    }

    /// The area as PNG in the note's assets and the picture with its source
    /// appended to `note` (Linux: `pdfAreaToNote`).
    func figure(_ area: PDFArea, into note: String, notes: Notes) throws {
        guard let image = render(page: area.page, rect: area.rect, scale: 2, gray: false) else {
            throw AtlasError.Invalid(message: "Der Bereich ist leer.")
        }
        let page = UInt32(area.page + 1)
        let target = try notes.imageTarget(note: note, pdf: path, page: page)
        guard let destination = CGImageDestinationCreateWithURL(URL(filePath: target) as CFURL, "public.png" as CFString, 1, nil)
        else { throw AtlasError.Failed(message: "\(target) kann nicht geschrieben werden.") }
        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination) else {
            throw AtlasError.Failed(message: "\(target) kann nicht geschrieben werden.")
        }
        _ = try notes.append(path: note, block: notes.imageQuote(note: note, image: target, pdf: path, page: page))
    }

    /// LaTeX of the formula in the area (pix2tex in the core, on this Mac),
    /// copied as `$…$` (Linux: `recognizeLatex`).
    func recognize(_ area: PDFArea) async {
        // A little margin, as on Linux; about 2× of 72 dpi on white.
        guard let image = render(page: area.page, rect: area.rect.insetBy(dx: -3, dy: -3), scale: 2, gray: true),
              let data = image.dataProvider?.data as Data?
        else { return }
        // Rows may be padded: copy them without.
        var pixels = [UInt8](repeating: 255, count: image.width * image.height)
        data.withUnsafeBytes { raw in
            for y in 0..<image.height {
                for x in 0..<image.width { pixels[y * image.width + x] = raw[y * image.bytesPerRow + x] }
            }
        }
        latexBusy = true
        latex = nil
        latexError = nil
        defer { latexBusy = false }
        do {
            let text = try await LatexOcr().recognize(image: GrayImage(width: UInt32(image.width), height: UInt32(image.height), pixels: Data(pixels)))
            latex = text
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString("$\(text)$", forType: .string)
        } catch {
            latexError = atlasMessage(error)
        }
    }
}
