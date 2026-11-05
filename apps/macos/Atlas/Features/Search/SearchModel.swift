// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Foundation
import Observation

/// Project-wide search (P3.8): runs `Search.start` in the core and collects
/// the streamed matches of this window.
@MainActor
@Observable
final class SearchModel {
    /// Type filter of the search window (core: `search::KINDS`).
    static let kinds: [(id: String, title: String)] = [
        ("markdown", "Notizen"), ("pdf", "PDF"), ("latex", "LaTeX"), ("code", "Code"), ("text", "Text"),
    ]

    let project: ProjectModel
    var pattern = ""
    var regex = false
    var caseSensitive = false
    var wholeWord = false
    var globs = ""
    var buildDirs = false
    var kinds: Set<String> = []
    private(set) var matches: [SearchMatch] = []
    private(set) var summary: SearchSummary?
    private(set) var running = false
    var error: String?
    var selected: SearchMatch?
    private(set) var context: [ContextLine] = []
    private var searchID: UInt64?
    @ObservationIgnored private var token: UUID?

    init(project: ProjectModel) {
        self.project = project
        token = project.listen { [weak self] event in self?.handle(event) }
    }

    func stop() {
        if let token { project.stopListening(token) }
        project.core.search().cancel()
    }

    func start() {
        matches = []
        summary = nil
        selected = nil
        context = []
        error = nil
        guard !pattern.isEmpty else { return }
        let query = SearchQuery(
            pattern: pattern, regex: regex, caseSensitive: caseSensitive, wholeWord: wholeWord,
            globs: globs.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty },
            buildDirs: buildDirs, kinds: Array(kinds), root: nil)
        do {
            searchID = try project.core.search().start(query: query)
            running = true
        } catch {
            self.error = "\(error)"
        }
    }

    func cancel() {
        project.core.search().cancel()
    }

    private func handle(_ event: Event) {
        switch event {
        case let .searchMatches(id, batch) where id == searchID:
            matches.append(contentsOf: batch)
        case let .searchFinished(id, s) where id == searchID:
            summary = s
            running = false
            if let e = s.error { error = e }
        default:
            break
        }
    }

    /// Matches grouped by file, in the order they arrived.
    var groups: [(path: String, relative: String, matches: [SearchMatch])] {
        var order: [String] = []
        var by: [String: [SearchMatch]] = [:]
        for m in matches {
            if by[m.path] == nil { order.append(m.path) }
            by[m.path, default: []].append(m)
        }
        return order.map { ($0, by[$0]?.first?.relative ?? $0, by[$0] ?? []) }
    }

    func count(of kind: String) -> UInt32 {
        summary?.kinds.first { $0.kind == kind }?.count ?? 0
    }

    func select(_ match: SearchMatch) {
        selected = match
        context = project.core.search().context(path: match.path, line: match.line, radius: 3, pdf: match.pdf)
    }

    /// Opens the file at the match (PDF: the page).
    func open(_ match: SearchMatch) {
        project.editor.open(path: match.path, line: Int(match.line), mode: .tab)
    }
}

extension SearchMatch {
    /// The line with the matches marked (ranges are UTF-16, like NSString).
    var attributed: AttributedString {
        let ns = text as NSString
        let out = NSMutableAttributedString(string: text)
        for r in ranges {
            let range = NSRange(location: Int(r.start), length: Int(r.end) - Int(r.start))
            guard NSMaxRange(range) <= ns.length else { continue }
            out.addAttribute(.backgroundColor, value: NSColor.findHighlightColor, range: range)
            out.addAttribute(.foregroundColor, value: NSColor.black, range: range)
        }
        return (try? AttributedString(out, including: \.appKit)) ?? AttributedString(text)
    }
}
