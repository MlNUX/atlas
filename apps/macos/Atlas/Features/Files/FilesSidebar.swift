// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import SwiftUI

/// The project tree in the sidebar. P3.0 provides a simple working tree
/// (open files, folders expand lazily); P3.1 replaces it with the full one
/// (drag and drop, context menus, Quick Look, trash, places).
struct FilesSidebar: View {
    let project: ProjectModel

    var body: some View {
        List {
            Section(project.name) {
                FolderRows(project: project, dir: project.root)
            }
        }
        .listStyle(.sidebar)
        .id(project.filesRevision)
    }
}

private struct FolderRows: View {
    let project: ProjectModel
    let dir: String

    var body: some View {
        let options = ListOptions(showHidden: false, foldersOnly: false, sort: .name, ascending: true)
        let entries = (try? project.core.files().list(dir: dir, options: options)) ?? []
        ForEach(entries, id: \.path) { entry in
            if entry.isDir {
                DisclosureGroup {
                    FolderRows(project: project, dir: entry.path)
                } label: {
                    Label(entry.name, systemImage: "folder")
                }
            } else {
                Button {
                    project.editor.open(path: entry.path, mode: .tab)
                } label: {
                    Label(entry.name, systemImage: "doc")
                }
                .buttonStyle(.plain)
            }
        }
    }
}
