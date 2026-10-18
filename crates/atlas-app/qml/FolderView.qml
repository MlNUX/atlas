// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// File list of one folder (details view) with sortable columns.
Rectangle {
    id: root
    required property string source
    property alias showHidden: folder.showHidden

    /// A row was opened: folders navigate, files open a viewer.
    signal activated(string path, bool isDir, bool newPane)

    color: Theme.editor

    function viewState() {
        return { sortKey: folder.sortKey, sortAscending: folder.sortAscending, showHidden: folder.showHidden }
    }
    function applyState(state) {
        if (state.sortKey) folder.sortKey = state.sortKey
        if (state.sortAscending !== undefined) folder.sortAscending = state.sortAscending
        if (state.showHidden !== undefined) folder.showHidden = state.showHidden
    }

    FolderModel {
        id: folder
        path: root.source
    }

    function parentOf(path) {
        const i = path.lastIndexOf("/")
        return i > 0 ? path.slice(0, i) : "/"
    }

    function sortBy(key) {
        if (folder.sortKey === key) folder.sortAscending = !folder.sortAscending
        else { folder.sortKey = key; folder.sortAscending = true }
    }

    function openRow(row, newPane) {
        if (row < 0 || row >= list.count) return
        root.activated(folder.pathAt(row), folder.isDirAt(row), newPane)
    }

    readonly property int sizeWidth: 90
    readonly property int dateWidth: 140
    readonly property int kindWidth: 150

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 30
            color: Theme.header
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 6
                anchors.rightMargin: 8
                spacing: 4
                IconButton {
                    glyph: "↑"
                    tip: qsTr("Übergeordneter Ordner (Alt+↑)")
                    enabled: root.source !== "/"
                    onClicked: root.activated(root.parentOf(root.source), true, false)
                }
                Label {
                    Layout.fillWidth: true
                    text: root.source
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideMiddle
                }
                Label {
                    text: folder.error ? folder.error : qsTr("%n Einträge", "", folder.count)
                    color: folder.error ? Theme.error : Theme.textFaint
                    font.pixelSize: Theme.smallFontSize
                }
                IconButton {
                    glyph: "◌"
                    tip: qsTr("Versteckte Dateien anzeigen")
                    checkable: true
                    checked: folder.showHidden
                    onToggled: folder.showHidden = checked
                }
            }
        }

        // Column headers.
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 24
            color: Theme.editor
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
            Row {
                anchors.fill: parent
                anchors.leftMargin: 10
                Repeater {
                    model: [
                        { key: "name", label: qsTr("Name"), width: -1 },
                        { key: "size", label: qsTr("Größe"), width: root.sizeWidth },
                        { key: "modified", label: qsTr("Geändert"), width: root.dateWidth },
                        { key: "kind", label: qsTr("Typ"), width: root.kindWidth }
                    ]
                    delegate: Item {
                        id: column
                        required property var modelData
                        width: modelData.width > 0 ? modelData.width
                            : parent.width - root.sizeWidth - root.dateWidth - root.kindWidth - 10
                        height: parent.height
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: column.modelData.label
                                + (folder.sortKey === column.modelData.key ? (folder.sortAscending ? "  ▴" : "  ▾") : "")
                            color: folder.sortKey === column.modelData.key ? Theme.textStrong : Theme.textMuted
                            font.pixelSize: Theme.smallFontSize
                        }
                        MouseArea { anchors.fill: parent; onClicked: root.sortBy(column.modelData.key) }
                    }
                }
            }
        }

        ListView {
            id: list
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: folder
            focus: true
            reuseItems: true
            keyNavigationEnabled: true
            boundsBehavior: Flickable.StopAtBounds
            highlightMoveDuration: 0
            ScrollBar.vertical: ScrollBar {}

            Keys.onReturnPressed: (event) => root.openRow(currentIndex, event.modifiers & Qt.ControlModifier)
            Keys.onEnterPressed: (event) => root.openRow(currentIndex, event.modifiers & Qt.ControlModifier)
            Keys.onPressed: (event) => {
                if ((event.key === Qt.Key_Up && (event.modifiers & Qt.AltModifier)) || event.key === Qt.Key_Backspace) {
                    root.activated(root.parentOf(root.source), true, false)
                    event.accepted = true
                }
            }

            delegate: Rectangle {
                id: row
                required property int index
                required property string name
                required property bool isDir
                required property bool isSymlink
                required property bool isHidden
                required property string sizeText
                required property real modified
                required property string kind
                width: ListView.view.width
                height: Theme.rowHeight
                color: ListView.isCurrentItem ? (list.activeFocus ? Theme.activeSelection : Theme.selection)
                     : hover.hovered ? Theme.hover : "transparent"
                HoverHandler { id: hover }

                Row {
                    anchors.fill: parent
                    anchors.leftMargin: 10
                    Text {
                        width: parent.width - root.sizeWidth - root.dateWidth - root.kindWidth - 10
                        anchors.verticalCenter: parent.verticalCenter
                        text: (row.isDir ? "📁  " : "📄  ") + row.name + (row.isSymlink ? "  ↪" : "")
                        color: row.isHidden ? Theme.textMuted : Theme.text
                        font.pixelSize: Theme.fontSize
                        elide: Text.ElideRight
                    }
                    Text {
                        width: root.sizeWidth
                        anchors.verticalCenter: parent.verticalCenter
                        text: row.sizeText
                        color: Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                    }
                    Text {
                        width: root.dateWidth
                        anchors.verticalCenter: parent.verticalCenter
                        text: Qt.formatDateTime(new Date(row.modified), "dd.MM.yyyy  HH:mm")
                        color: Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                    }
                    Text {
                        width: root.kindWidth
                        anchors.verticalCenter: parent.verticalCenter
                        text: row.kind
                        color: Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                        elide: Text.ElideRight
                    }
                }

                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                    onClicked: (mouse) => {
                        list.currentIndex = row.index
                        list.forceActiveFocus()
                        if (mouse.button === Qt.MiddleButton) root.openRow(row.index, true)
                    }
                    onDoubleClicked: (mouse) => root.openRow(row.index, mouse.modifiers & Qt.ControlModifier)
                }
            }

            Label {
                anchors.centerIn: parent
                visible: list.count === 0 && !folder.loading && !folder.error
                text: qsTr("Dieser Ordner ist leer.")
                color: Theme.textFaint
            }
        }
    }
}
