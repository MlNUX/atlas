// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Foundation
import Observation

/// The context bar of one window (P3.6; Linux: `ContextPanel.qml`,
/// `ContextProfiles.qml`). Layouts, profiles and edits come from the core
/// (`Context`, `[context]` in `config.toml`), so the bar looks the same on
/// Linux and Mac.
@MainActor
@Observable
final class ContextModel {
    let project: ProjectModel
    private var context: Context { project.core.context() }

    /// Profile of the current tab (`default`, `pdf`, `markdown`, `code`, `calendar`).
    private(set) var profile = "default"
    private(set) var layout: ContextLayout
    private(set) var options: ContextOptions
    private(set) var catalog: [String: ContextBlock] = [:]
    private(set) var cards: [ExtensionCard] = []
    /// „overview“ or a tab id.
    var page = "overview"
    var editing = false
    /// Edits go to the current profile only, or to all („Gilt für alle Dateitypen“).
    var applyToAll = false
    /// Copy before the last hiding or removing (for „Rückgängig“).
    private(set) var undo: ContextSnapshot?
    var error: String?
    /// Bumped on calendar changes (blocks Heute, Woche, Aufgaben).
    private(set) var calendarRevision = 0
    let pomodoro: Pomodoro
    @ObservationIgnored private var token: UUID?

    init(project: ProjectModel) {
        self.project = project
        let context = project.core.context()
        layout = context.layout(profile: "default")
        let options = context.options()
        self.options = options
        pomodoro = Pomodoro(options: options)
        catalog = Dictionary(context.catalog().map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        token = project.listen { [weak self] event in
            switch event {
            case .calendarChanged: self?.calendarRevision += 1
            case .extensionsChanged, .settingsChanged: self?.reload()
            default: break
            }
        }
        reload()
    }

    func stop() {
        if let token { project.stopListening(token) }
        pomodoro.stop()
    }

    // MARK: layout

    /// The current tab's kind as the core names it.
    var kind: String {
        guard let tab = project.editor.currentTab else { return "" }
        if tab.path.hasPrefix("calendar:") { return "calendar" }
        if tab.isVirtual { return "" }
        return project.core.files().fileKind(path: tab.path)
    }

    /// Re-reads profile, layout and options (tab changed, settings changed).
    func reload() {
        let context = self.context
        options = context.options()
        pomodoro.options = options
        profile = context.profileFor(kind: kind)
        layout = context.layout(profile: profile)
        cards = project.core.extensions().contextCards()
        if page != "overview", !layout.tabs.contains(page) { page = "overview" }
    }

    /// Blocks of the shown page.
    var blocks: [String] {
        if page == "overview" { return layout.overview }
        return layout.pages.first { $0.id == page }?.blocks ?? []
    }

    var pageInfo: ContextPage? { layout.pages.first { $0.id == page } }

    /// Pages of one's own (not a built-in tab, not a single block as tab).
    var ownPages: [ContextPage] {
        layout.pages.filter { catalog[$0.id] == nil && !["links", "graph", "actions"].contains($0.id) && !$0.id.hasPrefix("ext:") }
    }

    /// A page of one block filling it (`links`, `graph`).
    var fullPage: Bool { pageInfo?.full ?? false }

    func isHidden(_ id: String) -> Bool { layout.hidden.contains(id) }
    func isCollapsed(_ id: String) -> Bool { layout.collapsed.contains(id) }

    // MARK: names

    static let texts: [String: (String, String)] = [
        "annotations": ("Anmerkungen", "Markierungen und Kommentare der PDF"),
        "outline": ("Gliederung", "Lesezeichen der PDF bzw. Überschriften der Notiz"),
        "links": ("Verknüpfungen", "Links und Backlinks"),
        "tags": ("Tags", "#Tags der Notiz, Klick sucht im Projekt"),
        "fileinfo": ("Dateiinfo", "Größe, Seiten, geändert, Pfad"),
        "graph": ("Graph", "Verbundene Dateien rund um diese"),
        "actions": ("Aktionen", "Eingebaute und eigene Aktionen für die Datei, kurz"),
        "builtin-actions": ("Für diese Datei", "Eingebaute Aktionen: anzeigen, exportieren, ausführen …"),
        "user-actions": ("Eigene Aktionen", "Aktionen des Projekts und eigene Befehle"),
        "today": ("Heute", "Termine, Blöcke und Fristen von heute"),
        "week": ("Diese Woche", "Fristen und Blöcke der nächsten 7 Tage"),
        "tasks": ("Aufgaben", "Offene Aufgaben abhaken und neue anlegen"),
        "pomodoro": ("Lernzeit", "Pomodoro: lernen, Pause, nach vier Runden lange Pause"),
    ]

    static let groups: [(String, String)] = [("file", "Diese Datei"), ("plan", "Planung"), ("ext", "Erweiterungen")]

    static func profileTitle(_ name: String) -> String {
        ["default": "Allgemein", "pdf": "PDF", "markdown": "Markdown", "code": "Code", "calendar": "Kalender"][name] ?? name
    }

    func card(_ id: String) -> ExtensionCard? {
        guard id.hasPrefix("ext:") else { return nil }
        return cards.first { "ext:" + $0.name == id }
    }

    func title(_ id: String) -> String {
        if let t = Self.texts[id] { return t.0 }
        if let c = card(id) { return c.title.isEmpty ? c.name : c.title }
        return id.hasPrefix("ext:") ? String(id.dropFirst(4)) : id
    }

    func detail(_ id: String) -> String {
        Self.texts[id]?.1 ?? card(id)?.description ?? ""
    }

    /// SF Symbol for the core's icon names.
    func symbol(_ id: String) -> String {
        if id.hasPrefix("ext:") { return "puzzlepiece.extension" }
        switch catalog[id]?.icon ?? "" {
        case "highlighter": return "highlighter"
        case "list": return "list.bullet.indent"
        case "link": return "link"
        case "bookmark": return "number"
        case "info": return "info.circle"
        case "graph": return "point.3.connected.trianglepath.dotted"
        case "play": return "play.circle"
        case "terminal": return "terminal"
        case "calendar": return id == "week" ? "calendar.day.timeline.left" : "calendar"
        case "check-square": return "checkmark.square"
        case "focus": return "timer"
        default: return "square.dashed"
        }
    }

    func tabTitle(_ id: String) -> String {
        if let p = layout.pages.first(where: { $0.id == id }), !p.title.isEmpty { return p.title }
        if let builtin = ["links": "Verknüpfungen", "graph": "Graph", "actions": "Aktionen"][id] { return builtin }
        return title(id)
    }

    func tabSymbol(_ id: String) -> String {
        if catalog[id] != nil || id.hasPrefix("ext:") { return symbol(id) }
        if let first = layout.pages.first(where: { $0.id == id })?.blocks.first { return symbol(first) }
        return "rectangle.stack"
    }

    // MARK: edit mode

    private var target: ContextTarget { applyToAll ? .all : .profile(name: profile) }

    /// The area of the shown page.
    var area: ContextArea { page == "overview" ? .overview : .page(id: page) }

    private func edit(_ work: () throws -> Void) {
        do {
            try work()
            error = nil
        } catch {
            self.error = atlasMessage(error)
        }
        reload()
    }

    private func keepUndo() { undo = context.snapshot() }

    func moveBy(_ id: String, in area: ContextArea, _ delta: Int) {
        edit { try context.moveBy(target: target, area: area, block: id, delta: Int64(delta)) }
    }

    func move(_ id: String, from: ContextArea, to: ContextArea) {
        edit { try context.moveBlock(target: target, block: id, from: from, to: to) }
    }

    func setHidden(_ id: String, _ hidden: Bool) {
        if hidden { keepUndo() }
        edit { try context.setHidden(target: target, block: id, hidden: hidden) }
    }

    func setCollapsed(_ id: String, _ collapsed: Bool) {
        // Collapsing is a reading aid: it also works outside the edit mode.
        edit { try context.setCollapsed(target: target, block: id, collapsed: collapsed) }
    }

    func unplace(_ id: String) {
        keepUndo()
        edit { try context.unplace(target: target, block: id) }
    }

    /// From the gallery onto the shown page (or as a tab of its own).
    func add(_ id: String, asTab: Bool = false) {
        edit { try context.moveBlock(target: target, block: id, from: .hidden, to: asTab ? .tabs : area) }
        if asTab { page = id }
    }

    func newPage(_ title: String) {
        edit { page = try context.newPage(target: target, title: title) }
    }

    func renamePage(_ id: String, _ title: String) {
        edit { try context.renamePage(target: target, id: id, title: title) }
    }

    func removeTab(_ id: String) {
        keepUndo()
        edit { try context.removeTab(target: target, id: id) }
    }

    func reset() {
        keepUndo()
        edit { try context.reset(profile: profile) }
    }

    func restoreUndo() {
        guard let undo else { return }
        self.undo = nil
        edit { try context.restore(snapshot: undo) }
    }

    /// Blocks placed nowhere in this layout (the gallery offers them).
    var unplaced: [String] {
        let placed = Set(layout.overview + layout.pinned + layout.tabs + layout.pages.flatMap(\.blocks))
        let all = context.catalog().map(\.id) + cards.map { "ext:" + $0.name }
        return all.filter { !placed.contains($0) && !["builtin-actions", "user-actions"].contains($0) }
    }

    func group(of id: String) -> String { id.hasPrefix("ext:") ? "ext" : catalog[id]?.group ?? "file" }
}

/// „Lernzeit“: work, short break, after four rounds a long one
/// (Linux: `PomodoroTimer.qml`; minutes from the bar's options).
@MainActor
@Observable
final class Pomodoro {
    enum Phase: String { case work = "Lernen", shortBreak = "Pause", longBreak = "Lange Pause" }

    var options: ContextOptions
    private(set) var phase: Phase = .work
    private(set) var running = false
    private(set) var remaining: Int
    /// Finished work rounds.
    private(set) var rounds = 0
    @ObservationIgnored private var task: Task<Void, Never>?

    init(options: ContextOptions) {
        self.options = options
        remaining = Int(options.pomodoroWork) * 60
    }

    private func minutes(_ phase: Phase) -> Int {
        switch phase {
        case .work: Int(options.pomodoroWork)
        case .shortBreak: Int(options.pomodoroBreak)
        case .longBreak: Int(options.pomodoroLongBreak)
        }
    }

    var text: String { String(format: "%02d:%02d", remaining / 60, remaining % 60) }

    func toggle() { running ? pause() : start() }

    func start() {
        running = true
        task?.cancel()
        task = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                guard let self, !Task.isCancelled else { return }
                self.tick()
            }
        }
    }

    func pause() {
        running = false
        task?.cancel()
    }

    func reset() {
        pause()
        phase = .work
        rounds = 0
        remaining = minutes(.work) * 60
    }

    func skip() { advance() }

    func stop() { task?.cancel() }

    func tick() {
        guard running else { return }
        remaining -= 1
        if remaining <= 0 { advance() }
    }

    private func advance() {
        if phase == .work {
            rounds += 1
            phase = rounds % 4 == 0 ? .longBreak : .shortBreak
        } else {
            phase = .work
        }
        remaining = minutes(phase) * 60
        NSSound.beep()
    }
}
