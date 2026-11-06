// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// Menu „Markdown“ and ⌘S (P3.3). Shortcuts: docs/shortcuts.md, Mac column (E10).
struct MarkdownCommands: Commands {
    @FocusedValue(\.markdown) private var markdown
    /// ⌘S also saves code files (P3.4).
    @FocusedValue(\.code) private var code

    var body: some Commands {
        CommandGroup(replacing: .saveItem) {
            Button("Sichern") {
                if let markdown { markdown.save() } else { code?.save() }
            }
            .keyboardShortcut("s")
            .disabled(markdown == nil && code == nil)
        }
        CommandMenu("Markdown") {
            Button("Bearbeiten") { markdown?.mode = .edit }
                .keyboardShortcut("1", modifiers: [.command, .option])
                .disabled(markdown == nil)
            Button("Geteilt") { markdown?.mode = .split }
                .keyboardShortcut("2", modifiers: [.command, .option])
                .disabled(markdown == nil)
            Button("Vorschau") { markdown?.mode = .preview }
                .keyboardShortcut("3", modifiers: [.command, .option])
                .disabled(markdown == nil)
            Divider()
            if let markdown {
                Menu("Einfügen") { InsertItems(model: markdown) }
            } else {
                Button("Einfügen") {}.disabled(true)
            }
            Button("Tabellen ausrichten") { markdown?.alignTables() }
                .disabled(markdown == nil)
            Divider()
            Button("Als PDF exportieren …") { markdown?.exportPDF() }
                .disabled(markdown == nil || markdown?.exporting == true)
        }
    }
}
