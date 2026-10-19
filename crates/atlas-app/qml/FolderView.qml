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
    required property var actions

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

    // Selection is keyed by path, so it survives external changes that
    // shift rows.
    property var selected: ({})
    property string anchorPath
    property bool mouseSelecting: false
    onSourceChanged: { selected = ({}); anchorPath = "" }

    function selectOnly(path) { selected = ({ [path]: true }); anchorPath = path }
    function toggle(path) {
        const s = Object.assign({}, selected)
        if (s[path]) delete s[path]; else s[path] = true
        selected = s
        anchorPath = path
    }
    function selectRange(path) {
        const a = folder.rowOf(anchorPath), b = folder.rowOf(path)
        if (a < 0) { selectOnly(path); return }
        const s = {}
        for (let i = Math.min(a, b); i <= Math.max(a, b); ++i) s[folder.pathAt(i)] = true
        selected = s
    }
    function selectAll() {
        const s = {}
        for (let i = 0; i < list.count; ++i) s[folder.pathAt(i)] = true
        selected = s
    }
    /// Selected paths in list order (existing ones only).
    function selectedPaths() {
        const out = []
        for (let i = 0; i < list.count; ++i) {
            const p = folder.pathAt(i)
            if (selected[p]) out.push(p)
        }
        return out
    }

    function openRow(row, newPane) {
        if (row < 0 || row >= list.count) return
        root.activated(folder.pathAt(row), folder.isDirAt(row), newPane)
    }

    // Narrow splits drop the type and date columns before the name gets cramped.
    readonly property int sizeWidth: width < 300 ? 0 : 90
    readonly property int dateWidth: width < 420 ? 0 : 140
    readonly property int kindWidth: width < 600 ? 0 : 150

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
                    iconName: "hidden"
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
                        width: modelData.key === "name" ? parent.width - root.sizeWidth - root.dateWidth - root.kindWidth - 10
                            : modelData.width
                        visible: width > 0
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
                const ctrl = event.modifiers & Qt.ControlModifier
                if ((event.key === Qt.Key_Up && (event.modifiers & Qt.AltModifier)) || event.key === Qt.Key_Backspace) {
                    root.activated(root.parentOf(root.source), true, false)
                } else if (event.key === Qt.Key_Space && list.currentIndex >= 0) {
                    root.actions.quickLook(folder.pathAt(list.currentIndex), (step) => {
                        const next = list.currentIndex + step
                        if (next < 0 || next >= list.count) return ""
                        list.currentIndex = next
                        list.positionViewAtIndex(next, ListView.Contain)
                        return folder.pathAt(next)
                    })
                } else if ((event.modifiers & Qt.AltModifier) && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && root.selectedPaths().length === 1) {
                    root.actions.showProperties(root.selectedPaths()[0])
                } else if (event.key === Qt.Key_F2 && root.selectedPaths().length === 1) {
                    root.actions.rename(root.selectedPaths()[0])
                } else if (event.key === Qt.Key_Delete) {
                    root.actions.trash(root.selectedPaths())
                } else if (ctrl && event.key === Qt.Key_C) {
                    root.actions.copy(root.selectedPaths())
                } else if (ctrl && event.key === Qt.Key_X) {
                    root.actions.cut(root.selectedPaths())
                } else if (ctrl && event.key === Qt.Key_V) {
                    root.actions.paste(root.source)
                } else if (ctrl && event.key === Qt.Key_A) {
                    root.selectAll()
                } else {
                    return
                }
                event.accepted = true
            }
            // Keyboard navigation moves the selection like a click would.
            onCurrentIndexChanged: if (currentIndex >= 0 && !root.mouseSelecting) root.selectOnly(folder.pathAt(currentIndex))

            // The empty area below the rows: clears the selection, right click
            // offers new file/folder and paste.
            footer: MouseArea {
                width: list.width
                height: Math.max(Theme.rowHeight, list.height - list.count * Theme.rowHeight)
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                onClicked: (mouse) => {
                    list.forceActiveFocus()
                    root.selected = ({})
                    if (mouse.button === Qt.RightButton) root.actions.showMenu([], root.source)
                }
                DropArea {
                    anchors.fill: parent
                    keys: ["text/uri-list"]
                    onDropped: (drop) => root.actions.dropInto(root.source, drop)
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
                required property string path
                width: ListView.view.width
                height: Theme.rowHeight
                color: root.selected[path] ? (list.activeFocus ? Theme.activeSelection : Theme.selection)
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
                        visible: width > 0
                        anchors.verticalCenter: parent.verticalCenter
                        text: row.sizeText
                        color: Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                    }
                    Text {
                        width: root.dateWidth
                        visible: width > 0
                        anchors.verticalCenter: parent.verticalCenter
                        text: Qt.formatDateTime(new Date(row.modified), "dd.MM.yyyy  HH:mm")
                        color: Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                    }
                    Text {
                        width: root.kindWidth
                        visible: width > 0
                        anchors.verticalCenter: parent.verticalCenter
                        text: row.kind
                        color: Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                        elide: Text.ElideRight
                    }
                }

                // Drag source (all selected rows if this row is selected).
                Item { id: dragProxy }
                Drag.active: rowMouse.drag.active
                Drag.dragType: Drag.Automatic
                Drag.supportedActions: Qt.CopyAction | Qt.MoveAction
                Drag.proposedAction: Qt.MoveAction
                Drag.mimeData: ({ "text/uri-list": root.actions.uriList(root.selected[row.path] ? root.selectedPaths() : [row.path]) })

                // Drop target: into this folder, or the shown folder.
                DropArea {
                    anchors.fill: parent
                    keys: ["text/uri-list"]
                    onDropped: (drop) => root.actions.dropInto(row.isDir ? row.path : root.source, drop)
                    Rectangle {
                        anchors.fill: parent
                        visible: parent.containsDrag && row.isDir
                        color: "transparent"
                        border.color: Theme.accent
                    }
                }

                MouseArea {
                    id: rowMouse
                    drag.target: dragProxy
                    drag.threshold: 8
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
                    onClicked: (mouse) => {
                        list.forceActiveFocus()
                        if (mouse.button === Qt.MiddleButton) {
                            root.openRow(row.index, true)
                            return
                        }
                        root.mouseSelecting = true
                        list.currentIndex = row.index
                        root.mouseSelecting = false
                        if (mouse.modifiers & Qt.ControlModifier) root.toggle(row.path)
                        else if (mouse.modifiers & Qt.ShiftModifier) root.selectRange(row.path)
                        // Right click on a selected row keeps the selection.
                        else if (!(mouse.button === Qt.RightButton && root.selected[row.path])) root.selectOnly(row.path)
                        if (mouse.button === Qt.RightButton) root.actions.showMenu(root.selectedPaths(), root.source)
                    }
                    onDoubleClicked: (mouse) => {
                        if (mouse.button === Qt.LeftButton) root.openRow(row.index, mouse.modifiers & Qt.ControlModifier)
                    }
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
