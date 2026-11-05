// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// Placeholder of P3.1 (folder view, trash); same name and properties stay.
struct FolderView: View {
    let project: ProjectModel
    let path: String

    var body: some View {
        FeaturePlaceholder(task: "P3.1", title: "Ordneransicht", path: path)
    }
}
