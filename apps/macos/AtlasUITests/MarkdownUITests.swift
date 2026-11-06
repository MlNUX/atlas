// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import XCTest

/// S1 (steps 1–3): open, edit and save Markdown automatically.
final class MarkdownUITests: AtlasUITestCase {
    func testS1MarkdownOpenEditAutosave() throws {
        launch(open: "notes/idee.md")
        let editor = app.textViews.firstMatch
        check("Markdown-Ansicht", editor.waitForExistence(timeout: 10))
        check("Markdown-Text geladen", waitFor { (editor.value as? String)?.contains("# Einleitung") == true },
              String(describing: editor.value).prefix(80).description)

        // Type at the end of the note; Atlas saves a second later.
        editor.click()
        editor.typeKey(.downArrow, modifierFlags: .command)
        editor.typeText("\nAutosave-Test [Plan](plan.md#ziele)\n")
        check("Bearbeitung erkannt", (editor.value as? String)?.contains("Autosave-Test") == true)
        check("Autosave geschrieben", waitFor(6) { self.read("notes/idee.md").contains("Autosave-Test [Plan](plan.md#ziele)") },
              read("notes/idee.md").suffix(80).description)
        // The front matter and the rest stay as they were.
        let text = read("notes/idee.md")
        check("Datei sonst unverändert", text.hasPrefix("---\ntitle: Idee\n") && text.contains("```rust\nfn main() {}\n```"),
              text.prefix(60).description)
    }
}
