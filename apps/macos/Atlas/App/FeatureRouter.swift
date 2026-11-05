// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import SwiftUI

/// Which feature shows a tab (owned by P3.0; features fill only their own
/// views in `Features/<Name>/`).
enum FeatureRouter {
    @MainActor @ViewBuilder
    static func view(for tab: EditorTab, project: ProjectModel) -> some View {
        switch project.core.files().fileKind(path: tab.path) {
        case "pdf":
            PDFDocumentView(project: project, path: tab.path)
        case "markdown":
            MarkdownDocumentView(project: project, path: tab.path, line: tab.line)
        case "text":
            CodeDocumentView(project: project, path: tab.path, line: tab.line)
        case "terminal":
            TerminalTabView(project: project, path: tab.path)
        case "calendar":
            CalendarTabView(project: project)
        case "ilias":
            IliasTabView(project: project)
        case "dir", "trash":
            FolderView(project: project, path: tab.path)
        default:
            // image, atlasnote, binary, missing
            OtherDocumentView(project: project, path: tab.path)
        }
    }

    static func title(for tab: EditorTab) -> String {
        if tab.path.hasPrefix("terminal:") { return "Terminal" }
        if tab.path.hasPrefix("calendar:") { return "Kalender" }
        if tab.path.hasPrefix("ilias:") { return "ILIAS" }
        return (tab.path as NSString).lastPathComponent
    }
}

/// Placeholder of a feature that is not built yet (P3.x fills it).
struct FeaturePlaceholder: View {
    let task: String
    let title: String
    let path: String

    var body: some View {
        ContentUnavailableView {
            Label(title, systemImage: "hammer")
        } description: {
            Text("\((path as NSString).lastPathComponent)\nkommt mit \(task)")
        }
    }
}
