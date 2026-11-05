// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import PDFKit
import SwiftUI

/// A PDF tab (P3.2): PDFKit display with Atlas annotations, marking from the
/// text selection, comments, thumbnails, search.
struct PDFDocumentView: View {
    let project: ProjectModel
    let path: String
    @State private var model: PDFModel?

    var body: some View {
        Group {
            if let model {
                PDFContent(model: model)
            } else {
                ProgressView()
            }
        }
        .task(id: path) { model = PDFModel(project: project, path: path) }
        .onChange(of: project.annotationsRevision) {
            if project.annotationsChangedPath == path { model?.reload() }
        }
        .focusedSceneValue(\.pdf, model)
    }
}

private struct PDFContent: View {
    @Bindable var model: PDFModel
    @State private var editing: String?
    @State private var comment = ""

    var body: some View {
        VStack(spacing: 0) {
            toolbar
            Divider()
            if let error = model.error {
                Text(error).foregroundStyle(.red).padding(6)
            }
            if let status = model.status, !status.message.isEmpty {
                Text(status.message).font(.callout).padding(6)
                    .frame(maxWidth: .infinity).background(.yellow.opacity(0.2))
            }
            HStack(spacing: 0) {
                if model.sidebarVisible {
                    PDFThumbnails(model: model).frame(width: 150)
                    Divider()
                }
                PDFKitView(model: model) { id in
                    model.selectedAnnotation = id
                    comment = model.core?.annotation(id: id)?.comment ?? ""
                    editing = id
                }
            }
        }
        .popover(isPresented: Binding(get: { editing != nil }, set: { if !$0 { editing = nil } })) {
            VStack(alignment: .leading, spacing: 8) {
                Text("Kommentar").font(.headline)
                TextEditor(text: $comment).frame(width: 280, height: 100)
                HStack {
                    Button("Löschen", role: .destructive) {
                        model.removeSelectedAnnotation()
                        editing = nil
                    }
                    Spacer()
                    Button("Sichern") {
                        if let id = editing { model.setComment(comment, of: id) }
                        editing = nil
                    }
                    .keyboardShortcut(.defaultAction)
                }
            }
            .padding(12)
        }
    }

    private var toolbar: some View {
        HStack(spacing: 8) {
            Button("Miniaturen", systemImage: "sidebar.left") { model.sidebarVisible.toggle() }
                .labelStyle(.iconOnly)
            Divider().frame(height: 16)
            ForEach(PDFModel.colors, id: \.self) { hex in
                Circle()
                    .fill(Color(nsColor: NSColor(atlasHex: hex)))
                    .frame(width: 14, height: 14)
                    .overlay(Circle().stroke(.primary, lineWidth: model.color == hex ? 2 : 0))
                    .onTapGesture { model.color = hex }
                    .accessibilityLabel("Farbe \(hex)")
            }
            Button("Markieren", systemImage: "highlighter") { model.mark(.highlight) }
                .help("Auswahl markieren")
            Button("Unterstreichen", systemImage: "underline") { model.mark(.underline) }
            Button("Durchstreichen", systemImage: "strikethrough") { model.mark(.strikeout) }
            Spacer()
            TextField("Im PDF suchen", text: $model.searchText)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 220)
                .onSubmit { model.search() }
            if !model.searchHits.isEmpty {
                Text("\(model.searchHits.count) Treffer").font(.caption).foregroundStyle(.secondary)
            }
        }
        .labelStyle(.iconOnly)
        .buttonStyle(.borderless)
        .padding(.horizontal, 8)
        .padding(.vertical, 5)
    }
}

/// PDFKit's thumbnail view next to the document.
private struct PDFThumbnails: NSViewRepresentable {
    let model: PDFModel

    func makeNSView(context: NSViewRepresentableContext<Self>) -> PDFThumbnailView {
        let view = PDFThumbnailView()
        view.thumbnailSize = CGSize(width: 110, height: 140)
        view.backgroundColor = .clear
        return view
    }

    func updateNSView(_ view: PDFThumbnailView, context: NSViewRepresentableContext<Self>) {
        view.pdfView = PDFKitView.registry[ObjectIdentifier(model)]
    }
}

/// The PDFView (PDFKit) of a model.
struct PDFKitView: NSViewRepresentable {
    let model: PDFModel
    let annotationClicked: (String) -> Void

    /// PDFViews by model, so the thumbnails find theirs.
    @MainActor static var registry: [ObjectIdentifier: PDFView] = [:]

    func makeCoordinator() -> Coordinator { Coordinator(annotationClicked: annotationClicked) }

    func makeNSView(context: NSViewRepresentableContext<Self>) -> PDFView {
        let view = PDFView()
        view.document = model.document
        view.autoScales = true
        view.displayMode = .singlePageContinuous
        view.displaysPageBreaks = true
        view.backgroundColor = .windowBackgroundColor
        model.currentSelection = { [weak view] in view?.currentSelection }
        Self.registry[ObjectIdentifier(model)] = view
        NotificationCenter.default.addObserver(
            context.coordinator, selector: #selector(Coordinator.annotationHit(_:)),
            name: .PDFViewAnnotationHit, object: view)
        return view
    }

    func updateNSView(_ view: PDFView, context: NSViewRepresentableContext<Self>) {
        _ = model.revision  // redraw after annotation changes
        view.highlightedSelections = model.searchHits.isEmpty ? nil : model.searchHits
        if let first = model.searchHits.first, context.coordinator.lastSearch != model.searchText {
            context.coordinator.lastSearch = model.searchText
            view.go(to: first)
        }
        view.needsDisplay = true
    }

    static func dismantleNSView(_ view: PDFView, coordinator: Coordinator) {
        NotificationCenter.default.removeObserver(coordinator)
        registry = registry.filter { $0.value !== view }
    }

    @MainActor
    final class Coordinator: NSObject {
        let annotationClicked: (String) -> Void
        var lastSearch = ""

        init(annotationClicked: @escaping (String) -> Void) {
            self.annotationClicked = annotationClicked
        }

        @objc func annotationHit(_ note: Notification) {
            guard let annotation = note.userInfo?["PDFAnnotationHit"] as? PDFAnnotation,
                  let id = AtlasPDFKit.atlasID(of: annotation) else { return }
            annotationClicked(id)
        }
    }
}
