// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import SwiftUI

/// A folder in a tab (P3.1; Linux: `FolderView.qml`, `TrashView.qml`):
/// details or icons, path bar, back/up, sorting, status bar; the trash
/// with restore and delete.
struct FolderView: View {
    let project: ProjectModel
    let path: String
    @State private var current = ""
    @State private var history: [String] = []

    var body: some View {
        let files = FilesModel.of(project)
        Group {
            if files.files?.fileKind(path: current.isEmpty ? path : current) == "trash" {
                TrashView(files: files)
            } else {
                FolderContent(files: files, project: project, dir: current.isEmpty ? path : current, go: go, back: back,
                              canGoBack: !history.isEmpty)
            }
        }
        .onAppear { files.watch(extra: [current.isEmpty ? path : current]) }
        .onChange(of: path) { current = path; history = [] }
    }

    private func go(_ dir: String) {
        let from = current.isEmpty ? path : current
        guard dir != from else { return }
        history.append(from)
        current = dir
        FilesModel.of(project).watch(extra: [dir])
    }

    private func back() {
        guard let last = history.popLast() else { return }
        current = last
    }
}

private struct FolderContent: View {
    let files: FilesModel
    let project: ProjectModel
    let dir: String
    let go: (String) -> Void
    let back: () -> Void
    let canGoBack: Bool
    @State private var selection = Set<String>()
    @FocusState private var focused: Bool
    @State private var sortOrder = [KeyPathComparator(\FileEntry.name)]
    @AppStorage("AtlasFolderIcons") private var icons = false

    private var entries: [FileEntry] {
        let _ = project.filesRevision
        let (key, ascending) = Self.sort(sortOrder)
        return files.list(dir, sort: key, ascending: ascending)
    }

    var body: some View {
        let entries = self.entries
        VStack(spacing: 0) {
            bar
            Group {
                if icons {
                    IconGrid(files: files, entries: entries, selection: $selection, open: open)
                } else {
                    table(entries)
                }
            }
            .dropDestination(for: URL.self) { urls, _ in files.drop(urls, into: dir) }
            .focused($focused)
            .onChange(of: focused) { files.setFocus("folder:" + dir, focused) }
            .onDisappear { files.setFocus("folder:" + dir, false) }
            .onKeyPress(.space) {
                guard let first = selection.sorted().first, files.files?.isDir(path: first) == false else { return .ignored }
                files.showQuickLook(first, among: entries.filter { !$0.isDir }.map(\.path))
                return .handled
            }
            status(entries)
        }
        .onChange(of: selection) {
            files.selection = selection.sorted()
            files.targetDir = dir
        }
        .modifier(FileDialogs(files: files))
    }

    private func table(_ entries: [FileEntry]) -> some View {
        Table(entries, selection: $selection, sortOrder: $sortOrder) {
            TableColumn("Name", value: \.name) { entry in
                HStack(spacing: 6) {
                    Image(nsImage: NSWorkspace.shared.icon(forFile: entry.path)).resizable().frame(width: 16, height: 16)
                    Text(entry.name).lineLimit(1)
                }
                .opacity(entry.isHidden ? 0.6 : 1)
                .draggable(URL(filePath: entry.path))
            }
            TableColumn("Geändert", value: \.modified) { entry in
                Text(Date(timeIntervalSince1970: TimeInterval(entry.modified)).formatted(date: .abbreviated, time: .shortened))
                    .foregroundStyle(.secondary)
            }
            .width(min: 120, ideal: 150)
            TableColumn("Größe", value: \.sortSize) { entry in
                Text(entry.size.map { ByteCountFormatter.string(fromByteCount: Int64(clamping: $0), countStyle: .file) } ?? "–")
                    .foregroundStyle(.secondary)
            }
            .width(min: 60, ideal: 80)
            TableColumn("Art", value: \.kind) { entry in
                Text(entry.kind).foregroundStyle(.secondary)
            }
            .width(min: 80, ideal: 140)
        }
        .contextMenu(forSelectionType: String.self) { items in
            FileContextMenu(files: files, paths: items.sorted(), dir: dir)
        } primaryAction: { items in
            if let first = items.first { open(first) }
        }
    }

    private func open(_ path: String) {
        if files.files?.isDir(path: path) == true { go(path) } else { files.open(path) }
    }

    /// Back, up, path bar, hidden files, details/icons.
    private var bar: some View {
        HStack(spacing: 8) {
            Button("Zurück", systemImage: "chevron.left", action: back)
                .labelStyle(.iconOnly).disabled(!canGoBack)
                .keyboardShortcut("[", modifiers: .command)
            Button("Übergeordneter Ordner", systemImage: "chevron.up") { go((dir as NSString).deletingLastPathComponent) }
                .labelStyle(.iconOnly).disabled(dir == "/")
                .keyboardShortcut(.upArrow, modifiers: .command)
            PathBar(dir: dir, go: go)
            Spacer()
            Toggle(isOn: Binding(get: { files.showHidden }, set: { files.showHidden = $0 })) {
                Label("Versteckte", systemImage: "eye")
            }
            .toggleStyle(.button).labelStyle(.iconOnly)
            .help("Versteckte Dateien zeigen")
            Picker("Ansicht", selection: $icons) {
                Label("Liste", systemImage: "list.bullet").tag(false)
                Label("Symbole", systemImage: "square.grid.2x2").tag(true)
            }
            .pickerStyle(.segmented).labelsHidden().fixedSize()
        }
        .padding(.horizontal, 10).padding(.vertical, 5)
        .background(.bar)
    }

    private func status(_ entries: [FileEntry]) -> some View {
        let dirs = entries.filter(\.isDir).count
        let space = files.files?.diskSpace(path: dir)
        return HStack {
            Text("\(dirs) Ordner, \(entries.count - dirs) Dateien" + (selection.isEmpty ? "" : " · \(selection.count) ausgewählt"))
            Spacer()
            if let space {
                Text("\(ByteCountFormatter.string(fromByteCount: Int64(clamping: space.free), countStyle: .file)) frei")
            }
        }
        .font(.caption).foregroundStyle(.secondary)
        .padding(.horizontal, 10).padding(.vertical, 4)
        .background(.bar)
    }

    static func sort(_ order: [KeyPathComparator<FileEntry>]) -> (SortKey, Bool) {
        guard let first = order.first else { return (.name, true) }
        let ascending = first.order == .forward
        let key: PartialKeyPath<FileEntry> = first.keyPath
        if key == \FileEntry.modified { return (.modified, ascending) }
        if key == \FileEntry.sortSize { return (.size, ascending) }
        if key == \FileEntry.kind { return (.kind, ascending) }
        return (.name, ascending)
    }
}

extension FileEntry: @retroactive Identifiable {
    public var id: String { path }
    /// Size for sorting (folders first, as 0).
    var sortSize: UInt64 { size ?? 0 }
}

/// The folders of the path; a click goes there.
private struct PathBar: View {
    let dir: String
    let go: (String) -> Void

    var body: some View {
        let parts = dir.split(separator: "/").map(String.init)
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 2) {
                ForEach(parts.indices, id: \.self) { i in
                    if i > 0 { Image(systemName: "chevron.right").font(.caption2).foregroundStyle(.tertiary) }
                    Button(parts[i]) { go("/" + parts[...i].joined(separator: "/")) }
                        .buttonStyle(.plain)
                        .fontWeight(i == parts.count - 1 ? .semibold : .regular)
                }
            }
        }
    }
}

/// Icons in a grid (Linux: the icon view of the folder view).
private struct IconGrid: View {
    let files: FilesModel
    let entries: [FileEntry]
    @Binding var selection: Set<String>
    let open: (String) -> Void

    var body: some View {
        ScrollView {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 96), spacing: 12)], spacing: 12) {
                ForEach(entries, id: \.path) { entry in
                    VStack(spacing: 4) {
                        Image(nsImage: NSWorkspace.shared.icon(forFile: entry.path)).resizable().frame(width: 56, height: 56)
                        Text(entry.name).font(.caption).lineLimit(2).multilineTextAlignment(.center)
                    }
                    .frame(width: 96, height: 92)
                    .background(selection.contains(entry.path) ? Color.accentColor.opacity(0.25) : .clear,
                                in: RoundedRectangle(cornerRadius: 6))
                    .contentShape(Rectangle())
                    .onTapGesture(count: 2) { open(entry.path) }
                    .onTapGesture { selection = [entry.path] }
                    .draggable(URL(filePath: entry.path))
                    .contextMenu {
                        FileContextMenu(files: files, paths: selection.contains(entry.path) ? selection.sorted() : [entry.path],
                                        dir: (entry.path as NSString).deletingLastPathComponent)
                    }
                }
            }
            .padding(12)
        }
    }
}

/// The system trash: restore or delete for good.
private struct TrashView: View {
    let files: FilesModel
    @State private var items: [TrashItem] = []
    @State private var selection = Set<String>()
    @State private var confirmDelete = false

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Papierkorb").font(.headline)
                Spacer()
                Button("Wiederherstellen") { files.restore(Array(selection)); reload() }
                    .disabled(selection.isEmpty)
                Button("Endgültig löschen …", role: .destructive) { confirmDelete = true }
                    .disabled(selection.isEmpty)
            }
            .padding(.horizontal, 10).padding(.vertical, 5)
            .background(.bar)
            if items.isEmpty {
                ContentUnavailableView("Der Papierkorb ist leer", systemImage: "trash")
            } else {
                Table(items, selection: $selection) {
                    TableColumn("Name") { item in
                        Label(item.name, systemImage: item.isDir ? "folder" : "doc")
                    }
                    TableColumn("Ursprünglicher Ort") { item in
                        Text(item.originalPath.isEmpty ? "–" : (item.originalPath as NSString).deletingLastPathComponent)
                            .foregroundStyle(.secondary)
                    }
                    TableColumn("Gelöscht") { item in
                        Text(Date(timeIntervalSince1970: TimeInterval(item.deleted)).formatted(date: .abbreviated, time: .shortened))
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
        .confirmationDialog("\(selection.count) Einträge endgültig löschen?", isPresented: $confirmDelete) {
            Button("Löschen", role: .destructive) { files.purge(Array(selection)); reload() }
        } message: {
            Text("Das lässt sich nicht rückgängig machen.")
        }
        .task { reload() }
    }

    private func reload() {
        items = (try? files.files?.trashItems()) ?? []
        selection = selection.filter { id in items.contains { $0.id == id } }
    }
}

extension TrashItem: @retroactive Identifiable {}
