// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// „Kontextleiste bearbeiten“ (P3.6), next to „Kontextleiste ein/aus“.
struct ContextCommands: Commands {
    @FocusedValue(\.project) private var project

    var body: some Commands {
        CommandGroup(after: .sidebar) {
            Button(project?.contextEditing == true ? "Kontextleiste fertig bearbeiten" : "Kontextleiste bearbeiten") {
                guard let project else { return }
                project.inspectorVisible = true
                project.contextEditing.toggle()
            }
            .disabled(project == nil)
        }
    }
}
