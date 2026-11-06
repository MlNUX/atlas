// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import SwiftUI

/// Files without an editor of their own (P3.12; the router in
/// `App/FeatureRouter.swift` calls it): pictures, Atlas notebooks, other
/// files and missing ones.
struct OtherDocumentView: View {
    let project: ProjectModel
    let path: String

    var body: some View {
        switch project.core.files().fileKind(path: path) {
        case "image": ImageDocumentView(project: project, path: path)
        case "atlasnote": AtlasNoteDocumentView(project: project, path: path)
        case "missing": MissingFileView(project: project, path: path)
        default: BinaryFileView(project: project, path: path)
        }
    }
}

/// The file is gone (moved, deleted, not synced yet).
private struct MissingFileView: View {
    let project: ProjectModel
    let path: String

    var body: some View {
        ContentUnavailableView {
            Label("Datei nicht gefunden", systemImage: "questionmark.folder")
        } description: {
            Text(project.core.relativePath(path: path) ?? path)
        } actions: {
            Button("Tab schließen") { project.editor.closeCurrentTab() }
        }
    }
}

/// A file Atlas does not show: size, kind, open with the system's app.
private struct BinaryFileView: View {
    let project: ProjectModel
    let path: String

    var body: some View {
        let url = URL(filePath: path)
        let props = try? project.core.files().properties(path: path)
        ContentUnavailableView {
            Label {
                Text(url.lastPathComponent)
            } icon: {
                Image(nsImage: NSWorkspace.shared.icon(forFile: path))
            }
        } description: {
            Text([props?.kind, props?.size.map { ByteCountFormatter.string(fromByteCount: Int64($0), countStyle: .file) }]
                .compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · "))
            Text("Atlas zeigt diese Datei nicht selbst an.")
        } actions: {
            HStack {
                Button("Mit Standard-App öffnen") { NSWorkspace.shared.open(url) }
                Button("Im Finder zeigen") { NSWorkspace.shared.activateFileViewerSelecting([url]) }
                Button("Als Text öffnen") { project.editor.open(path: path, mode: .tab, processor: "atlas:source") }
            }
        }
    }
}
