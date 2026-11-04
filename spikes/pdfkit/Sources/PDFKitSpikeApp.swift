// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import PDFKit
import SwiftUI

/// Spike app (P0.4): shows spike.pdf with the Atlas annotations from
/// spike.atlas.json in a PDFKit view; a text selection is shown as Atlas
/// rectangles below the view.
@main
struct PDFKitSpikeApp: App {
    var body: some Scene {
        WindowGroup("Atlas PDFKit-Spike") {
            SpikeView()
                .frame(minWidth: 1500, minHeight: 950)
        }
    }
}

@MainActor
@Observable
final class SpikeModel {
    let document: PDFDocument?
    var added = 0
    var selectionInfo = "Text markieren: Rechtecke in Atlas-Koordinaten erscheinen hier."

    init() {
        let bundle = Bundle.main
        guard let pdf = bundle.url(forResource: "spike", withExtension: "pdf"),
            let doc = PDFDocument(url: pdf)
        else {
            document = nil
            return
        }
        if let json = bundle.url(forResource: "spike.atlas", withExtension: "json"),
            let file = try? AtlasAnnotationFile.load(json)
        {
            added = AtlasPDFKit.apply(file, to: doc)
        }
        document = doc
    }

    func selectionChanged(_ selection: PDFSelection?) {
        guard let selection, let page = selection.pages.first else { return }
        let rects = AtlasPDFKit.atlasRects(of: selection, on: page)
        let text = (selection.string ?? "").replacingOccurrences(of: "\n", with: " ")
        let index = (page.document?.index(for: page) ?? 0) + 1
        selectionInfo = "S. \(index) „\(text)“: "
            + rects.map { "[" + $0.map { String(format: "%.2f", $0) }.joined(separator: ", ") + "]" }.joined(separator: " ")
    }
}

struct SpikeView: View {
    @State private var model = SpikeModel()

    var body: some View {
        VStack(spacing: 0) {
            if let doc = model.document {
                PDFKitView(document: doc) { model.selectionChanged($0) }
            } else {
                ContentUnavailableView("spike.pdf fehlt im Bundle", systemImage: "doc.questionmark")
            }
            Divider()
            HStack {
                Text("\(model.added) Anmerkungen aus spike.atlas.json")
                    .foregroundStyle(.secondary)
                Divider().frame(height: 16)
                Text(model.selectionInfo)
                    .font(.system(.body, design: .monospaced))
                    .textSelection(.enabled)
                    .lineLimit(2)
                Spacer()
            }
            .padding(10)
        }
    }
}

/// `PDFView` in SwiftUI; reports selection changes.
struct PDFKitView: NSViewRepresentable {
    let document: PDFDocument
    let onSelection: @MainActor (PDFSelection?) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(onSelection: onSelection) }

    func makeNSView(context: Context) -> PDFView {
        let view = PDFView()
        view.document = document
        view.displayMode = .singlePageContinuous
        view.displayDirection = .horizontal
        view.displaysPageBreaks = true
        view.autoScales = true
        view.backgroundColor = .windowBackgroundColor
        context.coordinator.observe(view)
        return view
    }

    func updateNSView(_ view: PDFView, context: Context) {
        if view.document !== document { view.document = document }
    }

    @MainActor
    final class Coordinator: NSObject {
        let onSelection: @MainActor (PDFSelection?) -> Void

        init(onSelection: @escaping @MainActor (PDFSelection?) -> Void) {
            self.onSelection = onSelection
        }

        func observe(_ view: PDFView) {
            NotificationCenter.default.addObserver(
                self, selector: #selector(selectionChanged(_:)), name: .PDFViewSelectionChanged, object: view)
        }

        @objc func selectionChanged(_ note: Notification) {
            onSelection((note.object as? PDFView)?.currentSelection)
        }
    }
}
