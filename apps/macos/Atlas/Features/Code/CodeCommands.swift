// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// Menu „Code“ (P3.4). Shortcuts: docs/shortcuts.md, Mac column (E10).
/// ⌘S for code files is in `MarkdownCommands` (one „Sichern“ item for both).
struct CodeCommands: Commands {
    @FocusedValue(\.code) private var code

    var body: some Commands {
        CommandMenu("Code") {
            // ⌘F / ⌥⌘F: „Bearbeiten → Suchen“ of macOS works on the editor.
            Button("Suchen und ersetzen …") { code?.showFind(replace: true) }
                .disabled(code == nil)
            Button("Gehe zu Zeile …") { code?.goToLineVisible = true }
                .keyboardShortcut("l")
                .disabled(code == nil)
            Divider()
            Button("Symbole ein/aus") { code?.symbolsVisible.toggle() }
                .keyboardShortcut("o", modifiers: [.command, .shift])
                .disabled(code == nil)
            Button("Skript ausführen") { code?.run() }
                .keyboardShortcut("r")
                .disabled(code?.runnable != true)
        }
    }
}
