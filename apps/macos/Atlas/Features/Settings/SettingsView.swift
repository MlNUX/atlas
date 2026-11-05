// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// Placeholder of P3.9; the task replaces this view (same name and
/// properties: the router in `App/FeatureRouter.swift` uses them).
struct SettingsView: View {
    let app: AppModel

    var body: some View {
        FeaturePlaceholder(task: "P3.9", title: "Einstellungen", path: "")
            .frame(width: 520, height: 360)
    }
}
