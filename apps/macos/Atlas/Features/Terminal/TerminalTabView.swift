// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// A terminal in an editor tab (`terminal:<dir>#<n>`; P3.5). The terminal
/// belongs to `TerminalModel`, so it keeps running when the tab is dragged
/// to another split; it ends when its tab is closed.
struct TerminalTabView: View {
    let project: ProjectModel
    let path: String

    var body: some View {
        let model = TerminalModel.of(project)
        let session = model.tabSession(path)
        TerminalHostView(session: session)
            .id(path)
            .onAppear { model.focusTab(session) }
            .onDisappear {
                // A dragged tab disappears and appears again elsewhere.
                Task { @MainActor in
                    try? await Task.sleep(for: .milliseconds(500))
                    model.tabMaybeClosed(path)
                }
            }
            .accessibilityLabel("Terminal")
    }
}
