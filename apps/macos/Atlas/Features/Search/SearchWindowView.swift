// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import SwiftUI

/// „Im Projekt suchen“ (P3.8): text files and PDFs, type filter, options,
/// matches by file with context, opens at the match.
struct SearchWindowView: View {
    let project: ProjectModel
    @State private var model: SearchModel?
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        Group {
            if let model {
                SearchContent(model: model) { match in
                    model.open(match)
                    dismiss()
                }
            }
        }
        .frame(minWidth: 760, minHeight: 520)
        .onAppear {
            let m = SearchModel(project: project)
            // CI and UI tests: `-AtlasSearch <pattern>` searches at once.
            if let pattern = UserDefaults.standard.string(forKey: "AtlasSearch") {
                m.pattern = pattern
                m.start()
            }
            model = m
        }
        .onDisappear { model?.stop() }
    }
}

private struct SearchContent: View {
    @Bindable var model: SearchModel
    let open: (SearchMatch) -> Void
    @FocusState private var focused: Bool

    var body: some View {
        VStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    TextField("Im Projekt suchen", text: $model.pattern)
                        .textFieldStyle(.roundedBorder)
                        .font(.title3)
                        .focused($focused)
                        .onSubmit { model.start() }
                    if model.running {
                        ProgressView().controlSize(.small)
                        Button("Abbrechen") { model.cancel() }
                    } else {
                        Button("Suchen") { model.start() }.keyboardShortcut(.defaultAction)
                    }
                }
                HStack(spacing: 14) {
                    Toggle("Regulärer Ausdruck", isOn: $model.regex)
                    Toggle("Groß/Klein", isOn: $model.caseSensitive)
                    Toggle("Ganzes Wort", isOn: $model.wholeWord)
                    Toggle("Build-Ordner", isOn: $model.buildDirs)
                        .help("Auch target/, node_modules/ … durchsuchen")
                    TextField("Dateien (z. B. *.md, !alt/**)", text: $model.globs)
                        .textFieldStyle(.roundedBorder)
                        .frame(maxWidth: 240)
                }
                .toggleStyle(.checkbox)
                .font(.callout)
                HStack(spacing: 6) {
                    ForEach(SearchModel.kinds, id: \.id) { kind in
                        let on = model.kinds.contains(kind.id)
                        Button {
                            if on { model.kinds.remove(kind.id) } else { model.kinds.insert(kind.id) }
                            if !model.pattern.isEmpty { model.start() }
                        } label: {
                            let n = model.count(of: kind.id)
                            Text(n > 0 ? "\(kind.title) \(n)" : kind.title)
                        }
                        .buttonStyle(.bordered)
                        .tint(on ? .accentColor : .secondary)
                    }
                    Spacer()
                    if let s = model.summary {
                        Text("\(s.matches) Treffer in \(s.files) Dateien · \(s.elapsedMs) ms\(s.truncated ? " · gekürzt" : "")")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }
                if let error = model.error {
                    Text(error).foregroundStyle(.red).font(.callout)
                }
            }
            .padding(12)
            Divider()
            HSplitView {
                List(selection: Binding(get: { model.selected?.id }, set: { _ in })) {
                    ForEach(model.groups, id: \.path) { group in
                        Section {
                            ForEach(group.matches, id: \.id) { m in
                                HStack(alignment: .firstTextBaseline, spacing: 8) {
                                    Text(m.pdf ? "S. \(m.line)" : "\(m.line)")
                                        .font(.caption.monospacedDigit())
                                        .foregroundStyle(.secondary)
                                        .frame(width: 44, alignment: .trailing)
                                    Text(m.attributed).lineLimit(1)
                                }
                                .contentShape(Rectangle())
                                .onTapGesture { model.select(m) }
                                .onTapGesture(count: 2) { open(m) }
                                .tag(m.id)
                            }
                        } header: {
                            Label(group.relative, systemImage: group.matches.first?.pdf == true ? "doc.richtext" : "doc.text")
                        }
                    }
                }
                .frame(minWidth: 380)
                VStack(alignment: .leading, spacing: 0) {
                    if let m = model.selected {
                        HStack {
                            Text(m.relative).font(.headline).lineLimit(1)
                            Spacer()
                            Button("Öffnen") { open(m) }
                        }
                        .padding(8)
                        Divider()
                        ScrollView {
                            VStack(alignment: .leading, spacing: 2) {
                                ForEach(model.context, id: \.line) { l in
                                    HStack(alignment: .firstTextBaseline) {
                                        Text("\(l.line)").font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                                            .frame(width: 36, alignment: .trailing)
                                        Text(l.text)
                                            .fontWeight(!m.pdf && l.line == m.line ? .semibold : .regular)
                                    }
                                }
                            }
                            .font(.system(.body, design: .monospaced))
                            .padding(8)
                            .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    } else {
                        ContentUnavailableView("Treffer wählen", systemImage: "text.magnifyingglass",
                                               description: Text("Doppelklick öffnet die Datei an der Stelle"))
                    }
                }
                .frame(minWidth: 260)
            }
        }
        .onAppear { focused = true }
    }
}

extension SearchMatch: @retroactive Identifiable {
    public var id: String { "\(path)#\(line)#\(ranges.first?.start ?? 0)" }
}
