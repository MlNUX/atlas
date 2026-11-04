// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
@preconcurrency import SwiftTerm
import XCTest

@testable import TerminalSpike

/// Collects measurements of both variants into $ATLAS_CI_OUT/perf.md.
enum Report {
    nonisolated(unsafe) static var rows: [String] = []

    static func add(_ variant: String, _ what: String, _ value: String) {
        rows.append("| \(variant) | \(what) | \(value) |")
        let env = ProcessInfo.processInfo.environment["ATLAS_CI_OUT"]
        let dir = env.map { URL(fileURLWithPath: $0) } ?? FileManager.default.temporaryDirectory
        let md = "## Messungen\n\n| Variante | Messung | Wert |\n|---|---|---|\n" + rows.joined(separator: "\n") + "\n"
        try? md.write(to: dir.appendingPathComponent("perf.md"), atomically: true, encoding: .utf8)
    }

    static func ms(_ seconds: Double) -> String { String(format: "%.2f ms", seconds * 1000) }
}

/// `seq 1 200000` output, about 1.3 MB.
private let seqBytes: [UInt8] = Array((1...200_000).map { "\($0)\n" }.joined().utf8)

// MARK: - B: atlas-terminal

final class CoreTerminalTests: XCTestCase {
    /// Polls the frame until `done` holds or the timeout passes.
    private func waitFrame(_ t: CoreTerminal, timeout: TimeInterval = 10, _ done: (Frame) -> Bool) -> Frame? {
        let end = Date().addingTimeInterval(timeout)
        while Date() < end {
            if let f = t.frame(), done(f) { return f }
            Thread.sleep(forTimeInterval: 0.05)
        }
        return t.frame()
    }

    func testTextColoursAndWideCells() throws {
        let t = try XCTUnwrap(
            CoreTerminal(
                command: ["/bin/sh", "-c", "printf 'hello \\033[31mred\\033[0m 日本x e\\314\\201\\n'; sleep 5"],
                cols: 40, lines: 5, onEvent: { _ in }))
        let f = try XCTUnwrap(waitFrame(t) { $0.lines.first?.runs.contains { $0.text == "x" } == true })
        let runs = f.lines[0].runs
        let red = try XCTUnwrap(runs.first { $0.text == "red" })
        XCTAssertGreaterThan(red.fg[0], 150)
        XCTAssertLessThan(red.fg[1], 120)
        let wide = try XCTUnwrap(runs.first { $0.text == "日" })
        XCTAssertEqual(wide.cells, 2)
        XCTAssertEqual(runs.first { $0.text == "x" }?.col, wide.col + 4)
        XCTAssertTrue(runs.contains { $0.text == "e\u{301}" }, "combining mark kept in one cell")
        XCTAssertTrue(t.text().contains("hello red"))
    }

    func testInputEcho() throws {
        let t = try XCTUnwrap(CoreTerminal(command: ["/bin/cat"], cols: 40, lines: 5, onEvent: { _ in }))
        t.write("abc äöü\r")
        let f = waitFrame(t) { f in f.lines.contains { $0.runs.map(\.text).joined().contains("abcäöü") } }
        XCTAssertNotNil(f)
        XCTAssertTrue(t.text().contains("abc äöü"), t.text())
    }

    func testResize() throws {
        let t = try XCTUnwrap(CoreTerminal(command: ["/bin/cat"], cols: 40, lines: 5, onEvent: { _ in }))
        t.resize(cols: 100, lines: 30, cellWidth: 8, cellHeight: 16)
        let f = try XCTUnwrap(waitFrame(t) { $0.cols == 100 && $0.lines.count == 30 })
        XCTAssertEqual(f.cols, 100)
        XCTAssertEqual(f.lines.count, 30)
    }

    func testThroughputSeq() throws {
        let exited = expectation(description: "exit")
        // atlas-terminal reports the end twice (ChildExit and Exit).
        exited.assertForOverFulfill = false
        let start = Date()
        let t = try XCTUnwrap(
            CoreTerminal(command: ["/usr/bin/seq", "1", "200000"], cols: 120, lines: 40) { event in
                if case .exited = event { exited.fulfill() }
            })
        wait(for: [exited], timeout: 60)
        let elapsed = Date().timeIntervalSince(start)
        Report.add("B atlas-terminal", "`seq 1 200000` über PTY (1,3 MB), bis Prozessende", Report.ms(elapsed))
        XCTAssertTrue(t.text().contains("200000"))
    }

    @MainActor
    func testRenderSpeed() throws {
        let view = CoreTerminalNSView()
        view.command = ["/bin/bash", "-c", "bash '\(Bundle(for: Self.self).path(forResource: "demo", ofType: "sh")!)'; sleep 30"]
        view.setFrameSize(CGSize(width: 1000, height: 700))
        let t = try XCTUnwrap(view.terminal)
        _ = waitFrame(t) { f in f.lines.contains { $0.runs.contains { $0.text.contains("$") } } }

        let n = 200
        var start = Date()
        for _ in 0..<n { _ = t.frame() }
        let frameTime = Date().timeIntervalSince(start) / Double(n)

        let rep = try XCTUnwrap(view.bitmapImageRepForCachingDisplay(in: view.bounds))
        let frame = t.frame()
        start = Date()
        let base = try XCTUnwrap(NSGraphicsContext(bitmapImageRep: rep))
        let scale = CGFloat(rep.pixelsHigh) / view.bounds.height
        for _ in 0..<n {
            let cg = base.cgContext
            cg.saveGState()
            NSGraphicsContext.saveGraphicsState()
            // The renderer expects a flipped context like the view's.
            cg.translateBy(x: 0, y: CGFloat(rep.pixelsHigh))
            cg.scaleBy(x: scale, y: -scale)
            NSGraphicsContext.current = NSGraphicsContext(cgContext: cg, flipped: true)
            CoreTerminalNSView.draw(
                frame, in: cg, bounds: view.bounds, font: view.font, boldFont: view.boldFont,
                cell: view.cellSize)
            NSGraphicsContext.restoreGraphicsState()
            cg.restoreGState()
        }
        let drawTime = Date().timeIntervalSince(start) / Double(n)
        if let png = rep.representation(using: .png, properties: [:]),
            let out = ProcessInfo.processInfo.environment["ATLAS_CI_OUT"]
        {
            try? png.write(to: URL(fileURLWithPath: out).appendingPathComponent("render-b-atlas-terminal.png"))
        }
        Report.add("B atlas-terminal", "Frame holen (Rust → JSON → Swift), Demo-Bildschirm", Report.ms(frameTime))
        Report.add("B atlas-terminal", "Frame zeichnen (CoreText), 1000×700 pt", Report.ms(drawTime))
    }
}

// MARK: - A: SwiftTerm

private final class Sink: TerminalDelegate {
    func send(source: Terminal, data: ArraySlice<UInt8>) {}
}

final class SwiftTermTests: XCTestCase {
    private func line(_ t: Terminal, _ row: Int) -> String {
        t.getLine(row: row)?.translateToString(trimRight: true) ?? ""
    }

    func testTextAndWideCells() {
        let sink = Sink()
        let t = Terminal(delegate: sink, options: TerminalOptions(cols: 40, rows: 5))
        t.feed(text: "hello \u{1b}[31mred\u{1b}[0m 日本x e\u{301}\r\n")
        let text = line(t, 0)
        XCTAssertTrue(text.hasPrefix("hello red"), text)
        // translateToString keeps the spacer cell after a wide character.
        // (not necessarily U+0020): compare without blanks and NULs.
        let compact = String(text.unicodeScalars.filter { !$0.properties.isWhitespace && $0.value != 0 }.map(Character.init))
        XCTAssertTrue(compact.contains("日本x"), text.unicodeScalars.map { String($0.value, radix: 16) }.joined(separator: " "))
    }

    func testThroughputFeed() {
        let sink = Sink()
        let t = Terminal(delegate: sink, options: TerminalOptions(cols: 120, rows: 40))
        let start = Date()
        t.feed(byteArray: seqBytes)
        let elapsed = Date().timeIntervalSince(start)
        Report.add("A SwiftTerm", "1,3 MB `seq`-Ausgabe direkt in den Parser (ohne PTY)", Report.ms(elapsed))
        XCTAssertTrue((0..<40).contains { line(t, $0).contains("200000") })
    }

    @MainActor
    func testThroughputProcess() {
        let view = LocalProcessTerminalView(frame: CGRect(x: 0, y: 0, width: 1000, height: 700))
        let done = expectation(description: "exit")
        final class Delegate: LocalProcessTerminalViewDelegate {
            let done: XCTestExpectation
            init(_ done: XCTestExpectation) { self.done = done }
            func sizeChanged(source: LocalProcessTerminalView, newCols: Int, newRows: Int) {}
            func setTerminalTitle(source: LocalProcessTerminalView, title: String) {}
            func hostCurrentDirectoryUpdate(source: TerminalView, directory: String?) {}
            func processTerminated(source: TerminalView, exitCode: Int32?) { done.fulfill() }
        }
        let delegate = Delegate(done)
        view.processDelegate = delegate
        let start = Date()
        view.startProcess(executable: "/usr/bin/seq", args: ["1", "200000"])
        wait(for: [done], timeout: 60)
        let elapsed = Date().timeIntervalSince(start)
        Report.add("A SwiftTerm", "`seq 1 200000` über PTY (1,3 MB), bis Prozessende", Report.ms(elapsed))
        _ = delegate
    }

    @MainActor
    func testRenderSpeed() throws {
        let view = TerminalView(frame: CGRect(x: 0, y: 0, width: 1000, height: 700))
        view.font = .monospacedSystemFont(ofSize: 13, weight: .regular)
        // SwiftTerm only draws inside a window.
        let window = NSWindow(
            contentRect: view.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = view
        let demo = try String(contentsOfFile: Bundle(for: Self.self).path(forResource: "demo", ofType: "sh")!, encoding: .utf8)
        // Feed what the demo prints (run it through bash once).
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/bin/bash")
        p.arguments = ["-c", demo]
        let pipe = Pipe()
        p.standardOutput = pipe
        try p.run()
        p.waitUntilExit()
        let output = pipe.fileHandleForReading.readDataToEndOfFile()
        view.feed(byteArray: ArraySlice([UInt8](output).flatMap { $0 == 10 ? [13, 10] : [$0] }))
        let rep = try XCTUnwrap(view.bitmapImageRepForCachingDisplay(in: view.bounds))
        let n = 200
        let start = Date()
        for _ in 0..<n { view.cacheDisplay(in: view.bounds, to: rep) }
        let drawTime = Date().timeIntervalSince(start) / Double(n)
        Report.add("A SwiftTerm", "Bildschirm zeichnen (TerminalView.draw), 1000×700 pt", Report.ms(drawTime))
        _ = window
        if let png = rep.representation(using: .png, properties: [:]),
            let out = ProcessInfo.processInfo.environment["ATLAS_CI_OUT"]
        {
            try? png.write(to: URL(fileURLWithPath: out).appendingPathComponent("render-a-swiftterm.png"))
        }
    }
}
