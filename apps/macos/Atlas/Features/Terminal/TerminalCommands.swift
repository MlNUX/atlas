// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// Menu „Terminal“ (P3.5). Shortcuts as on Linux with ⌃ (docs/shortcuts.md,
/// Mac column): ⌃` shows/hides the panel, ⌃⇧` opens a new terminal.
struct TerminalCommands: Commands {
    @FocusedValue(\.project) private var project

    var body: some Commands {
        CommandMenu("Terminal") {
            let model = project.map(TerminalModel.of)
            Button(model?.visible == true ? "Terminal ausblenden" : "Terminal einblenden") { model?.toggle() }
                .keyboardShortcut("`", modifiers: .control)
                .disabled(model == nil)
            Button("Neues Terminal") { model?.newTerminal() }
                .keyboardShortcut("`", modifiers: [.control, .shift])
                .disabled(model == nil)
            Button("Terminal als Tab") {
                guard let project else { return }
                project.editor.open(path: "terminal:\(project.root)#\(Int(Date().timeIntervalSince1970))", mode: .tab)
            }
            .disabled(project == nil)
            Divider()
            Button("Rechts teilen") { model?.split(vertical: false) }
                .disabled(model?.visible != true)
            Button("Unten teilen") { model?.split(vertical: true) }
                .disabled(model?.visible != true)
            Button("Terminal beenden") { model?.killActive() }
                .disabled(model?.currentGroup?.active == nil)
            Divider()
            Button("Ausgabe in Notiz übernehmen …") {
                if let model, let session = model.currentGroup?.active { model.askSendToNote(session) }
            }
            .disabled(model?.currentGroup?.active == nil)
        }
    }
}
