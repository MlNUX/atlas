// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// „ILIAS öffnen“ (P3.11): the `ilias:` tab, as on Linux.
struct IliasCommands: Commands {
    @FocusedValue(\.project) private var project

    var body: some Commands {
        CommandGroup(after: .sidebar) {
            Button("ILIAS") { project?.editor.open(path: "ilias:", mode: .tab) }
                .keyboardShortcut("i", modifiers: [.command, .shift])
                .disabled(project == nil)
        }
    }
}
