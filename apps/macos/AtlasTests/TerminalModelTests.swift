// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Foundation
import Testing

@testable import Atlas

/// P3.5: the terminal panel's splits and session format, and a real
/// terminal (SwiftTerm, own PTY) with Atlas' shell and folder lookup.
@MainActor
struct TerminalModelTests {
    private func project() throws -> ProjectModel {
        let dir = FileManager.default.temporaryDirectory.appending(path: "atlas-terminal-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir.appending(path: "Notizen"), withIntermediateDirectories: true)
        try "# Log\n".write(to: dir.appending(path: "Notizen/Log.md"), atomically: true, encoding: .utf8)
        return try ProjectModel(app: AppModel(), root: dir.path)
    }

    @Test func startFolderOfTabs() {
        #expect(TerminalModel.startDir(of: "terminal:/a/b c#17", root: "/r") == "/a/b c")
        #expect(TerminalModel.startDir(of: "terminal:/a", root: "/r") == "/a")
        #expect(TerminalModel.startDir(of: "terminal:", root: "/r") == "/r")
    }

    @Test func splitsAsATreeInTheSession() throws {
        let project = try project()
        let model = TerminalModel(project: project)
        model.show()
        #expect(model.visible)
        #expect(model.groups.count == 1)
        let group = try #require(model.currentGroup)
        let first = try #require(group.active)

        // Beside, then below the second one: [1, [2 / 3]].
        model.split(vertical: false)
        let second = try #require(group.active)
        model.split(vertical: true)
        #expect(group.sessions.count == 3)
        #expect(!group.root.vertical)
        #expect(group.root.items.count == 2)
        guard case let .split(inner) = group.root.items[1] else { Issue.record("no nested split"); return }
        #expect(inner.vertical)
        let third = try #require(group.active)
        #expect(inner.sessions.map(\.id) == [second.id, third.id])

        // Stored like Linux: {visible, current, groups: [{orientation, items, terminals}]}.
        let state = try #require(project.editor.layout.extra["terminal"]?.object)
        #expect(state["visible"]?.bool == true)
        let saved = try #require(state["groups"]?.array?.first?.object)
        #expect(saved["orientation"]?.string == "horizontal")
        #expect(saved["items"]?.array?.count == 2)
        #expect(saved["items"]?.array?[1]["orientation"]?.string == "vertical")
        #expect(saved["terminals"]?.array?.count == 3)
        #expect(saved["items"]?.array?[0]["cwd"]?.string == project.root)

        // Ending the third leaves [1, 2] side by side.
        model.killActive()
        #expect(group.sessions.map(\.id) == [first.id, second.id])
        #expect(!group.root.vertical)
        #expect(group.root.items.count == 2)

        // A second window of the same session gets the same tree back.
        model.split(vertical: true)
        let restored = TerminalModel(project: project)
        #expect(restored.visible)
        let again = try #require(restored.currentGroup)
        #expect(again.sessions.count == 3)
        guard case .split = again.root.items[1] else { Issue.record("tree not restored"); return }

        // Hidden: the processes stay, the session remembers it.
        model.hide()
        #expect(project.editor.layout.extra["terminal"]?["visible"]?.bool == false)
        for s in model.groups.flatMap(\.sessions) + restored.groups.flatMap(\.sessions) { s.terminate() }
    }

    @Test func runsACommandAndReadsTheScreen() async throws {
        let project = try project()
        let model = TerminalModel(project: project)
        let prepared = Prepared(name: "echo", argv: ["/bin/echo", "Hallo aus dem Terminal"], display: "echo",
                                cwd: project.root, env: [], confirm: false, destructive: false, placement: .panel)
        let session = model.makeSession(dir: project.root, prepared: prepared)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 640, height: 300), styleMask: [.titled],
                              backing: .buffered, defer: false)
        window.contentView = session.view
        session.applyPalette(dark: true)
        session.start()
        for _ in 0..<250 where !session.screenText().contains("Hallo aus dem Terminal") {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(session.screenText().contains("Hallo aus dem Terminal"), "\(session.screenText())")
        // The status line of a finished command stays until Enter.
        for _ in 0..<100 where !session.screenText().contains("Beendet mit Status 0") {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(session.screenText().contains("Beendet mit Status 0"))
        #expect(session.outputText().contains("Hallo aus dem Terminal"))
        session.terminate()
        _ = window
    }

    @Test func shellFolderAndOutputToANote() async throws {
        let project = try project()
        let model = TerminalModel(project: project)
        let notes = project.root + "/Notizen"
        let session = model.makeSession(dir: notes)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 640, height: 300), styleMask: [.titled],
                              backing: .buffered, defer: false)
        window.contentView = session.view
        session.start()
        // The shell's folder comes from the process (`proc_pidinfo`); the
        // temporary folder may be reported as /private/var/….
        for _ in 0..<250 where !session.cwd().hasSuffix("/Notizen") || !session.started {
            try await Task.sleep(for: .milliseconds(20))
        }
        try await Task.sleep(for: .milliseconds(300))
        #expect(session.cwd().hasSuffix((project.root as NSString).lastPathComponent + "/Notizen"), "\(session.cwd())")

        model.send("$ ls\nLog.md", to: notes + "/Log.md")
        let text = try String(contentsOfFile: notes + "/Log.md", encoding: .utf8)
        #expect(text.contains("```console\n$ ls\nLog.md\n```"), "\(text)")
        session.terminate()
        _ = window
    }
}
