// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// Placeholder of P3.4; the task replaces this view (same name and
/// properties: the router in `App/FeatureRouter.swift` uses them).
struct CodeDocumentView: View {
    let project: ProjectModel
    let path: String
    let line: Int

    var body: some View {
        FeaturePlaceholder(task: "P3.4", title: "Text und Code", path: path)
    }
}
