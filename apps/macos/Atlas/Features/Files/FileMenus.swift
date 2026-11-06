// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import QuickLook
import SwiftUI

/// Context menu of files and folders (sidebar and folder view; Linux:
/// `FileActions.qml`). `paths` are the selected items, `dir` the folder new
/// items and pasted ones go to.
struct FileContextMenu: View {
    let files: FilesModel
    let paths: [String]
    let dir: String

    var body: some View {
        let project = files.project
        let single = paths.count == 1 ? paths[0] : nil
        let isDir = single.map { files.files?.isDir(path: $0) ?? false } ?? false
        if let single {
            Button("Öffnen") { files.open(single) }
            Button("In neuem Split öffnen") { files.open(single, newSplit: true) }
            if !isDir {
                Button("Schnellansicht") { files.showQuickLook(single) }
            }
            Divider()
        }
        Button("Neue Datei …") { files.askNewFile(in: dir) }
        Button("Neuer Ordner …") { files.askNewFolder(in: dir) }
        if !paths.isEmpty {
            Divider()
            if let single {
                Button("Umbenennen …") { files.askRename(single) }
            }
            Button("Kopieren") { files.copy(paths) }
            Button("Ausschneiden") { files.copy(paths, cut: true) }
        }
        Button("Einfügen") { files.paste(into: dir) }
            .disabled(files.pasteboardFiles.isEmpty)
        if !paths.isEmpty {
            Divider()
            Button("Pfad kopieren") { files.copyPath(paths) }
            Button("Im Finder zeigen") { files.revealInFinder(paths) }
            if let single {
                Button("Terminal hier öffnen") {
                    let folder = isDir ? single : (single as NSString).deletingLastPathComponent
                    project?.editor.open(path: "terminal:" + folder, mode: .tab)
                }
                Button("Eigenschaften …") { files.showProperties(single) }
            }
            Divider()
            if let project {
                let actions = project.actions.actions(for: paths)
                if !actions.isEmpty {
                    Menu("Aktionen") {
                        ForEach(actions, id: \.id) { a in
                            Button(a.name) { _ = project.actions.run(id: a.id, paths: paths) }
                        }
                    }
                }
                if let single, !isDir, project.core.actions().isRunnable(path: single) {
                    Button("Skript ausführen") { project.actions.runScript(single) }
                }
            }
            Button("In den Papierkorb", role: .destructive) { files.trash(paths) }
        }
    }
}

/// The dialogs of `FilesModel` (naming, conflicts, properties) and Quick Look.
struct FileDialogs: ViewModifier {
    @Bindable var files: FilesModel

    func body(content: Content) -> some View {
        content
            .sheet(item: $files.naming) { naming in
                NamingSheet(files: files, naming: naming)
            }
            .sheet(item: $files.properties) { item in
                PropertiesSheet(files: files, properties: item.value)
            }
            .confirmationDialog(
                conflictTitle, isPresented: Binding(get: { files.conflict != nil }, set: { if !$0 { files.conflict = nil } }),
                presenting: files.conflict
            ) { pending in
                Button("Ersetzen", role: .destructive) { files.resolve(pending, .replace) }
                Button("Überspringen") { files.resolve(pending, .skip) }
                Button("Beide behalten") { files.resolve(pending, .keepBoth) }
                Button("Abbrechen", role: .cancel) { files.conflict = nil }
            } message: { pending in
                Text(pending.clashes.prefix(5).map { ($0 as NSString).lastPathComponent }.joined(separator: ", ")
                     + (pending.clashes.count > 5 ? " …" : "")
                     + "\n„Ersetzen“ legt die vorhandenen Dateien in den Papierkorb.")
            }
            .quickLookPreview($files.quickLook, in: files.quickLookItems)
    }

    private var conflictTitle: String {
        let n = files.conflict?.clashes.count ?? 0
        return n == 1 ? "Eine Datei gibt es dort schon" : "\(n) Dateien gibt es dort schon"
    }
}

/// `Identifiable` wrapper for sheets on values that are not.
struct IdentifiableBox<Value>: Identifiable {
    let id = UUID()
    let value: Value
}

/// New file, new folder, rename.
private struct NamingSheet: View {
    let files: FilesModel
    @State var naming: FilesModel.Naming
    @State private var error: String?
    @Environment(\.dismiss) private var dismiss

    var title: String {
        switch naming.kind {
        case .newFile: "Neue Datei"
        case .newFolder: "Neuer Ordner"
        case .rename: "Umbenennen"
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title).font(.headline)
            TextField("Name", text: $naming.name)
                .textFieldStyle(.roundedBorder)
                .frame(width: 320)
                .onSubmit(commit)
                .onChange(of: naming.name) { error = files.check(naming.name) }
            if let error {
                Text(error).font(.callout).foregroundStyle(.red)
            }
            HStack {
                Spacer()
                Button("Abbrechen") { files.naming = nil; dismiss() }.keyboardShortcut(.cancelAction)
                Button(naming.kind == .rename ? "Umbenennen" : "Anlegen") { commit() }
                    .keyboardShortcut(.defaultAction)
                    .disabled(files.check(naming.name) != nil)
            }
        }
        .padding(20)
    }

    private func commit() {
        guard files.check(naming.name) == nil else { return }
        if let problem = files.commit(naming) {
            error = problem
        } else {
            dismiss()
        }
    }
}

/// Owner, group, size, dates and permissions (Linux: `PropertiesDialog.qml`).
private struct PropertiesSheet: View {
    let files: FilesModel
    let properties: FileProperties
    @State private var size: String?
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 12) {
                Image(nsImage: NSWorkspace.shared.icon(forFile: properties.path))
                    .resizable().frame(width: 48, height: 48)
                VStack(alignment: .leading) {
                    Text((properties.path as NSString).lastPathComponent).font(.headline)
                    Text(properties.kind).foregroundStyle(.secondary)
                }
            }
            Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 6) {
                row("Ort", (properties.path as NSString).deletingLastPathComponent)
                row("Größe", size ?? properties.size.map(Self.bytes) ?? "–")
                row("Geändert", Self.date(properties.modified))
                if let c = properties.created { row("Erstellt", Self.date(c)) }
                row("Besitzer", properties.owner)
                row("Gruppe", properties.group)
                row("Rechte", Self.mode(properties.mode))
                if let target = properties.symlinkTarget { row("Verweist auf", target) }
            }
            HStack {
                Spacer()
                Button("Schließen") { files.properties = nil; dismiss() }.keyboardShortcut(.defaultAction)
            }
        }
        .padding(20)
        .frame(minWidth: 420)
        .task {
            // Folders: the size of the whole tree, counted in the background.
            guard properties.size == nil, let tree = try? await files.files?.treeSize(path: properties.path) else { return }
            size = "\(Self.bytes(tree.bytes)) · \(tree.files) Dateien, \(tree.dirs) Ordner"
        }
    }

    private func row(_ label: String, _ value: String) -> some View {
        GridRow {
            Text(label).foregroundStyle(.secondary)
            Text(value).textSelection(.enabled)
        }
    }

    static func bytes(_ n: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(clamping: n), countStyle: .file)
    }

    static func date(_ t: Int64) -> String {
        Date(timeIntervalSince1970: TimeInterval(t)).formatted(date: .abbreviated, time: .shortened)
    }

    /// `rw-r--r--` of the permission bits.
    static func mode(_ mode: UInt32) -> String {
        let chars = Array("rwxrwxrwx")
        return String((0..<9).map { i in mode & (1 << (8 - UInt32(i))) != 0 ? chars[i] : "-" })
            + String(format: " (%o)", mode & 0o777)
    }
}
