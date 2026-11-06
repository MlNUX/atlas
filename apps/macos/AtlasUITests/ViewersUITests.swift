// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import XCTest

/// S26 (steps 115–119): the HTML view.
final class ViewersUITests: AtlasUITestCase {
    func testS26HtmlView() throws {
        launch(open: "docs/seite.html")
        // Opened as source first; ⌥⌘U shows the page.
        check("HTML: Quelltext geöffnet", waitFor(10) { (self.app.textViews.firstMatch.value as? String)?.contains("<h1 id=\"kopf\">") == true })
        app.typeKey("u", modifierFlags: [.command, .option])
        let web = app.webViews.firstMatch
        check("HTML gerendert", web.waitForExistence(timeout: 10) && waitFor(10) { web.staticTexts["Hallo HTML"].exists },
              texts(in: web).joined(separator: " | "))
        check("HTML: Skript und CSS wirken", waitFor(5) { web.staticTexts["mit Skript"].exists }, texts(in: web).joined(separator: " | "))
        check("HTML: Ansicht im eigenen Tab", app.staticTexts["seite.html · Webseite"].exists)
        screenshot("html")

        // Saving the file reloads the page.
        try write("docs/seite.html", read("docs/seite.html").replacingOccurrences(of: "Hallo HTML", with: "Neu geladen"))
        check("HTML lädt beim Speichern neu", waitFor(10) { web.staticTexts["Neu geladen"].exists }, texts(in: web).joined(separator: " | "))

        // ⌥⌘U again: back to the source.
        app.typeKey("u", modifierFlags: [.command, .option])
        check("HTML: Quelltext im selben Fenster", waitFor(8) {
            (self.app.textViews.firstMatch.value as? String)?.contains("Neu geladen") == true
        })
    }
}
