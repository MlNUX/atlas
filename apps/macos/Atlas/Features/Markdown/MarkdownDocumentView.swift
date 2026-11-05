// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

extension FocusedValues {
    /// The Markdown file of the key window's active tab.
    @Entry var markdown: MarkdownModel?
}

/// A Markdown file in a tab (P3.3; Linux: `MarkdownView.qml`): editor,
/// preview, or both side by side.
struct MarkdownDocumentView: View {
    let project: ProjectModel
    let path: String
    let line: Int
    @State private var model: MarkdownModel?

    var body: some View {
        Group {
            if let model {
                MarkdownContent(model: model)
                    .focusedSceneValue(\.markdown, model)
            } else {
                ProgressView()
            }
        }
        .task(id: path) {
            model?.close()
            let m = MarkdownModel(project: project, path: path, line: line)
            model = m
            await m.load()
        }
        .onDisappear { model?.close() }
    }
}

private struct MarkdownContent: View {
    @Bindable var model: MarkdownModel

    var body: some View {
        VStack(spacing: 0) {
            toolbar
            if let error = model.error {
                ContentUnavailableView {
                    Label("Nicht lesbar", systemImage: "exclamationmark.triangle")
                } description: {
                    Text(error)
                }
            } else {
                if model.conflict {
                    HStack {
                        Text("Die Datei wurde außerhalb von Atlas geändert.").font(.callout)
                        Spacer()
                        Button("Neu laden") { model.reloadFromDisk() }
                        Button("Meine Fassung sichern") { model.save() }
                    }
                    .padding(8)
                    .background(Color.orange.opacity(0.18))
                }
                content
            }
        }
    }

    @ViewBuilder private var content: some View {
        switch model.mode {
        case .edit:
            TextEditorView(controller: model.editor)
        case .preview:
            if let preview = model.preview { MarkdownPreviewView(preview: preview) }
        case .split:
            HSplitView {
                TextEditorView(controller: model.editor).frame(minWidth: 200)
                if let preview = model.preview { MarkdownPreviewView(preview: preview).frame(minWidth: 200) }
            }
        }
    }

    private var toolbar: some View {
        HStack(spacing: 10) {
            Picker("Ansicht", selection: $model.mode) {
                ForEach(MarkdownModel.Mode.allCases, id: \.self) { Text($0.title).tag($0) }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .fixedSize()
            InsertMenu(model: model)
            Spacer()
            if let info = model.info, info.brokenLinks > 0 {
                Label("\(info.brokenLinks) defekte Links", systemImage: "link.badge.plus")
                    .foregroundStyle(.orange)
                    .font(.callout)
                    .help("Links, deren Ziel es nicht gibt (in der Vorschau markiert)")
            }
            if model.modified {
                Text("geändert").font(.caption).foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        .background(.bar)
    }
}

/// „Einfügen“: table, diagram, folder tree, pictures and files.
struct InsertMenu: View {
    let model: MarkdownModel

    var body: some View {
        Menu {
            InsertItems(model: model)
        } label: {
            Label("Einfügen", systemImage: "plus")
        }
        .fixedSize()
    }
}

struct InsertItems: View {
    let model: MarkdownModel

    var body: some View {
        Button("Tabelle") { model.insertTable() }
        Menu("Diagramm") {
            ForEach(MarkdownModel.diagrams.indices, id: \.self) { i in
                Button(MarkdownModel.diagrams[i].name) { model.insertDiagram(i) }
            }
        }
        Button("Ordnerbaum …") { model.insertFolderTree() }
        Button("Bild oder Datei …") { model.insertFiles() }
    }
}
