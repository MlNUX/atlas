// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// „Kalender“ (P3.7): the `calendar:` tab, as on Linux.
struct CalendarCommands: Commands {
    @FocusedValue(\.project) private var project

    var body: some Commands {
        CommandGroup(after: .sidebar) {
            Button("Kalender") { project?.editor.open(path: "calendar:", mode: .tab) }
                .keyboardShortcut("k", modifiers: [.command, .shift])
                .disabled(project == nil)
        }
    }
}
