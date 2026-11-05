// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import Foundation
import Testing

@testable import Atlas

/// Serialized: the profiles live in the user's config.toml (restored after each test).
@MainActor
@Suite(.serialized)
struct ContextModelTests {
    func makeProject() throws -> (ProjectModel, String) {
        let dir = FileManager.default.temporaryDirectory.appending(path: "atlas-context-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir.appending(path: "Notizen"), withIntermediateDirectories: true)
        try "# Analysis\n\n## Folgen\n\n[[Algebra]] #lernen\n".write(to: dir.appending(path: "Notizen/Analysis.md"), atomically: true, encoding: .utf8)
        try "# Algebra\n".write(to: dir.appending(path: "Notizen/Algebra.md"), atomically: true, encoding: .utf8)
        let project = try ProjectModel(app: AppModel(), root: dir.path)
        return (project, project.core.absolutePath(relative: "Notizen/Analysis.md"))
    }

    @Test func layoutFollowsTheTabAndEditsAreUndone() throws {
        let (project, note) = try makeProject()
        let before = project.core.context().snapshot()
        defer { try? project.core.context().restore(snapshot: before) }

        let model = ContextModel(project: project)
        project.editor.open(path: note)
        model.reload()
        #expect(model.profile == project.core.context().profileFor(kind: "markdown"))
        #expect(!model.layout.overview.isEmpty)
        #expect(model.title("outline") == "Gliederung")

        let first = try #require(model.layout.overview.first)
        model.setHidden(first, true)
        #expect(model.isHidden(first))
        #expect(model.undo != nil)
        model.restoreUndo()
        #expect(!model.isHidden(first))

        model.newPage("Lernen")
        #expect(model.layout.tabs.contains(model.page))
        #expect(model.tabTitle(model.page) == "Lernen")
        #expect(model.ownPages.contains { $0.id == model.page })
        let page = model.page
        model.removeTab(page)
        #expect(!model.layout.tabs.contains(page))
        #expect(model.page == "overview")
        model.stop()
    }

    @Test func pomodoroRounds() throws {
        let (project, _) = try makeProject()
        let model = ContextModel(project: project)
        let pomodoro = model.pomodoro
        var options = pomodoro.options
        options.pomodoroWork = 1
        options.pomodoroBreak = 1
        pomodoro.options = options
        pomodoro.reset()
        pomodoro.start()
        for _ in 0..<60 { pomodoro.tick() }
        #expect(pomodoro.phase == .shortBreak)
        #expect(pomodoro.rounds == 1)
        pomodoro.skip()
        #expect(pomodoro.phase == .work)
        model.stop()
    }

    @Test func graphRings() {
        let graph = Graph(
            nodes: [GraphNode(path: "/a", title: "a", depth: 0, missing: false),
                    GraphNode(path: "/b", title: "b", depth: 1, missing: false),
                    GraphNode(path: "/c", title: "c", depth: 2, missing: true)],
            edges: [])
        let places = ContextGraphBlock.layout(graph, in: CGSize(width: 200, height: 200))
        #expect(places[0] == CGPoint(x: 100, y: 100))
        let d1 = hypot(places[1]!.x - 100, places[1]!.y - 100), d2 = hypot(places[2]!.x - 100, places[2]!.y - 100)
        #expect(d1 < d2)
    }
}
