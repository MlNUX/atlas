// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Observation

/// Terminals of one project window (P3.5; Linux: `TerminalPanel.qml`,
/// `TerminalGroup.qml`): the panel below the editor area with its tabs
/// ("groups") of split terminals, and the terminals shown in editor tabs
/// (`terminal:<dir>#<n>`). The panel is stored in the session as on Linux
/// (`layout.terminal`: `visible`, `height`, `current`, `groups` as trees).
@MainActor
@Observable
final class TerminalModel {
    @ObservationIgnored private(set) weak var project: ProjectModel?
    private(set) var groups: [TerminalGroup] = []
    var current = 0
    private(set) var visible = false
    /// Height of the panel (pt), from and for the session.
    var height: Double = 260
    /// Text for „In Notiz übernehmen“ (sheet).
    var sendToNote: SendToNote?
    /// Terminals of editor tabs, by tab path (they outlive the tab view).
    @ObservationIgnored private var tabSessions: [String: TerminalSession] = [:]
    /// Commands waiting for their tab (`runInTerminal` with `ownTab`).
    @ObservationIgnored private var tabCommands: [String: Prepared] = [:]
    /// The saved panel, restored when the panel is first shown.
    @ObservationIgnored private var saved: [String: JSONValue]?
    @ObservationIgnored private var tabCounter = 0

    struct SendToNote: Identifiable {
        let id = UUID()
        let text: String
        let cwd: String
    }

    private static var models: [ObjectIdentifier: TerminalModel] = [:]

    /// The model of `project` (one per window).
    static func of(_ project: ProjectModel) -> TerminalModel {
        let key = ObjectIdentifier(project)
        if let m = models[key], m.project != nil { return m }
        let m = TerminalModel(project: project)
        models[key] = m
        return m
    }

    init(project: ProjectModel) {
        self.project = project
        let state = project.editor.layout.extra["terminal"]?.object
        saved = state
        if let h = state?["height"], case let .number(n) = h, n >= 80 { height = n }
        project.runInTerminal = { [weak self] prepared, ownTab in self?.run(prepared, ownTab: ownTab) }
        // CI (`defaults write de.minux.atlas AtlasTerminalPanel …`): show the
        // panel, optionally running a command in it.
        // Not in unit tests (they run inside the app, with the same defaults).
        let testing = ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil
        if !testing, let command = UserDefaults.standard.string(forKey: "AtlasTerminalPanel") {
            show()
            if !command.isEmpty {
                run(Prepared(name: "sh", argv: ["/bin/sh", "-c", command], display: command, cwd: project.root,
                             env: [], confirm: false, destructive: false, placement: .panel), ownTab: false)
            }
        } else if state?["visible"]?.bool == true {
            show()
        }
    }

    var currentGroup: TerminalGroup? {
        groups.indices.contains(current) ? groups[current] : groups.last
    }

    // MARK: - panel

    /// Shows or hides the panel (Linux: Strg+`); processes keep running.
    func toggle() {
        visible ? hide() : show()
    }

    func show() {
        visible = true
        if groups.isEmpty {
            if let saved, let list = saved["groups"]?.array, !list.isEmpty {
                for state in list.compactMap(\.object) { restoreGroup(state) }
                current = max(0, min(saved["current"]?.int ?? 0, groups.count - 1))
                self.saved = nil
            }
            if groups.isEmpty { newTerminal() }
        }
        save()
        focus()
    }

    func hide() {
        visible = false
        save()
    }

    /// A new terminal tab in the panel, in `dir` (default: the project).
    func newTerminal(in dir: String? = nil) {
        visible = true
        let group = TerminalGroup()
        groups.append(group)
        current = groups.count - 1
        add(makeSession(dir: dir ?? project?.root ?? NSHomeDirectory()), to: group.root, in: group)
        save()
        focus()
    }

    /// Splits the active terminal: the new one goes beside it (`vertical`
    /// false) or below it (Linux: `splitActive`).
    func split(vertical: Bool) {
        guard let group = currentGroup, let target = group.active,
              let container = group.root.parent(of: target) else { return newTerminal() }
        let session = makeSession(dir: target.cwd())
        let at = container.index(of: target) ?? container.items.count - 1
        if container.vertical == vertical || container.items.count == 1 {
            container.vertical = vertical
            container.items.insert(.terminal(session), at: at + 1)
        } else {
            // The other direction: a new split in the active terminal's place.
            let inner = TerminalSplit(vertical: vertical, items: [.terminal(target), .terminal(session)])
            container.items[at] = .split(inner)
        }
        hook(session, in: group)
        group.active = session
        save()
        focus()
    }

    /// Ends the active terminal of the current tab.
    func killActive() {
        guard let group = currentGroup, let session = group.active else { return }
        session.terminate()
        remove(session, from: group)
    }

    func selectGroup(_ index: Int) {
        current = index
        focus()
    }

    /// Removes a terminal; empty tabs go, an empty panel hides.
    func remove(_ session: TerminalSession, from group: TerminalGroup) {
        group.root.remove(session)
        group.root.collapse()
        if group.sessions.isEmpty {
            groups.removeAll { $0 === group }
            current = max(0, min(current, groups.count - 1))
            if groups.isEmpty { visible = false }
        } else if group.active === session {
            group.active = group.sessions.last
        }
        save()
        focus()
    }

    /// Keyboard focus to a tab's terminal.
    func focusTab(_ session: TerminalSession) {
        let view = session.view
        Task { @MainActor in view.window?.makeFirstResponder(view) }
    }

    /// Keyboard focus to the active terminal.
    func focus() {
        guard let view = currentGroup?.active?.view else { return }
        Task { @MainActor in view.window?.makeFirstResponder(view) }
    }

    // MARK: - commands and tabs

    /// Runs a prepared command (actions, scripts, external editors) in a new
    /// panel tab or in an editor tab of its own (Linux: `runCommand`).
    func run(_ prepared: Prepared, ownTab: Bool) {
        guard let project else { return }
        let dir = prepared.cwd.isEmpty ? project.root : prepared.cwd
        if ownTab {
            tabCounter += 1
            let path = "terminal:\(dir)#\(Int(Date().timeIntervalSince1970))\(tabCounter)"
            tabCommands[path] = prepared
            project.editor.open(path: path, mode: .tab)
            project.editor.setState(path: path, key: "transient", value: .bool(true))
            return
        }
        visible = true
        let group = TerminalGroup()
        groups.append(group)
        current = groups.count - 1
        add(makeSession(dir: dir, prepared: prepared), to: group.root, in: group)
        save()
        focus()
    }

    /// The terminal of the editor tab `path` (created on first use).
    func tabSession(_ path: String) -> TerminalSession {
        if let s = tabSessions[path] { return s }
        let prepared = tabCommands.removeValue(forKey: path)
        let tab = project?.editor.groups.flatMap(\.tabs).first { $0.path == path }
        let dir = tab?.extra["cwd"]?.string ?? Self.startDir(of: path, root: project?.root ?? NSHomeDirectory())
        let session = makeSession(dir: dir, prepared: prepared)
        session.onExit = { [weak self] in self?.closeTab(path) }
        session.onChange = { [weak self, weak session] in
            guard let session, prepared == nil else { return }
            self?.project?.editor.setState(path: path, key: "cwd", value: .string(session.cwd()))
        }
        session.menu = { [weak self, weak session] in
            guard let self, let session else { return NSMenu() }
            return self.menu(for: session, group: nil)
        }
        tabSessions[path] = session
        return session
    }

    /// Ends the terminal of a tab that is no longer open (the view calls it
    /// a moment after it disappears: dragging a tab keeps it).
    func tabMaybeClosed(_ path: String) {
        guard let project, tabSessions[path] != nil,
              !project.editor.groups.contains(where: { $0.tabs.contains { $0.path == path } }) else { return }
        tabSessions.removeValue(forKey: path)?.terminate()
    }

    private func closeTab(_ path: String) {
        tabSessions[path] = nil
        guard let project else { return }
        for (g, group) in project.editor.groups.enumerated() {
            if let t = group.tabs.firstIndex(where: { $0.path == path }) {
                project.editor.close(group: g, tab: t)
                return
            }
        }
    }

    /// `terminal:<dir>#<id>` → `<dir>` (empty: the project).
    static func startDir(of path: String, root: String) -> String {
        var rest = path.hasPrefix("terminal:") ? String(path.dropFirst(9)) : path
        if let hash = rest.lastIndex(of: "#") { rest = String(rest[..<hash]) }
        return rest.isEmpty ? root : rest
    }

    // MARK: - text to a note

    func askSendToNote(_ session: TerminalSession) {
        let text = session.outputText()
        if text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            project?.show(notice: "Keine Ausgabe zum Übernehmen")
        } else {
            sendToNote = SendToNote(text: text, cwd: session.cwd())
        }
    }

    /// Appends `text` as a `console` code block to `note`.
    func send(_ text: String, to note: String) {
        guard let project else { return }
        let block = project.core.editor().codeBlock(text: text, language: "console")
        do {
            _ = try project.core.notes().append(path: note, block: block)
            project.show(notice: "Ausgabe in \((note as NSString).lastPathComponent) übernommen")
        } catch {
            project.show(notice: "Übernehmen fehlgeschlagen: \(atlasMessage(error))")
        }
    }

    // MARK: - menu

    /// Context menu of a terminal (Linux: `TerminalPane.qml`); with `group`
    /// the panel's split commands too.
    func menu(for session: TerminalSession, group: TerminalGroup?) -> NSMenu {
        let menu = NSMenu()
        let selection = !(session.view.getSelection() ?? "").isEmpty
        menu.addItem(ClosureMenuItem("Kopieren", enabled: selection) { session.copy() })
        menu.addItem(ClosureMenuItem("Einfügen") { session.paste() })
        menu.addItem(.separator())
        menu.addItem(ClosureMenuItem(selection ? "Auswahl in Notiz übernehmen …" : "Ausgabe in Notiz übernehmen …") {
            [weak self] in self?.askSendToNote(session)
        })
        menu.addItem(ClosureMenuItem("Ausgabe kopieren") {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(session.outputText(), forType: .string)
        })
        menu.addItem(.separator())
        menu.addItem(ClosureMenuItem("Auswahl aufheben", enabled: selection) { session.view.selectNone() })
        if let group {
            menu.addItem(.separator())
            menu.addItem(ClosureMenuItem("Rechts teilen") { [weak self] in group.active = session; self?.split(vertical: false) })
            menu.addItem(ClosureMenuItem("Unten teilen") { [weak self] in group.active = session; self?.split(vertical: true) })
            menu.addItem(ClosureMenuItem("Terminal beenden") { [weak self] in
                session.terminate()
                self?.remove(session, from: group)
            })
        }
        return menu
    }

    // MARK: - sessions

    func makeSession(dir: String, prepared: Prepared? = nil) -> TerminalSession {
        TerminalSession(startDir: dir, prepared: prepared, projectRoot: project?.root ?? dir)
    }

    private func add(_ session: TerminalSession, to split: TerminalSplit, in group: TerminalGroup) {
        split.items.append(.terminal(session))
        hook(session, in: group)
        group.active = session
    }

    private func hook(_ session: TerminalSession, in group: TerminalGroup) {
        session.onExit = { [weak self, weak session, weak group] in
            guard let self, let session, let group else { return }
            self.remove(session, from: group)
        }
        session.onChange = { [weak self] in self?.saveSoon() }
        session.onFocus = { [weak session, weak group] in
            if let session, let group, group.active !== session { group.active = session }
        }
        session.menu = { [weak self, weak session, weak group] in
            guard let self, let session else { return NSMenu() }
            return self.menu(for: session, group: group)
        }
    }

    // MARK: - session file

    private func restoreGroup(_ state: [String: JSONValue]) {
        let group = TerminalGroup()
        func build(_ split: TerminalSplit, _ node: [String: JSONValue]) {
            split.vertical = node["orientation"]?.string == "vertical"
            for item in (node["items"]?.array ?? []).compactMap(\.object) {
                if let items = item["items"]?.array {
                    guard !items.isEmpty else { continue }
                    let inner = TerminalSplit(vertical: false, items: [])
                    build(inner, item)
                    if !inner.items.isEmpty { split.items.append(.split(inner)) }
                } else {
                    add(makeSession(dir: item["cwd"]?.string ?? project?.root ?? NSHomeDirectory()), to: split, in: group)
                }
            }
        }
        if let items = state["items"]?.array, !items.isEmpty { build(group.root, state) }
        if group.sessions.isEmpty {
            // Older sessions: one direction, a flat list.
            group.root.vertical = state["orientation"]?.string == "vertical"
            let list = state["terminals"]?.array?.compactMap(\.object) ?? []
            for t in list.isEmpty ? [[:]] : list {
                add(makeSession(dir: t["cwd"]?.string ?? project?.root ?? NSHomeDirectory()), to: group.root, in: group)
            }
        }
        group.root.collapse()
        groups.append(group)
    }

    /// The panel as stored in the session (same shape as on Linux).
    func serialize() -> JSONValue {
        var state = project?.editor.layout.extra["terminal"]?.object ?? [:]
        state["visible"] = .bool(visible)
        state["height"] = .number(height.rounded())
        if !groups.isEmpty || saved == nil {
            state["current"] = .number(Double(current))
            state["groups"] = .array(groups.map { $0.serialize() })
        }
        return .object(state)
    }

    @ObservationIgnored private var saveTask: Task<Void, Never>?

    func save() {
        saveTask?.cancel()
        project?.editor.setLayoutExtra("terminal", serialize())
    }

    /// Titles change often (each prompt): save a moment later.
    private func saveSoon() {
        saveTask?.cancel()
        saveTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .seconds(2))
            guard !Task.isCancelled else { return }
            self?.save()
        }
    }
}

/// A tab of the panel: terminals in nested splits.
@MainActor
@Observable
final class TerminalGroup: Identifiable {
    let id = UUID()
    let root = TerminalSplit(vertical: false, items: [])
    var active: TerminalSession?

    /// All terminals, in order.
    var sessions: [TerminalSession] { root.sessions }

    var title: String {
        let name = active?.title ?? sessions.first?.title ?? "Terminal"
        return sessions.count > 1 ? "\(name)  ⫴\(sessions.count)" : name
    }

    /// `{orientation, items: [{cwd} | {orientation, items}], terminals}`;
    /// programs started by Atlas are not restored (as on Linux).
    func serialize() -> JSONValue {
        guard case var .object(tree) = root.serialize() else { return .object([:]) }
        tree["terminals"] = .array(sessions.filter { $0.prepared == nil }.map { .object(["cwd": .string($0.cwd())]) })
        return .object(tree)
    }
}

/// A split: terminals and nested splits side by side or one below the other.
@MainActor
@Observable
final class TerminalSplit: Identifiable {
    let id = UUID()
    var vertical: Bool
    var items: [TerminalItem]

    init(vertical: Bool, items: [TerminalItem]) {
        self.vertical = vertical
        self.items = items
    }

    var sessions: [TerminalSession] {
        items.flatMap { item -> [TerminalSession] in
            switch item {
            case let .terminal(s): [s]
            case let .split(s): s.sessions
            }
        }
    }

    func index(of session: TerminalSession) -> Int? {
        items.firstIndex { if case let .terminal(s) = $0 { s === session } else { false } }
    }

    /// The split that holds `session` directly.
    func parent(of session: TerminalSession) -> TerminalSplit? {
        if index(of: session) != nil { return self }
        for case let .split(inner) in items {
            if let found = inner.parent(of: session) { return found }
        }
        return nil
    }

    func remove(_ session: TerminalSession) {
        items.removeAll { if case let .terminal(s) = $0 { s === session } else { false } }
        for case let .split(inner) in items { inner.remove(session) }
    }

    /// Empty splits go; a nested split with one part is replaced by that
    /// part; the outermost split takes over a lone nested split.
    func collapse() {
        items = items.compactMap { item in
            guard case let .split(inner) = item else { return item }
            inner.collapse()
            if inner.items.isEmpty { return nil }
            if inner.items.count == 1 { return inner.items[0] }
            return item
        }
        if items.count == 1, case let .split(inner) = items[0] {
            vertical = inner.vertical
            items = inner.items
        }
    }

    func serialize() -> JSONValue {
        let children: [JSONValue] = items.compactMap { item in
            switch item {
            case let .terminal(s): s.prepared == nil ? .object(["cwd": .string(s.cwd())]) : nil
            case let .split(inner): inner.serialize()
            }
        }
        return .object(["orientation": .string(vertical ? "vertical" : "horizontal"), "items": .array(children)])
    }
}

enum TerminalItem: Identifiable {
    case terminal(TerminalSession)
    case split(TerminalSplit)

    var id: UUID {
        switch self {
        case let .terminal(s): s.id
        case let .split(s): s.id
        }
    }
}
