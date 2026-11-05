// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import Foundation
import Observation
import PDFKit

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
    var searchText = ""
    private(set) var searchHits: [PDFSelection] = []
    /// Bumped whenever the shown annotations must be rebuilt.
    private(set) var revision = 0
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
}
