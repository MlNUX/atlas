// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// Placeholder of P3.5; the task replaces this view (same name and
/// properties: the router in `App/FeatureRouter.swift` uses them).
struct TerminalTabView: View {
    let project: ProjectModel
    let path: String

    var body: some View {
        FeaturePlaceholder(task: "P3.5", title: "Terminal", path: path)
    }
}
