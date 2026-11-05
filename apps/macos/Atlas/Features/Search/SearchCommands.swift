// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// „Im Projekt suchen …“ (P3.8): ⇧⌘F, as in Xcode and most Mac editors (E10).
struct SearchCommands: Commands {
    @FocusedValue(\.project) private var project

    var body: some Commands {
        CommandGroup(after: .textEditing) {
            Button("Im Projekt suchen …") { project?.searchVisible = true }
                .keyboardShortcut("f", modifiers: [.command, .shift])
                .disabled(project == nil)
        }
    }
}
