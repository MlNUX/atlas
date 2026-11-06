// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Observation

/// A text or code file in a tab (P3.4; Linux: `TextEditor.qml` for code):
/// the editor of P3.3 with the language of the file (syntect in the core),
/// symbols, go to line, find and replace (the text view's find bar).
/// Saved automatically a second after the last change, like notes.
@MainActor
@Observable
final class CodeModel {
    @ObservationIgnored let project: ProjectModel
    let path: String
    /// The core's language name (`Rust`, `Python` …), `nil` for plain text.
    let language: String?
    private(set) var loaded = false
    private(set) var error: String?
    private(set) var modified = false
    private(set) var conflict = false
    /// Functions, types, headings … of the file (Linux: the symbol list).
    private(set) var symbols: [Symbol] = []
    var symbolsVisible = false
    var goToLineVisible = false
    /// The file is a script Atlas can run (`.sh`, or executable with `#!`).
    let runnable: Bool
    @ObservationIgnored let editor: TextEditorController
    @ObservationIgnored private var lastSaved = ""
    @ObservationIgnored private var saveTask: Task<Void, Never>?
    @ObservationIgnored private var symbolTask: Task<Void, Never>?
    @ObservationIgnored private var listener: UUID?
    @ObservationIgnored private var startLine: Int
    @ObservationIgnored private var find: (text: String, caseSensitive: Bool, line: Int)?

    init(project: ProjectModel, path: String, line: Int) {
        self.project = project
        self.path = path
        startLine = line
        let core = project.core.editor()
        language = core.language(path: path)
        runnable = project.core.actions().isRunnable(path: path)
        editor = TextEditorController(editor: core, language: language, markdown: false)
        let tab = project.editor.groups.flatMap(\.tabs).first { $0.path == path }
        if let f = tab?.extra["find"]?.object, let text = f["text"]?.string, !text.isEmpty {
            find = (text, f["caseSensitive"]?.bool ?? false, f["line"]?.int ?? line)
        }
        // CI and screenshots: `-AtlasCodeSymbols YES` (or `defaults write`).
        symbolsVisible = tab?.extra["symbols"]?.bool ?? UserDefaults.standard.bool(forKey: "AtlasCodeSymbols")
        editor.onChange = { [weak self] text in self?.changed(text) }
        listener = project.listen { [weak self] event in self?.handle(event) }
    }

    func close() {
        if modified { save() }
        if let listener { project.stopListening(listener) }
        saveTask?.cancel()
        symbolTask?.cancel()
    }

    // MARK: - file

    func load() async {
        do {
            let text = try await project.core.files().readText(path: path, maxBytes: 20 << 20)
            lastSaved = text
            editor.text = text
            modified = false
            conflict = false
            loaded = true
            error = nil
            updateSymbols()
            if let find {
                if !editor.select(term: find.text, caseSensitive: find.caseSensitive, fromLine: find.line) {
                    editor.go(toLine: find.line)
                }
                self.find = nil
            } else if startLine > 0 {
                editor.go(toLine: startLine)
            }
            startLine = 0
        } catch {
            self.error = atlasMessage(error)
        }
    }

    private func changed(_ text: String) {
        modified = text != lastSaved
        saveTask?.cancel()
        if modified {
            saveTask = Task { [weak self] in
                try? await Task.sleep(for: .seconds(1))
                guard !Task.isCancelled else { return }
                self?.save()
            }
        }
        symbolTask?.cancel()
        symbolTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(400))
            guard !Task.isCancelled else { return }
            self?.updateSymbols()
        }
    }

    /// Writes the file now (⌘S, or a second after the last change).
    func save() {
        saveTask?.cancel()
        let text = editor.text
        do {
            try project.core.files().writeText(path: path, text: text)
            lastSaved = text
            modified = false
            conflict = false
        } catch {
            project.show(notice: "Nicht gespeichert: \(atlasMessage(error))")
        }
    }

    func reloadFromDisk() {
        Task { await load() }
    }

    private func handle(_ event: Event) {
        guard case let .fileModified(changed) = event, changed == path else { return }
        if modified {
            conflict = true
        } else {
            Task { [weak self] in
                guard let self else { return }
                let text = (try? await project.core.files().readText(path: path, maxBytes: 20 << 20)) ?? lastSaved
                if text != lastSaved { await load() }
            }
        }
    }

    // MARK: - symbols, lines

    func updateSymbols() {
        symbols = project.core.editor().symbols(path: path, text: editor.text)
    }

    /// Moves to `line` (1-based) and gives the editor the focus.
    func go(toLine line: Int) {
        editor.go(toLine: line)
        editor.textView.window?.makeFirstResponder(editor.textView)
    }

    var lineCount: Int {
        max(1, editor.text.reduce(into: 1) { n, c in if c == "\n" { n += 1 } })
    }

    /// The line of the cursor (1-based).
    var currentLine: Int {
        let text = editor.textView.string as NSString
        let end = min(editor.textView.selectedRange().location, text.length)
        var line = 1
        for i in 0..<end where text.character(at: i) == 10 { line += 1 }
        return line
    }

    // MARK: - find, run

    /// Shows the find bar (⌘F; with replace: ⌥⌘F).
    func showFind(replace: Bool) {
        let item = NSMenuItem()
        item.tag = Int((replace ? NSTextFinder.Action.showReplaceInterface : .showFindInterface).rawValue)
        editor.textView.window?.makeFirstResponder(editor.textView)
        editor.textView.performTextFinderAction(item)
    }

    /// Runs the script in the terminal (with the usual confirmation).
    func run() {
        if modified { save() }
        project.actions.runScript(path)
    }
}
