// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import SwiftUI

/// A file shown through a processor of an extension (P3.10; Linux:
/// `ProcessedView.qml`): Markdown, text, or a produced PDF.
struct ProcessedView: View {
    let project: ProjectModel
    let processor: String
    let path: String
    @State private var result: Processed?
    @State private var error: String?

    var body: some View {
        Group {
            if let error {
                ContentUnavailableView {
                    Label("Anzeige fehlgeschlagen", systemImage: "exclamationmark.triangle")
                } description: {
                    Text(error)
                } actions: {
                    Button("Erneut versuchen") { Task { await run() } }
                }
            } else if let result {
                switch result.kind {
                case .pdf:
                    PDFDocumentView(project: project, path: result.content)
                case .markdown:
                    ScrollView {
                        Text((try? AttributedString(markdown: result.content, options: .init(
                            interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(result.content))
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(16)
                    }
                case .text, .view:
                    ScrollView {
                        Text(result.content)
                            .font(.system(.body, design: .monospaced))
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(12)
                    }
                }
            } else {
                ProgressView("Verarbeite …")
            }
        }
        .task(id: processor + "\n" + path) { await run() }
        .onChange(of: project.filesRevision) {
            // Only when its folder changed (the file may have).
            if project.changedDirs.contains((path as NSString).deletingLastPathComponent) { Task { await run() } }
        }
    }

    private func run() async {
        do {
            result = try await project.core.extensions().process(id: processor, path: path)
            error = nil
        } catch {
            self.error = atlasMessage(error)
        }
    }
}

/// Files without a viewer of their own: an automatic processor or view of
/// an extension if one fits, else the plain viewer.
struct ProcessedOrOtherView: View {
    let project: ProjectModel
    let path: String

    var body: some View {
        let _ = project.actions.revision
        if let auto = project.actions.processors(for: path).first(where: \.auto) {
            if auto.output == .view {
                ExtensionPageView(project: project, viewId: auto.id, source: path)
            } else {
                ProcessedView(project: project, processor: auto.id, path: path)
            }
        } else {
            OtherDocumentView(project: project, path: path)
        }
    }
}
