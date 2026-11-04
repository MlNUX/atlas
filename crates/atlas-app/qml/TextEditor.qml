// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.syntaxhighlighting
import Atlas

// Text and source editor with syntax highlighting, line numbers, search in
// the document, autosave, recovery copies and conflict handling.
//
// Edits are saved shortly after typing stops (and with Ctrl+S); while there
// are unsaved changes a recovery copy is kept in `.atlas/recovery/`. If the
// file changes outside of Atlas, unmodified documents reload silently and
// modified ones pause autosave and ask what to do.
Rectangle {
    id: root
    required property AppController app
    required property string source
    property int line: 0
    /// Soft wrap (Markdown, prose); code is shown unwrapped with line numbers.
    property bool wrap: false
    property bool showLineNumbers: true
    property bool autosave: true
    readonly property alias text: area.text
    readonly property string selectedText: area.selectedText
    readonly property bool modified: editGeneration !== savedGeneration
    readonly property bool readOnly: doc.readOnly !== ""
    readonly property alias document: doc
    /// First visible line (1-based), for scroll sync and session state.
    readonly property int topLine: wrap ? lineAtY(flick.contentY) : Math.floor(flick.contentY / lineHeight) + 1
    readonly property real lineHeight: area.lineCount > 0 ? area.contentHeight / area.lineCount : 16
    readonly property bool large: area.length > 2 * 1024 * 1024
    /// Syntax definition for the highlighting. Markdown uses Atlas' own
    /// variant (bundled, see scripts/gen-markdown-syntax.py), which colours
    /// code blocks in some 300 languages instead of about 40.
    readonly property string syntax: {
        const name = Repository.definitionForFileName(root.source).name
        if (name === "Markdown" && Repository.definitionForName("Atlas Markdown").name === "Atlas Markdown")
            return "Atlas Markdown"
        return name
    }

    /// FileActions (for "open in editor"); optional.
    property var actions: null
    /// Suggest project files after `[[` (Markdown).
    property bool wikiCompletion: false
    /// Faster, more direct touchpad scrolling (TouchpadFling.brisk), as in
    /// the .atlasnote view.
    property bool briskScrolling: false

    /// Shows file suggestions while the cursor is inside an unfinished
    /// `[[…` (no `|`, `#` or `]]` typed yet).
    // The suggestions live here, not on the Popup: `var` properties of a
    // Popup were lost when the JavaScript garbage collector ran.
    property var completionResults: []
    property int completionCurrent: 0
    /// Start of the typed path (after `[[`) or tag (after `#`).
    property int completionStart: -1
    /// "wiki" (file names after `[[`) or "tag" (tags of the project after `#`).
    property string completionKind: "wiki"
    function updateCompletion() {
        const pos = area.cursorPosition
        const before = area.getText(Math.max(0, pos - 200), pos)
        // `#ta…` after a space or at the line start: tags of the project.
        const tag = /(^|[\s(])#([^\s\d#.,;:!?()\[\]{}"'`<>|*~=+-][^\s#.,;:!?()\[\]{}"'`<>|*~=+]*)$/.exec(before)
        if (tag && area.activeFocus && area.selectionStart === area.selectionEnd) {
            const typed = tag[2].toLowerCase()
            let tags = []
            try { tags = JSON.parse(root.app.noteTags()) } catch (e) {}
            const results = tags.filter(t => t.key.startsWith(typed) && t.key !== typed)
                .sort((a, b) => b.count - a.count).slice(0, 8)
                .map(t => ({ kind: "tag", title: "#" + t.display, relative: t.count === 1 ? qsTr("1 Notiz") : qsTr("%1 Notizen").arg(t.count), tag: t.display }))
            if (results.length) {
                root.completionKind = "tag"
                root.completionResults = results
                root.completionCurrent = 0
                root.completionStart = pos - tag[2].length
                completion.open()
                return
            }
        }
        root.completionKind = "wiki"
        const open = before.lastIndexOf("[[")
        const query = open >= 0 ? before.slice(open + 2) : ""
        if (open < 0 || !area.activeFocus || /[\]\n|#]/.test(query) || area.selectionStart !== area.selectionEnd) {
            completion.close()
            return
        }
        const results = JSON.parse(root.app.quickOpen(query.replace(/^\//, ""), 12))
            .filter(r => r.kind !== "dir" && root.app.exists(r.path)).slice(0, 8)
        if (!results.length) { completion.close(); return }
        root.completionResults = results
        root.completionCurrent = 0
        root.completionStart = pos - query.length
        completion.open()
    }
    property bool showSymbols: false
    property var symbolList: []
    /// Index of the symbol containing the cursor.
    readonly property int currentSymbol: {
        let found = -1
        for (let i = 0; i < symbolList.length && symbolList[i].line <= area.cursorLine; ++i) found = i
        return found
    }
    function updateSymbols() { if (!root.wrap && !root.large) root.symbolList = JSON.parse(root.app.symbols(root.source, area.text)) }
    function symbolGlyph(kind) {
        return ({ function: "ƒ", method: "ƒ", type: "T", class: "C", interface: "I", impl: "⊞",
                  module: "M", macro: "!", section: "§", property: "•", signal: "⚡", object: "◇",
                  attribute: "=", target: "◎" })[kind] || "•"
    }
    onShowSymbolsChanged: if (showSymbols) updateSymbols()
    Timer { id: symbolTimer; interval: 500; onTriggered: root.updateSymbols() }

    property int editGeneration: 0
    property int savedGeneration: 0
    property bool loading: false
    property string lastText

    /// The text changed by editing (not by loading).
    signal edited()
    /// The file was saved or reloaded from disk.
    signal synced()
    /// "Save as" made `path` the edited file.
    signal savedAs(string path)

    color: Theme.editor

    TextDocument {
        id: doc
        recoveryDir: root.app.recoveryDir
        onTextLoaded: root.loadText()
        onSaved: (generation) => {
            root.savedGeneration = Math.max(root.savedGeneration, generation)
            root.synced()
        }
        onDiskChanged: doc.checkDisk(root.modified)
    }

    function open() {
        doc.open(root.source)
        loadText()
    }
    function loadText() {
        const top = flick.contentY
        const cursor = area.cursorPosition
        loading = true
        // Underlines of the old text would point past the new one.
        root.spellRanges = []
        root.spellMarks = []
        root.lastText = doc.text()
        area.text = root.lastText
        loading = false
        editGeneration = 0
        savedGeneration = 0
        area.cursorPosition = Math.min(cursor, area.length)
        flick.contentY = Math.min(top, Math.max(0, flick.contentHeight - flick.height))
        if (root.line > 0) Qt.callLater(root.reveal, root.line)
        if (root.pendingFind) Qt.callLater(root.applyFind)
        if (root.showSymbols) root.updateSymbols()
        spellTimer.restart()
        root.synced()
    }

    /// Saves now (no-op without changes).
    function save() {
        autosaveTimer.stop()
        if (!modified || readOnly) return
        recoveryTimer.stop()
        doc.save(area.text, editGeneration)
    }
    /// Inserts text at the cursor, or at the end (e.g. terminal output).
    function insertText(text, atEnd) {
        if (root.readOnly) return false
        area.insert(atEnd ? area.length : area.cursorPosition, text)
        return true
    }
    /// Checks (`done`) or unchecks the task list item on line `n` (1-based):
    /// `- [ ]` ↔ `- [x]`, as an ordinary edit (undo, autosave). False if
    /// the line holds no task or the file is read-only.
    function toggleTask(n, done) {
        if (root.readOnly) return false
        const start = positionOfLine(n)
        let end = area.text.indexOf("\n", start)
        if (end < 0) end = area.length
        const line = area.text.slice(start, end)
        const m = /^(\s*(?:[-*+]|\d+[.)])\s+\[)([ xX])\]/.exec(line)
        if (!m) return false
        const at = start + m[1].length
        area.remove(at, at + 1)
        area.insert(at, done ? "x" : " ")
        return true
    }
    function keepMine() { doc.saveKeepingMine(area.text, editGeneration) }

    function reveal(newLine) {
        if (newLine <= 0) return
        const pos = positionOfLine(newLine)
        area.cursorPosition = pos
        const rect = area.positionToRectangle(pos)
        flick.contentY = Math.max(0, Math.min(rect.y - flick.height / 3, flick.contentHeight - flick.height))
        flashLine.y = rect.y
        flashLine.opacity = 0.5
        flashAnim.restart()
        if (root.pendingFind) applyFind()
    }
    /// Indents (or with `outdent` moves back) the list items on the lines of
    /// the cursor or selection by two spaces; false if the first of these
    /// lines is no list item (Tab then works as usual).
    function indentListItems(outdent) {
        const item = /^\s*([-*+]|\d+[.)])(\s|$)/
        const text = area.text
        const start = Math.min(area.selectionStart, area.selectionEnd), end = Math.max(area.selectionStart, area.selectionEnd)
        let lineStart = text.lastIndexOf("\n", start - 1) + 1
        const firstLine = text.slice(lineStart, (text.indexOf("\n", lineStart) + 1 || text.length + 1) - 1)
        if (!item.test(firstLine)) return false
        const cursor = area.cursorPosition, anchor = cursor === area.selectionStart ? area.selectionEnd : area.selectionStart
        let shiftCursor = 0, shiftAnchor = 0
        // Back to front, so earlier positions stay valid.
        const starts = []
        for (let p = lineStart; p <= Math.max(start, end - (end > start ? 1 : 0)) && p <= text.length; ) {
            starts.push(p)
            const next = text.indexOf("\n", p)
            if (next < 0) break
            p = next + 1
        }
        for (let i = starts.length - 1; i >= 0; --i) {
            const p = starts[i]
            const line = text.slice(p, (text.indexOf("\n", p) + 1 || text.length + 1) - 1)
            if (!item.test(line)) continue
            let delta = 0
            if (!outdent) {
                area.insert(p, "  ")
                delta = 2
            } else {
                const lead = line.startsWith("\t") ? 1 : line.startsWith("  ") ? 2 : line.startsWith(" ") ? 1 : 0
                if (!lead) continue
                area.remove(p, p + lead)
                delta = -lead
            }
            if (cursor >= p) shiftCursor += delta
            if (anchor >= p) shiftAnchor += delta
        }
        if (anchor === cursor) area.cursorPosition = Math.max(lineStart, cursor + shiftCursor)
        else area.select(anchor + shiftAnchor, cursor + shiftCursor)
        return true
    }

    /// Enter in a Markdown list continues it: a new bullet, a new open
    /// checkbox (`- [ ] `), the next number, or `> ` in a quote. Enter on an
    /// empty item ends the list (an indented one moves out a level first).
    /// False if the cursor is not after such a marker.
    function continueList() {
        if (area.selectionStart !== area.selectionEnd) return false
        const pos = area.cursorPosition
        const text = area.text
        const lineStart = text.lastIndexOf("\n", pos - 1) + 1
        const lineEnd = (text.indexOf("\n", pos) + 1 || text.length + 1) - 1
        const line = text.slice(lineStart, lineEnd)
        const m = /^(\s*)(?:([-*+])|(\d+)([.)])|(>))(\s+)(\[[ xX]\]\s+)?/.exec(line)
        if (!m || pos < lineStart + m[0].length) return false
        if (line.slice(m[0].length).trim() === "") {
            if (pos !== lineEnd) return false
            if (m[1].length > 0 && !m[5]) indentListItems(true)
            else area.remove(lineStart, lineEnd)
            return true
        }
        const marker = m[5] ? ">" : m[3] ? (parseInt(m[3]) + 1) + m[4] : m[2]
        const insert = "\n" + m[1] + marker + m[6] + (m[7] ? "[ ] " : "")
        area.insert(pos, insert)
        area.cursorPosition = pos + insert.length
        return true
    }

    // ---- Markdown tables, with aligned columns in the text: Tab / Shift+Tab
    // go from cell to cell, Enter at the end of a row adds one, leaving a
    // table aligns it; alignTables() does the whole note. Never in code blocks.

    /// The delimiter row under the header: `| --- | :-: | --: |`.
    readonly property var tableDelimiter: /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/
    /// Cells of a table row (trimmed; `\|` stays inside a cell).
    function tableCells(line) {
        let s = line.trim()
        if (s.startsWith("|")) s = s.slice(1)
        if (s.endsWith("|") && !s.endsWith("\\|")) s = s.slice(0, -1)
        const cells = []
        let cell = ""
        for (let i = 0; i < s.length; ++i) {
            if (s[i] === "\\" && i + 1 < s.length) { cell += s[i] + s[i + 1]; ++i }
            else if (s[i] === "|") { cells.push(cell.trim()); cell = "" }
            else cell += s[i]
        }
        cells.push(cell.trim())
        return cells
    }
    /// Columns `s` takes in a monospace font (wide CJK and emoji count two,
    /// combining marks none).
    function textColumns(s) {
        let w = 0
        for (const ch of s) {
            const c = ch.codePointAt(0)
            if ((c >= 0x300 && c <= 0x36f) || c === 0x200d || (c >= 0xfe00 && c <= 0xfe0f)) continue
            const wide = (c >= 0x1100 && c <= 0x115f) || (c >= 0x2e80 && c <= 0xa4cf) || (c >= 0xac00 && c <= 0xd7a3)
                || (c >= 0xf900 && c <= 0xfaff) || (c >= 0xfe30 && c <= 0xfe4f) || (c >= 0xff00 && c <= 0xff60)
                || (c >= 0xffe0 && c <= 0xffe6) || (c >= 0x1f300 && c <= 0x1faff) || (c >= 0x20000 && c <= 0x3fffd)
            w += wide ? 2 : 1
        }
        return w
    }
    /// The table rows `lines` (header, delimiter, body) with aligned
    /// columns: {lines, offsets} – offsets[row][col] is where a cell's text
    /// starts in its line. Alignment from the delimiter row is kept and
    /// applied (`:-:` centred, `--:` right).
    function formatTable(lines) {
        const indent = /^\s*/.exec(lines[0])[0]
        const rows = lines.map(l => root.tableCells(l))
        const cols = Math.max(...rows.map(r => r.length))
        const align = [], width = []
        for (let c = 0; c < cols; ++c) {
            const d = rows[1][c] || ""
            align.push(d.length > 1 && d.startsWith(":") && d.endsWith(":") ? "c" : d.endsWith(":") ? "r" : d.startsWith(":") ? "l" : "")
            let w = 3
            rows.forEach((r, i) => { if (i !== 1) w = Math.max(w, root.textColumns(r[c] || "")) })
            width.push(w)
        }
        const out = [], offsets = []
        rows.forEach((r, i) => {
            let line = indent + "|"
            const offs = []
            for (let c = 0; c < cols; ++c) {
                const w = width[c], a = align[c]
                if (i === 1) {
                    const cell = a === "c" ? ":" + "-".repeat(w - 2) + ":" : a === "r" ? "-".repeat(w - 1) + ":"
                        : a === "l" ? ":" + "-".repeat(w - 1) : "-".repeat(w)
                    line += " "
                    offs.push(line.length)
                    line += cell + " |"
                } else {
                    const t = r[c] || ""
                    const pad = w - root.textColumns(t)
                    const left = a === "r" ? pad : a === "c" ? Math.floor(pad / 2) : 0
                    line += " " + " ".repeat(left)
                    offs.push(line.length)
                    line += t + " ".repeat(pad - left) + " |"
                }
            }
            out.push(line)
            offsets.push(offs)
        })
        return { lines: out, offsets: offsets }
    }
    /// A table given as text, aligned (for inserting).
    function alignTable(markdown) { return formatTable(markdown.split("\n")).lines.join("\n") }

    /// Inside a fenced code block at `pos`?
    function inCodeFence(pos) {
        const fences = area.text.slice(0, pos).match(/^\s*(```|~~~)/gm)
        return !!fences && fences.length % 2 === 1
    }
    /// The table with the line at `pos`: {start, end, lines, starts}, or null.
    function tableAt(pos) {
        if (!root.wikiCompletion) return null
        const text = area.text
        const lineStart = (p) => text.lastIndexOf("\n", p - 1) + 1
        const lineEnd = (p) => { const e = text.indexOf("\n", p); return e < 0 ? text.length : e }
        const isRow = (l) => l.trim() !== "" && l.indexOf("|") >= 0
        let s = lineStart(pos)
        if (!isRow(text.slice(s, lineEnd(s)))) return null
        // The block of consecutive lines with a pipe around `pos`.
        while (s > 0) {
            const prev = lineStart(s - 1)
            if (!isRow(text.slice(prev, s - 1))) break
            s = prev
        }
        const lines = [], starts = []
        for (let p = s; ; ) {
            const e = lineEnd(p)
            const l = text.slice(p, e)
            if (!isRow(l)) break
            lines.push(l)
            starts.push(p)
            if (e >= text.length) break
            p = e + 1
        }
        // The table starts at the header above the first delimiter row.
        const k = lines.findIndex((l, i) => i > 0 && root.tableDelimiter.test(l))
        if (k < 1) return null
        const first = k - 1
        if (pos < starts[first] || root.inCodeFence(starts[first])) return null
        const last = lines.length - 1
        return { start: starts[first], end: starts[last] + lines[last].length,
                 lines: lines.slice(first), starts: starts.slice(first) }
    }
    /// Row and cell of `pos` in table `t`, and the offset into the cell text.
    function tableCellAt(t, pos) {
        let row = t.starts.length - 1
        while (row > 0 && t.starts[row] > pos) --row
        const line = t.lines[row], col = pos - t.starts[row]
        const lead = /^\s*\|?/.exec(line)[0].length
        let cell = 0, cellStart = lead
        for (let i = lead; i < col && i < line.length; ++i) {
            if (line[i] === "\\") { ++i; continue }
            if (line[i] === "|") { ++cell; cellStart = i + 1 }
        }
        return { row: row, cell: cell, offset: line.slice(cellStart, col).replace(/^\s+/, "").length }
    }
    /// Replaces table `t` by `lines` (only the part that changed: a small
    /// edit for undo); returns the new start of each row.
    function replaceTable(t, lines) {
        const old = area.text.slice(t.start, t.end), now = lines.join("\n")
        if (old !== now) {
            let p = 0
            while (p < old.length && p < now.length && old[p] === now[p]) ++p
            let q = 0
            while (q < old.length - p && q < now.length - p && old[old.length - 1 - q] === now[now.length - 1 - q]) ++q
            area.remove(t.start + p, t.start + old.length - q)
            area.insert(t.start + p, now.slice(p, now.length - q))
        }
        const starts = []
        let at = t.start
        for (const l of lines) { starts.push(at); at += l.length + 1 }
        return starts
    }
    /// Puts the cursor into a cell, its text selected (typing replaces it).
    function selectCell(f, starts, row, cell) {
        const begin = starts[row] + f.offsets[row][cell]
        const line = f.lines[row]
        let end = begin
        while (end - starts[row] < line.length && !(line[end - starts[row]] === "|" && line[end - starts[row] - 1] !== "\\")) ++end
        const text = area.text.slice(begin, end).replace(/\s+$/, "")
        area.select(begin, begin + text.length)
    }
    /// Tab / Shift+Tab in a table: align it and go to the next / previous
    /// cell (Tab in the last cell adds a row). False outside a table.
    function tableTab(back) {
        if (root.readOnly) return false
        const pos = area.cursorPosition
        const t = tableAt(pos)
        if (!t) return false
        const at = tableCellAt(t, pos)
        let lines = t.lines
        const cols = Math.max(...lines.map(l => root.tableCells(l).length))
        let row = at.row, cell = Math.min(at.cell, cols - 1) + (back ? -1 : 1)
        if (cell >= cols) { ++row; cell = 0 }
        if (cell < 0) { --row; cell = cols - 1 }
        if (row === 1) row += back ? -1 : 1
        if (row < 0) { row = 0; cell = 0 }
        if (row >= lines.length) lines = lines.concat(["|" + " |".repeat(cols)])
        const f = formatTable(lines)
        selectCell(f, replaceTable(t, f.lines), row, cell)
        return true
    }
    /// Enter at the end of a table row: align the table and add an empty row
    /// below (below the delimiter for the header). In an empty last row:
    /// the row goes and the table ends, as with lists. False otherwise.
    function tableEnter() {
        if (root.readOnly || area.selectionStart !== area.selectionEnd) return false
        const pos = area.cursorPosition
        const t = tableAt(pos)
        if (!t) return false
        const at = tableCellAt(t, pos)
        if (pos !== t.starts[at.row] + t.lines[at.row].length) return false
        const last = t.lines.length - 1
        if (at.row === last && at.row > 1 && root.tableCells(t.lines[last]).every(c => c === "")) {
            const f = formatTable(t.lines.slice(0, last))
            const starts = replaceTable({ start: t.start, end: t.end }, f.lines.concat([""]))
            area.cursorPosition = starts[last]
            return true
        }
        const cols = Math.max(...t.lines.map(l => root.tableCells(l).length))
        const row = Math.max(2, at.row + 1)
        const lines = t.lines.slice(0, row).concat(["|" + " |".repeat(cols)], t.lines.slice(row))
        const f = formatTable(lines)
        selectCell(f, replaceTable(t, f.lines), row, 0)
        return true
    }
    /// Aligns the table at `pos`, keeping the cursor in its cell.
    function alignTableAt(pos) {
        const t = tableAt(pos)
        if (!t) return
        const f = formatTable(t.lines)
        if (f.lines.join("\n") === area.text.slice(t.start, t.end)) return
        const cursor = area.cursorPosition
        const inside = cursor >= t.start && cursor <= t.end
        const at = inside ? tableCellAt(t, cursor) : null
        const delta = f.lines.join("\n").length - (t.end - t.start)
        const starts = replaceTable(t, f.lines)
        if (at) {
            const cellText = root.tableCells(f.lines[at.row])[Math.min(at.cell, f.offsets[at.row].length - 1)] || ""
            area.cursorPosition = starts[at.row] + f.offsets[at.row][Math.min(at.cell, f.offsets[at.row].length - 1)]
                + Math.min(at.offset, cellText.length)
        } else if (cursor > t.end) area.cursorPosition = cursor + delta
        else area.cursorPosition = cursor
    }
    /// Aligns every table of the note (from the bottom, so positions above
    /// stay valid). Returns how many changed.
    function alignTables() {
        if (root.readOnly || !root.wikiCompletion) return 0
        const found = []
        const text = area.text
        let p = 0
        while (p < text.length) {
            const e = text.indexOf("\n", p)
            const end = e < 0 ? text.length : e
            const t = root.tableDelimiter.test(text.slice(p, end)) && p > 0 ? tableAt(p) : null
            if (t && (found.length === 0 || found[found.length - 1].start !== t.start)) found.push(t)
            p = (t ? t.end : end) + 1
        }
        let changed = 0
        for (let i = found.length - 1; i >= 0; --i) {
            const before = area.text
            alignTableAt(found[i].start)
            if (area.text !== before) ++changed
        }
        return changed
    }
    // Leaving a table aligns it (a moment later: not while selecting).
    property int lastTablePos: -1
    function cursorMoved() {
        if (!root.wikiCompletion || root.readOnly || root.large) return
        tableLeaveTimer.restart()
    }
    Timer {
        id: tableLeaveTimer
        interval: 300
        onTriggered: {
            const pos = area.cursorPosition
            const was = root.lastTablePos
            const now = area.selectionStart === area.selectionEnd ? root.tableAt(pos) : null
            root.lastTablePos = now ? pos : -1
            if (was < 0 || area.selectionStart !== area.selectionEnd || was > area.length) return
            const t = root.tableAt(was)
            if (t && !(pos >= t.start && pos <= t.end)) root.alignTableAt(was)
        }
    }

    // ---- spellcheck (notes, German and English): the misspelled words are
    // underlined in the visible part; a right click offers suggestions.
    readonly property bool spellOn: root.wikiCompletion && !root.large && !!root.app && root.app.spellcheck
    onSpellOnChanged: updateSpelling()
    /// [start, length] of each misspelled word (characters).
    property var spellRanges: []
    /// Underlines of the visible ones: {x, y, w}.
    property var spellMarks: []
    function updateSpelling() {
        let ranges = null
        if (spellOn) try { ranges = JSON.parse(root.app.spellCheck(area.text)) } catch (e) {}
        spellRanges = ranges || []
        updateSpellMarks()
    }
    function updateSpellMarks() {
        const top = flick.contentY - 40, bottom = flick.contentY + flick.height + 40
        const marks = []
        for (const r of spellRanges) {
            // Ranges of an older text (checked again shortly).
            if (r[0] + r[1] > area.length) break
            const a = area.positionToRectangle(r[0])
            if (a.y > bottom) break
            if (a.y + a.height < top) continue
            const b = area.positionToRectangle(r[0] + r[1])
            // A word broken across two rows is left out.
            if (Math.abs(b.y - a.y) > 1 || b.x <= a.x) continue
            marks.push({ x: a.x, y: a.y + a.height - 1, w: b.x - a.x })
            if (marks.length >= 500) break
        }
        spellMarks = marks
    }
    /// The misspelled word at character `pos`: [start, length] or null.
    function spellRangeAt(pos) {
        return spellRanges.find(r => pos >= r[0] && pos <= r[0] + r[1]) || null
    }
    Timer { id: spellTimer; interval: 450; onTriggered: root.updateSpelling() }
    Timer { id: spellMarksTimer; interval: 50; onTriggered: root.updateSpellMarks() }
    Connections {
        target: flick
        function onContentYChanged() { if (root.spellMarks.length || root.spellRanges.length) spellMarksTimer.restart() }
        function onWidthChanged() { spellMarksTimer.restart() }
    }

    // Right click: suggestions for a misspelled word, cut, copy, paste.
    property var menuWord: null
    property var menuSuggestions: []
    function showEditMenu(x, y) {
        const pos = area.positionAt(x, y)
        const r = spellOn ? spellRangeAt(pos) : null
        menuWord = r ? { start: r[0], length: r[1], text: area.text.slice(r[0], r[0] + r[1]) } : null
        menuSuggestions = menuWord ? root.app.spellSuggest(menuWord.text) : []
        editMenu.popup()
    }
    function replaceWord(start, length, text) {
        area.remove(start, start + length)
        area.insert(start, text)
        area.cursorPosition = start + text.length
    }

    /// Inserts `text` at the cursor (replacing a selection).
    function insertAtCursor(text) {
        if (!text) return
        const start = Math.min(area.selectionStart, area.selectionEnd)
        if (area.selectionStart !== area.selectionEnd) area.remove(start, Math.max(area.selectionStart, area.selectionEnd))
        area.insert(start, text)
        area.cursorPosition = start + text.length
        area.forceActiveFocus()
    }
    /// Inserts a block (table, diagram, picture …) at the cursor, with an
    /// empty line before and after it.
    function insertBlock(block) {
        if (!block) return
        const pos = area.cursorPosition
        const before = area.text.slice(0, pos), after = area.text.slice(pos)
        const lead = before.length === 0 || before.endsWith("\n\n") ? "" : before.endsWith("\n") ? "\n" : "\n\n"
        const body = block.replace(/\n+$/, "")
        const tail = after.startsWith("\n\n") ? "" : after.startsWith("\n") ? "\n" : "\n\n"
        area.insert(pos, lead + body + tail)
        area.cursorPosition = pos + lead.length + body.length
        area.forceActiveFocus()
    }
    /// Markdown: Ctrl+V with a picture or files on the clipboard puts them
    /// into `assets/` and links them; false if there are none (plain paste).
    function pasteSpecial() {
        const markdown = root.app.pasteIntoNote(root.source)
        if (!markdown) return false
        insertAtCursor(markdown)
        return true
    }

    /// Lines of the text (not wrapped rows), for the gutter width.
    readonly property int logicalLines: wrap ? area.text.split("\n").length : 0
    /// Numbers of the visible lines of wrapped text and where they start:
    /// [{n, y}], updated after scrolling, typing and resizing.
    property var wrappedNumbers: []
    function updateWrappedNumbers() {
        if (!wrap || !showLineNumbers) return
        const text = area.text
        const bottom = flick.contentY + flick.height
        let n = Math.max(1, lineAtY(flick.contentY))
        let pos = positionOfLine(n)
        const out = []
        for (;;) {
            const y = area.positionToRectangle(pos).y
            if (y > bottom) break
            out.push({ n: n, y: y })
            const next = text.indexOf("\n", pos)
            if (next < 0) break
            pos = next + 1
            n++
        }
        wrappedNumbers = out
    }
    Timer { id: numbersTimer; interval: 0; onTriggered: root.updateWrappedNumbers() }
    Connections {
        target: flick
        function onContentYChanged() { numbersTimer.restart() }
        function onWidthChanged() { numbersTimer.restart() }
        function onHeightChanged() { numbersTimer.restart() }
    }
    Connections {
        target: area
        function onTextChanged() { numbersTimer.restart() }
        function onContentHeightChanged() { numbersTimer.restart() }
    }
    function positionOfLine(n) {
        let pos = 0
        for (let i = 1; i < n; ++i) {
            const next = area.text.indexOf("\n", pos)
            if (next < 0) break
            pos = next + 1
        }
        return pos
    }
    function lineAtY(y) {
        const pos = area.positionAt(0, y)
        let count = 1, idx = -1
        while ((idx = area.text.indexOf("\n", idx + 1)) >= 0 && idx < pos) count++
        return count
    }
    readonly property alias fling: touchpadFling
    /// Vertical scroll position, for scroll sync.
    readonly property real scrollY: flick.contentY
    /// Line (with the fraction) at height `viewY` of the view (scroll
    /// sync, #7).
    function exactLineAt(viewY) {
        const y = flick.contentY + viewY
        const n = wrap ? lineAtY(y) : Math.max(1, Math.floor((y - area.topPadding) / lineHeight) + 1)
        const start = positionOfLine(n)
        const y0 = area.positionToRectangle(start).y
        const next = area.text.indexOf("\n", start)
        const y1 = next < 0 ? y0 + lineHeight : area.positionToRectangle(next + 1).y
        return n + (y1 > y0 ? Math.max(0, Math.min(1, (y - y0) / (y1 - y0))) : 0)
    }
    function exactTopLine() { return exactLineAt(0) }
    /// Scrolls so that line `line` (fractional) is at height `viewY`
    /// (default: the top).
    function scrollToExactLine(line, viewY) {
        const n = Math.max(1, Math.floor(line))
        const start = positionOfLine(n)
        const y0 = area.positionToRectangle(start).y
        const next = area.text.indexOf("\n", start)
        const y1 = next < 0 ? y0 + lineHeight : area.positionToRectangle(next + 1).y
        const y = y0 + (y1 - y0) * (line - n) - (viewY || 0)
        flick.contentY = Math.max(0, Math.min(y, flick.contentHeight - flick.height))
    }
    readonly property real viewHeight: flick.height
    function scrollToLine(n) {
        const rect = area.positionToRectangle(positionOfLine(n))
        flick.contentY = Math.max(0, Math.min(rect.y, flick.contentHeight - flick.height))
    }

    function viewState() { return { line: root.topLine, cursor: area.cursorPosition, symbols: root.showSymbols } }
    function focusText() { area.forceActiveFocus() }
    /// Line of the cursor (1-based).
    function currentLine() { return area.getText(0, area.cursorPosition).split("\n").length }
    /// Wiki link suggestions are shown (see `updateCompletion`).
    readonly property bool completing: completion.visible
    /// Inserts the chosen suggestion.
    function acceptCompletion() {
        const r = root.completionResults[root.completionCurrent]
        if (!r || root.completionStart < 0) { completion.close(); return }
        const end = area.cursorPosition
        if (r.kind === "tag") {
            area.remove(root.completionStart, end)
            area.insert(root.completionStart, r.tag)
            area.cursorPosition = root.completionStart + r.tag.length
            completion.close()
            return
        }
        const closing = area.text.slice(end, end + 2) === "]]" ? "" : "]]"
        area.remove(root.completionStart, end)
        area.insert(root.completionStart, "/" + r.relative + closing)
        area.cursorPosition = root.completionStart + r.relative.length + 1 + 2
        completion.close()
    }
    function moveCompletion(step) {
        root.completionCurrent = (root.completionCurrent + step + root.completionResults.length) % root.completionResults.length
    }

    function applyState(state) {
        if (state.cursor !== undefined) area.cursorPosition = Math.min(state.cursor, area.length)
        if (state.symbols !== undefined) root.showSymbols = state.symbols
        if (state.line) Qt.callLater(root.scrollToLine, state.line)
        if (state.find) { root.pendingFind = state.find; Qt.callLater(root.applyFind) }
    }

    /// A hit of the project search to select once the text is there:
    /// `{ line, start, end }` (UTF-16 offsets in the line).
    property var pendingFind: null
    function applyFind() {
        const f = root.pendingFind
        if (!f || area.length === 0) return
        root.pendingFind = null
        const lineStart = positionOfLine(f.line)
        const lineEnd = area.text.indexOf("\n", lineStart) < 0 ? area.length : area.text.indexOf("\n", lineStart)
        area.select(Math.min(lineStart + (f.start || 0), lineEnd), Math.min(lineStart + (f.end || 0), lineEnd))
        const rect = area.positionToRectangle(area.selectionStart)
        flick.contentY = Math.max(0, Math.min(rect.y - flick.height / 3, flick.contentHeight - flick.height))
    }

    function find(text, backwards) {
        if (!text) return
        const hay = area.text.toLowerCase()
        const needle = text.toLowerCase()
        let at
        if (backwards) {
            at = hay.lastIndexOf(needle, Math.max(0, area.selectionStart - 1))
            if (at < 0) at = hay.lastIndexOf(needle)
        } else {
            at = hay.indexOf(needle, area.selectionEnd)
            if (at < 0) at = hay.indexOf(needle)
        }
        findBar.found = at >= 0
        if (at < 0) return
        area.select(at, at + text.length)
        const rect = area.positionToRectangle(at)
        if (rect.y < flick.contentY || rect.y > flick.contentY + flick.height - root.lineHeight)
            flick.contentY = Math.max(0, Math.min(rect.y - flick.height / 3, flick.contentHeight - flick.height))
    }
    function openFind() {
        findBar.visible = true
        if (area.selectedText && area.selectedText.indexOf("\n") < 0) findField.text = area.selectedText
        findField.forceActiveFocus()
        findField.selectAll()
    }

    onSourceChanged: open()
    onLineChanged: if (line > 0) reveal(line)
    Component.onCompleted: open()
    Component.onDestruction: save()

    Connections {
        target: root.app
        function onSaveAllRequested() { root.save() }
    }

    Timer {
        id: autosaveTimer
        interval: 1000
        onTriggered: if (root.autosave) root.save()
    }
    Timer {
        id: recoveryTimer
        interval: 300
        onTriggered: if (root.modified) doc.writeRecovery(area.text)
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ---- banners
        Banner {
            visible: doc.hasRecovery && !root.readOnly
            tone: Theme.warn
            message: qsTr("Ungespeicherte Änderungen vom %1 gefunden.").arg(new Date(doc.hasRecovery ? doc.recoveryTime() : 0).toLocaleString(Qt.locale(), Locale.ShortFormat))
            actions: [
                { text: qsTr("Wiederherstellen"), run: () => {
                    const text = doc.restoreRecovery()
                    if (!text) return
                    area.text = text
                    root.editGeneration++
                    if (!doc.conflict) autosaveTimer.restart()
                } },
                { text: qsTr("Verwerfen"), run: () => doc.discardRecovery() },
            ]
        }
        Banner {
            visible: doc.conflict !== ""
            tone: Theme.error
            message: doc.conflict === "deleted"
                ? qsTr("Die Datei wurde außerhalb von Atlas gelöscht oder verschoben. Autosave ist angehalten.")
                : qsTr("Die Datei wurde außerhalb von Atlas geändert. Autosave ist angehalten.")
            actions: doc.conflict === "deleted" ? [
                { text: qsTr("Erneut speichern"), run: () => root.keepMine() },
                { text: qsTr("Als neue Datei speichern …"), run: () => saveAsDialog.ask() },
            ] : [
                { text: qsTr("Neu laden"), run: () => doc.reload() },
                { text: qsTr("Meine Fassung behalten"), run: () => root.keepMine() },
                { text: qsTr("Als neue Datei speichern …"), run: () => saveAsDialog.ask() },
            ]
        }
        Banner {
            visible: root.readOnly
            tone: Theme.textFaint
            message: qsTr("Schreibgeschützt: %1").arg(doc.readOnly)
        }
        Banner {
            visible: doc.error !== "" && !root.readOnly
            tone: Theme.error
            message: doc.error
            actions: [{ text: qsTr("Erneut versuchen"), run: () => root.save() }]
        }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            // ---- line numbers: unwrapped text has a fixed line height;
            // wrapped text (Markdown) numbers each line where it starts.
            Rectangle {
                id: gutter
                visible: root.showLineNumbers
                width: visible ? Math.max(3, String(root.wrap ? root.logicalLines : area.lineCount).length) * 8 + 20 : 0
                height: parent.height
                color: Theme.editor
                clip: true
                Repeater {
                    model: gutter.visible && root.wrap ? root.wrappedNumbers : []
                    Text {
                        required property var modelData
                        x: 0
                        width: gutter.width - 10
                        // (positionToRectangle includes the top padding.)
                        y: modelData.y - flick.contentY
                        height: root.lineHeight
                        horizontalAlignment: Text.AlignRight
                        verticalAlignment: Text.AlignVCenter
                        text: modelData.n
                        color: area.cursorLine === modelData.n ? Theme.text : Theme.textFaint
                        font.family: Theme.monoFont
                        font.pixelSize: 12
                    }
                }
                Repeater {
                    model: gutter.visible && !root.wrap ? Math.ceil(flick.height / root.lineHeight) + 1 : 0
                    Text {
                        required property int index
                        readonly property int number: Math.floor(flick.contentY / root.lineHeight) + index + 1
                        visible: number <= area.lineCount
                        x: 0
                        width: gutter.width - 10
                        y: (number - 1) * root.lineHeight - flick.contentY + area.topPadding
                        height: root.lineHeight
                        horizontalAlignment: Text.AlignRight
                        verticalAlignment: Text.AlignVCenter
                        text: number
                        color: area.cursorLine === number ? Theme.text : Theme.textFaint
                        font.family: Theme.monoFont
                        font.pixelSize: 12
                    }
                }
            }

            Flickable {
                id: flick
                anchors.fill: parent
                anchors.leftMargin: gutter.width
                anchors.rightMargin: symbolPanel.width
                clip: true
                contentWidth: root.wrap ? width : Math.max(width, area.implicitWidth)
                contentHeight: area.height
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar {}
                ScrollBar.horizontal: ScrollBar { visible: !root.wrap }

                function ensureVisible(r) {
                    if (contentX >= r.x) contentX = r.x
                    else if (contentX + width <= r.x + r.width) contentX = r.x + r.width - width
                    if (contentY >= r.y) contentY = r.y
                    else if (contentY + height <= r.y + r.height) contentY = r.y + r.height - height
                }

                Rectangle {
                    id: currentLine
                    visible: area.activeFocus && area.selectionStart === area.selectionEnd
                    width: flick.contentWidth
                    height: area.cursorRectangle.height
                    y: area.cursorRectangle.y
                    color: Theme.light ? "#f0f3f8" : "#262626"
                }
                Rectangle {
                    id: flashLine
                    width: flick.contentWidth
                    height: root.lineHeight
                    color: Theme.activeSelection
                    opacity: 0
                    NumberAnimation on opacity { id: flashAnim; running: false; to: 0; duration: 1200; easing.type: Easing.InQuad }
                }

                TextEdit {
                    id: area
                    // (Wrapped text: count the line breaks before the cursor.)
                    readonly property int cursorLine: root.wrap ? text.slice(0, cursorPosition).split("\n").length
                                                                : Math.floor(cursorRectangle.y / root.lineHeight) + 1
                    width: root.wrap ? flick.width : Math.max(flick.width, implicitWidth)
                    padding: 8
                    leftPadding: root.wrap ? 24 : 6
                    rightPadding: root.wrap ? 24 : 12
                    readOnly: root.readOnly
                    selectByMouse: true
                    persistentSelection: true
                    wrapMode: root.wrap ? TextEdit.Wrap : TextEdit.NoWrap
                    textFormat: TextEdit.PlainText
                    color: Theme.text
                    selectionColor: Theme.activeSelection
                    selectedTextColor: Theme.textStrong
                    font.family: Theme.monoFont
                    font.pixelSize: 13
                    tabStopDistance: 4 * fontMetrics.averageCharacterWidth
                    focus: true
                    onCursorRectangleChanged: if (activeFocus) flick.ensureVisible(cursorRectangle)
                    // Highlighting changes formats only, which also emits
                    // textChanged; count real edits only.
                    onTextChanged: {
                        if (root.loading || text === root.lastText) return
                        root.lastText = text
                        root.editGeneration++
                        root.edited()
                        recoveryTimer.restart()
                        if (root.autosave) autosaveTimer.restart()
                        if (root.showSymbols) symbolTimer.restart()
                        // Old underlines would sit in the wrong place until checked again.
                        if (root.spellOn) { root.spellRanges = []; root.spellMarks = []; spellTimer.restart() }
                    }
                    onCursorPositionChanged: if (root.wikiCompletion) { completionTimer.restart(); root.cursorMoved() }
                    Keys.onPressed: (event) => {
                        const ctrl = event.modifiers & Qt.ControlModifier
                        if (completion.visible) {
                            if (event.key === Qt.Key_Down) { root.moveCompletion(1); event.accepted = true; return }
                            if (event.key === Qt.Key_Up) { root.moveCompletion(-1); event.accepted = true; return }
                            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Tab) {
                                root.acceptCompletion(); event.accepted = true; return
                            }
                            if (event.key === Qt.Key_Escape) { completion.close(); event.accepted = true; return }
                        }
                        // Markdown tables: Tab / Shift+Tab go from cell to cell, Enter
                        // at the end of a row adds one (all aligning the table).
                        if (root.wikiCompletion && !ctrl && (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab)
                                && root.tableTab(event.key === Qt.Key_Backtab || (event.modifiers & Qt.ShiftModifier))) {
                            event.accepted = true
                            return
                        }
                        if (root.wikiCompletion && !ctrl && !(event.modifiers & Qt.ShiftModifier)
                                && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && root.tableEnter()) {
                            event.accepted = true
                            return
                        }
                        // Markdown lists: Enter continues them (Shift+Enter: a plain line break).
                        if (root.wikiCompletion && !ctrl && !(event.modifiers & Qt.ShiftModifier)
                                && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && root.continueList()) {
                            event.accepted = true
                            return
                        }
                        // Markdown lists: Tab indents the item, Shift+Tab outdents it.
                        if (root.wrap && !ctrl && (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab)
                                && root.indentListItems(event.key === Qt.Key_Backtab || (event.modifiers & Qt.ShiftModifier))) {
                            event.accepted = true
                            return
                        }
                        // Markdown: pictures and files from the clipboard.
                        if (root.wikiCompletion && ((ctrl && event.key === Qt.Key_V) || ((event.modifiers & Qt.ShiftModifier) && event.key === Qt.Key_Insert))
                                && root.pasteSpecial()) {
                            event.accepted = true
                            return
                        }
                        if (ctrl && event.key === Qt.Key_S) { root.save(); event.accepted = true }
                        else if (ctrl && event.key === Qt.Key_F) { root.openFind(); event.accepted = true }
                        else if (ctrl && event.key === Qt.Key_G) { gotoBar.open(); event.accepted = true }
                        else if (event.key === Qt.Key_Escape && findBar.visible) { findBar.visible = false; event.accepted = true }
                    }
                    FontMetrics { id: fontMetrics; font: area.font }

                    // Misspelled words.
                    Repeater {
                        model: root.spellMarks
                        Rectangle {
                            required property var modelData
                            x: modelData.x
                            y: modelData.y
                            width: modelData.w
                            height: 1.6
                            color: Theme.error
                            opacity: 0.9
                        }
                    }
                    TapHandler {
                        acceptedButtons: Qt.RightButton
                        onTapped: (point) => root.showEditMenu(point.position.x, point.position.y)
                    }
                    Menu {
                        id: editMenu
                        Instantiator {
                            model: root.menuSuggestions
                            delegate: MenuItem {
                                required property string modelData
                                text: modelData
                                font.bold: true
                                onTriggered: if (root.menuWord) root.replaceWord(root.menuWord.start, root.menuWord.length, modelData)
                            }
                            onObjectAdded: (index, object) => editMenu.insertItem(index, object)
                            onObjectRemoved: (index, object) => editMenu.removeItem(object)
                        }
                        MenuItem {
                            visible: !!root.menuWord && root.menuSuggestions.length === 0
                            height: visible ? implicitHeight : 0
                            enabled: false
                            text: qsTr("Keine Vorschläge")
                        }
                        MenuItem {
                            visible: !!root.menuWord
                            height: visible ? implicitHeight : 0
                            text: root.menuWord ? qsTr("„%1“ zum Wörterbuch hinzufügen").arg(root.menuWord.text) : ""
                            onTriggered: { root.app.spellAdd(root.menuWord.text); root.updateSpelling() }
                        }
                        MenuSeparator { visible: !!root.menuWord; height: visible ? implicitHeight : 0 }
                        MenuItem { text: qsTr("Ausschneiden"); enabled: area.selectedText.length > 0; onTriggered: area.cut() }
                        MenuItem { text: qsTr("Kopieren"); enabled: area.selectedText.length > 0; onTriggered: area.copy() }
                        MenuItem {
                            text: qsTr("Einfügen")
                            onTriggered: { if (!(root.wikiCompletion && root.pasteSpecial())) area.paste() }
                        }
                        MenuItem { text: qsTr("Alles auswählen"); onTriggered: area.selectAll() }
                    }

                    // Markdown: files dropped on the text are copied into
                    // `assets/` (pictures) and linked where they were dropped.
                    DropArea {
                        anchors.fill: parent
                        enabled: root.wikiCompletion
                        keys: ["text/uri-list"]
                        onDropped: (drop) => {
                            const markdown = root.app.importIntoNote(root.source, JSON.stringify(drop.urls.map(u => u.toString())))
                            if (!markdown) return
                            area.cursorPosition = area.positionAt(drop.x, drop.y)
                            root.insertAtCursor(markdown)
                            drop.accept(Qt.CopyAction)
                        }
                    }

                    // Completion of wiki links: `[[` suggests project files.
                    Popup {
                        id: completion
                        x: area.cursorRectangle.x
                        y: area.cursorRectangle.y + area.cursorRectangle.height + 2
                        width: 420
                        height: Math.min(8, root.completionResults.length) * 26 + 8
                        padding: 4
                        focus: false
                        closePolicy: Popup.CloseOnPressOutside
                        background: Rectangle { color: Theme.header; border.color: Theme.border; radius: 4 }
                        ListView {
                            anchors.fill: parent
                            model: root.completionResults
                            currentIndex: root.completionCurrent
                            clip: true
                            delegate: Rectangle {
                                required property var modelData
                                required property int index
                                width: ListView.view.width
                                height: 26
                                radius: 3
                                color: index === root.completionCurrent ? Theme.activeSelection : "transparent"
                                Row {
                                    anchors.verticalCenter: parent.verticalCenter
                                    x: 6
                                    spacing: 8
                                    Text { text: modelData.kind === "tag" ? "#" : modelData.kind === "note" ? "📝" : "📄"; font.pixelSize: 12; color: Theme.link }
                                    Text {
                                        text: modelData.title ? modelData.title + "  ·  " + modelData.relative : modelData.relative
                                        color: Theme.text
                                        font.pixelSize: Theme.smallFontSize
                                        width: completion.width - 50
                                        elide: Text.ElideMiddle
                                    }
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    onClicked: { root.completionCurrent = parent.index; root.acceptCompletion() }
                                }
                            }
                        }
                    }
                }
            }
            // Momentum after touchpad swipes (#4).
            TouchpadFling {
                id: touchpadFling
                anchors.fill: flick
                flickable: flick
                brisk: root.briskScrolling
                active: root.app.touchpadFling
                app: root.app
            }
            Timer {
                id: completionTimer
                interval: 120
                onTriggered: root.updateCompletion()
            }

            SyntaxHighlighter {
                textEdit: area
                definition: root.large ? "None" : root.syntax
                theme: Repository.theme(Theme.light ? "Breeze Light" : "Breeze Dark")
            }

            // ---- symbol overview (where it can be determined reliably)
            Rectangle {
                id: symbolPanel
                visible: root.showSymbols
                anchors.right: parent.right
                width: visible ? Math.min(260, parent.width * 0.4) : 0
                height: parent.height
                color: Theme.sidebar
                Rectangle { width: 1; height: parent.height; color: Theme.border }
                ListView {
                    id: symbolList
                    anchors.fill: parent
                    anchors.leftMargin: 1
                    clip: true
                    model: root.symbolList
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar {}
                    header: Label {
                        width: symbolList.width
                        padding: 8
                        text: qsTr("SYMBOLE")
                        color: Theme.text
                        font.bold: true
                        font.pixelSize: Theme.smallFontSize
                    }
                    delegate: ItemDelegate {
                        required property var modelData
                        required property int index
                        width: ListView.view.width
                        height: Theme.rowHeight
                        highlighted: index === root.currentSymbol
                        leftPadding: 8 + modelData.depth * 12
                        background: Rectangle { color: parent.highlighted ? Theme.selection : parent.hovered ? Theme.hover : "transparent" }
                        contentItem: Text {
                            text: root.symbolGlyph(modelData.kind) + "  " + modelData.name
                            color: Theme.text
                            font.pixelSize: Theme.fontSize
                            elide: Text.ElideRight
                            verticalAlignment: Text.AlignVCenter
                        }
                        onClicked: { root.reveal(modelData.line); area.forceActiveFocus() }
                    }
                    Label {
                        anchors.centerIn: parent
                        width: parent.width - 24
                        visible: symbolList.count === 0
                        wrapMode: Text.Wrap
                        horizontalAlignment: Text.AlignHCenter
                        color: Theme.textFaint
                        text: qsTr("Für diesen Dateityp ist keine Symbolübersicht verfügbar.")
                    }
                }
            }
        }

        // ---- search in the document (Ctrl+F)
        Rectangle {
            id: findBar
            property bool found: true
            Layout.fillWidth: true
            Layout.preferredHeight: visible ? 34 : 0
            visible: false
            color: Theme.header
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 8
                Label { text: qsTr("Suchen:"); color: Theme.textMuted }
                TextField {
                    id: findField
                    Layout.preferredWidth: 260
                    color: findBar.found ? Theme.text : Theme.error
                    onTextChanged: root.find(text, false)
                    Keys.onReturnPressed: (event) => root.find(text, event.modifiers & Qt.ShiftModifier)
                    Keys.onEscapePressed: { findBar.visible = false; area.forceActiveFocus() }
                }
                IconButton { glyph: "↑"; tip: qsTr("Vorheriger Treffer (Umschalt+Enter)"); onClicked: root.find(findField.text, true) }
                IconButton { glyph: "↓"; tip: qsTr("Nächster Treffer (Enter)"); onClicked: root.find(findField.text, false) }
                Item { Layout.fillWidth: true }
                IconButton { iconName: "close"; tip: qsTr("Schließen (Esc)"); onClicked: { findBar.visible = false; area.forceActiveFocus() } }
            }
        }

        // ---- status line
        ToolStrip {
            Layout.preferredHeight: 22
            leftMargin: 10
            rightMargin: 10
            spacing: 14
            line: "none"
            Label {
                text: doc.saving ? qsTr("Speichert …")
                    : doc.conflict ? qsTr("Konflikt")
                    : root.modified ? qsTr("● Ungespeichert")
                    : root.readOnly ? qsTr("Nur lesen") : qsTr("Gespeichert")
                color: root.modified || doc.conflict ? Theme.warn : Theme.textFaint
                font.pixelSize: Theme.smallFontSize
            }
            Item { Layout.fillWidth: true }
            Label {
                visible: !root.wrap
                text: qsTr("Zeile %1").arg(area.cursorLine)
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
            }
            Label {
                text: root.large ? qsTr("Ohne Hervorhebung (große Datei)") : Repository.definitionForFileName(root.source).translatedName
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
            }
            IconButton {
                visible: !root.wrap
                implicitHeight: 20
                iconName: "list"
                tip: qsTr("Symbolübersicht")
                checkable: true
                checked: root.showSymbols
                onClicked: root.showSymbols = !root.showSymbols
            }
            IconButton {
                visible: !!root.actions
                implicitHeight: 20
                iconName: "external"
                tip: qsTr("Im Editor öffnen")
                onClicked: { root.save(); root.actions.openInEditor(root.source, Math.max(1, area.cursorLine)) }
            }
        }
    }

    // ---- go to line (Ctrl+G)
    Popup {
        id: gotoBar
        x: (root.width - width) / 2
        y: 8
        padding: 8
        onOpened: { gotoField.text = ""; gotoField.forceActiveFocus() }
        background: Rectangle { color: Theme.header; border.color: Theme.border; radius: 6 }
        RowLayout {
            Label { text: qsTr("Gehe zu Zeile:"); color: Theme.textMuted }
            TextField {
                id: gotoField
                Layout.preferredWidth: 100
                validator: IntValidator { bottom: 1 }
                onAccepted: { root.reveal(parseInt(text)); gotoBar.close(); area.forceActiveFocus() }
            }
        }
    }

    // ---- save as new file (conflict resolution)
    Dialog {
        id: saveAsDialog
        function ask() {
            const name = root.app.fileName(root.source)
            const dot = name.lastIndexOf(".")
            nameField.text = dot > 0 ? name.slice(0, dot) + qsTr(" (lokal)") + name.slice(dot) : name + qsTr(" (lokal)")
            open()
            nameField.forceActiveFocus()
        }
        anchors.centerIn: parent
        modal: true
        title: qsTr("Als neue Datei speichern")
        standardButtons: Dialog.Save | Dialog.Cancel
        ColumnLayout {
            Label { text: qsTr("Neuer Name im selben Ordner:"); color: Theme.textMuted }
            TextField { id: nameField; Layout.preferredWidth: 320; onAccepted: saveAsDialog.accept() }
        }
        onAccepted: {
            const dir = root.source.slice(0, root.source.lastIndexOf("/"))
            const text = area.text
            if (doc.saveAs(dir + "/" + nameField.text, text)) {
                root.savedGeneration = root.editGeneration
                root.app.noteChanged(dir + "/" + nameField.text)
                root.savedAs(dir + "/" + nameField.text)
            }
        }
    }
}
