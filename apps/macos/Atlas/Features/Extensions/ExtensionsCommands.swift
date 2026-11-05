// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import SwiftUI

/// Menu „Aktionen“ (P3.10): run an action, background jobs, show or export
/// the current file through an extension, extension views, extensions.
struct ExtensionsCommands: Commands {
    @FocusedValue(\.project) private var project

    var body: some Commands {
        CommandMenu("Aktionen") {
            Button("Aktion ausführen …") { project?.actions.paletteVisible = true }
                .keyboardShortcut("p", modifiers: [.command, .shift])
                .disabled(project == nil)
            Button("Hintergrundaufgaben") { project?.actions.jobsVisible = true }
                .disabled(project == nil)
            Divider()
            fileMenus
            viewsMenu
            Divider()
            Button("Aktionen des Projekts bearbeiten") { project?.actions.openActionsFile(global: false) }
                .disabled(project == nil)
            Button("Eigene Aktionen bearbeiten") { project?.actions.openActionsFile(global: true) }
                .disabled(project == nil)
            Button("Erweiterungen …") { project?.actions.extensionsVisible = true }
                .disabled(project == nil)
        }
    }

    /// „Anzeigen mit“ and „Exportieren mit“ for the current file.
    @ViewBuilder
    private var fileMenus: some View {
        let path = project?.actions.currentPaths.first ?? ""
        let _ = project?.actions.revision
        let processors = path.isEmpty ? [] : project?.actions.processors(for: path) ?? []
        let exporters = path.isEmpty ? [] : project?.actions.exporters(for: path) ?? []
        Menu("Anzeigen mit") {
            Button("Atlas (normal)") { project?.editor.open(path: path, mode: .tab) }
            ForEach(processors, id: \.id) { p in
                Button("\(p.name) (\(p.extension))") { project?.actions.show(path, with: p.id) }
            }
        }
        .disabled(processors.isEmpty)
        Menu("Exportieren mit") {
            ForEach(exporters, id: \.id) { x in
                Button("\(x.name) → .\(x.target)") {
                    guard let project else { return }
                    Task { await project.actions.export(path, with: x) }
                }
            }
        }
        .disabled(exporters.isEmpty)
        if let project, project.core.actions().isRunnable(path: path) {
            Button("Skript ausführen") { project.actions.runScript(path) }
        }
    }

    @ViewBuilder
    private var viewsMenu: some View {
        let _ = project?.actions.revision
        let views = project?.actions.views() ?? []
        Menu("Ansichten") {
            ForEach(views, id: \.id) { v in
                Button(v.name) { project?.actions.openView(v.id) }
            }
        }
        .disabled(views.isEmpty)
    }
}
