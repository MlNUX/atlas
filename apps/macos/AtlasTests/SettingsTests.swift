// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Foundation
import Testing

@testable import Atlas

/// The user's config.toml is changed and restored.
@MainActor
@Suite(.serialized)
struct SettingsTests {
    @Test func themeIsAppliedAndPdfDarkIsTheDefault() throws {
        let app = AppModel()
        let settings = app.core.settings()
        let before = settings.get()
        defer {
            try? settings.save(settings: before)
            app.applyTheme()
        }
        var s = before
        s.theme = .dark
        s.pdfDark = true
        try settings.save(settings: s)
        app.applyTheme()
        #expect(NSApplication.shared.appearance?.name == .darkAqua)
        s.theme = .system
        try settings.save(settings: s)
        app.applyTheme()
        #expect(NSApplication.shared.appearance == nil)

        // New PDF tabs start dark.
        let dir = FileManager.default.temporaryDirectory.appending(path: "atlas-settings-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let project = try ProjectModel(app: app, root: dir.path)
        #expect(PDFModel(project: project, path: dir.appending(path: "fehlt.pdf").path).dark)
    }

    @Test func lastProjectIsTheOneOpenedLast() throws {
        let app = AppModel()
        let a = FileManager.default.temporaryDirectory.appending(path: "atlas-a-\(UUID().uuidString)")
        let b = FileManager.default.temporaryDirectory.appending(path: "atlas-b-\(UUID().uuidString)")
        for d in [a, b] { try FileManager.default.createDirectory(at: d, withIntermediateDirectories: true) }
        #expect(app.lastProject == nil)
        let pa = try #require(app.project(at: a.path))
        let pb = try #require(app.project(at: b.path))
        #expect(app.lastProject === pb)
        _ = app.project(at: a.path)
        #expect(app.lastProject === pa)
    }
}
