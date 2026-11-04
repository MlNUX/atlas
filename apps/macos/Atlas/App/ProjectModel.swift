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
    /// Every event, for features that listen themselves (search, calendar …).
    private(set) var lastEvent: Event?
    private var saveTask: Task<Void, Never>?

    init(app: AppModel, root: String) throws {
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
        editor.onChange = { [weak self] in self?.scheduleSave() }
        if let path = UserDefaults.standard.string(forKey: "AtlasOpenFile") {
            editor.open(path: core.absolutePath(relative: path), mode: .tab)
        }
    }

    private func handle(_ event: Event) {
        lastEvent = event
        switch event {
        case let .indexStatus(ready, text):
            indexReady = ready
            indexText = text
        case let .filesChanged(dirs):
            changedDirs = dirs
            filesRevision += 1
        case let .notesChanged(revision):
            notesRevision = revision
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

    func close() {
        saveNow()
        core.flushSession()
    }
}
