// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Foundation
import Testing

@testable import Atlas

/// P3.4 against the real core: language, colours, symbols, lines, saving.
@MainActor
struct CodeModelTests {
    func makeProject() throws -> (ProjectModel, String) {
        let dir = FileManager.default.temporaryDirectory.appending(path: "atlas-code-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir.appending(path: "src"), withIntermediateDirectories: true)
        let file = dir.appending(path: "src/main.rs").path
        try """
            struct Punkt {
                x: f64,
            }

            fn main() {
                println!("Hallo");
            }
            """.write(toFile: file, atomically: true, encoding: .utf8)
        try "#!/bin/sh\necho hallo\n".write(toFile: dir.appending(path: "run.sh").path, atomically: true, encoding: .utf8)
        let project = try ProjectModel(app: AppModel(), root: dir.path)
        return (project, file)
    }

    @Test func languageColoursAndSymbols() async throws {
        let (project, file) = try makeProject()
        let model = CodeModel(project: project, path: file, line: 0)
        await model.load()
        #expect(model.loaded)
        #expect(model.language == "Rust")
        // `fn` is a keyword.
        let text = model.editor.text as NSString
        let fn = text.range(of: "fn main").location
        let colour = model.editor.textView.textStorage?.attribute(.foregroundColor, at: fn, effectiveRange: nil) as? NSColor
        #expect(colour != NSColor.textColor, "keywords are coloured")
        #expect(model.symbols.contains { $0.name.contains("main") && $0.line == 5 })
        #expect(model.symbols.contains { $0.name.contains("Punkt") })
        #expect(!model.runnable)
        #expect(model.lineCount == 7)
    }

    @Test func goToLineAndSave() async throws {
        let (project, file) = try makeProject()
        let model = CodeModel(project: project, path: file, line: 5)
        await model.load()
        #expect(model.currentLine == 5, "opened on the line asked for")
        model.go(toLine: 2)
        #expect(model.currentLine == 2)
        model.editor.textView.insertText("    y: f64,\n", replacementRange: model.editor.textView.selectedRange())
        #expect(model.modified)
        model.close()
        #expect(try String(contentsOfFile: file, encoding: .utf8).contains("y: f64"), "closing saves")
    }

    @Test func scriptsAreRunnable() async throws {
        let (project, file) = try makeProject()
        let script = (file as NSString).deletingLastPathComponent.replacingOccurrences(of: "/src", with: "") + "/run.sh"
        let model = CodeModel(project: project, path: script, line: 0)
        await model.load()
        #expect(model.runnable)
    }
}
