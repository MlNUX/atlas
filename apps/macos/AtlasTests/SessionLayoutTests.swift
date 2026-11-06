// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import Foundation
import Testing

@testable import Atlas

/// The session layout is exchanged with Linux (E12): it must read what the
/// Linux app writes and keep what it does not understand.
struct SessionLayoutTests {
    let root = "/Users/a/Projekt"

    /// Shaped like `EditorArea.qml`/`EditorGroup.qml` serialize it.
    let linux = """
    {"version":2,"sidebarWidth":280,"terminal":{"visible":true,"groups":[]},
     "editor":{"orientation":"horizontal","active":1,"groups":[
       {"temporary":false,"current":0,"tabs":[{"path":"docs/a.md","line":3,"preview":false,"scroll":0.5}]},
       {"temporary":false,"current":1,"tabs":[{"path":"skript.pdf","line":0,"preview":false,"page":12,"zoom":1.5},
                                              {"path":"terminal:/Users/a/Projekt#1","line":0,"preview":false}]}]}}
    """

    @Test func readsTheLinuxLayout() {
        let l = SessionLayout(json: linux, root: root)
        #expect(l.groups.count == 2)
        #expect(l.active == 1)
        #expect(l.groups[0].tabs[0].path == "/Users/a/Projekt/docs/a.md")
        #expect(l.groups[0].tabs[0].line == 3)
        #expect(l.groups[1].current == 1)
        #expect(l.groups[1].tabs[1].path == "terminal:/Users/a/Projekt#1")
        #expect(l.documents(root: root) == ["/Users/a/Projekt/docs/a.md", "/Users/a/Projekt/skript.pdf"])
    }

    @Test func keepsWhatItDoesNotKnow() throws {
        let back = SessionLayout(json: linux, root: root).json(root: root)
        let original = try JSONDecoder().decode(JSONValue.self, from: Data(linux.utf8))
        let written = try JSONDecoder().decode(JSONValue.self, from: Data(back.utf8))
        #expect(written == original)
    }

    @Test func readsPhaseZeroAndBrokenLayouts() {
        let old = SessionLayout(json: #"{"editor":{"panes":[{"path":"a.md"},{"path":"b.md"}]}}"#, root: root)
        #expect(old.groups.count == 2)
        #expect(old.groups[1].tabs[0].path == "/Users/a/Projekt/b.md")
        #expect(SessionLayout(json: "{kaputt", root: root).groups.count == 1)
        #expect(SessionLayout(json: "", root: root).groups[0].tabs.isEmpty)
    }
}

@MainActor
struct EditorAreaModelTests {
    @Test func tabsAndSplits() {
        let model = EditorAreaModel(layout: SessionLayout())
        var changes = 0
        model.onChange = { changes += 1 }
        model.open(path: "/p/a.md")
        model.open(path: "/p/b.md")
        model.open(path: "/p/a.md", line: 7)
        #expect(model.groups[0].tabs.map(\.path) == ["/p/a.md", "/p/b.md"])
        #expect(model.currentTab?.line == 7, "existing tab reused")
        model.open(path: "/p/c.pdf", mode: .split)
        #expect(model.groups.count == 2)
        #expect(model.active == 1)
        model.splitCurrentTab()
        #expect(model.groups.count == 2, "the only tab of a split does not leave an empty split")
        model.close(group: 1, tab: 0)
        #expect(model.groups.count == 1)
        for i in 0..<6 { model.open(path: "/p/\(i)", mode: .split) }
        #expect(model.groups.count == EditorAreaModel.maxGroups)
        model.pathMoved(from: "/p", to: "/q")
        #expect(model.groups.allSatisfy { $0.tabs.allSatisfy { $0.path.hasPrefix("/q/") } })
        #expect(changes > 0)
    }

    @Test func tabStateIsSavedAndOnlyOnChange() throws {
        let model = EditorAreaModel(layout: SessionLayout())
        var changes = 0
        model.onChange = { changes += 1 }
        model.open(path: "/p/a.md")
        model.open(path: "/p/a.md", processor: "view:x/y")
        changes = 0
        model.setState(path: "/p/a.md", key: "mode", value: .string("split"))
        model.setState(path: "/p/a.md", key: "mode", value: .string("split"))
        #expect(changes == 1)
        let plain = try #require(model.groups[0].tabs.first { $0.processor.isEmpty })
        #expect(plain.extra["mode"] == .string("split"))
        // The tab through an extension keeps its own state.
        #expect(model.groups[0].tabs.first { !$0.processor.isEmpty }?.extra["mode"] == nil)
        let json = model.layout.json(root: "/p")
        #expect(json.contains("\"mode\""))
        model.setState(path: "/p/a.md", key: "mode", value: nil)
        #expect(model.groups[0].tabs.first { $0.processor.isEmpty }?.extra["mode"] == nil)
    }
}
