// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import Foundation
import Observation

/// One open project (one window): the core project, its editor area and
/// session, and the events of this window.
@MainActor
@Observable
final class ProjectModel {
    let app: AppModel
    let core: Project
    let root: String
    let name: String
    let editor: EditorAreaModel
    var indexReady = false
    var indexText = ""
    var inspectorVisible = true
    /// Folders expanded in the file tree (part of the session; P3.1 changes it).
    var expandedDirs: [String]
    var quickOpenVisible = false
    /// Folders changed on disk (views reload them); bumped per event.
    private(set) var filesRevision = 0
    private(set) var changedDirs: [String] = []
    private(set) var notesRevision: UInt32 = 0
    /// Annotations of these PDFs changed (another window, sync); bumped per event.
    private(set) var annotationsRevision = 0
    private(set) var annotationsChangedPath = ""
    /// Every event, for features that listen themselves (search, calendar …).
    private(set) var lastEvent: Event?
    private var saveTask: Task<Void, Never>?
    /// Actions, background jobs and extension dialogs of this window (P3.10).
    let actions = ActionsModel()
    /// Short note at the bottom of the window (actions, exports, core messages).
    private(set) var notice = ""
    @ObservationIgnored private var noticeTask: Task<Void, Never>?
    /// Set by the terminal (P3.5): runs a prepared command in the terminal
    /// panel or (`ownTab`) in a terminal tab of its own.
    @ObservationIgnored var runInTerminal: ((_ prepared: Prepared, _ ownTab: Bool) -> Void)?
    /// Features that need every event (search batches arrive faster than
    /// SwiftUI observes `lastEvent`).
    @ObservationIgnored private var listeners: [UUID: (Event) -> Void] = [:]
    var searchVisible = false

    /// Calls `listener` with every event of this window until `stopListening(_:)`.
    func listen(_ listener: @escaping (Event) -> Void) -> UUID {
        let id = UUID()
        listeners[id] = listener
        return id
    }

    func stopListening(_ id: UUID) {
        listeners[id] = nil
    }

    init(app: AppModel, root: String) throws {
        self.app = app
        let box = WeakBox<ProjectModel>()
        core = try app.core.openProject(root: root, sink: MainActorEventSink { event in box.value?.handle(event) })
        self.root = core.root()
        name = core.name()
        editor = EditorAreaModel(layout: SessionLayout(json: core.sessionLayout(), root: core.root()))
        expandedDirs = core.sessionExpanded()
        let state = core.indexState()
        indexReady = state.ready
        indexText = state.text
        box.value = self
        actions.attach(self)
        editor.onChange = { [weak self] in self?.scheduleSave() }
        if UserDefaults.standard.string(forKey: "AtlasSearch") != nil { searchVisible = true }
        if let path = UserDefaults.standard.string(forKey: "AtlasOpenFile") {
            editor.open(path: core.absolutePath(relative: path), mode: .tab)
        }
    }

    private func handle(_ event: Event) {
        lastEvent = event
        for listener in listeners.values { listener(event) }
        switch event {
        case let .indexStatus(ready, text):
            indexReady = ready
            indexText = text
        case let .filesChanged(dirs):
            changedDirs = dirs
            filesRevision += 1
        case let .notesChanged(revision):
            notesRevision = revision
        case let .annotationsChanged(path):
            annotationsChangedPath = path
            annotationsRevision += 1
        case let .message(_, text):
            show(notice: text)
        default:
            break
        }
    }

    /// Saves the session shortly after the last change (the core writes in
    /// the background, in the format Linux reads, E12).
    func scheduleSave() {
        saveTask?.cancel()
        saveTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(400))
            guard !Task.isCancelled else { return }
            self?.saveNow()
        }
    }

    func saveNow() {
        let layout = editor.layout
        try? core.saveSession(
            layout: layout.json(root: root), expanded: expandedDirs, documents: layout.documents(root: root))
    }

    func show(notice text: String) {
        notice = text
        noticeTask?.cancel()
        noticeTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(4))
            guard !Task.isCancelled else { return }
            self?.notice = ""
        }
    }

    func close() {
        saveNow()
        core.flushSession()
    }
}
