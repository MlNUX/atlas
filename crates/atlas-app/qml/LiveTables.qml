// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import Atlas

// Markdown tables drawn as tables over their text in the editor, like
// Obsidian's Live Preview: lines, a bold header, the columns' alignment,
// **bold**, *italic*, `code` and links in the cells. Each drawn table
// covers exactly the lines of its text, so nothing moves. Where the cursor
// is, the text shows (to edit it); a click into a cell puts the cursor
// into that cell's text. Tables from atlas_core::markdown_edit::drawn_tables.
// Place it inside the TextEdit.
Item {
    id: live
    /// The TextEdit.
    required property var area
    /// Tables of the text (`drawnTables`).
    property var tables: []
    /// Geometry of each table, in the TextEdit's coordinates.
    property var layouts: []
    anchors.fill: parent
    z: 5

    FontMetrics { id: metrics; font.pixelSize: 13 }
    FontMetrics { id: boldMetrics; font.pixelSize: 13; font.bold: true }

    function plain(markdown) {
        return markdown.replace(/\\\|/g, "|").replace(/!\[([^\]]*)\]\([^)]*\)/g, "▣ $1").replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
            .replace(/\[\[([^\]|]+\|)?([^\]]+)\]\]/g, "$2").replace(/(\*\*|__|~~|`)/g, "").replace(/<br\s*\/?>/gi, " ")
    }
    function htmlEscape(text) { return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;") }
    /// The inline Markdown of a cell as styled text.
    function rich(markdown) {
        const codes = []
        let s = markdown.replace(/\\\|/g, "|").replace(/<br\s*\/?>/gi, " ").replace(/`([^`]+)`/g, (m, c) => { codes.push(c); return "\u0000" + (codes.length - 1) + "\u0000" })
        s = htmlEscape(s)
        s = s.replace(/!\[([^\]]*)\]\([^)]*\)/g, "▣ $1")
            .replace(/\[\[([^\]|]+\|)?([^\]]+)\]\]/g, "<font color='" + Theme.link + "'>$2</font>")
            .replace(/\[([^\]]+)\]\([^)]*\)/g, "<font color='" + Theme.link + "'><u>$1</u></font>")
            .replace(/(\*\*|__)(?=\S)(.*?\S)\1/g, "<b>$2</b>")
            .replace(/(^|[^\w*])\*(?=\S)([^*]+?)\*(?!\w)/g, "$1<i>$2</i>")
            .replace(/(^|\W)_(?=\S)([^_]+?)_(?!\w)/g, "$1<i>$2</i>")
            .replace(/~~(.+?)~~/g, "<s>$1</s>")
        s = s.replace(/\u0000(\d+)\u0000/g, (m, i) => "<font color='" + (Theme.light ? "#a3305a" : "#f08bb4") + "'>" + htmlEscape(codes[Number(i)]) + "</font>")
        return s
    }

    /// Where the rows and columns of every table are (after edits, resizes).
    function relayout() {
        const out = []
        const width = Math.max(80, area.width - area.leftPadding - area.rightPadding)
        for (const t of tables) {
            const rect = (pos) => area.positionToRectangle(Math.max(0, Math.min(pos, area.length)))
            const top = rect(t.header.start).y
            const lines = [t.header].concat(t.rows)
            const last = rect(t.end)
            const bottom = last.y + last.height
            // The rows share the height of all lines (with the delimiter
            // line), so they are a bit taller than a line of text.
            const rowHeight = (bottom - top) / lines.length
            const rows = lines.map((r, i) => ({ y: Math.round(i * rowHeight), h: Math.round((i + 1) * rowHeight) - Math.round(i * rowHeight),
                                                cells: r.cells, header: i === 0 }))
            // Columns as wide as their longest text; narrower if the table is too wide.
            const widths = []
            for (let c = 0; c < t.columns; ++c) {
                let w = 0
                for (const [i, r] of lines.entries()) {
                    const cell = r.cells[c]
                    if (cell) w = Math.max(w, (i === 0 ? boldMetrics : metrics).advanceWidth(plain(cell.text)))
                }
                widths.push(Math.ceil(w) + 22)
            }
            const total = widths.reduce((a, b) => a + b, 0)
            if (total > width) {
                const scale = width / total
                for (let c = 0; c < widths.length; ++c) widths[c] = Math.max(40, Math.floor(widths[c] * scale))
            }
            const xs = [0]
            for (const w of widths) xs.push(xs[xs.length - 1] + w)
            out.push({ table: t, y: top, height: bottom - top, rows: rows, xs: xs })
        }
        layouts = out
    }
    onTablesChanged: relayout()

    /// Where the cursor goes for a click at (`x`, `y`) into drawn table
    /// `layout` (`x` from the table's left edge): the end of that cell's text.
    function cursorFor(layout, x, y) {
        const rows = layout.rows
        let r = rows.length - 1
        while (r > 0 && rows[r].y > y) r--
        const xs = layout.xs
        let c = 0
        while (c + 1 < xs.length - 1 && xs[c + 1] <= x) c++
        const cells = rows[r].cells
        const cell = cells[Math.min(c, cells.length - 1)]
        return cell ? cell.end : (r === 0 ? layout.table.header.end : layout.table.rows[r - 1].end)
    }
    Connections {
        target: live.area
        function onWidthChanged() { Qt.callLater(live.relayout) }
    }

    Repeater {
        model: live.layouts
        Item {
            id: drawn
            required property var modelData
            readonly property var table: modelData.table
            readonly property real tableWidth: modelData.xs[modelData.xs.length - 1]
            // The text shows where the cursor (or a selection) is.
            readonly property bool editing: live.area.activeFocus
                && ((live.area.cursorPosition >= table.start && live.area.cursorPosition <= table.end)
                    || (live.area.selectionStart < live.area.selectionEnd
                        && live.area.selectionStart <= table.end && live.area.selectionEnd >= table.start))
            visible: !editing
            x: 0
            y: modelData.y
            width: live.width
            height: modelData.height

            // Covers the text of the table, also right of the drawn columns.
            Rectangle { anchors.fill: parent; color: Theme.editor }
            Rectangle {
                id: frame
                x: live.area.leftPadding
                width: drawn.tableWidth
                height: parent.height
                radius: 4
                color: "transparent"
                border.color: Theme.border
                clip: true
                Repeater {
                    model: drawn.modelData.rows
                    Item {
                        id: rowItem
                        required property var modelData
                        required property int index
                        y: modelData.y
                        width: frame.width
                        height: modelData.h
                        Rectangle {
                            anchors.fill: parent
                            color: rowItem.modelData.header ? Theme.header : rowItem.index % 2 === 0 ? Theme.raised : "transparent"
                            opacity: rowItem.modelData.header ? 1 : 0.45
                        }
                        Rectangle {
                            visible: rowItem.index > 0
                            width: parent.width
                            height: 1
                            color: Theme.border
                        }
                        Repeater {
                            model: drawn.table.columns
                            Item {
                                required property int index
                                readonly property var cell: rowItem.modelData.cells[index]
                                readonly property string align: drawn.table.align[index] || ""
                                x: drawn.modelData.xs[index]
                                width: drawn.modelData.xs[index + 1] - x
                                height: rowItem.height
                                Rectangle {
                                    visible: index > 0
                                    width: 1
                                    height: parent.height
                                    color: Theme.border
                                }
                                Text {
                                    anchors.fill: parent
                                    anchors.leftMargin: 10
                                    anchors.rightMargin: 10
                                    verticalAlignment: Text.AlignVCenter
                                    horizontalAlignment: parent.align === "r" ? Text.AlignRight : parent.align === "c" ? Text.AlignHCenter : Text.AlignLeft
                                    text: parent.cell ? live.rich(parent.cell.text) : ""
                                    textFormat: Text.StyledText
                                    elide: Text.ElideRight
                                    maximumLineCount: 1
                                    color: rowItem.modelData.header ? Theme.textStrong : Theme.text
                                    font.pixelSize: 13
                                    font.bold: rowItem.modelData.header
                                }
                            }
                        }
                    }
                }
            }
            // A click puts the cursor at the end of the cell's text (the text then shows).
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.IBeamCursor
                acceptedButtons: Qt.LeftButton
                onPressed: (mouse) => {
                    const pos = live.cursorFor(drawn.modelData, mouse.x - frame.x, mouse.y)
                    live.area.forceActiveFocus()
                    live.area.cursorPosition = pos
                }
            }
        }
    }
}
