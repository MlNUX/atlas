// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import SwiftUI
import UniformTypeIdentifiers

/// The sidebar (P3.1; Linux: `ProjectTree.qml`, `PlacesList.qml`): the
/// project tree (expanded folders are part of the session) and the places.
/// Drag and drop (also from and to the Finder), context menus, Quick Look
/// with the space bar, trash with ⌘⌫.
struct FilesSidebar: View {
    let project: ProjectModel
    @State private var selection = Set<String>()

    var body: some View {
        let files = FilesModel.of(project)
        List(selection: $selection) {
            Section(project.name) {
                TreeRows(files: files, dir: project.root, depth: 0)
            }
            .dropDestination(for: URL.self) { urls, _ in files.drop(urls, into: project.root) }
            PlacesSection(files: files, places: project.app.core.places())
        }
        .listStyle(.sidebar)
        .id(project.filesRevision)
        .contextMenu(forSelectionType: String.self) { items in
            let paths = items.filter { !$0.hasPrefix("place:") }.sorted()
            FileContextMenu(files: files, paths: paths, dir: Self.dir(for: paths, files: files, root: project.root))
        } primaryAction: { items in
            guard let item = items.first else { return }
            if item.hasPrefix("place:") {
                files.open(String(item.dropFirst(6)))
            } else if files.files?.isDir(path: item) == true {
                files.setExpanded(item, !files.isExpanded(item))
            } else {
                files.open(item)
            }
        }
        .onChange(of: selection) {
            let paths = selection.filter { !$0.hasPrefix("place:") }.sorted()
            files.selection = paths
            files.targetDir = Self.dir(for: paths, files: files, root: project.root)
            // A single click opens files (as on Linux: lasting tabs) and places.
            if selection.count == 1, let item = selection.first {
                if item.hasPrefix("place:") {
                    files.open(String(item.dropFirst(6)))
                } else if files.files?.isDir(path: item) == false {
                    files.open(item)
                }
            }
        }
        .onKeyPress(.space) {
            guard let path = files.selection.first, files.files?.isDir(path: path) == false else { return .ignored }
            files.showQuickLook(path, among: files.selection)
            return .handled
        }
        .modifier(FileDialogs(files: files))
        .onAppear { files.watch() }
    }

    /// The folder new and pasted items go to: the selected folder, else the
    /// folder of the selected file, else the project.
    static func dir(for paths: [String], files: FilesModel, root: String) -> String {
        guard let first = paths.first else { return root }
        return files.files?.isDir(path: first) == true ? first : (first as NSString).deletingLastPathComponent
    }

    static func icon(_ name: String) -> String {
        switch name {
        case "home", "user-home": "house"
        case "desktop", "user-desktop": "menubar.dock.rectangle"
        case "documents", "folder-documents": "doc.text"
        case "downloads", "folder-download": "arrow.down.circle"
        case "pictures", "folder-pictures": "photo"
        case "music", "folder-music": "music.note"
        case "videos", "folder-videos": "film"
        case "trash", "user-trash": "trash"
        case "ilias": "graduationcap"
        default: "folder"
        }
    }
}

/// The entries of `dir`; folders expand (lazily, state in the session).
private struct TreeRows: View {
    let files: FilesModel
    let dir: String
    let depth: Int

    var body: some View {
        ForEach(files.list(dir), id: \.path) { entry in
            if entry.isDir {
                DisclosureGroup(isExpanded: Binding(
                    get: { files.isExpanded(entry.path) },
                    set: { files.setExpanded(entry.path, $0) })
                ) {
                    if files.isExpanded(entry.path) {
                        TreeRows(files: files, dir: entry.path, depth: depth + 1)
                    }
                } label: {
                    Row(entry: entry)
                        .dropDestination(for: URL.self) { urls, _ in files.drop(urls, into: entry.path) }
                }
                .tag(entry.path)
            } else {
                Row(entry: entry).tag(entry.path)
            }
        }
    }
}

/// Icon and name; draggable as a file (to folders, other apps, the Finder).
private struct Row: View {
    let entry: FileEntry

    var body: some View {
        Label {
            Text(entry.name).lineLimit(1)
        } icon: {
            Image(nsImage: NSWorkspace.shared.icon(forFile: entry.path))
                .resizable()
                .frame(width: 16, height: 16)
        }
        .opacity(entry.isHidden ? 0.6 : 1)
        .draggable(URL(filePath: entry.path))
    }
}

/// The places (home, documents, trash, …); drops go there.
private struct PlacesSection: View {
    let files: FilesModel
    let places: [Place]

    var body: some View {
        Section("Orte") {
            ForEach(places, id: \.path) { (place: Place) in
                PlaceRow(files: files, place: place)
            }
        }
    }
}

private struct PlaceRow: View {
    let files: FilesModel
    let place: Place

    var body: some View {
        let path: String = place.path
        Label(place.name, systemImage: FilesSidebar.icon(place.icon))
            .tag("place:" + path)
            .dropDestination(for: URL.self) { (urls: [URL], _) -> Bool in files.drop(urls, into: path) }
    }
}
