// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import SwiftUI

/// The text editor of Atlas (P3.3; P3.4 uses it for code): an `NSTextView`
/// (TextKit 1, for exact line positions in the split view) whose colours,
/// list and table keys and completion come from the core (`Editor`,
/// `Highlighter`; Linux: `TextEditor.qml`). Positions crossing the core are
/// UTF-16 offsets, as `NSString` counts them.
@MainActor
final class TextEditorController: NSObject, NSTextViewDelegate {
    let textView: AtlasTextView
    let scrollView: NSScrollView
    private let editor: Editor
    private var highlighter: Highlighter?
    /// Markdown keys (lists, tables) and completion of wiki links and tags.
    let markdown: Bool
    /// The text changed (typed, pasted, undone or by the core's edits).
    var onChange: ((String) -> Void)?
    /// The view scrolled (for the split view).
    var onScroll: (() -> Void)?
    /// ⌘V: `true` if the owner inserted the pasteboard itself (pictures, files).
    var onPaste: (() -> Bool)?
    /// Cursor position of the last selection inside a table (aligned when
    /// the cursor leaves it, as on Linux).
    private var tablePosition: Int?
    private var completionContext: CompletionContext?
    private var completionItems: [Completion] = []
    private var applyingCore = false

    init(editor: Editor, language: String?, markdown: Bool) {
        self.editor = editor
        self.markdown = markdown
        scrollView = NSScrollView()
        textView = AtlasTextView(usingTextLayoutManager: false)
        super.init()
        if let language { highlighter = editor.highlighter(language: language) }
        textView.controller = self
        textView.delegate = self
        textView.isRichText = false
        textView.importsGraphics = false
        textView.allowsUndo = true
        textView.usesFindBar = true
        textView.isIncrementalSearchingEnabled = true
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticTextReplacementEnabled = false
        textView.isAutomaticSpellingCorrectionEnabled = false
        // Spelling of notes: NSSpellChecker (decision in docs/porting/api.md).
        textView.isContinuousSpellCheckingEnabled = markdown
        textView.font = Self.font
        textView.textColor = .textColor
        textView.backgroundColor = .textBackgroundColor
        textView.drawsBackground = true
        textView.textContainerInset = NSSize(width: 12, height: 10)
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.autoresizingMask = [.width]
        textView.minSize = NSSize(width: 0, height: 0)
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.textContainer?.widthTracksTextView = true
        textView.textContainer?.containerSize = NSSize(width: 0, height: CGFloat.greatestFiniteMagnitude)
        textView.setAccessibilityLabel(markdown ? "Markdown-Editor" : "Texteditor")
        scrollView.documentView = textView
        scrollView.hasVerticalScroller = true
        // Line numbers (as on Linux).
        let ruler = LineNumberRuler(textView: textView)
        scrollView.verticalRulerView = ruler
        scrollView.hasVerticalRuler = true
        scrollView.rulersVisible = true
        scrollView.drawsBackground = false
        scrollView.contentView.postsBoundsChangedNotifications = true
        NotificationCenter.default.addObserver(
            self, selector: #selector(scrolled), name: NSView.boundsDidChangeNotification, object: scrollView.contentView)
    }

    static let font = NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)

    @objc private func scrolled() {
        scrollView.verticalRulerView?.needsDisplay = true
        onScroll?()
    }

    var text: String {
        get { textView.string }
        set {
            applyingCore = true
            textView.string = newValue
            applyingCore = false
            highlight()
            scrollView.verticalRulerView?.needsDisplay = true
        }
    }

    // MARK: - colours

    func highlight() {
        guard let storage = textView.textStorage else { return }
        let text = textView.string
        let full = NSRange(location: 0, length: (text as NSString).length)
        let spans = highlighter?.update(text: text) ?? []
        storage.beginEditing()
        storage.setAttributes([.font: Self.font, .foregroundColor: NSColor.textColor], range: full)
        for span in spans {
            let start = Int(span.start), end = min(Int(span.end), full.length)
            guard start < end else { continue }
            storage.addAttributes(Self.attributes(span.kind), range: NSRange(location: start, length: end - start))
        }
        storage.endEditing()
        textView.typingAttributes = [.font: Self.font, .foregroundColor: NSColor.textColor]
    }

    static func attributes(_ kind: HighlightKind) -> [NSAttributedString.Key: Any] {
        let bold = NSFont.monospacedSystemFont(ofSize: font.pointSize, weight: .bold)
        let italic = NSFontManager.shared.convert(font, toHaveTrait: .italicFontMask)
        switch kind {
        case .comment, .quote: return [.foregroundColor: NSColor.secondaryLabelColor]
        case .string: return [.foregroundColor: NSColor.systemGreen]
        case .escape, .number, .constant, .attribute: return [.foregroundColor: NSColor.systemOrange]
        case .keyword: return [.foregroundColor: NSColor.systemPink]
        case .`operator`, .punctuation: return [.foregroundColor: NSColor.tertiaryLabelColor]
        case .function: return [.foregroundColor: NSColor.systemBlue]
        case .type: return [.foregroundColor: NSColor.systemTeal]
        case .variable: return [.foregroundColor: NSColor.labelColor]
        case .tag: return [.foregroundColor: NSColor.systemRed]
        case .heading: return [.foregroundColor: NSColor.controlAccentColor, .font: bold]
        case .emphasis: return [.font: italic]
        case .strong: return [.font: bold]
        case .link: return [.foregroundColor: NSColor.linkColor]
        case .code: return [.foregroundColor: NSColor.systemBrown]
        case .list: return [.foregroundColor: NSColor.controlAccentColor]
        case .math: return [.foregroundColor: NSColor.systemPurple]
        }
    }

    // MARK: - edits of the core

    /// Applies an edit of the core as one undo step and selects its range.
    func apply(_ edit: Edit) {
        let range = NSRange(location: Int(edit.start), length: Int(edit.end) - Int(edit.start))
        if textView.shouldChangeText(in: range, replacementString: edit.text) {
            textView.textStorage?.replaceCharacters(in: range, with: edit.text)
            textView.didChangeText()
        }
        let a = Int(edit.anchor), c = Int(edit.cursor)
        textView.setSelectedRange(NSRange(location: min(a, c), length: abs(c - a)))
        textView.scrollRangeToVisible(NSRange(location: c, length: 0))
    }

    /// Inserts a block on lines of its own at the cursor (insert menu).
    func insertBlock(_ block: String) {
        let ns = textView.string as NSString
        let at = textView.selectedRange().location
        var text = block
        if at > 0, ns.character(at: at - 1) != 10 { text = "\n" + text }
        if at < ns.length, ns.character(at: at) != 10 { text += "\n" }
        if !text.hasSuffix("\n") { text += "\n" }
        let range = NSRange(location: at, length: textView.selectedRange().length)
        if textView.shouldChangeText(in: range, replacementString: text) {
            textView.textStorage?.replaceCharacters(in: range, with: text)
            textView.didChangeText()
        }
        textView.setSelectedRange(NSRange(location: at + (text as NSString).length, length: 0))
        textView.window?.makeFirstResponder(textView)
    }

    private var cursor: Int { textView.selectedRange().location + textView.selectedRange().length }

    fileprivate func handleNewline() -> Bool {
        guard markdown, textView.selectedRange().length == 0 else { return false }
        let text = textView.string, pos = UInt32(cursor)
        guard let edit = editor.tableEnter(text: text, position: pos) ?? editor.continueList(text: text, position: pos)
        else { return false }
        apply(edit)
        return true
    }

    fileprivate func handleTab(back: Bool) -> Bool {
        guard markdown else { return false }
        let text = textView.string, sel = textView.selectedRange()
        if sel.length == 0, let edit = editor.tableTab(text: text, position: UInt32(sel.location), back: back) {
            apply(edit)
            return true
        }
        if let edit = editor.indentListItems(
            text: text, anchor: UInt32(sel.location), cursor: UInt32(sel.location + sel.length), outdent: back) {
            apply(edit)
            return true
        }
        return false
    }

    /// Checks or unchecks the task on `line` (checkbox in the preview).
    func toggleTask(line: Int, done: Bool) -> Bool {
        guard let edit = editor.toggleTask(text: textView.string, line: UInt32(line), done: done) else { return false }
        let keep = textView.selectedRange()
        apply(edit)
        textView.setSelectedRange(keep)
        return true
    }

    // MARK: - completion (wiki links, tags)

    fileprivate func afterTyping() {
        guard markdown else { return }
        let context = editor.completionAt(text: textView.string, position: UInt32(cursor))
        completionContext = context
        guard let context else { return }
        completionItems = editor.completions(context: context, limit: 12)
        if !completionItems.isEmpty {
            DispatchQueue.main.async { [weak self] in self?.textView.complete(nil) }
        }
    }

    fileprivate var completionRange: NSRange? {
        guard let c = completionContext else { return nil }
        return NSRange(location: Int(c.start), length: max(0, cursor - Int(c.start)))
    }

    fileprivate var completionLabels: [String] { completionItems.map(\.label) }

    /// The chosen entry, applied by the core (closing `]]` and the like).
    fileprivate func acceptCompletion(label: String) -> Bool {
        guard let context = completionContext, let item = completionItems.first(where: { $0.label == label })
        else { return false }
        let edit = editor.acceptCompletion(text: textView.string, context: context, end: UInt32(cursor), item: item)
        completionContext = nil
        apply(edit)
        return true
    }

    // MARK: - NSTextViewDelegate

    func textDidChange(_ notification: Notification) {
        scrollView.verticalRulerView?.needsDisplay = true
        guard !applyingCore else { return }
        highlight()
        onChange?(textView.string)
    }

    func textViewDidChangeSelection(_ notification: Notification) {
        guard markdown, !applyingCore else { return }
        let text = textView.string as NSString
        let pos = cursor
        let line = text.lineRange(for: NSRange(location: min(pos, text.length), length: 0))
        if let previous = tablePosition, previous <= text.length {
            let before = text.lineRange(for: NSRange(location: previous, length: 0))
            if before.location != line.location,
               let edit = editor.alignTableAt(text: textView.string, position: UInt32(previous), cursor: UInt32(pos)) {
                tablePosition = nil
                applyingCore = true
                apply(edit)
                applyingCore = false
                highlight()
                onChange?(textView.string)
                return
            }
        }
        let current = text.substring(with: line).trimmingCharacters(in: .whitespaces)
        tablePosition = current.hasPrefix("|") ? pos : nil
    }

    // MARK: - lines (split view)

    /// The source line (1-based, with fraction) at `y` of the visible area.
    func exactLine(atY y: CGFloat) -> Double {
        guard let lm = textView.layoutManager, let tc = textView.textContainer else { return 1 }
        let text = textView.string as NSString
        let point = NSPoint(x: 0, y: scrollView.contentView.bounds.minY + y - textView.textContainerOrigin.y)
        var fraction: CGFloat = 0
        let glyph = lm.glyphIndex(for: point, in: tc, fractionOfDistanceThroughGlyph: &fraction)
        let index = min(lm.characterIndexForGlyph(at: glyph), text.length)
        let lineRange = text.lineRange(for: NSRange(location: index, length: 0))
        let number = lineNumber(of: lineRange.location)
        let top = lineTop(lineRange.location)
        let bottom = lineRange.upperBound < text.length ? lineTop(lineRange.upperBound) : top + Self.font.boundingRectForFont.height
        let span = max(1, bottom - top)
        return Double(number) + Double(min(1, max(0, (point.y - top) / span)))
    }

    /// Scrolls so that source line `line` (with fraction) is at `y` of the view.
    func scroll(toExactLine line: Double, at y: CGFloat) {
        let whole = max(1, Int(line.rounded(.down)))
        guard let start = offset(ofLine: whole) else { return }
        let top = lineTop(start)
        let next = offset(ofLine: whole + 1).map(lineTop) ?? (top + Self.font.boundingRectForFont.height)
        let target = top + (next - top) * CGFloat(line - Double(whole)) + textView.textContainerOrigin.y - y
        let maxY = max(0, textView.frame.height - scrollView.contentView.bounds.height)
        scrollView.contentView.scroll(to: NSPoint(x: 0, y: min(maxY, max(0, target))))
        scrollView.reflectScrolledClipView(scrollView.contentView)
    }

    /// Selects the first occurrence of `term` at or after `line` (else the
    /// first one) and shows it; `false` if there is none.
    @discardableResult
    func select(term: String, caseSensitive: Bool, fromLine line: Int) -> Bool {
        let text = textView.string as NSString
        let options: NSString.CompareOptions = caseSensitive ? [] : [.caseInsensitive]
        let start = offset(ofLine: max(1, line)) ?? 0
        var found = text.range(of: term, options: options, range: NSRange(location: start, length: text.length - start))
        if found.location == NSNotFound { found = text.range(of: term, options: options) }
        guard found.location != NSNotFound else { return false }
        textView.setSelectedRange(found)
        textView.scrollRangeToVisible(found)
        textView.showFindIndicator(for: found)
        return true
    }

    /// Moves the cursor to the start of `line` (1-based) and shows it.
    func go(toLine line: Int) {
        guard let start = offset(ofLine: max(1, line)) else { return }
        textView.setSelectedRange(NSRange(location: start, length: 0))
        textView.scrollRangeToVisible(NSRange(location: start, length: 0))
    }

    private func lineTop(_ index: Int) -> CGFloat {
        guard let lm = textView.layoutManager else { return 0 }
        let length = (textView.string as NSString).length
        if length == 0 { return 0 }
        let glyph = lm.glyphIndexForCharacter(at: min(index, length - 1))
        return lm.lineFragmentRect(forGlyphAt: glyph, effectiveRange: nil).minY
    }

    /// 1-based line of character `index`.
    private func lineNumber(of index: Int) -> Int {
        let text = textView.string as NSString
        var count = 1
        for i in 0..<min(index, text.length) where text.character(at: i) == 10 { count += 1 }
        return count
    }

    /// Offset of the start of `line` (1-based), `nil` past the end.
    private func offset(ofLine line: Int) -> Int? {
        if line <= 1 { return 0 }
        let text = textView.string as NSString
        var n = 1
        for i in 0..<text.length where text.character(at: i) == 10 {
            n += 1
            if n == line { return i + 1 }
        }
        return nil
    }
}

/// `NSTextView` that hands Return, Tab and completion to its controller.
final class AtlasTextView: NSTextView {
    weak var controller: TextEditorController?

    override func insertNewline(_ sender: Any?) {
        if controller?.handleNewline() == true { return }
        super.insertNewline(sender)
    }

    override func insertTab(_ sender: Any?) {
        if controller?.handleTab(back: false) == true { return }
        super.insertTab(sender)
    }

    override func insertBacktab(_ sender: Any?) {
        if controller?.handleTab(back: true) == true { return }
        super.insertBacktab(sender)
    }

    override func paste(_ sender: Any?) {
        if controller?.onPaste?() == true { return }
        super.paste(sender)
    }

    override func insertText(_ string: Any, replacementRange: NSRange) {
        super.insertText(string, replacementRange: replacementRange)
        controller?.afterTyping()
    }

    override var rangeForUserCompletion: NSRange {
        controller?.completionRange ?? super.rangeForUserCompletion
    }

    override func completions(
        forPartialWordRange charRange: NSRange, indexOfSelectedItem index: UnsafeMutablePointer<Int>
    ) -> [String]? {
        guard let labels = controller?.completionLabels, !labels.isEmpty else {
            return super.completions(forPartialWordRange: charRange, indexOfSelectedItem: index)
        }
        index.pointee = 0
        return labels
    }

    override func insertCompletion(_ word: String, forPartialWordRange charRange: NSRange, movement: Int, isFinal flag: Bool) {
        // Only the chosen entry changes the text (no preview while moving
        // through the list); Escape keeps what was typed.
        guard flag, movement != NSTextMovement.cancel.rawValue else { return }
        if controller?.acceptCompletion(label: word) == true { return }
        super.insertCompletion(word, forPartialWordRange: charRange, movement: movement, isFinal: flag)
    }
}

/// Line numbers beside the editor: one per source line (wrapped lines
/// show it on their first part).
final class LineNumberRuler: NSRulerView {
    private weak var textView: NSTextView?

    init(textView: NSTextView) {
        self.textView = textView
        super.init(scrollView: nil, orientation: .verticalRuler)
        clientView = textView
        ruleThickness = 40
        setAccessibilityElement(false)
    }

    @available(*, unavailable)
    required init(coder: NSCoder) { fatalError() }

    override var isFlipped: Bool { true }

    private static let attributes: [NSAttributedString.Key: Any] = [
        .font: NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .regular),
        .foregroundColor: NSColor.tertiaryLabelColor,
    ]

    override func drawHashMarksAndLabels(in rect: NSRect) {
        guard let textView, let lm = textView.layoutManager, let tc = textView.textContainer else { return }
        NSColor.textBackgroundColor.setFill()
        rect.fill()
        let text = textView.string as NSString
        let visible = textView.visibleRect
        let glyphs = lm.glyphRange(forBoundingRect: visible, in: tc)
        let chars = lm.characterRange(forGlyphRange: glyphs, actualGlyphRange: nil)
        // Number of the first visible line.
        var line = 1
        for i in 0..<min(chars.location, text.length) where text.character(at: i) == 10 { line += 1 }
        let inset = textView.textContainerOrigin.y
        let offset = convert(NSPoint.zero, from: textView).y
        var index = chars.location
        let needed = max(40, CGFloat(String(max(1, line + 200)).count) * 7.5 + 14)
        if abs(ruleThickness - needed) > 0.5 { ruleThickness = needed }
        while index <= NSMaxRange(chars), index <= text.length {
            let y: CGFloat
            if index < text.length || text.length == 0 {
                let glyph = lm.glyphIndexForCharacter(at: min(index, max(0, text.length - 1)))
                y = text.length == 0 ? 0 : lm.lineFragmentRect(forGlyphAt: glyph, effectiveRange: nil).minY
            } else {
                y = lm.extraLineFragmentRect.minY
            }
            let label = "\(line)" as NSString
            let size = label.size(withAttributes: Self.attributes)
            label.draw(at: NSPoint(x: ruleThickness - size.width - 6, y: y + inset + offset + 1), withAttributes: Self.attributes)
            let next = text.lineRange(for: NSRange(location: min(index, text.length), length: 0))
            if NSMaxRange(next) <= index { break }
            // The last line ends with a newline: one more (empty) line.
            if NSMaxRange(next) == text.length, text.length > 0, text.character(at: text.length - 1) != 10 { break }
            index = NSMaxRange(next)
            line += 1
        }
    }
}

/// The editor in SwiftUI.
struct TextEditorView: NSViewRepresentable {
    let controller: TextEditorController

    func makeNSView(context: NSViewRepresentableContext<Self>) -> NSScrollView { controller.scrollView }
    func updateNSView(_ view: NSScrollView, context: NSViewRepresentableContext<Self>) {}
}
