// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import SwiftUI

/// Which feature shows a tab (owned by P3.0; features fill only their own
/// views in `Features/<Name>/`).
enum FeatureRouter {
    @MainActor @ViewBuilder
    static func view(for tab: EditorTab, project: ProjectModel) -> some View {
        let processor = tab.processor
        if processor.hasPrefix("view:") {
            ExtensionPageView(project: project, viewId: processor, source: tab.path)
        } else if processor == "atlas:html" {
            HtmlDocumentView(project: project, path: tab.path)
        } else if processor == "atlas:source" {
            // The file itself as text (Linux: „Anzeigen mit: Atlas (Datei selbst)“).
            CodeDocumentView(project: project, path: tab.path, line: tab.line)
        } else if !processor.isEmpty, !processor.hasPrefix("atlas:") {
            ProcessedView(project: project, processor: processor, path: tab.path)
        } else {
            kindView(for: tab, project: project)
        }
    }

    @MainActor @ViewBuilder
    private static func kindView(for tab: EditorTab, project: ProjectModel) -> some View {
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
        case "missing":
            OtherDocumentView(project: project, path: tab.path)
        default:
            // image, atlasnote, binary: an automatic processor of an
            // extension if one fits (P3.10), else the viewer
            ProcessedOrOtherView(project: project, path: tab.path)
        }
    }

    static func title(for tab: EditorTab) -> String {
        if tab.path.hasPrefix("terminal:") { return "Terminal" }
        if tab.path.hasPrefix("calendar:") { return "Kalender" }
        if tab.path.hasPrefix("ilias:") { return "ILIAS" }
        let name = (tab.path as NSString).lastPathComponent
        switch tab.processor {
        case let p where p.hasPrefix("view:"): return "\(name) · Ansicht"
        case "atlas:html": return "\(name) · Webseite"
        default: return name
        }
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
