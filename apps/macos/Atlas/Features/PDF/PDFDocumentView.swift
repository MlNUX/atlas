// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// Placeholder of P3.2; the task replaces this view (same name and
/// properties: the router in `App/FeatureRouter.swift` uses them).
struct PDFDocumentView: View {
    let project: ProjectModel
    let path: String

    var body: some View {
        FeaturePlaceholder(task: "P3.2", title: "PDF", path: path)
    }
}
