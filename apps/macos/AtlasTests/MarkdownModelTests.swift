// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Foundation
import PDFKit
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
        // The text is laid out with a width (not in a container of width 0).
        let lm = try #require(editor.textView.layoutManager)
        let container = try #require(editor.textView.textContainer)
        lm.ensureLayout(for: container)
        #expect(lm.usedRect(for: container).width > 50)
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

    @Test func searchHitsAndLineNumbers() throws {
        let (project, _) = try makeProject()
        let editor = TextEditorController(editor: project.core.editor(), language: "Markdown", markdown: true)
        editor.text = "Eins\nzwei Wort\ndrei\nvier Wort\n"
        #expect(editor.select(term: "wort", caseSensitive: false, fromLine: 3))
        #expect(editor.textView.selectedRange() == NSRange(location: 25, length: 4), "the hit at or after line 3")
        #expect(!editor.select(term: "wort", caseSensitive: true, fromLine: 1))
        #expect(editor.scrollView.verticalRulerView is LineNumberRuler)
    }

    @Test func exportsPDF() async throws {
        let (project, note) = try makeProject()
        let model = MarkdownModel(project: project, path: note, line: 0)
        await model.load()
        let output = FileManager.default.temporaryDirectory.appending(path: "atlas-md-\(UUID().uuidString).pdf")
        #expect(await model.exportPDF(to: output))
        let pdf = try #require(PDFDocument(url: output))
        #expect(pdf.pageCount >= 1)
        #expect(pdf.string?.contains("Analysis") == true, "the note's text is in the PDF")
        model.close()
    }

    /// Dark pixels right of `skip` points of `view` drawn in a window.
    func darkPixels(_ view: NSView, skip: CGFloat) throws -> Int {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.appearance = NSAppearance(named: .aqua)
        view.frame = NSRect(x: 0, y: 0, width: 600, height: 400)
        window.contentView = view
        window.layoutIfNeeded()
        window.displayIfNeeded()
        let rep = try #require(view.bitmapImageRepForCachingDisplay(in: view.bounds))
        view.cacheDisplay(in: view.bounds, to: rep)
        let scale = CGFloat(rep.pixelsWide) / max(1, view.bounds.width)
        var dark = 0
        for y in stride(from: 0, to: rep.pixelsHigh, by: 2) {
            for x in stride(from: Int(skip * scale) + 4, to: rep.pixelsWide, by: 2) {
                if let c = rep.colorAt(x: x, y: y), c.brightnessComponent < 0.5 { dark += 1 }
            }
        }
        return dark
    }

    /// The editor draws its text in a window (CI showed line numbers but no
    /// text). Variants narrow down where it goes missing.
    @Test func drawsTextInAWindow() throws {
        let (project, _) = try makeProject()
        let text = "# Überschrift\n\nText Text Text Text Text\nMehr Text hier\n"
        let full = TextEditorController(editor: project.core.editor(), language: "Markdown", markdown: true)
        full.text = text
        let a = try darkPixels(full.scrollView, skip: full.scrollView.verticalRulerView?.frame.width ?? 0)
        let frames = "text \(full.textView.frame) clip \(full.scrollView.contentView.frame) "
            + "ruler \(full.scrollView.verticalRulerView?.frame ?? .zero) container \(full.textView.textContainer?.size ?? .zero)"

        let noRuler = TextEditorController(editor: project.core.editor(), language: "Markdown", markdown: true)
        noRuler.scrollView.rulersVisible = false
        noRuler.text = text
        let b = try darkPixels(noRuler.scrollView, skip: 0)

        let plain = TextEditorController(editor: project.core.editor(), language: nil, markdown: false)
        plain.scrollView.rulersVisible = false
        plain.text = text
        let c = try darkPixels(plain.scrollView, skip: 0)

        let bare = NSTextView.scrollableTextView()
        (bare.documentView as? NSTextView)?.string = text
        let d = try darkPixels(bare, skip: 0)

        if a < 50 {
            Issue.record("dark pixels – editor: \(a), without ruler: \(b), without colours: \(c), bare NSTextView: \(d); \(frames)")
        }
    }
}
