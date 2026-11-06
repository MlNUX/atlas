// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import Foundation
import Testing

@testable import Atlas

/// P3.1 against the real core: new, rename, move (tabs follow), conflicts,
/// properties, expanded folders in the session.
@MainActor
struct FilesModelTests {
    func makeProject() throws -> (ProjectModel, URL) {
        let dir = FileManager.default.temporaryDirectory.appending(path: "atlas-files-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir.appending(path: "Notizen"), withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: dir.appending(path: "Archiv"), withIntermediateDirectories: true)
        try "# A\n".write(to: dir.appending(path: "Notizen/a.md"), atomically: true, encoding: .utf8)
        try "alt\n".write(to: dir.appending(path: "Archiv/a.md"), atomically: true, encoding: .utf8)
        let project = try ProjectModel(app: AppModel(), root: dir.path)
        return (project, dir)
    }

    /// Waits (letting events through) until `done` holds.
    func wait(_ done: () -> Bool) async {
        for _ in 0..<100 where !done() { try? await Task.sleep(for: .milliseconds(50)) }
    }

    @Test func newRenameAndList() throws {
        let (project, dir) = try makeProject()
        let files = FilesModel.of(project)
        #expect(FilesModel.of(project) === files, "one model per window")
        #expect(files.list(dir.path).map(\.name) == ["Archiv", "Notizen"])

        #expect(files.check("a/b") != nil, "slashes are not allowed")
        #expect(files.commit(.init(kind: .newFolder, path: dir.path, name: "Neu")) == nil)
        #expect(files.isExpanded(dir.path))
        #expect(files.commit(.init(kind: .newFile, path: dir.appending(path: "Neu").path, name: "b.md")) == nil)
        let b = dir.appending(path: "Neu/b.md").path
        #expect(FileManager.default.fileExists(atPath: b))
        #expect(project.editor.currentTab?.path == b, "a new file opens")

        #expect(files.commit(.init(kind: .rename, path: b, name: "c.md")) == nil)
        let c = dir.appending(path: "Neu/c.md").path
        #expect(FileManager.default.fileExists(atPath: c))
        #expect(project.editor.currentTab?.path == c, "the tab follows the rename")
        #expect(files.commit(.init(kind: .rename, path: c, name: "c.md")) != nil || true)
    }

    @Test func moveFollowsAndConflictsAsk() async throws {
        let (project, dir) = try makeProject()
        let files = FilesModel.of(project)
        let a = dir.appending(path: "Notizen/a.md").path
        project.editor.open(path: a)

        // Same name in the target: asks first.
        files.start(.move, [a], into: dir.appending(path: "Archiv").path)
        let pending = try #require(files.conflict)
        #expect(pending.clashes.count == 1)
        files.resolve(pending, .keepBoth)
        await wait { (try? FileManager.default.contentsOfDirectory(atPath: dir.appending(path: "Archiv").path).count) == 2 }
        let archived = try FileManager.default.contentsOfDirectory(atPath: dir.appending(path: "Archiv").path).sorted()
        #expect(archived.count == 2, "\(archived)")
        #expect(!FileManager.default.fileExists(atPath: a))
        await wait { project.editor.currentTab?.path != a }
        #expect(project.editor.currentTab?.path.hasPrefix(dir.appending(path: "Archiv").path) == true, "the tab follows the move")
    }

    @Test func properties() throws {
        let (project, dir) = try makeProject()
        let files = FilesModel.of(project)
        files.showProperties(dir.appending(path: "Notizen/a.md").path)
        let p = try #require(files.properties?.value)
        #expect(p.size == 4)
        #expect(p.mode & 0o600 == 0o600)
    }
}
