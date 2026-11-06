// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import SwiftUI

extension View {
    /// The terminal panel below this view (the editor area of the window).
    func terminalPanel(_ project: ProjectModel) -> some View {
        modifier(TerminalPanelHost(project: project))
    }
}

/// Editor area above, terminal panel below (Linux: `Main.qml`).
private struct TerminalPanelHost: ViewModifier {
    let project: ProjectModel

    func body(content: Content) -> some View {
        let model = TerminalModel.of(project)
        VSplitView {
            content
                .frame(minHeight: 160)
            if model.visible {
                TerminalPanelView(model: model)
                    .frame(minHeight: 80, idealHeight: model.height)
                    .onGeometryChange(for: Double.self) { $0.size.height } action: { h in
                        if abs(h - model.height) >= 1 { model.height = h }
                    }
            }
        }
        .sheet(item: Binding(get: { model.sendToNote }, set: { model.sendToNote = $0 })) { item in
            SendToNoteSheet(project: project, model: model, item: item)
        }
    }
}

/// Tabs of the panel with their split terminals (Linux: `TerminalPanel.qml`).
struct TerminalPanelView: View {
    let model: TerminalModel

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 4) {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 4) {
                        ForEach(Array(model.groups.enumerated()), id: \.element.id) { index, group in
                            let selected = index == model.current
                            Button { model.selectGroup(index) } label: {
                                Label(group.title, systemImage: "terminal")
                                    .labelStyle(.titleAndIcon)
                                    .lineLimit(1)
                                    .padding(.horizontal, 8).padding(.vertical, 3)
                                    .background(selected ? Color.secondary.opacity(0.18) : .clear, in: .rect(cornerRadius: 6))
                            }
                            .buttonStyle(.plain)
                            .foregroundStyle(selected ? .primary : .secondary)
                        }
                    }
                }
                Button("Neues Terminal", systemImage: "plus") { model.newTerminal() }
                    .help("Neues Terminal (⌃⇧`)")
                Spacer(minLength: 8)
                Button("Rechts teilen", systemImage: "rectangle.split.2x1") { model.split(vertical: false) }
                    .help("Terminal rechts teilen")
                Button("Unten teilen", systemImage: "rectangle.split.1x2") { model.split(vertical: true) }
                    .help("Terminal unten teilen")
                Button("Terminal beenden", systemImage: "trash") { model.killActive() }
                    .help("Terminal beenden")
                    .disabled(model.currentGroup?.active == nil)
                Button("Panel ausblenden", systemImage: "chevron.down") { model.hide() }
                    .help("Panel ausblenden (⌃`)")
            }
            .labelStyle(.iconOnly)
            .buttonStyle(.borderless)
            .padding(.horizontal, 8).padding(.vertical, 4)
            .background(.bar)
            Divider()
            ZStack {
                // All tabs stay in the view tree, so their terminals keep
                // their views; only the current one is visible.
                ForEach(Array(model.groups.enumerated()), id: \.element.id) { index, group in
                    TerminalSplitView(model: model, group: group, split: group.root)
                        .opacity(index == model.current ? 1 : 0)
                        .allowsHitTesting(index == model.current)
                }
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Terminal-Panel")
    }
}

/// A split of terminals (recursive).
private struct TerminalSplitView: View {
    let model: TerminalModel
    let group: TerminalGroup
    let split: TerminalSplit

    var body: some View {
        if split.vertical {
            VSplitView { children }
        } else {
            HSplitView { children }
        }
    }

    private var children: some View {
        ForEach(split.items) { item in
            switch item {
            case let .terminal(session):
                TerminalHostView(session: session, active: group.active === session && group.sessions.count > 1)
                    .frame(minWidth: 120, minHeight: 60)
            case let .split(inner):
                AnyView(TerminalSplitView(model: model, group: group, split: inner))
            }
        }
    }
}

/// Shows a session's terminal view; the view itself belongs to the session
/// and moves between hosts (tab dragged, split changed).
struct TerminalHostView: NSViewRepresentable {
    let session: TerminalSession
    var active = false
    @Environment(\.colorScheme) var colorScheme

    func makeNSView(context: NSViewRepresentableContext<Self>) -> NSView {
        let container = TerminalContainer()
        container.wantsLayer = true
        container.session = session
        // Clicks into the terminal make it the active one (the recognizer of
        // the container sees the terminal view's clicks without taking them).
        let click = NSClickGestureRecognizer(target: container, action: #selector(TerminalContainer.clicked))
        click.delaysPrimaryMouseButtonEvents = false
        container.addGestureRecognizer(click)
        return container
    }

    func updateNSView(_ container: NSView, context: NSViewRepresentableContext<Self>) {
        let view = session.view
        (container as? TerminalContainer)?.session = session
        if view.superview !== container {
            view.removeFromSuperview()
            view.frame = container.bounds
            view.autoresizingMask = [.width, .height]
            container.addSubview(view)
        }
        session.applyPalette(dark: colorScheme == .dark)
        container.layer?.borderWidth = active ? 1 : 0
        container.layer?.borderColor = NSColor.controlAccentColor.withAlphaComponent(0.6).cgColor
        // Started once it has a size (the shell gets the right columns).
        if !session.started {
            Task { @MainActor in session.start() }
        }
    }

    /// Keeps the terminal view filling the container.
    final class TerminalContainer: NSView {
        weak var session: TerminalSession?

        @objc func clicked() { session?.onFocus?() }

        override func layout() {
            super.layout()
            subviews.first?.frame = bounds
        }
    }
}

/// „In Notiz übernehmen“: the output as a `console` code block at the end
/// of a note (Linux: `SendToNoteDialog.qml`).
struct SendToNoteSheet: View {
    let project: ProjectModel
    let model: TerminalModel
    let item: TerminalModel.SendToNote
    @State private var filter = ""
    @State private var newName = "Terminal-Ausgabe"
    @Environment(\.dismiss) private var dismiss

    private var notes: [String] {
        // Open Markdown tabs first, then all notes of the project.
        let open = project.editor.groups.flatMap(\.tabs).map(\.path)
            .filter { project.core.files().fileKind(path: $0) == "markdown" }
        var seen = Set<String>()
        return (open + project.core.notes().list()).filter { seen.insert($0).inserted }
            .filter { filter.isEmpty || relative($0).localizedCaseInsensitiveContains(filter) }
    }

    var body: some View {
        let lines = item.text.split(separator: "\n", omittingEmptySubsequences: false).count
        VStack(alignment: .leading, spacing: 10) {
            Text("In Notiz übernehmen").font(.headline)
            Text(lines == 1 ? "Eine Zeile als Codeblock anhängen an:" : "\(lines) Zeilen als Codeblock anhängen an:")
                .foregroundStyle(.secondary)
            TextField("Notiz suchen …", text: $filter).textFieldStyle(.roundedBorder)
            List(notes, id: \.self) { path in
                Button(relative(path)) { send(to: path) }.buttonStyle(.plain)
            }
            .frame(minHeight: 200)
            .overlay {
                if notes.isEmpty { Text("Keine passende Notiz.").foregroundStyle(.secondary) }
            }
            HStack {
                TextField("Neue Notiz: Name", text: $newName).textFieldStyle(.roundedBorder)
                Button("Neue Notiz anlegen") { newNote() }
            }
            HStack {
                Spacer()
                Button("Abbrechen") { model.sendToNote = nil; dismiss() }.keyboardShortcut(.cancelAction)
            }
        }
        .padding(20)
        .frame(width: 520, height: 440)
    }

    private func relative(_ path: String) -> String {
        project.core.relativePath(path: path) ?? path
    }

    private func send(to path: String) {
        model.send(item.text, to: path)
        model.sendToNote = nil
        dismiss()
    }

    /// A new note in the terminal's folder (inside the project), else the project.
    private func newNote() {
        let root = project.root
        let dir = (item.cwd + "/").hasPrefix(root + "/") ? item.cwd : root
        var name = newName.trimmingCharacters(in: .whitespaces)
        if name.isEmpty { name = "Terminal-Ausgabe" }
        if !name.hasSuffix(".md") { name += ".md" }
        do {
            let path = try project.core.files().newFile(dir: dir, name: name)
            send(to: path)
        } catch {
            project.show(notice: atlasMessage(error))
        }
    }
}
