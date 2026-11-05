// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import SwiftUI

/// Atlas for macOS: one window per project (E9), the menu bar, settings.
@main
struct AtlasApp: App {
    @State private var app = AppModel()

    var body: some Scene {
        // The value is the project root; `nil` shows the welcome window.
        WindowGroup(id: "project", for: String.self) { $root in
            RootView(app: app, root: $root)
        }
        .defaultSize(width: 1180, height: 740)
        .commands {
            AtlasCommands(app: app)
            DocumentFeatureCommands()
            ToolFeatureCommands()
        }

        Settings {
            SettingsView(app: app)
        }
    }
}

/// A project window, or the welcome view while no project is chosen.
struct RootView: View {
    let app: AppModel
    @Binding var root: String?

    var body: some View {
        if let root, let project = app.project(at: root) {
            ProjectWindow(project: project)
                .navigationTitle(project.name)
                .focusedSceneValue(\.project, project)
        } else {
            WelcomeView(app: app) { chosen in root = chosen }
                .onAppear {
                    // CI and UI tests: `-AtlasOpenProject <path>` opens a project at launch.
                    if root == nil, let path = UserDefaults.standard.string(forKey: "AtlasOpenProject") {
                        root = path
                    }
                }
        }
    }
}

extension FocusedValues {
    /// The project of the key window (for menu commands).
    @Entry var project: ProjectModel?
}
