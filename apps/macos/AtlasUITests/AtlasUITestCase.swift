// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import XCTest

/// Base of the UI tests (P4.1): the same test project and the same check
/// names as the Linux self-test (`crates/atlas-app/qml/SelfTest.qml`,
/// catalogue `docs/porting/testkatalog.md`), so both outputs compare line by
/// line (`selftest: ok   <name>` / `selftest: FAIL <name> – <detail>`).
///
/// The project comes from `scripts/smoke-test.sh` (run by
/// `apps/macos/scripts/ci-uitests.sh`, which passes its folder as
/// `TEST_RUNNER_ATLAS_UITEST_TEMPLATE`); every test works on its own copy.
class AtlasUITestCase: XCTestCase {
    var app: XCUIApplication!
    /// Copy of the test folder: `projekt/`, `config/`, `state/`, `lang.md` …
    var work: URL!
    var root: String { work.appending(path: "projekt").path }

    override func setUpWithError() throws {
        continueAfterFailure = true
        guard let template = ProcessInfo.processInfo.environment["ATLAS_UITEST_TEMPLATE"] else {
            throw XCTSkip("Kein Testprojekt: apps/macos/scripts/ci-uitests.sh legt es an")
        }
        let name = String(name.filter { $0.isLetter || $0.isNumber }.prefix(40))
        work = FileManager.default.temporaryDirectory.appending(path: "atlas-ui-\(name)-\(UUID().uuidString.prefix(8))")
        try FileManager.default.copyItem(at: URL(filePath: template), to: work)
    }

    override func tearDownWithError() throws {
        if let app, app.state != .notRunning {
            screenshot("ende")
            app.terminate()
        }
    }

    /// Starts Atlas on the test project, optionally with `file` (relative)
    /// open, and waits for the window.
    func launch(open file: String? = nil, extra: [String] = []) {
        app = XCUIApplication()
        var args = ["-AtlasOpenProject", root, "-AtlasWindowSize", "1280x780", "-ApplePersistenceIgnoreState", "YES"]
        if let file { args += ["-AtlasOpenFile", file] }
        app.launchArguments = args + extra
        app.launchEnvironment["ATLAS_CONFIG_DIR"] = work.appending(path: "config").path
        app.launchEnvironment["ATLAS_STATE_DIR"] = work.appending(path: "state").path
        app.launch()
        check("Projekt geöffnet", app.windows.firstMatch.waitForExistence(timeout: 20))
        app.activate()
    }

    // MARK: - checks

    /// Logs like the Linux self-test and records a failure.
    func check(_ name: String, _ ok: Bool, _ detail: @autoclosure () -> String = "",
               file: StaticString = #filePath, line: UInt = #line) {
        let more = ok ? "" : " – " + detail()
        print("selftest: \(ok ? "ok  " : "FAIL") \(name)\(more)")
        XCTAssertTrue(ok, name + more, file: file, line: line)
    }

    /// Waits up to `timeout` seconds for `condition`.
    @discardableResult
    func waitFor(_ timeout: TimeInterval = 8, _ condition: () -> Bool) -> Bool {
        let end = Date().addingTimeInterval(timeout)
        while Date() < end {
            if condition() { return true }
            RunLoop.current.run(until: Date().addingTimeInterval(0.2))
        }
        return condition()
    }

    // MARK: - project files

    func path(_ relative: String) -> String { root + "/" + relative }

    func read(_ relative: String) -> String {
        (try? String(contentsOfFile: path(relative), encoding: .utf8)) ?? ""
    }

    func write(_ relative: String, _ text: String) throws {
        try text.write(toFile: path(relative), atomically: true, encoding: .utf8)
    }

    /// The session file the app wrote (`.atlas/sessions/<device>/*.json`).
    func session() -> [String: Any]? {
        let dir = work.appending(path: "projekt/.atlas/sessions")
        guard let files = FileManager.default.enumerator(at: dir, includingPropertiesForKeys: nil) else { return nil }
        for case let url as URL in files where url.pathExtension == "json" {
            if let data = try? Data(contentsOf: url),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                return json
            }
        }
        return nil
    }

    // MARK: - UI helpers

    /// Picks a menu item from the menu bar.
    func menu(_ menu: String, _ item: String) {
        let bar = app.menuBars.menuBarItems[menu]
        bar.click()
        let entry = bar.menuItems[item]
        if entry.waitForExistence(timeout: 3) {
            entry.click()
        } else {
            app.typeKey(.escape, modifierFlags: [])
            XCTFail("Menüeintrag „\(menu) › \(item)“ fehlt")
        }
    }

    /// Text of all static texts below `element` (for summaries, labels).
    func texts(in element: XCUIElement) -> [String] {
        element.staticTexts.allElementsBoundByIndex.compactMap { $0.value as? String ?? $0.label }
    }

    /// A screenshot into the test result and, in the CI, into `ATLAS_CI_OUT`
    /// (printed into the log by macos-setup.yml).
    func screenshot(_ label: String) {
        let shot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: shot)
        attachment.name = label
        attachment.lifetime = .keepAlways
        add(attachment)
        guard let out = ProcessInfo.processInfo.environment["ATLAS_CI_OUT"] else { return }
        let test = String(name.filter { $0.isLetter || $0.isNumber }.suffix(40))
        try? shot.pngRepresentation.write(to: URL(filePath: out).appending(path: "ui-\(test)-\(label).png"))
    }
}
