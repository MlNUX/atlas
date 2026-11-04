// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import Foundation
import Testing

/// Calls every service that is filled in (P2.2, P2.4, P2.8, P2.10) once
/// through the Swift bindings; the others follow when their P2 task lands.
@MainActor
final class Events {
    var all: [Event] = []
}

@MainActor
func makeProject() throws -> (Atlas, Project, URL, Events) {
    let dir = FileManager.default.temporaryDirectory.appending(path: "atlaskit-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: dir.appending(path: "docs"), withIntermediateDirectories: true)
    try "# Notiz\n[[Zweite]] #wiki\n".write(to: dir.appending(path: "docs/Notiz.md"), atomically: true, encoding: .utf8)
    try "fourier\n".write(to: dir.appending(path: "docs/Zweite.md"), atomically: true, encoding: .utf8)
    let events = Events()
    let atlas = Atlas(sink: MainActorEventSink { _ in })
    let project = try atlas.openProject(root: dir.path, sink: MainActorEventSink { events.all.append($0) })
    return (atlas, project, dir, events)
}

@MainActor
func waitFor(_ events: Events, timeout: Double = 15, _ ok: (Event) -> Bool) async -> Bool {
    let end = Date().addingTimeInterval(timeout)
    while Date() < end {
        if events.all.contains(where: ok) { return true }
        try? await Task.sleep(for: .milliseconds(20))
    }
    return false
}

@MainActor
@Test func appAndProject() async throws {
    let (atlas, project, dir, events) = try makeProject()
    #expect(!atlas.version().isEmpty)
    #expect(atlas.places().contains { $0.icon == "home" })
    #expect(project.name() == dir.lastPathComponent)
    let indexed = await waitFor(events) { if case .indexStatus(true, _) = $0 { true } else { false } }
    #expect(indexed)
    #expect(project.files().quickOpen(query: "zweit", limit: 3).first?.relative == "docs/Zweite.md")
    try project.saveSession(layout: "{\"editor\":{}}", expanded: [], documents: [])
    project.flushSession()
    #expect(project.sessions().count == 1)
}

@MainActor
@Test func searchStreamsEventsToTheMainActor() async throws {
    let (_, project, _, events) = try makeProject()
    let id = try project.search().start(query: SearchQuery(
        pattern: "fourier", regex: false, caseSensitive: false, wholeWord: false,
        globs: [], buildDirs: false, kinds: [], root: nil))
    let done = await waitFor(events) { if case .searchFinished(let sid, _) = $0 { sid == id } else { false } }
    #expect(done)
    let hits = events.all.compactMap { if case .searchMatches(_, let m) = $0 { m } else { nil } }.flatMap { $0 }
    let summary = events.all.compactMap { if case .searchFinished(_, let s) = $0 { s } else { nil } }.first
    #expect(summary?.error == nil, "\(String(describing: summary))")
    #expect(hits.contains { $0.relative == "docs/Zweite.md" }, "\(hits) \(String(describing: summary)) rg=\(ProcessInfo.processInfo.environment["ATLAS_RG_BIN"] ?? "-")")
}

@MainActor
@Test func editorSettingsTerminalIlias() async throws {
    let (atlas, project, _, _) = try makeProject()
    let editor = project.editor()
    let edit = editor.continueList(text: "- eins", position: 6)
    #expect(edit?.text == "\n- ")
    let h = editor.highlighter(language: editor.language(path: "/x/a.rs") ?? "")
    #expect(h.update(text: "fn main() {}").contains { $0.kind == .keyword })
    #expect(!project.context().catalog().isEmpty)
    _ = atlas.settings().get()
    #expect(Terminal().palette(dark: true).ansi.count == 16)
    #expect(!atlas.ilias().config().baseUrl.isEmpty)
    let text = try await project.files().readText(path: project.absolutePath(relative: "docs/Zweite.md"), maxBytes: 100)
    #expect(text == "fourier\n")
}
