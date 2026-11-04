// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import SwiftUI

/// Shown while a window has no project: open a folder or a recent project.
struct WelcomeView: View {
    let app: AppModel
    let open: (String) -> Void
    @State private var choosing = false

    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: "books.vertical")
                .font(.system(size: 48))
                .foregroundStyle(.tint)
            Text("Atlas").font(.largeTitle.bold())
            Button("Projektordner öffnen …") { choosing = true }
                .buttonStyle(.glassProminent)
                .keyboardShortcut("o")
            let recent = app.knownProjects()
            if !recent.isEmpty {
                List(recent, id: \.root) { project in
                    Button {
                        open(project.root)
                    } label: {
                        VStack(alignment: .leading) {
                            Text(project.name)
                            Text(project.root).font(.caption).foregroundStyle(.secondary)
                        }
                    }
                    .buttonStyle(.plain)
                }
                .frame(maxWidth: 420, maxHeight: 260)
            }
            if let error = app.error {
                Text(error).foregroundStyle(.red)
            }
        }
        .padding(40)
        .frame(minWidth: 520, minHeight: 420)
        .fileImporter(isPresented: $choosing, allowedContentTypes: [.folder]) { result in
            if case .success(let url) = result { open(url.path) }
        }
    }
}

/// ⌘P: fuzzy file search over the project (core: `Files.quickOpen`).
struct QuickOpenView: View {
    let project: ProjectModel
    @State private var query = ""
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let hits = query.isEmpty ? [] : project.core.files().quickOpen(query: query, limit: 30)
        VStack(spacing: 0) {
            TextField("Datei suchen (name:zeile)", text: $query)
                .textFieldStyle(.plain)
                .font(.title3)
                .padding(12)
                .onSubmit {
                    if let first = hits.first { choose(first) }
                }
            Divider()
            List(hits, id: \.path) { hit in
                Button(hit.relative) { choose(hit) }.buttonStyle(.plain)
            }
        }
        .frame(width: 520, height: 380)
    }

    private func choose(_ hit: QuickOpenHit) {
        project.editor.open(path: hit.path, line: Int(hit.line), mode: .tab)
        dismiss()
    }
}
