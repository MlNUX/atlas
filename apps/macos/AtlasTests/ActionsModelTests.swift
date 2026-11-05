// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Foundation
import Testing

@testable import Atlas

@MainActor
struct ActionsModelTests {
    func makeProject() throws -> (ProjectModel, String) {
        let dir = FileManager.default.temporaryDirectory.appending(path: "atlas-actions-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir.appending(path: ".atlas"), withIntermediateDirectories: true)
        try "eins zwei drei\n".write(to: dir.appending(path: "text.md"), atomically: true, encoding: .utf8)
        try """
            version: 1
            actions:
              - name: Wörter zählen
                applies_to: "*.md"
                command: [wc, -w, "{file}"]
              - name: Aufräumen
                when: project
                command: [rm, -rf, build]
            """.write(to: dir.appending(path: ".atlas/actions.yaml"), atomically: true, encoding: .utf8)
        let project = try ProjectModel(app: AppModel(), root: dir.path)
        return (project, project.core.absolutePath(relative: "text.md"))
    }

    @Test func runsWithoutTerminalInTheBackground() async throws {
        let (project, file) = try makeProject()
        let actions = project.actions
        let count = try #require(actions.actions(for: [file]).first { $0.name == "Wörter zählen" })
        #expect(actions.run(id: count.id, paths: [file]) == nil)
        #expect(actions.pending == nil)
        actions.refreshJobs()
        #expect(actions.jobs.contains { $0.key == count.id })
        #expect(!project.notice.isEmpty)
    }

    @Test func destructiveActionsAsk() throws {
        let (project, _) = try makeProject()
        let actions = project.actions
        let clean = try #require(actions.actions(for: []).first { $0.name == "Aufräumen" })
        #expect(actions.run(id: clean.id, paths: []) == nil)
        #expect(actions.pending?.prepared.destructive == true)
        actions.pending = nil
        #expect(actions.run(id: "weg", paths: []) != nil)
        #expect(actions.error != nil)
    }

    @Test func tabsShownThroughAnExtension() throws {
        let (project, file) = try makeProject()
        project.editor.open(path: file)
        project.editor.open(path: file, processor: "view:demo/karte")
        #expect(project.editor.groups[0].tabs.count == 2)
        #expect(project.editor.currentTab?.processor == "view:demo/karte")
        #expect(FeatureRouter.title(for: project.editor.currentTab!) == "text.md · Ansicht")
        project.editor.open(path: file)
        #expect(project.editor.groups[0].tabs.count == 2)
        #expect(project.editor.currentTab?.processor == "")
    }

    @Test func bridgeHelpers() {
        #expect(ExtensionPage.hex(NSColor(srgbRed: 1, green: 0, blue: 0.5, alpha: 1)) == "#ff0080")
        #expect(ExtensionPage.json("a\"b") == #""a\"b""#)
        let theme = ExtensionPage.theme(for: NSAppearance(named: .darkAqua)!)
        #expect(theme["accent"]?.hasPrefix("#") == true)
        #expect(theme.count == 14)
        #expect(atlasMessage(AtlasError.Invalid(message: "kaputt")) == "kaputt")
        // The shared web files are in the bundle; nothing outside them is served.
        let bridge = AtlasSchemeHandler.file(for: URL(string: "atlas://app/web/extension.js")!)
        #expect(bridge.map { FileManager.default.fileExists(atPath: $0.path) } == true)
        #expect(AtlasSchemeHandler.file(for: URL(string: "atlas://app/web/../Info.plist")!) == nil)
        #expect(AtlasSchemeHandler.file(for: URL(string: "atlas://app/etc/passwd")!) == nil)
    }
}
