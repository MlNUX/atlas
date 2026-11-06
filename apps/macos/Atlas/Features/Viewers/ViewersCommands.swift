// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// „Als Webseite anzeigen“ for HTML files, „Bild in eigenem Fenster“ (P3.12).
struct ViewersCommands: Commands {
    @FocusedValue(\.project) private var project

    private var path: String? {
        guard let tab = project?.editor.currentTab, !tab.isVirtual else { return nil }
        return tab.path
    }

    private var isHtml: Bool { ["html", "htm"].contains((path.map { ($0 as NSString).pathExtension.lowercased() }) ?? "") }

    private var isImage: Bool {
        guard let path, let project else { return false }
        return project.core.files().fileKind(path: path) == "image"
    }

    var body: some Commands {
        CommandGroup(after: .sidebar) {
            Button(project?.editor.currentTab?.processor == "atlas:html" ? "Quelltext anzeigen" : "Als Webseite anzeigen") {
                guard let project, let path else { return }
                let showing = project.editor.currentTab?.processor == "atlas:html"
                project.editor.open(path: path, mode: .tab, processor: showing ? "" : "atlas:html")
            }
            .keyboardShortcut("u", modifiers: [.command, .option])
            .disabled(!isHtml)
            Button("Bild in eigenem Fenster") { if let path { ImageWindow.show(path: path) } }
                .disabled(!isImage)
        }
    }
}
