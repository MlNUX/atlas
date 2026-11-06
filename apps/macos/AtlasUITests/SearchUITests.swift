// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import XCTest

/// S3 (steps 6–14): project search with options, type filter, PDF text,
/// opening a match.
final class SearchUITests: AtlasUITestCase {
    /// The summary line „N Treffer in M Dateien · … ms“ (nil while searching).
    private func summary(_ sheet: XCUIElement) -> (matches: Int, files: Int)? {
        for text in texts(in: sheet) where text.contains(" Treffer in ") {
            let numbers = text.split(whereSeparator: { !$0.isNumber }).compactMap { Int($0) }
            if numbers.count >= 2 { return (numbers[0], numbers[1]) }
        }
        return nil
    }

    /// Searches `pattern` and waits for the new summary.
    private func search(_ sheet: XCUIElement, _ pattern: String) -> (matches: Int, files: Int)? {
        let field = sheet.textFields["Im Projekt suchen"]
        field.click()
        field.typeKey("a", modifierFlags: .command)
        field.typeText(pattern + "\r")
        var last: (Int, Int)?
        // The summary appears when the search is done (old one replaced).
        waitFor(10) {
            last = self.summary(sheet)
            return last != nil && !sheet.buttons["Abbrechen"].exists
        }
        return last
    }

    private func toggle(_ sheet: XCUIElement, _ title: String) {
        sheet.checkBoxes[title].click()
    }

    func testS3ProjectSearch() throws {
        launch(open: "notes/idee.md")
        app.typeKey("f", modifierFlags: [.command, .shift])
        let sheet = app.sheets.firstMatch
        check("Suche im eigenen Fenster vor Atlas", sheet.waitForExistence(timeout: 5))

        var r = search(sheet, "EINZIGARTIG")
        check("Suche ohne Build-Ordner", r?.matches == 1 && r?.files == 1, "\(String(describing: r))")
        let hit = sheet.staticTexts.containing(NSPredicate(format: "value CONTAINS 'TODO: EINZIGARTIG' OR label CONTAINS 'TODO: EINZIGARTIG'")).firstMatch
        check("Suchtreffer markiert", hit.exists, texts(in: sheet).joined(separator: " | "))

        toggle(sheet, "Build-Ordner")
        r = search(sheet, "EINZIGARTIG")
        check("Suche mit Build-Ordnern", r?.matches == 3, "\(String(describing: r))")
        toggle(sheet, "Build-Ordner")

        r = search(sheet, "einzigartig")
        check("Suche ignoriert Groß-/Kleinschreibung", r?.matches == 1, "\(String(describing: r))")
        toggle(sheet, "Groß/Klein")
        r = search(sheet, "einzigartig")
        check("Suche beachtet Groß-/Kleinschreibung auf Wunsch", r?.matches == 0, "\(String(describing: r))")
        toggle(sheet, "Groß/Klein")

        toggle(sheet, "Ganzes Wort")
        r = search(sheet, "EINZIGART")
        check("Suche nach ganzen Wörtern", r?.matches == 0, "\(String(describing: r))")
        toggle(sheet, "Ganzes Wort")

        r = search(sheet, "zweite zeile")
        let pdfHeader = sheet.staticTexts["docs/quelle.pdf"]
        check("Suche findet Text in PDFs", (r?.matches ?? 0) >= 1 && pdfHeader.exists, "\(String(describing: r))")

        r = search(sheet, "ziele")
        let planHeader = sheet.staticTexts["notes/plan.md"]
        check("Suche: Treffer in plan.md", planHeader.exists, texts(in: sheet).joined(separator: " | "))
        screenshot("suche")

        // Opening a match (double click) closes the search and shows the file.
        let row = sheet.staticTexts.containing(NSPredicate(format: "value CONTAINS 'Ziele' OR label CONTAINS 'Ziele'")).firstMatch
        row.doubleClick()
        check("Suchtreffer schließt die Suche", waitFor(5) { !sheet.exists })
        let editor = app.textViews.firstMatch
        check("Markdown-Treffer im Editor geöffnet",
              waitFor(8) { (editor.value as? String)?.contains("## Ziele") == true }, String(describing: editor.value).prefix(60).description)
        check("Suchtreffer öffnet als fester Tab im aktiven Split", app.staticTexts["plan.md"].exists,
              texts(in: app.windows.firstMatch).prefix(12).joined(separator: " | "))
    }
}
