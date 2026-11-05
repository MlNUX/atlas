// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import SwiftUI

/// The sheets and questions of P3.10 on the project window (one line in
/// `ProjectWindow`).
struct ActionsWindowModifier: ViewModifier {
    let project: ProjectModel

    func body(content: Content) -> some View {
        @Bindable var actions = project.actions
        content
            .sheet(isPresented: $actions.paletteVisible) {
                ActionPaletteView(project: project)
            }
            .sheet(isPresented: $actions.jobsVisible) {
                BackgroundJobsView(actions: actions)
            }
            .sheet(isPresented: $actions.extensionsVisible) {
                ExtensionsView(project: project)
            }
            .alert("Aktion ausführen?", isPresented: Binding(
                get: { actions.pending != nil }, set: { if !$0 { actions.pending = nil } }
            ), presenting: actions.pending) { _ in
                Button("Ausführen", role: .destructive) { actions.confirmPending() }
                Button("Abbrechen", role: .cancel) { actions.pending = nil }
            } message: { run in
                Text("„\(run.prepared.name)“ führt in \(run.prepared.cwd) aus:\n\n\(run.prepared.display)"
                    + (run.prepared.destructive ? "\n\nDer Befehl kann Dateien löschen oder überschreiben." : ""))
            }
            .alert("Aktion", isPresented: Binding(
                get: { actions.error != nil }, set: { if !$0 { actions.error = nil } }
            )) {
                Button("OK") { actions.error = nil }
            } message: {
                Text(actions.error ?? "")
            }
    }
}

/// „Aktion ausführen“ (⇧⌘P; Linux: Arbeitsleiste mit `>`): the actions for
/// the current tab, filtered as you type.
struct ActionPaletteView: View {
    let project: ProjectModel
    @State private var filter = ""
    @State private var selection: String?
    @Environment(\.dismiss) private var dismiss

    private var paths: [String] { project.actions.currentPaths }

    private var list: [ActionInfo] {
        let all = project.actions.actions(for: paths).filter(\.inPalette)
        let words = filter.lowercased().split(separator: " ")
        return all.filter { a in
            let text = (a.name + " " + a.description + " " + a.extension).lowercased()
            return words.allSatisfy { text.contains($0) }
        }
    }

    var body: some View {
        let list = self.list
        VStack(spacing: 0) {
            TextField("Aktion suchen", text: $filter)
                .textFieldStyle(.roundedBorder)
                .font(.title3)
                .padding(12)
                .onSubmit { run(selection ?? list.first?.id) }
            Divider()
            if list.isEmpty {
                ContentUnavailableView {
                    Label("Keine Aktion", systemImage: "bolt")
                } description: {
                    Text("Aktionen stehen in .atlas/actions.yaml oder kommen von Erweiterungen.")
                } actions: {
                    Button("Aktionsdatei öffnen") {
                        dismiss()
                        project.actions.openActionsFile(global: false)
                    }
                }
            } else {
                List(list, id: \.id, selection: $selection) { a in
                    HStack {
                        Image(systemName: icon(a.source)).foregroundStyle(.secondary).frame(width: 18)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(a.name)
                            if !a.description.isEmpty {
                                Text(a.description).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                            }
                        }
                        Spacer()
                        if !a.keybinding.isEmpty {
                            Text(a.keybinding).font(.caption.monospaced()).foregroundStyle(.secondary)
                        }
                    }
                    .contentShape(Rectangle())
                    .onTapGesture(count: 2) { run(a.id) }
                    .tag(a.id)
                }
            }
            Divider()
            HStack {
                Text(paths.first.map { "Für \(($0 as NSString).lastPathComponent)" } ?? "Für das Projekt")
                    .font(.caption).foregroundStyle(.secondary)
                Spacer()
                Button("Abbrechen") { dismiss() }.keyboardShortcut(.cancelAction)
                Button("Ausführen") { run(selection ?? list.first?.id) }
                    .keyboardShortcut(.defaultAction)
                    .disabled(list.isEmpty)
            }
            .padding(10)
        }
        .frame(width: 560, height: 420)
    }

    private func icon(_ source: ActionSource) -> String {
        switch source {
        case .project: "folder"
        case .user: "person"
        case .extension: "puzzlepiece.extension"
        }
    }

    private func run(_ id: String?) {
        guard let id else { return }
        dismiss()
        project.actions.run(id: id, paths: paths, line: project.editor.currentTab?.line ?? 0)
    }
}

/// Background jobs (`terminal: background` actions, extension commands):
/// state, end of the output, stop.
struct BackgroundJobsView: View {
    let actions: ActionsModel
    @State private var selection: String?
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Hintergrundaufgaben").font(.headline)
                Spacer()
                Button("Aktualisieren", systemImage: "arrow.clockwise") { actions.refreshJobs() }
                    .labelStyle(.iconOnly)
                Button("Schließen") { dismiss() }.keyboardShortcut(.cancelAction)
            }
            .padding(10)
            Divider()
            if actions.jobs.isEmpty {
                ContentUnavailableView("Nichts läuft im Hintergrund", systemImage: "gearshape.2")
            } else {
                HSplitView {
                    List(actions.jobs, id: \.key, selection: $selection) { job in
                        HStack {
                            Image(systemName: job.running ? "circle.fill" : "checkmark.circle")
                                .foregroundStyle(job.running ? Color.green : (job.exitCode ?? 0) == 0 ? Color.secondary : Color.red)
                            VStack(alignment: .leading, spacing: 1) {
                                Text(job.name)
                                Text(job.running ? "läuft" : "beendet (\(job.exitCode.map(String.init) ?? "?"))")
                                    .font(.caption).foregroundStyle(.secondary)
                            }
                            Spacer()
                            if job.running {
                                Button("Beenden") { actions.stop(job.key) }.buttonStyle(.borderless)
                            }
                        }
                        .tag(job.key)
                    }
                    .frame(minWidth: 220)
                    ScrollView {
                        Text(actions.jobs.first { $0.key == (selection ?? actions.jobs.first?.key) }?.log ?? "")
                            .font(.system(.callout, design: .monospaced))
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(8)
                    }
                    .frame(minWidth: 320)
                }
            }
        }
        .frame(width: 720, height: 440)
        .onAppear { actions.refreshJobs() }
    }
}
