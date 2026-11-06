// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import SwiftUI

/// An Atlas notebook from the tablet (`.atlasnote`, P3.12; Linux:
/// `AtlasNoteView.qml`): its pages one below the other, zoomable, read only.
/// The core reads the file (`AtlasNote`), `NoteRenderer` draws the pages.
struct AtlasNoteDocumentView: View {
    let project: ProjectModel
    let path: String
    @State private var note: AtlasNote?
    @State private var pages: [NotePage] = []
    @State private var error: String?
    @State private var zoom: CGFloat = 1
    @State private var revision = 0

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                Image(systemName: "pencil.and.scribble").foregroundStyle(.tint)
                Text("Atlas-Notiz · \(pages.count) Seiten").font(.headline)
                Text("Nur Ansicht – bearbeitet wird auf dem Tablet").font(.caption).foregroundStyle(.secondary)
                Spacer()
                Button("Verkleinern", systemImage: "minus.magnifyingglass") { zoom = max(0.25, zoom / 1.25) }
                Text("\(Int((zoom * 100).rounded())) %").font(.caption.monospacedDigit()).frame(width: 44)
                Button("Vergrößern", systemImage: "plus.magnifyingglass") { zoom = min(4, zoom * 1.25) }
                Button("Neu laden", systemImage: "arrow.clockwise") { revision += 1 }
            }
            .labelStyle(.iconOnly)
            .buttonStyle(.borderless)
            .padding(.horizontal, 10).padding(.vertical, 6)
            Divider()
            if let error {
                ContentUnavailableView("Die Atlas-Notiz kann nicht angezeigt werden: \(error)", systemImage: "exclamationmark.triangle")
            } else if let note {
                GeometryReader { geo in
                    let width = max(200, (geo.size.width - 40) * zoom)
                    ScrollView([.vertical, .horizontal]) {
                        LazyVStack(spacing: 16) {
                            ForEach(Array(pages.enumerated()), id: \.offset) { i, page in
                                NotePageView(page: page, pageWidth: note.pageWidth(), width: width, index: i + 1)
                            }
                        }
                        .padding(20)
                        .frame(minWidth: geo.size.width)
                    }
                }
                .background(Color(nsColor: .underPageBackgroundColor))
            } else {
                ProgressView()
            }
        }
        .task(id: "\(path)#\(revision)") { load() }
        .onChange(of: project.filesRevision) {
            if project.changedDirs.contains((path as NSString).deletingLastPathComponent) { revision += 1 }
        }
    }

    private func load() {
        do {
            let n = try AtlasNote.open(path: path)
            note = n
            pages = n.pages()
            error = nil
        } catch {
            self.error = atlasMessage(error)
        }
    }
}

/// One page, drawn for its width on screen (Retina pixels).
private struct NotePageView: View {
    let page: NotePage
    let pageWidth: Double
    let width: CGFloat
    let index: Int
    @State private var image: CGImage?
    @Environment(\.displayScale) private var displayScale

    var body: some View {
        let height = width * page.height / pageWidth
        ZStack(alignment: .bottomTrailing) {
            if let image {
                Image(decorative: image, scale: displayScale).resizable()
            } else {
                Rectangle().fill(Color(nsColor: .textBackgroundColor))
            }
            Text("\(index)").font(.caption2).foregroundStyle(.secondary).padding(6)
        }
        .frame(width: width, height: height)
        .shadow(color: .black.opacity(0.2), radius: 3, y: 1)
        .task(id: Int(width * displayScale)) {
            // Rendering once per width; a short pause while zooming.
            try? await Task.sleep(for: .milliseconds(60))
            guard !Task.isCancelled else { return }
            image = NoteRenderer.image(of: page, pageWidth: pageWidth, widthPx: Int(width * displayScale))
        }
    }
}
