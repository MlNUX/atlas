// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Trash: original location, deletion date; restore or delete for good.
Rectangle {
    id: root
    required property var actions

    color: Theme.editor

    TrashModel { id: trash }
    Connections {
        target: root.actions.ops
        function onFinished() { trash.reload() }
        function onTrashed() { trash.reload() }
    }

    property var selected: ({})
    property int anchorRow: -1
    readonly property var selectedRows: Object.keys(selected).map(Number).sort((a, b) => a - b)

    function ids(rows) { return rows.map(r => trash.idAt(r)) }
    function selectOnly(row) { selected = ({ [row]: true }); anchorRow = row }
    function rowsOrCurrent() { return selectedRows.length ? selectedRows : (list.currentIndex >= 0 ? [list.currentIndex] : []) }
    function restoreSelected() { root.actions.restore(ids(rowsOrCurrent())); selected = ({}) }
    function purgeSelected() {
        const rows = rowsOrCurrent()
        if (rows.length === 0) return
        root.actions.purge(ids(rows), rows.map(r => list.itemAtIndex(r) ? list.itemAtIndex(r).name : ""))
        selected = ({})
    }
    function emptyTrash() {
        const rows = []
        for (let i = 0; i < trash.count; ++i) rows.push(i)
        if (rows.length) root.actions.purge(ids(rows), [])
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 34
            color: Theme.header
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 10
                anchors.rightMargin: 8
                spacing: 6
                Label {
                    Layout.fillWidth: true
                    text: trash.error ? trash.error : qsTr("Papierkorb · %n Element(e)", "", trash.count)
                    color: trash.error ? Theme.error : Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                }
                Button {
                    text: qsTr("Wiederherstellen")
                    enabled: root.rowsOrCurrent().length > 0
                    onClicked: root.restoreSelected()
                }
                Button {
                    text: qsTr("Endgültig löschen")
                    enabled: root.rowsOrCurrent().length > 0
                    onClicked: root.purgeSelected()
                }
                Button {
                    text: qsTr("Papierkorb leeren")
                    enabled: trash.count > 0
                    onClicked: root.emptyTrash()
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 24
            color: Theme.editor
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
            Row {
                anchors.fill: parent
                anchors.leftMargin: 10
                Repeater {
                    model: [qsTr("Name"), qsTr("Ursprünglicher Ort"), qsTr("Gelöscht am"), qsTr("Größe")]
                    delegate: Text {
                        required property string modelData
                        required property int index
                        width: [list.width * 0.3, list.width * 0.7 - 250, 150, 90][index]
                        anchors.verticalCenter: parent.verticalCenter
                        text: modelData
                        color: Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                    }
                }
            }
        }

        ListView {
            id: list
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: trash
            focus: true
            boundsBehavior: Flickable.StopAtBounds
            highlightMoveDuration: 0
            ScrollBar.vertical: ScrollBar {}
            onCountChanged: root.selected = ({})

            Keys.onDeletePressed: root.purgeSelected()

            delegate: Rectangle {
                id: row
                required property int index
                required property string name
                required property string originalDir
                required property real deleted
                required property bool isDir
                required property string sizeText
                width: ListView.view.width
                height: Theme.rowHeight
                color: root.selected[index] ? Theme.activeSelection : hover.hovered ? Theme.hover : "transparent"
                HoverHandler { id: hover }
                Row {
                    anchors.fill: parent
                    anchors.leftMargin: 10
                    Text {
                        width: list.width * 0.3
                        anchors.verticalCenter: parent.verticalCenter
                        text: (row.isDir ? "📁  " : "📄  ") + row.name
                        color: Theme.text
                        font.pixelSize: Theme.fontSize
                        elide: Text.ElideRight
                    }
                    Text {
                        width: list.width * 0.7 - 250
                        anchors.verticalCenter: parent.verticalCenter
                        text: row.originalDir
                        color: Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                        elide: Text.ElideMiddle
                    }
                    Text {
                        width: 150
                        anchors.verticalCenter: parent.verticalCenter
                        text: Qt.formatDateTime(new Date(row.deleted), "dd.MM.yyyy  HH:mm")
                        color: Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                    }
                    Text {
                        width: 90
                        anchors.verticalCenter: parent.verticalCenter
                        text: row.sizeText
                        color: Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                    }
                }
                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    onClicked: (mouse) => {
                        list.currentIndex = row.index
                        list.forceActiveFocus()
                        if (mouse.modifiers & Qt.ControlModifier) {
                            const s = Object.assign({}, root.selected)
                            if (s[row.index]) delete s[row.index]; else s[row.index] = true
                            root.selected = s
                        } else if ((mouse.modifiers & Qt.ShiftModifier) && root.anchorRow >= 0) {
                            const s = {}
                            const [a, b] = [Math.min(root.anchorRow, row.index), Math.max(root.anchorRow, row.index)]
                            for (let i = a; i <= b; ++i) s[i] = true
                            root.selected = s
                        } else if (!(mouse.button === Qt.RightButton && root.selected[row.index])) {
                            root.selectOnly(row.index)
                        }
                        if (mouse.button === Qt.RightButton) trashMenu.popup()
                    }
                }
            }

            Label {
                anchors.centerIn: parent
                visible: trash.count === 0 && !trash.error
                text: qsTr("Der Papierkorb ist leer.")
                color: Theme.textFaint
            }
        }
    }

    Menu {
        id: trashMenu
        MenuItem { text: qsTr("Wiederherstellen"); onTriggered: root.restoreSelected() }
        MenuItem { text: qsTr("Endgültig löschen …"); onTriggered: root.purgeSelected() }
    }
}
