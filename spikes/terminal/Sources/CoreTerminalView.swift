// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import SwiftUI

/// Variant B renderer: draws `Frame`s of `CoreTerminal` with CoreText,
/// one run at a time like `terminal_item.rs` does with QPainter.
final class CoreTerminalNSView: NSView {
    let font: NSFont
    let boldFont: NSFont
    let cellSize: CGSize
    private(set) var terminal: CoreTerminal?
    private var frameData: Frame?
    private var redrawPending = false
    var command: [String] = []
    var onExit: ((Int32) -> Void)?

    init(font: NSFont = .monospacedSystemFont(ofSize: 13, weight: .regular)) {
        self.font = font
        boldFont = NSFont.monospacedSystemFont(ofSize: font.pointSize, weight: .bold)
        // Exact advance (not rounded up): ASCII runs are drawn as one string,
        // so the cell width must equal the glyph advance or words drift apart.
        let advance = ("M" as NSString).size(withAttributes: [.font: font]).width
        cellSize = CGSize(width: advance, height: ceil(font.ascender - font.descender))
        super.init(frame: .zero)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }

    var gridSize: (cols: UInt16, lines: UInt16) {
        (UInt16(max(2, Int(bounds.width / cellSize.width))), UInt16(max(1, Int(bounds.height / cellSize.height))))
    }

    func start() {
        guard terminal == nil, bounds.width > 0 else { return }
        let (cols, lines) = gridSize
        terminal = CoreTerminal(
            command: command, cols: cols, lines: lines, cellWidth: UInt16(cellSize.width),
            cellHeight: UInt16(cellSize.height)
        ) { [weak self] event in
            Task { @MainActor in self?.handle(event) }
        }
        refresh()
    }

    private func handle(_ event: CoreTerminal.Event) {
        switch event {
        case .redraw:
            // Coalesce bursts of redraw events into one frame per runloop turn.
            guard !redrawPending else { return }
            redrawPending = true
            Task { @MainActor [weak self] in
                self?.redrawPending = false
                self?.refresh()
            }
        case .exited(let code): onExit?(code)
        case .title, .bell: break
        }
    }

    func refresh() {
        frameData = terminal?.frame(focused: window?.firstResponder === self)
        needsDisplay = true
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        if let terminal {
            let (cols, lines) = gridSize
            terminal.resize(cols: cols, lines: lines, cellWidth: UInt16(cellSize.width), cellHeight: UInt16(cellSize.height))
            refresh()
        } else {
            start()
        }
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        Self.draw(frameData, in: ctx, bounds: bounds, font: font, boldFont: boldFont, cell: cellSize)
    }

    /// Draws a frame into a flipped context (also used by the render benchmark).
    static func draw(_ frame: Frame?, in ctx: CGContext, bounds: CGRect, font: NSFont, boldFont: NSFont, cell: CGSize) {
        func color(_ c: [UInt8]) -> NSColor {
            c.count == 3 ? NSColor(srgbRed: CGFloat(c[0]) / 255, green: CGFloat(c[1]) / 255, blue: CGFloat(c[2]) / 255, alpha: 1) : .textColor
        }
        guard let frame else {
            NSColor.textBackgroundColor.setFill()
            bounds.fill()
            return
        }
        color(frame.background).setFill()
        bounds.fill()
        let italicFont = NSFontManager.shared.convert(font, toHaveTrait: .italicFontMask)
        for (row, line) in frame.lines.enumerated() {
            let y = CGFloat(row) * cell.height
            for bg in line.backgrounds {
                color(bg.color).setFill()
                CGRect(x: CGFloat(bg.col) * cell.width, y: y, width: CGFloat(bg.cells) * cell.width, height: cell.height).fill()
            }
            for run in line.runs {
                var attrs: [NSAttributedString.Key: Any] = [
                    .font: run.bold ? boldFont : run.italic ? italicFont : font,
                    .foregroundColor: color(run.fg),
                ]
                if run.underline { attrs[.underlineStyle] = NSUnderlineStyle.single.rawValue }
                if run.strikeout { attrs[.strikethroughStyle] = NSUnderlineStyle.single.rawValue }
                let x = CGFloat(run.col) * cell.width
                let s = NSAttributedString(string: run.text, attributes: attrs)
                if run.cells == run.text.count {
                    s.draw(at: CGPoint(x: x, y: y))
                } else {
                    // Wide or combined characters: centre in their cells so
                    // fallback fonts cannot shift the grid.
                    let w = s.size().width, box = CGFloat(run.cells) * cell.width
                    s.draw(at: CGPoint(x: x + (box - w) / 2, y: y))
                }
            }
        }
        if let c = frame.cursor {
            let r = CGRect(
                x: CGFloat(c.col) * cell.width, y: CGFloat(c.line) * cell.height,
                width: cell.width * (c.wide ? 2 : 1), height: cell.height)
            color(frame.cursorColor).withAlphaComponent(0.6).set()
            switch c.shape {
            case "Beam": CGRect(x: r.minX, y: r.minY, width: 2, height: r.height).fill()
            case "Underline": CGRect(x: r.minX, y: r.maxY - 2, width: r.width, height: 2).fill()
            case "HollowBlock": NSBezierPath(rect: r.insetBy(dx: 0.5, dy: 0.5)).stroke()
            default: r.fill()
            }
        }
    }

    // Minimal input: text and a few special keys. A full port of
    // atlas_terminal::input::encode_key (Qt key codes) to NSEvent is open work.
    override func keyDown(with event: NSEvent) {
        guard let terminal else { return }
        let special: [UInt16: String] = [
            36: "\r", 51: "\u{7f}", 48: "\t", 53: "\u{1b}",
            126: "\u{1b}[A", 125: "\u{1b}[B", 124: "\u{1b}[C", 123: "\u{1b}[D",
        ]
        if let seq = special[event.keyCode] {
            terminal.write(seq)
        } else if let chars = event.characters, !chars.isEmpty {
            terminal.write(chars)
        }
    }

    override func scrollWheel(with event: NSEvent) {
        let lines = Int32((event.scrollingDeltaY / cellSize.height).rounded())
        if lines != 0 {
            terminal?.scroll(lines)
            refresh()
        }
    }

    override func becomeFirstResponder() -> Bool {
        defer { refresh() }
        return true
    }
}

struct CoreTerminalPane: NSViewRepresentable {
    let command: [String]

    func makeNSView(context: Context) -> CoreTerminalNSView {
        let view = CoreTerminalNSView()
        view.command = command
        return view
    }

    func updateNSView(_ view: CoreTerminalNSView, context: Context) {}
}
