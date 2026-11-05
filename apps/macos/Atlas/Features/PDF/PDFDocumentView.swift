// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import PDFKit
import SwiftUI
import UniformTypeIdentifiers

/// A PDF tab (P3.2): PDFKit display with Atlas annotations, marking from the
/// text selection, freehand, comments, bookmarks, outline, search, dark
/// mode, presentation, export.
struct PDFDocumentView: View {
    let project: ProjectModel
    let path: String
    @State private var model: PDFModel?

    var body: some View {
        Group {
            if let model {
                PDFContent(project: project, model: model)
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

private enum SidebarTab: String, CaseIterable {
    case thumbnails = "Miniaturen"
    case outline = "Gliederung"
    case bookmarks = "Lesezeichen"

    var symbol: String {
        switch self {
        case .thumbnails: "square.grid.2x2"
        case .outline: "list.bullet.indent"
        case .bookmarks: "bookmark"
        }
    }
}

private struct PDFContent: View {
    let project: ProjectModel
    @Bindable var model: PDFModel
    @State private var editing: String?
    @State private var comment = ""
    @State private var sidebar: SidebarTab = .thumbnails

    var body: some View {
        VStack(spacing: 0) {
            toolbar
            Divider()
            if let error = model.error {
                Text(error).foregroundStyle(.red).padding(6)
            }
            if let status = model.status, !status.message.isEmpty {
                HStack {
                    Text(status.message).font(.callout)
                    Button("Verstanden") { model.core?.acknowledgeChange() }
                }
                .padding(6).frame(maxWidth: .infinity).background(.yellow.opacity(0.2))
            }
            HStack(spacing: 0) {
                if model.sidebarVisible {
                    sidebarView.frame(width: 180)
                    Divider()
                }
                PDFKitView(model: model) { id in openComment(id) }
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
                    Button("Als Zitat kopieren") {
                        copyQuote()
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
        .onChange(of: model.chosenArea) {
            guard let area = model.chosenArea else { return }
            model.chosenArea = nil
            if area.tool == .latex {
                Task { await model.recognize(area) }
            } else {
                figure(area)
            }
        }
        .sheet(isPresented: Binding(
            get: { model.latexBusy || model.latex != nil || model.latexError != nil },
            set: { if !$0 { model.latex = nil; model.latexError = nil } }
        )) {
            LatexResultView(model: model)
        }
        .onReceive(NotificationCenter.default.publisher(for: .atlasPDFNoteAdded)) { note in
            if let id = note.object as? String { openComment(id) }
        }
    }

    private func openComment(_ id: String) {
        model.selectedAnnotation = id
        comment = model.core?.annotation(id: id)?.comment ?? ""
        editing = id
    }

    /// The area as picture into a note: the first open note, else one to
    /// choose (Linux asks with the last note preselected).
    private func figure(_ area: PDFArea) {
        let open = project.editor.groups.flatMap(\.tabs).first { $0.path.hasSuffix(".md") }?.path
        guard let note = open ?? chooseNote() else { return }
        do {
            try model.figure(area, into: note, notes: project.core.notes())
            project.show(notice: "Bild von Seite \(area.page + 1) in \((note as NSString).lastPathComponent) übernommen")
        } catch {
            model.error = atlasMessage(error)
        }
    }

    private func chooseNote() -> String? {
        let panel = NSSavePanel()
        panel.title = "Bild übernehmen in"
        panel.message = "Notiz wählen oder neu anlegen; das Bild wird angehängt."
        panel.prompt = "Übernehmen"
        panel.directoryURL = URL(filePath: project.root, directoryHint: .isDirectory)
        panel.nameFieldStringValue = "Zitate.md"
        panel.allowedContentTypes = [.init(filenameExtension: "md")!]
        // Appending, not replacing: no "replace?" question for an existing note.
        panel.canCreateDirectories = true
        return panel.runModal() == .OK ? panel.url?.path : nil
    }

    /// Markdown quote of the selected annotation, linking back from the first
    /// open note (else from the project root), to the clipboard.
    private func copyQuote() {
        let note = project.editor.groups.flatMap(\.tabs).first { $0.path.hasSuffix(".md") }?.path
            ?? project.core.absolutePath(relative: "Notiz.md")
        guard let text = model.quoteSelected(into: note) else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }

    @ViewBuilder
    private var sidebarView: some View {
        VStack(spacing: 0) {
            Picker("", selection: $sidebar) {
                ForEach(SidebarTab.allCases, id: \.self) { tab in
                    Image(systemName: tab.symbol).tag(tab).help(tab.rawValue).accessibilityLabel(tab.rawValue)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .padding(6)
            switch sidebar {
            case .thumbnails:
                PDFThumbnails(model: model)
            case .outline:
                let entries = model.outline
                if entries.isEmpty {
                    ContentUnavailableView("Keine Gliederung", systemImage: "list.bullet.indent")
                } else {
                    List(entries) { e in
                        Button(e.title) { go(to: e.page) }
                            .buttonStyle(.plain)
                            .padding(.leading, CGFloat(e.level) * 12)
                    }
                }
            case .bookmarks:
                List {
                    ForEach(model.bookmarks, id: \.id) { b in
                        Button("S. \(b.page) · \(b.title.isEmpty ? "Lesezeichen" : b.title)") { go(to: Int(b.page) - 1) }
                            .buttonStyle(.plain)
                            .contextMenu {
                                Button("Löschen", role: .destructive) {
                                    model.selectedAnnotation = b.id
                                    model.removeSelectedAnnotation()
                                }
                            }
                    }
                    Button("Lesezeichen für diese Seite", systemImage: "bookmark") {
                        let page = PDFKitView.registry[ObjectIdentifier(model)]?.currentPage
                        let index = page.flatMap { model.document?.index(for: $0) } ?? 0
                        model.bookmark(page: index, title: "")
                    }
                }
            }
        }
    }

    private func go(to page: Int) {
        guard let view = PDFKitView.registry[ObjectIdentifier(model)], let p = model.document?.page(at: page) else { return }
        view.go(to: p)
    }

    private var toolbar: some View {
        HStack(spacing: 8) {
            Button("Seitenleiste", systemImage: "sidebar.left") { model.sidebarVisible.toggle() }
            Divider().frame(height: 16)
            Picker("Werkzeug", selection: $model.tool) {
                Image(systemName: "character.cursor.ibeam").tag(PDFTool.select).help("Text auswählen")
                Image(systemName: "pencil.tip").tag(PDFTool.ink).help("Freihand")
                Image(systemName: "text.bubble").tag(PDFTool.note).help("Kommentar setzen")
                Image(systemName: "photo.badge.plus").tag(PDFTool.figure).help("Bereich als Bild in eine Notiz")
                Image(systemName: "function").tag(PDFTool.latex).help("Formel als LaTeX erkennen")
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .frame(width: 180)
            ForEach(PDFModel.colors, id: \.self) { hex in
                Circle()
                    .fill(Color(nsColor: NSColor(atlasHex: hex)))
                    .frame(width: 14, height: 14)
                    .overlay(Circle().stroke(.primary, lineWidth: model.color == hex ? 2 : 0))
                    .onTapGesture { model.color = hex }
                    .accessibilityLabel("Farbe \(hex)")
            }
            Button("Markieren", systemImage: "highlighter") { model.mark(.highlight) }
                .help("Auswahl markieren (⇧⌘H)")
            Button("Unterstreichen", systemImage: "underline") { model.mark(.underline) }
            Button("Durchstreichen", systemImage: "strikethrough") { model.mark(.strikeout) }
            Spacer()
            TextField("Im PDF suchen", text: $model.searchText)
                .textFieldStyle(.roundedBorder)
                .frame(maxWidth: 200)
                .onSubmit { model.search() }
            if !model.searchHits.isEmpty {
                Text("\(model.searchHits.count) Treffer").font(.caption).foregroundStyle(.secondary)
            }
            Toggle(isOn: $model.dark) { Image(systemName: "moon") }
                .toggleStyle(.button)
                .help("Dunkle Seiten")
            Button("Präsentieren", systemImage: "play.rectangle") { PDFPresentation.show(model) }
            Button("Exportieren", systemImage: "square.and.arrow.up") { exportCopy() }
                .help("Kopie mit Anmerkungen speichern")
        }
        .labelStyle(.iconOnly)
        .buttonStyle(.borderless)
        .padding(.horizontal, 8)
        .padding(.vertical, 5)
    }

    private func exportCopy() {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.pdf]
        let suggested = URL(fileURLWithPath: model.defaultExportPath)
        panel.directoryURL = suggested.deletingLastPathComponent()
        panel.nameFieldStringValue = suggested.lastPathComponent
        if panel.runModal() == .OK, let url = panel.url, !model.export(to: url) {
            model.error = model.error ?? "Export fehlgeschlagen"
        }
    }
}

extension Notification.Name {
    /// A comment was pinned (object: its Atlas id); the view opens it.
    static let atlasPDFNoteAdded = Notification.Name("atlasPDFNoteAdded")
}

/// PDFKit's thumbnail view next to the document.
private struct PDFThumbnails: NSViewRepresentable {
    let model: PDFModel

    func makeNSView(context: NSViewRepresentableContext<Self>) -> PDFThumbnailView {
        let view = PDFThumbnailView()
        view.thumbnailSize = CGSize(width: 120, height: 150)
        view.backgroundColor = .clear
        return view
    }

    func updateNSView(_ view: PDFThumbnailView, context: NSViewRepresentableContext<Self>) {
        view.pdfView = PDFKitView.registry[ObjectIdentifier(model)]
    }
}

/// The PDFView (PDFKit) of a model, with an overlay for freehand and notes.
struct PDFKitView: NSViewRepresentable {
    let model: PDFModel
    let annotationClicked: (String) -> Void

    /// PDFViews by model, so the sidebar and presentation find theirs.
    @MainActor static var registry: [ObjectIdentifier: PDFView] = [:]

    func makeCoordinator() -> Coordinator { Coordinator(annotationClicked: annotationClicked) }

    func makeNSView(context: NSViewRepresentableContext<Self>) -> NSView {
        let container = NSView()
        let view = PDFView()
        view.document = model.document
        view.autoScales = true
        view.displayMode = .singlePageContinuous
        view.displaysPageBreaks = true
        view.backgroundColor = .windowBackgroundColor
        let overlay = PDFDrawingOverlay(pdfView: view, model: model)
        for v in [view, overlay as NSView] {
            v.translatesAutoresizingMaskIntoConstraints = false
            container.addSubview(v)
            NSLayoutConstraint.activate([
                v.leadingAnchor.constraint(equalTo: container.leadingAnchor),
                v.trailingAnchor.constraint(equalTo: container.trailingAnchor),
                v.topAnchor.constraint(equalTo: container.topAnchor),
                v.bottomAnchor.constraint(equalTo: container.bottomAnchor),
            ])
        }
        context.coordinator.pdfView = view
        context.coordinator.overlay = overlay
        model.currentSelection = { [weak view] in view?.currentSelection }
        Self.registry[ObjectIdentifier(model)] = view
        NotificationCenter.default.addObserver(
            context.coordinator, selector: #selector(Coordinator.annotationHit(_:)),
            name: .PDFViewAnnotationHit, object: view)
        return container
    }

    func updateNSView(_ container: NSView, context: NSViewRepresentableContext<Self>) {
        guard let view = context.coordinator.pdfView else { return }
        _ = model.revision  // redraw after annotation changes
        view.highlightedSelections = model.searchHits.isEmpty ? nil : model.searchHits
        if let first = model.searchHits.first, context.coordinator.lastSearch != model.searchText {
            context.coordinator.lastSearch = model.searchText
            view.go(to: first)
        }
        context.coordinator.overlay?.isHidden = model.tool == .select
        PDFDarkMode.apply(model.dark, to: view)
        view.needsDisplay = true
    }

    static func dismantleNSView(_ container: NSView, coordinator: Coordinator) {
        NotificationCenter.default.removeObserver(coordinator)
        if let view = coordinator.pdfView { registry = registry.filter { $0.value !== view } }
    }

    @MainActor
    final class Coordinator: NSObject {
        let annotationClicked: (String) -> Void
        var lastSearch = ""
        weak var pdfView: PDFView?
        weak var overlay: PDFDrawingOverlay?

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

/// Dark pages: invert, then turn the hue back so colours stay recognisable
/// (close to the Linux viewer's dark paper).
enum PDFDarkMode {
    @MainActor
    static func apply(_ on: Bool, to view: PDFView) {
        guard let content = view.documentView else { return }
        content.wantsLayer = true
        if on {
            let invert = CIFilter(name: "CIColorInvert")
            let hue = CIFilter(name: "CIHueAdjust")
            hue?.setValue(Float.pi, forKey: kCIInputAngleKey)
            content.layer?.filters = [invert, hue].compactMap { $0 }
        } else {
            content.layer?.filters = nil
        }
    }
}

/// Catches mouse input for the freehand and note tools (hidden while
/// selecting, so PDFKit's own text selection works).
@MainActor
final class PDFDrawingOverlay: NSView {
    weak var pdfView: PDFView?
    let model: PDFModel
    private var stroke: [CGPoint] = []
    private var strokePage: PDFPage?
    /// Start of the box of `figure`/`latex` (page space).
    private var boxStart: CGPoint?
    private let shape = CAShapeLayer()

    init(pdfView: PDFView, model: PDFModel) {
        self.pdfView = pdfView
        self.model = model
        super.init(frame: .zero)
        wantsLayer = true
        shape.fillColor = nil
        shape.lineCap = .round
        shape.lineJoin = .round
        layer?.addSublayer(shape)
        isHidden = true
    }

    required init?(coder: NSCoder) { fatalError("not used") }

    override func resetCursorRects() {
        addCursorRect(bounds, cursor: .crosshair)
    }

    override func mouseDown(with event: NSEvent) {
        guard let pdfView else { return }
        let inView = pdfView.convert(event.locationInWindow, from: nil)
        guard let page = pdfView.page(for: inView, nearest: true), let document = pdfView.document else { return }
        let onPage = pdfView.convert(inView, to: page)
        switch model.tool {
        case .note:
            if let id = model.addNote(page: document.index(for: page), at: onPage) {
                NotificationCenter.default.post(name: .atlasPDFNoteAdded, object: id)
            }
        case .ink:
            strokePage = page
            stroke = [onPage]
            shape.strokeColor = NSColor(atlasHex: model.color).cgColor
            shape.lineWidth = CGFloat(model.inkWidth) * pdfView.scaleFactor
            shape.lineDashPattern = nil
            redraw()
        case .figure, .latex:
            strokePage = page
            boxStart = onPage
            stroke = [onPage, onPage]
            shape.strokeColor = NSColor.controlAccentColor.cgColor
            shape.lineWidth = 1.5
            shape.lineDashPattern = [5, 3]
            redraw()
        case .select:
            break
        }
    }

    override func mouseDragged(with event: NSEvent) {
        guard let pdfView, let page = strokePage else { return }
        let inView = pdfView.convert(event.locationInWindow, from: nil)
        let onPage = pdfView.convert(inView, to: page)
        switch model.tool {
        case .ink:
            stroke.append(onPage)
        case .figure, .latex:
            if let start = boxStart {
                stroke = [start, CGPoint(x: onPage.x, y: start.y), onPage, CGPoint(x: start.x, y: onPage.y), start]
            }
        default:
            return
        }
        redraw()
    }

    override func mouseUp(with event: NSEvent) {
        if model.tool == .figure || model.tool == .latex {
            defer { boxStart = nil; strokePage = nil; stroke = []; shape.path = nil }
            guard let start = boxStart, let page = strokePage, let document = pdfView?.document, stroke.count > 2 else { return }
            let end = stroke[2]
            let rect = CGRect(x: min(start.x, end.x), y: min(start.y, end.y),
                              width: abs(end.x - start.x), height: abs(end.y - start.y))
            if rect.width > 4, rect.height > 4 {
                model.chosenArea = PDFArea(page: document.index(for: page), rect: rect, tool: model.tool)
            }
            return
        }
        guard model.tool == .ink, let page = strokePage, let document = pdfView?.document else { return }
        if stroke.count > 1 { model.addInk(page: document.index(for: page), strokes: [stroke]) }
        stroke = []
        strokePage = nil
        shape.path = nil
    }

    /// The stroke being drawn, in this view's coordinates.
    private func redraw() {
        guard let pdfView, let page = strokePage, let first = stroke.first else { return }
        let path = CGMutablePath()
        let toSelf = { (p: CGPoint) in self.convert(pdfView.convert(p, from: page), from: pdfView) }
        path.move(to: toSelf(first))
        for p in stroke.dropFirst() { path.addLine(to: toSelf(p)) }
        shape.path = path
    }
}

/// Full-screen presentation: one page, arrow keys / space to move, Esc ends.
@MainActor
enum PDFPresentation {
    private static var window: NSWindow?
    private static var view: PDFView?
    private static var monitor: Any?

    static func show(_ model: PDFModel) {
        guard let document = model.document else { return }
        let v = PDFView()
        v.document = document
        v.displayMode = .singlePage
        v.autoScales = true
        v.backgroundColor = .black
        if let current = PDFKitView.registry[ObjectIdentifier(model)]?.currentPage { v.go(to: current) }
        let w = NSWindow(contentRect: NSScreen.main?.frame ?? .zero, styleMask: [.titled, .closable, .resizable],
                         backing: .buffered, defer: false)
        w.title = "Präsentation"
        w.contentView = v
        w.isReleasedWhenClosed = false
        w.makeKeyAndOrderFront(nil)
        w.toggleFullScreen(nil)
        window = w
        view = v
        if monitor == nil {
            monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
                // Only plain values cross into the main actor (NSEvent is not Sendable);
                // local monitors run on the main thread anyway.
                let code = event.keyCode
                let number = event.window?.windowNumber
                let consumed = MainActor.assumeIsolated { handleKey(code, window: number) }
                return consumed ? nil : event
            }
        }
    }

    /// Whether the key was handled (then it does not reach the window).
    private static func handleKey(_ code: UInt16, window number: Int?) -> Bool {
        guard let w = window, let v = view, number == w.windowNumber else { return false }
        switch code {
        case 53:  // Esc
            w.close()
            window = nil
            view = nil
        case 124, 125, 49:  // →, ↓, space
            v.goToNextPage(nil)
        case 123, 126:  // ←, ↑
            v.goToPreviousPage(nil)
        default:
            return false
        }
        return true
    }
}

/// The recognised formula: check, edit, copy (Linux: the LaTeX popup).
private struct LatexResultView: View {
    @Bindable var model: PDFModel
    @Environment(\.dismiss) private var dismiss
    @State private var text = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Formel als LaTeX").font(.headline)
            if model.latexBusy {
                HStack {
                    ProgressView().controlSize(.small)
                    Text("Erkenne … Beim ersten Mal lädt Atlas das Formelmodell (etwa 170 MB).")
                        .font(.callout).foregroundStyle(.secondary)
                }
            } else if let error = model.latexError {
                Text("Formel nicht erkannt: \(error)").foregroundStyle(.red)
            } else {
                TextEditor(text: $text)
                    .font(.system(.body, design: .monospaced))
                    .frame(minHeight: 80)
                Text("Als $…$ kopiert – bitte prüfen.").font(.caption).foregroundStyle(.secondary)
            }
            HStack {
                Spacer()
                if model.latex != nil {
                    Button("Kopieren") {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString("$\(text)$", forType: .string)
                        dismiss()
                    }
                }
                Button("Schließen") { dismiss() }.keyboardShortcut(.cancelAction)
            }
        }
        .padding(16)
        .frame(width: 460)
        .onChange(of: model.latex, initial: true) { text = model.latex ?? "" }
    }
}
