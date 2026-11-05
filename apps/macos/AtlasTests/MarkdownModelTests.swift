// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Foundation
import Testing

@testable import Atlas

/// P3.3 against the real core: editing keys, colours, completion, saving,
/// preview HTML and inserting.
@MainActor
struct MarkdownModelTests {
    func makeProject() throws -> (ProjectModel, String) {
        let dir = FileManager.default.temporaryDirectory.appending(path: "atlas-md-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir.appending(path: "Notizen"), withIntermediateDirectories: true)
        let note = dir.appending(path: "Notizen/Analysis.md").path
        try "# Analysis\n\n- [ ] Blatt 3\n\n[[Lineare Algebra]] #lernen\n".write(toFile: note, atomically: true, encoding: .utf8)
        try "# Lineare Algebra\n".write(toFile: dir.appending(path: "Notizen/Lineare Algebra.md").path, atomically: true, encoding: .utf8)
        let project = try ProjectModel(app: AppModel(), root: dir.path)
        return (project, note)
    }

    /// Text and cursor set as if typed.
    func type(_ controller: TextEditorController, _ text: String, cursor: Int? = nil) {
        controller.text = text
        controller.textView.setSelectedRange(NSRange(location: cursor ?? (text as NSString).length, length: 0))
    }

    @Test func listsAndTablesFollowTheCore() throws {
        let (project, _) = try makeProject()
        let editor = TextEditorController(editor: project.core.editor(), language: "Markdown", markdown: true)

        type(editor, "- eins")
        editor.textView.insertNewline(nil)
        #expect(editor.text == "- eins\n- ", "Return continues the list")
        editor.textView.insertTab(nil)
        let indented = editor.text.components(separatedBy: "\n").last ?? ""
        #expect(indented.hasPrefix(" ") || indented.hasPrefix("\t"), "Tab indents the item: \(indented)")
        #expect(indented.trimmingCharacters(in: .whitespaces).hasPrefix("-"))
        editor.textView.insertBacktab(nil)
        #expect(editor.text == "- eins\n- ", "Shift-Tab outdents again")
        // An empty item ends the list.
        editor.textView.insertNewline(nil)
        #expect(!editor.text.hasSuffix("- "))

        type(editor, "| a | b |\n|---|---|\n| 1 | 2 |", cursor: 3)
        editor.textView.insertTab(nil)
        #expect(editor.textView.selectedRange().location > 3, "Tab goes to the next cell")

        // Outside lists and tables Tab is a tab.
        type(editor, "Text")
        editor.textView.insertTab(nil)
        #expect(editor.text == "Text\t")
    }

    @Test func coloursAndLines() throws {
        let (project, _) = try makeProject()
        let editor = TextEditorController(editor: project.core.editor(), language: "Markdown", markdown: true)
        editor.text = "# Titel\n\nText mit **fett**\n"
        let heading = editor.textView.textStorage?.attribute(.foregroundColor, at: 2, effectiveRange: nil) as? NSColor
        #expect(heading == NSColor.controlAccentColor)
        editor.go(toLine: 3)
        #expect(editor.textView.selectedRange().location == 9)
    }

    @Test func completesWikiLinks() async throws {
        let (project, _) = try makeProject()
        // The note index is built in the background.
        for _ in 0..<50 where project.notesRevision == 0 { try await Task.sleep(for: .milliseconds(100)) }
        let editor = TextEditorController(editor: project.core.editor(), language: "Markdown", markdown: true)
        type(editor, "Siehe [[Lin")
        let core = project.core.editor()
        let context = try #require(core.completionAt(text: editor.text, position: UInt32((editor.text as NSString).length)))
        let items = core.completions(context: context, limit: 5)
        let item = try #require(items.first { $0.label.contains("Lineare") })
        editor.apply(core.acceptCompletion(text: editor.text, context: context, end: UInt32((editor.text as NSString).length), item: item))
        // The core writes the link as it resolves it (project-relative path).
        #expect(editor.text.hasPrefix("Siehe [["))
        #expect(editor.text.contains("Lineare Algebra"))
        #expect(editor.text.hasSuffix("]]"))
    }

    @Test func savesRendersAndInserts() async throws {
        let (project, note) = try makeProject()
        let model = MarkdownModel(project: project, path: note, line: 0)
        await model.load()
        #expect(model.loaded)
        #expect(model.info?.html.contains("<h1") == true)
        #expect(model.info?.html.contains("class=\"task\"") == true)

        // The checkbox of the preview toggles the source line.
        #expect(model.editor.toggleTask(line: 3, done: true))
        #expect(model.editor.text.contains("- [x] Blatt 3"))
        model.save()
        #expect(!model.modified)
        #expect(try String(contentsOfFile: note, encoding: .utf8).contains("- [x] Blatt 3"))

        model.mode = .edit
        model.insertTable(rows: 1, columns: 2)
        #expect(model.editor.text.contains("| Spalte 1 | Spalte 2 |"))
        model.insertDiagram(0)
        #expect(model.editor.text.contains("```mermaid\nflowchart TD"))
        #expect(model.modified)
        model.close()
        #expect(try String(contentsOfFile: note, encoding: .utf8).contains("flowchart TD"), "closing saves")
    }

    @Test func previewServesBundleFilesAndPictures() {
        #expect(PreviewSchemeHandler.picture(for: URL(string: "atlas://file/tmp/Bild%201.png")!)?.path == "/tmp/Bild 1.png")
        #expect(PreviewSchemeHandler.picture(for: URL(string: "atlas://file/etc/passwd")!) == nil, "only pictures")
        #expect(MarkdownPreview.jsString("a\"b\n") == "\"a\\\"b\\n\"")
    }
}
