// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// The app's own menu items (Ablage, Darstellung, Fenster). Shortcuts follow
/// macOS conventions (E10); the Mac column of docs/shortcuts.md lists them.
/// Features add theirs in `Features/<Name>/<Name>Commands.swift`.
struct AtlasCommands: Commands {
    let app: AppModel
    @FocusedValue(\.project) private var project
    @Environment(\.openWindow) private var openWindow

    var body: some Commands {
        CommandGroup(after: .newItem) {
            Button("Projektordner öffnen …") { openWindow(id: "project") }
                .keyboardShortcut("o")
            Button("Schnell öffnen …") { project?.quickOpenVisible = true }
                .keyboardShortcut("p")
                .disabled(project == nil)
            Divider()
            Button("Tab schließen") { project?.editor.closeCurrentTab() }
                .keyboardShortcut("w")
                .disabled(project?.editor.currentTab == nil)
        }
        CommandMenu("Editor") {
            Button("Tab in neuen Split") { project?.editor.splitCurrentTab() }
                .keyboardShortcut("\\", modifiers: .command)
                .disabled(project?.editor.currentTab == nil)
            Button("Splits drehen") { project?.editor.toggleOrientation() }
                .keyboardShortcut("\\", modifiers: [.command, .option])
                .disabled(project == nil)
        }
        CommandGroup(after: .sidebar) {
            Button("Kontextleiste ein/aus") { project?.inspectorVisible.toggle() }
                .keyboardShortcut("i", modifiers: [.command, .option])
                .disabled(project == nil)
        }
    }
}

/// The menu items of the features (each in `Features/<Name>/<Name>Commands.swift`),
/// in two groups because a commands builder takes at most ten parts.
struct DocumentFeatureCommands: Commands {
    var body: some Commands {
        FilesCommands()
        PDFCommands()
        MarkdownCommands()
        CodeCommands()
        TerminalCommands()
        ContextCommands()
    }
}

struct ToolFeatureCommands: Commands {
    var body: some Commands {
        CalendarCommands()
        SearchCommands()
        ExtensionsCommands()
        IliasCommands()
        ViewersCommands()
    }
}
