// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// Placeholder of P3.8; the task replaces this view (same name and
/// properties: the router in `App/FeatureRouter.swift` uses them).
struct SearchWindowView: View {
    let project: ProjectModel

    var body: some View {
        FeaturePlaceholder(task: "P3.8", title: "Suche", path: "")
    }
}
