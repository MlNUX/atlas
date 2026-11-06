// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// File items in the menu „Ablage“ (P3.1), with the Finder's shortcuts.
/// They act on the selection of the sidebar or the folder view.
struct FilesCommands: Commands {
    @FocusedValue(\.project) private var project

    var body: some Commands {
        CommandGroup(after: .newItem) {
            Divider()
            Button("Neue Datei …") { files?.askNewFile(in: files?.targetDir ?? "") }
                .keyboardShortcut("n", modifiers: [.command, .option])
                .disabled(files == nil)
            Button("Neuer Ordner …") { files?.askNewFolder(in: files?.targetDir ?? "") }
                .keyboardShortcut("n", modifiers: [.command, .shift])
                .disabled(files == nil)
            Divider()
            Button("Schnellansicht") {
                if let files, let first = files.selection.first { files.showQuickLook(first, among: files.selection) }
            }
            .keyboardShortcut("y")
            .disabled(files?.selection.isEmpty ?? true)
            Button("Umbenennen …") { if let files, let first = files.selection.first { files.askRename(first) } }
                .disabled(files?.selection.count != 1)
            Button("Eigenschaften …") { if let files, let first = files.selection.first { files.showProperties(first) } }
                .keyboardShortcut("i")
                .disabled(files?.selection.count != 1)
            Button("In den Papierkorb") { if let files { files.trash(files.selection) } }
                .keyboardShortcut(.delete, modifiers: .command)
                .disabled(files?.selection.isEmpty ?? true)
        }
    }

    private var files: FilesModel? { project.map(FilesModel.of) }
}
