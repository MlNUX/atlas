// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import SwiftUI

extension FocusedValues {
    /// The text or code file of the key window's active tab.
    @Entry var code: CodeModel?
}

/// A text or code file in a tab (P3.4).
struct CodeDocumentView: View {
    let project: ProjectModel
    let path: String
    let line: Int
    @State private var model: CodeModel?

    var body: some View {
        Group {
            if let model {
                CodeContent(model: model)
                    .focusedSceneValue(\.code, model)
            } else {
                ProgressView()
            }
        }
        .task(id: path) {
            model?.close()
            let m = CodeModel(project: project, path: path, line: line)
            model = m
            await m.load()
        }
        .onDisappear { model?.close() }
    }
}

private struct CodeContent: View {
    @Bindable var model: CodeModel

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
                HStack(spacing: 0) {
                    TextEditorView(controller: model.editor)
                    if model.symbolsVisible {
                        Divider()
                        SymbolList(model: model).frame(width: 220)
                    }
                }
            }
        }
        .sheet(isPresented: $model.goToLineVisible) { GoToLineSheet(model: model) }
    }

    private var toolbar: some View {
        HStack(spacing: 10) {
            Text(model.language ?? "Text")
                .font(.callout)
                .foregroundStyle(.secondary)
            Spacer()
            if model.modified {
                Text("geändert").font(.caption).foregroundStyle(.secondary)
            }
            if model.runnable {
                Button("Ausführen", systemImage: "play.fill") { model.run() }
                    .help("Skript im Terminal ausführen")
            }
            Button("Gehe zu Zeile", systemImage: "arrow.down.to.line") { model.goToLineVisible = true }
                .labelStyle(.iconOnly)
                .help("Gehe zu Zeile (⌘L)")
            Toggle(isOn: $model.symbolsVisible) {
                Label("Symbole", systemImage: "list.bullet.indent")
            }
            .toggleStyle(.button)
            .labelStyle(.iconOnly)
            .help("Symbole der Datei")
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        .background(.bar)
    }
}

/// Functions, types and headings of the file; a click goes there.
private struct SymbolList: View {
    let model: CodeModel

    var body: some View {
        if model.symbols.isEmpty {
            ContentUnavailableView("Keine Symbole", systemImage: "list.bullet.indent",
                                   description: Text("Für diese Datei erkennt Atlas keine Symbole."))
        } else {
            List(Array(model.symbols.enumerated()), id: \.offset) { _, symbol in
                Button {
                    model.go(toLine: Int(symbol.line))
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: Self.icon(symbol.kind)).foregroundStyle(.secondary)
                        Text(symbol.name).lineLimit(1)
                        Spacer()
                        Text("\(symbol.line)").font(.caption.monospacedDigit()).foregroundStyle(.tertiary)
                    }
                    .padding(.leading, CGFloat(symbol.level) * 12)
                }
                .buttonStyle(.plain)
            }
            .listStyle(.sidebar)
        }
    }

    static func icon(_ kind: String) -> String {
        switch kind {
        case "heading": "number"
        case "function", "method": "function"
        case "class", "struct", "type", "enum", "interface", "trait": "cube"
        case "module", "namespace", "impl": "shippingbox"
        case "constant", "variable", "field": "textformat.abc"
        default: "circle.fill"
        }
    }
}

/// „Gehe zu Zeile“ (⌘L; Linux: Strg+G).
private struct GoToLineSheet: View {
    let model: CodeModel
    @State private var text = ""
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Gehe zu Zeile").font(.headline)
            TextField("Zeile (1–\(model.lineCount))", text: $text)
                .textFieldStyle(.roundedBorder)
                .frame(width: 240)
                .onSubmit(go)
            HStack {
                Spacer()
                Button("Abbrechen") { dismiss() }.keyboardShortcut(.cancelAction)
                Button("Gehe zu") { go() }.keyboardShortcut(.defaultAction)
                    .disabled(Int(text.trimmingCharacters(in: .whitespaces)) == nil)
            }
        }
        .padding(20)
        .onAppear { text = "\(model.currentLine)" }
    }

    private func go() {
        guard let line = Int(text.trimmingCharacters(in: .whitespaces)) else { return }
        dismiss()
        model.go(toLine: min(max(1, line), model.lineCount))
    }
}
