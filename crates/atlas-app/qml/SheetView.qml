// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// A spreadsheet (xlsx, xlsm, xlsb, xls, ods), read only: the sheets as tabs
// below, column letters and row numbers, the cells as Excel shows them
// (number formats, dates, bold, colours, fill, alignment). Above, the
// selected cell with its formula; Ctrl+F searches the sheet, Ctrl+C copies
// the selection as tab-separated text. Read by atlas-core (spreadsheet.rs).
Rectangle {
    id: root
    required property string source
    property var app: null
    property var actions: null
    color: Theme.editor
    /// A text table (CSV …) shown as a table: it can go back to the text.
    readonly property bool delimited: /\.(csv|tsv|tab)$/i.test(source)
    signal showSourceRequested()

    // Saved elsewhere (Excel, LibreOffice, the editor): read again.
    property real modified: 0
    function modifiedTime() {
        try { return JSON.parse(root.actions.ops.properties(root.source)).modified || 0 } catch (e) { return 0 }
    }
    Component.onCompleted: modified = modifiedTime()
    Timer {
        interval: 1000
        repeat: true
        running: root.visible && !!root.actions
        onTriggered: {
            const m = root.modifiedTime()
            if (m && m !== root.modified) {
                root.modified = m
                sheetModel.reload()
            }
        }
    }

    /// The selected cell and the other corner of the selection.
    property int currentRow: 0
    property int currentColumn: 0
    property int anchorRow: 0
    property int anchorColumn: 0
    readonly property int selTop: Math.min(currentRow, anchorRow)
    readonly property int selBottom: Math.max(currentRow, anchorRow)
    readonly property int selLeft: Math.min(currentColumn, anchorColumn)
    readonly property int selRight: Math.max(currentColumn, anchorColumn)
    /// A cell to show once the sheet is read (from the search or the session).
    property var pendingCell: null
    readonly property alias model: sheetModel
    readonly property var sheetList: { try { return JSON.parse(sheetModel.sheets || "[]") } catch (e) { return [] } }
    /// Column widths in characters (from the texts).
    property var widths: []
    readonly property real charWidth: metrics.averageCharacterWidth

    SheetModel {
        id: sheetModel
        source: root.source
        onRevisionChanged: {
            try { root.widths = JSON.parse(sheetModel.columnWidths()) } catch (e) { root.widths = [] }
            table.forceLayout()
            if (root.pendingCell) {
                const c = root.pendingCell
                root.pendingCell = null
                root.select(c[0], c[1], false)
            } else if (root.currentRow >= sheetModel.rows || root.currentColumn >= sheetModel.columns) {
                root.select(0, 0, false)
            }
        }
    }
    FontMetrics { id: metrics; font.pixelSize: Theme.fontSize }

    function applyState(state) {
        if (!state) return
        if (state.cell) pendingCell = state.cell
        if (state.sheet !== undefined && state.sheet !== sheetModel.sheet) sheetModel.sheet = state.sheet
        else if (state.cell && sheetModel.revision > 0 && !sheetModel.loading) {
            pendingCell = null
            select(state.cell[0], state.cell[1], false)
        }
    }
    function viewState() {
        const s = { sheet: sheetModel.sheet, cell: [currentRow, currentColumn] }
        if (delimited) s.processor = "atlas:table"
        return s
    }
    function focusContent() { table.forceActiveFocus() }

    function select(row, column, extend) {
        row = Math.max(0, Math.min(row, sheetModel.rows - 1))
        column = Math.max(0, Math.min(column, sheetModel.columns - 1))
        currentRow = row
        currentColumn = column
        if (!extend) { anchorRow = row; anchorColumn = column }
        if (sheetModel.rows > 0 && sheetModel.columns > 0) table.positionViewAtCell(Qt.point(column, row), TableView.Contain)
    }
    function copySelection() {
        if (!root.app) return
        root.app.copyToClipboard(sheetModel.copyText(selTop, selLeft, selBottom, selRight))
    }
    function findNext(backwards) {
        const text = findField.text
        if (!text) { findInfo.text = ""; return }
        let hit = {}
        try { hit = JSON.parse(sheetModel.find(text, currentRow, currentColumn, backwards)) } catch (e) {}
        if (hit.row === undefined) { findInfo.text = qsTr("Nicht gefunden"); return }
        select(hit.row, hit.column, false)
        findInfo.text = hit.count === 1 ? qsTr("1 Treffer") : qsTr("%1 Treffer").arg(hit.count)
    }
    function columnWidth(column) {
        const chars = column < widths.length ? widths[column] : 8
        return Math.round(Math.max(56, chars * charWidth + 18))
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        ToolStrip {
            implicitHeight: 36
            leftMargin: 12
            spacing: 8
            line: "none"
            Icon { name: "file-table"; color: Theme.kindColors.csv }
            // The selected cell and what is in it (the formula behind a value).
            Label {
                text: sheetModel.cellName(root.currentRow, root.currentColumn)
                color: Theme.textStrong
                font.family: Theme.monoFont
                font.pixelSize: Theme.smallFontSize
                Layout.minimumWidth: 44
            }
            Label {
                id: cellInfo
                readonly property string formula: sheetModel.revision >= 0 ? sheetModel.formulaAt(root.currentRow, root.currentColumn) : ""
                readonly property string value: sheetModel.revision >= 0 ? sheetModel.textAt(root.currentRow, root.currentColumn) : ""
                Layout.fillWidth: true
                text: formula ? "ƒx  =" + formula + "   →  " + value : value
                color: formula ? Theme.text : Theme.textMuted
                font.family: formula ? Theme.monoFont : metrics.font.family
                font.pixelSize: Theme.smallFontSize
                elide: Text.ElideRight
            }
            TextField {
                id: findField
                objectName: "sheetFind"
                Layout.preferredWidth: 180
                placeholderText: qsTr("Im Blatt suchen (Strg+F)")
                font.pixelSize: Theme.smallFontSize
                onAccepted: root.findNext(false)
                onTextChanged: findInfo.text = ""
                Keys.onPressed: (event) => {
                    if (event.key === Qt.Key_Escape) { text = ""; table.forceActiveFocus(); event.accepted = true }
                    else if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && (event.modifiers & Qt.ShiftModifier)) { root.findNext(true); event.accepted = true }
                }
            }
            Label { id: findInfo; color: Theme.textMuted; font.pixelSize: Theme.smallFontSize }
            Label {
                visible: sheetModel.info.length > 0
                text: sheetModel.info
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
            }
            Label {
                text: qsTr("Nur Ansicht")
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
            }
            IconButton {
                visible: root.delimited
                iconName: "file-text"
                tip: qsTr("Als Text anzeigen (zum Bearbeiten)")
                onClicked: root.showSourceRequested()
            }
            IconButton { iconName: "refresh"; tip: qsTr("Neu laden"); onClicked: sheetModel.reload() }
        }
        Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: Theme.border }

        Label {
            visible: sheetModel.error.length > 0
            Layout.fillWidth: true
            Layout.margins: 16
            wrapMode: Text.Wrap
            color: Theme.error
            text: qsTr("Die Tabelle kann nicht angezeigt werden: %1").arg(sheetModel.error)
        }
        Label {
            visible: sheetModel.truncated
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.topMargin: 4
            color: Theme.warn
            font.pixelSize: Theme.smallFontSize
            text: qsTr("Sehr großes Blatt: Es werden nur die ersten %1 Zeilen gezeigt.").arg(sheetModel.rows.toLocaleString(Qt.locale("de_DE"), "f", 0))
        }

        GridLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: sheetModel.error.length === 0
            columns: 2
            rowSpacing: 0
            columnSpacing: 0

            Rectangle { implicitWidth: rowHeader.width; implicitHeight: columnHeader.height; color: Theme.header }
            HorizontalHeaderView {
                id: columnHeader
                Layout.fillWidth: true
                syncView: table
                clip: true
                delegate: Rectangle {
                    required property int column
                    implicitHeight: 22
                    implicitWidth: 60
                    readonly property bool marked: column >= root.selLeft && column <= root.selRight
                    color: marked ? Theme.selection : Theme.header
                    Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: Theme.border }
                    Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
                    Text {
                        anchors.centerIn: parent
                        text: sheetModel.columnName(parent.column)
                        color: parent.marked ? Theme.accent : Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                        font.bold: parent.marked
                    }
                    MouseArea {
                        anchors.fill: parent
                        // The whole column (Shift: up to it).
                        onClicked: (mouse) => {
                            if (!(mouse.modifiers & Qt.ShiftModifier)) root.anchorColumn = parent.column
                            root.anchorRow = 0
                            root.currentColumn = parent.column
                            root.currentRow = sheetModel.rows - 1
                            table.forceActiveFocus()
                        }
                    }
                }
            }
            VerticalHeaderView {
                id: rowHeader
                Layout.fillHeight: true
                syncView: table
                clip: true
                delegate: Rectangle {
                    required property int row
                    implicitWidth: Math.max(40, metrics.averageCharacterWidth * String(sheetModel.rows).length + 16)
                    implicitHeight: 24
                    readonly property bool marked: row >= root.selTop && row <= root.selBottom
                    color: marked ? Theme.selection : Theme.header
                    Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: Theme.border }
                    Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
                    Text {
                        anchors.centerIn: parent
                        text: parent.row + 1
                        color: parent.marked ? Theme.accent : Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                        font.bold: parent.marked
                    }
                    MouseArea {
                        anchors.fill: parent
                        // The whole row (Shift: up to it).
                        onClicked: (mouse) => {
                            if (!(mouse.modifiers & Qt.ShiftModifier)) root.anchorRow = parent.row
                            root.anchorColumn = 0
                            root.currentRow = parent.row
                            root.currentColumn = sheetModel.columns - 1
                            table.forceActiveFocus()
                        }
                    }
                }
            }
            TableView {
                id: table
                objectName: "sheetTable"
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: sheetModel
                boundsBehavior: Flickable.StopAtBounds
                columnWidthProvider: (column) => root.columnWidth(column)
                rowHeightProvider: () => 24
                focus: true
                ScrollBar.vertical: ScrollBar {}
                ScrollBar.horizontal: ScrollBar {}

                delegate: Rectangle {
                    id: cell
                    required property int row
                    required property int column
                    required property string display
                    required property string kind
                    required property bool bold
                    required property bool italic
                    required property bool underline
                    required property bool strike
                    required property string textColor
                    required property string textColorDark
                    required property string fill
                    required property int align
                    required property bool hasFormula
                    readonly property bool selected: row >= root.selTop && row <= root.selBottom && column >= root.selLeft && column <= root.selRight
                    readonly property bool current: row === root.currentRow && column === root.currentColumn
                    color: fill || Theme.editor
                    Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: Theme.border; opacity: 0.7 }
                    Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border; opacity: 0.7 }
                    Rectangle {
                        anchors.fill: parent
                        visible: cell.selected && !cell.current
                        color: Theme.accent
                        opacity: 0.16
                    }
                    Text {
                        anchors.fill: parent
                        anchors.leftMargin: 5
                        anchors.rightMargin: 6
                        verticalAlignment: Text.AlignVCenter
                        horizontalAlignment: cell.align === 2 ? Text.AlignRight : cell.align === 1 ? Text.AlignHCenter : Text.AlignLeft
                        text: cell.display
                        elide: cell.align === 2 ? Text.ElideLeft : Text.ElideRight
                        maximumLineCount: 1
                        textFormat: Text.PlainText
                        font.pixelSize: Theme.fontSize
                        font.bold: cell.bold
                        font.italic: cell.italic
                        font.underline: cell.underline
                        font.strikeout: cell.strike
                        // On a fill the colours are Excel's (dark text unless set);
                        // without one, set colours are adjusted to the design.
                        color: cell.fill ? (cell.textColor || "#000000")
                             : cell.kind === "error" ? Theme.error
                             : Theme.light ? (cell.textColor || Theme.text) : (cell.textColorDark || Theme.text)
                    }
                    Rectangle {
                        anchors.fill: parent
                        visible: cell.current
                        color: "transparent"
                        border.color: Theme.accent
                        border.width: 2
                    }
                    MouseArea {
                        anchors.fill: parent
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        onPressed: (mouse) => {
                            table.forceActiveFocus()
                            if (mouse.button === Qt.RightButton && cell.selected) return
                            root.select(cell.row, cell.column, (mouse.modifiers & Qt.ShiftModifier) !== 0)
                        }
                        onPositionChanged: (mouse) => {
                            if (!pressed) return
                            const p = mapToItem(table.contentItem, mouse.x, mouse.y)
                            const at = table.cellAtPosition(p.x, p.y, true)
                            if (at.x >= 0 && at.y >= 0) { root.currentRow = at.y; root.currentColumn = at.x }
                        }
                        onClicked: (mouse) => { if (mouse.button === Qt.RightButton) cellMenu.popup() }
                    }
                }

                Keys.onPressed: (event) => {
                    const shift = (event.modifiers & Qt.ShiftModifier) !== 0
                    const ctrl = (event.modifiers & Qt.ControlModifier) !== 0
                    const page = Math.max(1, Math.floor(table.height / 24) - 1)
                    let r = root.currentRow, c = root.currentColumn
                    switch (event.key) {
                    case Qt.Key_Up: r = ctrl ? 0 : r - 1; break
                    case Qt.Key_Down: r = ctrl ? sheetModel.rows - 1 : r + 1; break
                    case Qt.Key_Left: c = ctrl ? 0 : c - 1; break
                    case Qt.Key_Right: c = ctrl ? sheetModel.columns - 1 : c + 1; break
                    case Qt.Key_PageUp: r -= page; break
                    case Qt.Key_PageDown: r += page; break
                    case Qt.Key_Home: c = 0; if (ctrl) r = 0; break
                    case Qt.Key_End: c = sheetModel.columns - 1; if (ctrl) r = sheetModel.rows - 1; break
                    case Qt.Key_Tab: c += 1; break
                    case Qt.Key_Backtab: c -= 1; break
                    case Qt.Key_Return: case Qt.Key_Enter: r += shift ? -1 : 1; break
                    case Qt.Key_C:
                        if (!ctrl) return
                        root.copySelection(); event.accepted = true; return
                    case Qt.Key_A:
                        if (!ctrl) return
                        root.anchorRow = 0; root.anchorColumn = 0
                        root.currentRow = sheetModel.rows - 1; root.currentColumn = sheetModel.columns - 1
                        event.accepted = true; return
                    case Qt.Key_F:
                        if (!ctrl) return
                        findField.forceActiveFocus(); findField.selectAll(); event.accepted = true; return
                    case Qt.Key_F3:
                        root.findNext(shift); event.accepted = true; return
                    default: return
                    }
                    const extend = shift && event.key !== Qt.Key_Tab && event.key !== Qt.Key_Backtab
                        && event.key !== Qt.Key_Return && event.key !== Qt.Key_Enter
                    root.select(r, c, extend)
                    event.accepted = true
                }
            }
        }

        // The sheets, as in Excel.
        Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: Theme.border }
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 30
            color: Theme.header
            ListView {
                id: sheetTabs
                objectName: "sheetTabs"
                anchors.fill: parent
                anchors.leftMargin: 6
                orientation: ListView.Horizontal
                spacing: 2
                clip: true
                model: root.sheetList
                boundsBehavior: Flickable.StopAtBounds
                delegate: Rectangle {
                    required property var modelData
                    required property int index
                    readonly property bool active: index === sheetModel.sheet
                    height: sheetTabs.height
                    width: tabLabel.implicitWidth + 24
                    color: active ? Theme.editor : tabArea.containsMouse ? Theme.hover : "transparent"
                    Rectangle { visible: parent.active; width: parent.width; height: 2; color: Theme.kindColors.csv }
                    Text {
                        id: tabLabel
                        anchors.centerIn: parent
                        text: parent.modelData.name + (parent.modelData.hidden ? qsTr(" (ausgeblendet)") : "")
                        color: parent.active ? Theme.textStrong : parent.modelData.hidden ? Theme.textFaint : Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                        font.italic: parent.modelData.hidden
                        font.bold: parent.active
                    }
                    MouseArea {
                        id: tabArea
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: { sheetModel.sheet = parent.index; table.forceActiveFocus() }
                    }
                }
            }
            Label {
                anchors.right: parent.right
                anchors.rightMargin: 10
                anchors.verticalCenter: parent.verticalCenter
                visible: sheetModel.loading
                text: qsTr("Wird gelesen …")
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
            }
        }
    }

    Menu {
        id: cellMenu
        MenuItem { text: qsTr("Kopieren (Strg+C)"); onTriggered: root.copySelection() }
        MenuItem {
            text: qsTr("Zelladresse kopieren")
            onTriggered: if (root.app) root.app.copyToClipboard(sheetModel.cellName(root.currentRow, root.currentColumn))
        }
        MenuItem {
            text: qsTr("Formel kopieren")
            enabled: cellInfo.formula.length > 0
            onTriggered: if (root.app) root.app.copyToClipboard("=" + cellInfo.formula)
        }
    }
}
