// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Observation
import UniformTypeIdentifiers

/// File operations of one project window (P3.1; Linux: `FileActions.qml`,
/// `file_ops.rs`): new, rename, copy/cut/paste (Finder compatible), drag and
/// drop, trash, properties, Quick Look. After a move, tabs, links, relations,
/// annotations and calendar links follow (as on Linux).
@MainActor
@Observable
final class FilesModel {
    @ObservationIgnored private(set) weak var project: ProjectModel?
    /// The selected files and folders (sidebar or folder view).
    var selection: [String] = []
    /// Lists with the keyboard focus („sidebar“, „folder:<dir>“): the menu
    /// items with shortcuts (⌘⌫, ⌘Y, ⌘I) act only then – in an editor ⌘⌫
    /// deletes to the start of the line.
    private(set) var focusedLists: Set<String> = []
    var hasListFocus: Bool { !focusedLists.isEmpty }

    func setFocus(_ list: String, _ focused: Bool) {
        if focused { focusedLists.insert(list) } else { focusedLists.remove(list) }
    }
    /// Folder new files and pasted items go to when nothing else is given.
    var targetDir: String = ""
    /// The file shown with Quick Look (space).
    var quickLook: URL?
    var quickLookItems: [URL] = []
    /// Dialogs.
    var naming: Naming?
    var conflict: PendingConflict?
    var properties: IdentifiableBox<FileProperties>?
    /// Hidden files are shown (folder view and tree).
    var showHidden = false
    @ObservationIgnored private var operations: [UInt64: Operation] = [:]
    @ObservationIgnored private var listener: UUID?

    struct Naming: Identifiable {
        enum Kind { case newFile, newFolder, rename }
        let id = UUID()
        let kind: Kind
        /// The folder (new) or the item (rename).
        let path: String
        var name: String
    }

    struct PendingConflict: Identifiable {
        let id = UUID()
        let kind: Transfer
        let paths: [String]
        let dest: String
        let clashes: [String]
    }

    private enum Operation {
        case transfer(Transfer)
        case trash([String])
    }

    /// Pasteboard marker: the files were cut, a paste moves them.
    static let cutType = NSPasteboard.PasteboardType("de.minux.atlas.cut")

    private static var models: [ObjectIdentifier: FilesModel] = [:]

    /// The model of `project` (one per window).
    static func of(_ project: ProjectModel) -> FilesModel {
        let key = ObjectIdentifier(project)
        if let m = models[key], m.project != nil { return m }
        let m = FilesModel(project: project)
        models[key] = m
        return m
    }

    private init(project: ProjectModel) {
        self.project = project
        targetDir = project.root
        listener = project.listen { [weak self] event in self?.handle(event) }
        watch()
    }

    var files: Files? { project?.core.files() }

    // MARK: - listing

    func list(_ dir: String, sort: SortKey = .name, ascending: Bool = true, foldersOnly: Bool = false) -> [FileEntry] {
        let options = ListOptions(showHidden: showHidden, foldersOnly: foldersOnly, sort: sort, ascending: ascending)
        return (try? files?.list(dir: dir, options: options)) ?? []
    }

    /// Watches the project, its expanded folders and `extra` (open folder views).
    func watch(extra: [String] = []) {
        guard let project else { return }
        files?.watch(dirs: Array(Set([project.root] + project.expandedDirs + extra)))
    }

    func isExpanded(_ dir: String) -> Bool {
        project?.expandedDirs.contains(dir) ?? false
    }

    func setExpanded(_ dir: String, _ expanded: Bool) {
        guard let project else { return }
        if expanded, !project.expandedDirs.contains(dir) {
            project.expandedDirs.append(dir)
        } else if !expanded {
            project.expandedDirs.removeAll { $0 == dir || $0.hasPrefix(dir + "/") }
        }
        project.scheduleSave()
        watch()
    }

    // MARK: - open

    func open(_ path: String, newSplit: Bool = false) {
        project?.editor.open(path: path, mode: newSplit ? .split : .tab)
    }

    func revealInFinder(_ paths: [String]) {
        NSWorkspace.shared.activateFileViewerSelecting(paths.map { URL(filePath: $0) })
    }

    func copyPath(_ paths: [String]) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(paths.joined(separator: "\n"), forType: .string)
    }

    func showQuickLook(_ path: String, among: [String] = []) {
        let items = (among.isEmpty ? [path] : among).filter { !(files?.isDir(path: $0) ?? false) }
        quickLookItems = items.map { URL(filePath: $0) }
        quickLook = URL(filePath: path)
    }

    // MARK: - new, rename

    func askNewFile(in dir: String) { naming = Naming(kind: .newFile, path: dir, name: "Neue Datei.md") }
    func askNewFolder(in dir: String) { naming = Naming(kind: .newFolder, path: dir, name: "Neuer Ordner") }
    func askRename(_ path: String) {
        naming = Naming(kind: .rename, path: path, name: (path as NSString).lastPathComponent)
    }

    /// Error text for `name`, `nil` if it can be used.
    func check(_ name: String) -> String? {
        files?.checkName(name: name)
    }

    /// Carries out the naming dialog; `nil` on success, else the error.
    func commit(_ naming: Naming) -> String? {
        guard let files else { return nil }
        do {
            switch naming.kind {
            case .newFile:
                let path = try files.newFile(dir: naming.path, name: naming.name)
                open(path)
            case .newFolder:
                _ = try files.newFolder(dir: naming.path, name: naming.name)
                setExpanded(naming.path, true)
            case .rename:
                let to = try files.rename(path: naming.path, newName: naming.name)
                moved([(naming.path, to)])
            }
            self.naming = nil
            return nil
        } catch {
            return atlasMessage(error)
        }
    }

    // MARK: - copy, cut, paste, drop

    func copy(_ paths: [String], cut: Bool = false) {
        let board = NSPasteboard.general
        board.clearContents()
        board.writeObjects(paths.map { URL(filePath: $0) as NSURL })
        if cut { board.setString("1", forType: Self.cutType) }
        project?.show(notice: "\(paths.count) \(paths.count == 1 ? "Eintrag" : "Einträge") \(cut ? "ausgeschnitten" : "kopiert")")
    }

    /// Files on the pasteboard (from Atlas or the Finder).
    var pasteboardFiles: [String] {
        let urls = NSPasteboard.general.readObjects(
            forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL] ?? []
        return urls.map { $0.path(percentEncoded: false) }
    }

    func paste(into dir: String) {
        let paths = pasteboardFiles
        guard !paths.isEmpty else { return }
        let cut = NSPasteboard.general.string(forType: Self.cutType) != nil
        start(cut ? .move : .copy, paths, into: dir)
        if cut { NSPasteboard.general.clearContents() }
    }

    /// Dropped items: a move inside the project, a copy from outside (as
    /// the Finder does between volumes); with ⌥ always a copy.
    func drop(_ urls: [URL], into dir: String) -> Bool {
        guard let project else { return false }
        let paths = urls.filter(\.isFileURL).map { $0.path(percentEncoded: false) }
            .filter { $0 != dir && !dir.hasPrefix($0 + "/") }
        guard !paths.isEmpty else { return false }
        let option = NSEvent.modifierFlags.contains(.option)
        let inside = paths.allSatisfy { $0.hasPrefix(project.root + "/") }
        start(inside && !option ? .move : .copy, paths, into: dir)
        return true
    }

    /// Copies or moves; asks first if names exist at the target.
    func start(_ kind: Transfer, _ paths: [String], into dest: String) {
        guard let files else { return }
        let clashes = files.conflicts(paths: paths, dest: dest)
        if clashes.isEmpty {
            run(kind, paths, dest, .abort)
        } else {
            conflict = PendingConflict(kind: kind, paths: paths, dest: dest, clashes: clashes)
        }
    }

    func resolve(_ pending: PendingConflict, _ policy: ConflictPolicy) {
        conflict = nil
        run(pending.kind, pending.paths, pending.dest, policy)
    }

    private func run(_ kind: Transfer, _ paths: [String], _ dest: String, _ policy: ConflictPolicy) {
        guard let files else { return }
        let id = files.transfer(kind: kind, paths: paths, dest: dest, policy: policy)
        operations[id] = .transfer(kind)
    }

    // MARK: - trash

    func trash(_ paths: [String]) {
        guard let files, !paths.isEmpty else { return }
        let id = files.moveToTrash(paths: paths)
        operations[id] = .trash(paths)
    }

    func restore(_ ids: [String]) {
        do {
            try files?.restoreFromTrash(ids: ids)
        } catch {
            project?.show(notice: atlasMessage(error))
        }
    }

    func purge(_ ids: [String]) {
        do {
            try files?.deleteFromTrash(ids: ids)
        } catch {
            project?.show(notice: atlasMessage(error))
        }
    }

    // MARK: - properties

    func showProperties(_ path: String) {
        do {
            if let p = try files?.properties(path: path) { properties = IdentifiableBox(value: p) }
        } catch {
            project?.show(notice: atlasMessage(error))
        }
    }

    // MARK: - events

    private func handle(_ event: Event) {
        guard case let .fileOperation(id, done, text, error) = event, let op = operations[id] else { return }
        guard done else { return }
        operations[id] = nil
        if let error {
            project?.show(notice: error)
            return
        }
        switch op {
        case .transfer(.move):
            let pairs = text.split(separator: "\n").compactMap { line -> (String, String)? in
                let parts = line.components(separatedBy: " → ")
                return parts.count == 2 ? (parts[0], parts[1]) : nil
            }
            moved(pairs)
            project?.show(notice: "\(pairs.count) verschoben")
        case .transfer(.copy):
            project?.show(notice: "Kopiert")
        case let .trash(paths):
            trashed(paths)
        }
    }

    /// Tabs, links, relations, annotations and calendar links follow a move.
    private func moved(_ pairs: [(String, String)]) {
        guard let project else { return }
        for (from, to) in pairs {
            project.editor.pathMoved(from: from, to: to)
            _ = try? project.core.notes().filesMoved(from: from, to: to)
            project.core.calendar().filesMoved(from: from, to: to)
            project.expandedDirs = project.expandedDirs.map {
                $0 == from ? to : ($0.hasPrefix(from + "/") ? to + $0.dropFirst(from.count) : $0)
            }
        }
        selection = pairs.map(\.1)
        project.scheduleSave()
    }

    /// Tabs of trashed files close; the note index forgets them.
    private func trashed(_ paths: [String]) {
        guard let project else { return }
        project.core.notes().filesTrashed(paths: paths)
        for path in paths { closeTabs(under: path) }
        selection.removeAll { paths.contains($0) }
        project.show(notice: "\(paths.count) in den Papierkorb gelegt")
    }

    private func closeTabs(under path: String) {
        guard let project else { return }
        var found = true
        while found {
            found = false
            for (g, group) in project.editor.groups.enumerated() {
                if let t = group.tabs.firstIndex(where: { $0.path == path || $0.path.hasPrefix(path + "/") }) {
                    project.editor.close(group: g, tab: t)
                    found = true
                    break
                }
            }
        }
    }
}
