// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import Foundation
import Testing

@testable import Atlas

@MainActor
struct IliasModelTests {
    func item(_ kind: String, url: String, info: String = "", children: [IliasItem] = []) -> IliasItem {
        IliasItem(kind: kind, id: url, title: url, url: url, download: nil, info: info, children: children, error: "")
    }

    @Test func containersHaveChildrenFilesNone() {
        let file = item("file", url: "f")
        let empty = item("fold", url: "e")
        #expect(file.childrenOrNil == nil)
        #expect(empty.childrenOrNil?.isEmpty == true)
        #expect(item("crs", url: "c", children: [file]).childrenOrNil?.count == 1)
    }

    @Test func modelStartsWithoutNetwork() throws {
        let dir = FileManager.default.temporaryDirectory.appending(path: "atlas-ilias-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let app = AppModel()
        let project = try ProjectModel(app: app, root: dir.path)
        let model = IliasModel(project: project, ilias: app.core.ilias())
        #expect(model.state != .scanning)
        #expect(!model.isCurrent(item("file", url: "https://example.invalid/never-downloaded")))
        #expect(!model.login.visible)
        #expect(IliasLoginWindow.pageScript.contains("password"))
        #expect(IliasLoginWindow.fillScript.contains("submit"))
    }
}
