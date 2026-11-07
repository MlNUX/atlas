// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import Foundation

/// Variant B: `atlas-terminal` (alacritty_terminal) via the spike's C ABI.
/// Owns the Rust terminal; events arrive on the PTY thread.
final class CoreTerminal: @unchecked Sendable {
    enum Event: Sendable { case redraw, title, bell, exited(Int32) }

    /// Retained box handed to Rust as callback context.
    private final class Box: @unchecked Sendable {
        let handler: @Sendable (Event) -> Void
        init(_ handler: @escaping @Sendable (Event) -> Void) { self.handler = handler }
    }

    private let handle: OpaquePointer
    private let box: Unmanaged<Box>

    /// `command` empty = default shell.
    init?(
        command: [String], cwd: String? = nil, cols: UInt16, lines: UInt16, cellWidth: UInt16 = 8,
        cellHeight: UInt16 = 16, onEvent: @escaping @Sendable (Event) -> Void
    ) {
        let box = Unmanaged.passRetained(Box(onEvent))
        let callback: AtsEventFn = { ctx, kind, code in
            guard let ctx else { return }
            let box = Unmanaged<Box>.fromOpaque(ctx).takeUnretainedValue()
            switch Int32(kind) {
            case ATS_REDRAW: box.handler(.redraw)
            case ATS_TITLE: box.handler(.title)
            case ATS_BELL: box.handler(.bell)
            case ATS_EXITED: box.handler(.exited(code))
            default: break
            }
        }
        let cStrings = command.map { strdup($0) }
        defer { cStrings.forEach { free($0) } }
        let argv: [UnsafePointer<CChar>?] = cStrings.map { UnsafePointer($0) }
        let raw = (cwd ?? "").withCString { cwdPtr in
            argv.withUnsafeBufferPointer { buf in
                ats_spawn(
                    buf.baseAddress, buf.count, cwd == nil ? nil : cwdPtr, cols, lines, cellWidth, cellHeight,
                    callback, box.toOpaque())
            }
        }
        guard let raw else {
            box.release()
            return nil
        }
        handle = raw
        self.box = box
    }

    deinit {
        ats_free(handle)
        box.release()
    }

    func write(_ data: Data) {
        data.withUnsafeBytes { buf in
            ats_write(handle, buf.bindMemory(to: UInt8.self).baseAddress, buf.count)
        }
    }

    func write(_ text: String) { write(Data(text.utf8)) }

    func resize(cols: UInt16, lines: UInt16, cellWidth: UInt16, cellHeight: UInt16) {
        ats_resize(handle, cols, lines, cellWidth, cellHeight)
    }

    func scroll(_ lines: Int32) { ats_scroll(handle, lines) }

    func frame(focused: Bool = true) -> Frame? {
        guard let s = ats_frame_json(handle, focused) else { return nil }
        defer { ats_string_free(s) }
        let data = Data(bytesNoCopy: s, count: strlen(s), deallocator: .none)
        return try? JSONDecoder().decode(Frame.self, from: data)
    }

    func text(history: Int = 0) -> String {
        guard let s = ats_text(handle, history) else { return "" }
        defer { ats_string_free(s) }
        return String(cString: s)
    }
}

/// Mirrors `atlas_terminal::Frame` (as JSON from the spike's C ABI).
struct Frame: Decodable, Sendable {
    struct Run: Decodable, Sendable {
        var col: Int, cells: Int, text: String, fg: [UInt8]
        var bold: Bool, italic: Bool, underline: Bool, strikeout: Bool
    }
    struct Background: Decodable, Sendable { var col: Int, cells: Int, color: [UInt8] }
    struct Line: Decodable, Sendable { var backgrounds: [Background], runs: [Run] }
    struct Cursor: Decodable, Sendable { var col: Int, line: Int, wide: Bool, shape: String }

    var cols: Int
    var background: [UInt8]
    var cursorColor: [UInt8]
    var displayOffset: Int
    var historySize: Int
    var cursor: Cursor?
    var lines: [Line]
}
