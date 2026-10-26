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
    readonly property bool modified: editGeneration !== savedGeneration
    readonly property bool readOnly: doc.readOnly !== ""
    readonly property alias document: doc
    /// First visible line (1-based), for scroll sync and session state.
    readonly property int topLine: wrap ? lineAtY(flick.contentY) : Math.floor(flick.contentY / lineHeight) + 1
    readonly property real lineHeight: area.lineCount > 0 ? area.contentHeight / area.lineCount : 16
    readonly property bool large: area.length > 2 * 1024 * 1024

    /// FileActions (for "open in editor"); optional.
    property var actions: null
    /// Suggest project files after `[[` (Markdown).
    property bool wikiCompletion: false

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
        root.lastText = doc.text()
        area.text = root.lastText
        loading = false
        editGeneration = 0
        savedGeneration = 0
        area.cursorPosition = Math.min(cursor, area.length)
        flick.contentY = Math.min(top, Math.max(0, flick.contentHeight - flick.height))
        if (root.line > 0) Qt.callLater(root.reveal, root.line)
        if (root.showSymbols) root.updateSymbols()
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
                    color: "#262626"
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
                    }
                    onCursorPositionChanged: if (root.wikiCompletion) completionTimer.restart()
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
                                    Text { text: modelData.kind === "tag" ? "#" : modelData.kind === "note" ? "📝" : "📄"; font.pixelSize: 12; color: "#9dd1ff" }
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
                definition: root.large ? "None" : Repository.definitionForFileName(root.source).name
                theme: Repository.theme("Breeze Dark")
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
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 22
            color: Theme.header
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 10
                anchors.rightMargin: 10
                spacing: 14
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
