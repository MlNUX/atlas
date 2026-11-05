// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import Foundation
import Testing

@testable import Atlas

/// P3.8 against the real core: streamed matches arrive in the window's model.
@MainActor
struct SearchModelTests {
    @Test func searchesTheProject() async throws {
        let dir = FileManager.default.temporaryDirectory.appending(path: "atlas-search-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir.appending(path: "Notizen"), withIntermediateDirectories: true)
        try "Die Fourier-Reihe\nund 🙂 Fourier-Analyse\n".write(
            to: dir.appending(path: "Notizen/Analysis.md"), atomically: true, encoding: .utf8)
        try "fn fourier() {}\n".write(to: dir.appending(path: "main.rs"), atomically: true, encoding: .utf8)
        let project = try ProjectModel(app: AppModel(), root: dir.path)
        let search = SearchModel(project: project)
        search.pattern = "fourier"
        search.start()
        for _ in 0..<300 where search.running { try await Task.sleep(for: .milliseconds(20)) }
        #expect(!search.running)
        #expect(search.summary?.matches == 3, "\(search.matches)")
        #expect(search.groups.count == 2)
        #expect(search.count(of: "markdown") == 2)

        // UTF-16 ranges mark the right characters, also after an emoji.
        let second = try #require(search.matches.first { $0.line == 2 && $0.relative == "Notizen/Analysis.md" })
        let r = try #require(second.ranges.first)
        #expect((second.text as NSString).substring(with: NSRange(location: Int(r.start), length: Int(r.end - r.start))) == "Fourier")

        search.select(second)
        #expect(search.context.contains { $0.line == 1 })
        search.kinds = ["code"]
        search.start()
        for _ in 0..<300 where search.running { try await Task.sleep(for: .milliseconds(20)) }
        #expect(search.matches.map(\.relative) == ["main.rs"])
        search.stop()
    }
}
