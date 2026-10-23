// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Explorer: the project folder as a VS Code style tree.
Rectangle {
    id: root
    required property ProjectTreeModel model
    required property string projectName
    property string currentPath
    property var actions
    /// Selected row (file or folder); new items go into its folder.
    property string selectedPath

    signal openRequested(string path, bool newPane)
    /// For the smoke test.
    readonly property alias rootRow: rootRow
    readonly property alias treeList: list

    /// The row item showing `path` (null if not visible); for the smoke test.
    function itemFor(path) {
        const row = root.model.rowOf(path)
        return row >= 0 ? list.itemAtIndex(row) : null
    }

    /// Quick look at `path`; arrow keys move through the visible rows.
    function quickLook(path) {
        root.actions.quickLook(path, (step) => {
            const next = root.model.rowOf(root.selectedPath) + step
            if (next < 0 || next >= list.count) return ""
            root.selectedPath = root.model.pathAt(next)
            list.positionViewAtIndex(next, ListView.Contain)
            return root.selectedPath
        })
    }
    signal openProjectRequested()
    /// Double click on a file: open as permanent tab.
    signal pinRequested(string path)
    /// The project row collapses the whole tree.

    color: Theme.sidebar

    /// Folder for new files/folders and paste: the selected folder, the
    /// folder of the selected file, or the project root.
    function targetDir() {
        if (!selectedPath) return root.model.root
        const row = root.model.rowOf(selectedPath)
        if (row >= 0 && list.itemAtIndex(row) && list.itemAtIndex(row).isDir) return selectedPath
        return root.actions.ops.parentDir(selectedPath) || root.model.root
    }

    function selectedList() { return root.selectedPath ? [root.selectedPath] : [] }

    Connections {
        target: root.actions ? root.actions : null
        function onCreated(path) { root.selectedPath = path }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 30
            Layout.leftMargin: 14
            Layout.rightMargin: 8
            spacing: 0
            HoverHandler { id: headerHover }
            Label {
                Layout.fillWidth: true
                text: qsTr("PROJEKTORDNER")
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
                font.bold: true
                font.letterSpacing: 0.6
                elide: Text.ElideRight
            }
            // Tools appear on hover, like the mockup's quiet header.
            Row {
                opacity: headerHover.hovered || root.model.showHidden ? 1 : 0
                Behavior on opacity { NumberAnimation { duration: 120 } }
                IconButton { iconName: "file-plus"; iconSize: 14; tip: qsTr("Neue Datei"); enabled: !!root.projectName; onClicked: root.actions.newFile(root.targetDir()) }
                IconButton { iconName: "folder-plus"; iconSize: 14; tip: qsTr("Neuer Ordner"); enabled: !!root.projectName; onClicked: root.actions.newFolder(root.targetDir()) }
                IconButton {
                    iconName: "hidden"
                    iconSize: 14
                    tip: qsTr("Versteckte Dateien anzeigen")
                    checkable: true
                    checked: root.model.showHidden
                    onToggled: root.model.showHidden = checked
                }
                IconButton { iconName: "refresh"; iconSize: 14; tip: qsTr("Aktualisieren"); onClicked: root.model.refresh() }
            }
        }

        // The project folder itself (always open; its contents follow).
        Rectangle {
            id: rootRow
            readonly property bool hasChevron: false
            Layout.fillWidth: true
            Layout.leftMargin: 8
            Layout.rightMargin: 8
            Layout.preferredHeight: 30
            visible: !!root.projectName
            radius: 6
            color: Theme.accentSoft
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 6
                anchors.rightMargin: 6
                spacing: 6
                Icon { name: "folder-fill"; size: 16; color: "#5aa2ff" }
                Label {
                    Layout.fillWidth: true
                    text: root.projectName
                    color: Theme.textStrong
                    font.pixelSize: Theme.fontSize
                    elide: Text.ElideRight
                }
            }
            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.RightButton
                onClicked: root.actions.showMenu([], root.model.root)
            }
        }

        ListView {
            id: list
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.topMargin: 2
            clip: true
            model: root.model
            boundsBehavior: Flickable.StopAtBounds
            reuseItems: true
            activeFocusOnTab: true
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

            Keys.onPressed: (event) => {
                const ctrl = event.modifiers & Qt.ControlModifier
                const row = root.model.rowOf(root.selectedPath)
                if (event.key === Qt.Key_Down || event.key === Qt.Key_Up) {
                    const next = Math.max(0, Math.min(list.count - 1, row + (event.key === Qt.Key_Down ? 1 : -1)))
                    root.selectedPath = root.model.pathAt(next)
                    list.positionViewAtIndex(next, ListView.Contain)
                } else if ((event.modifiers & Qt.AltModifier) && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && root.selectedPath) {
                    root.actions.showProperties(root.selectedPath)
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    const item = list.itemAtIndex(row)
                    if (item && item.isDir) root.model.toggle(row)
                    else if (root.selectedPath) root.openRequested(root.selectedPath, ctrl)
                } else if (event.key === Qt.Key_Space && root.selectedPath) {
                    root.quickLook(root.selectedPath)
                } else if (event.key === Qt.Key_F2 && root.selectedPath) {
                    root.actions.rename(root.selectedPath)
                } else if (event.key === Qt.Key_Delete && root.selectedPath) {
                    root.actions.trash([root.selectedPath])
                } else if (ctrl && event.key === Qt.Key_C && root.selectedPath) {
                    root.actions.copy([root.selectedPath])
                } else if (ctrl && event.key === Qt.Key_X && root.selectedPath) {
                    root.actions.cut([root.selectedPath])
                } else if (ctrl && event.key === Qt.Key_V) {
                    root.actions.paste(root.targetDir())
                } else {
                    return
                }
                event.accepted = true
            }

            footer: MouseArea {
                width: list.width
                height: Math.max(Theme.rowHeight, list.height - list.count * Theme.rowHeight)
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                enabled: !!root.projectName
                onClicked: (mouse) => {
                    list.forceActiveFocus()
                    root.selectedPath = ""
                    if (mouse.button === Qt.RightButton) root.actions.showMenu([], root.model.root)
                }
                DropArea {
                    anchors.fill: parent
                    keys: ["text/uri-list"]
                    onDropped: (drop) => root.actions.dropInto(root.model.root, drop)
                }
            }

            delegate: Rectangle {
                id: row
                required property int index
                required property string name
                required property string path
                required property int depth
                required property bool isDir
                required property bool expanded
                required property bool loading
                required property bool isSymlink
                required property bool isHidden

                width: ListView.view.width
                height: Theme.rowHeight + 2
                color: "transparent"
                readonly property var fileIcon: Theme.fileIcon(name, isDir)

                HoverHandler { id: hover }

                // Rounded highlight; the current document gets an accent bar.
                Rectangle {
                    anchors.fill: parent
                    anchors.leftMargin: 8
                    anchors.rightMargin: 8
                    anchors.topMargin: 1
                    anchors.bottomMargin: 1
                    radius: 6
                    color: row.path === root.selectedPath ? (list.activeFocus ? Theme.activeSelection : Theme.selection)
                         : row.path === root.currentPath ? Theme.selection
                         : hover.hovered ? Theme.hover : "transparent"
                    Rectangle {
                        visible: row.path === root.currentPath
                        width: 2
                        height: parent.height - 8
                        anchors.verticalCenter: parent.verticalCenter
                        radius: 1
                        color: Theme.accent
                    }
                }

                Row {
                    anchors.verticalCenter: parent.verticalCenter
                    x: 14 + (row.depth + 1) * 14
                    spacing: 7
                    Item {
                        width: 12
                        height: 16
                        Icon {
                            anchors.centerIn: parent
                            visible: row.isDir && !row.loading
                            name: row.expanded ? "chevron-down" : "chevron-right"
                            size: 12
                            color: Theme.textMuted
                        }
                        Text { visible: row.loading; text: "…"; color: Theme.textMuted; anchors.centerIn: parent }
                    }
                    Icon {
                        name: row.fileIcon[0]
                        color: row.fileIcon[1]
                        size: 16
                        opacity: row.isHidden ? 0.55 : 1
                        anchors.verticalCenter: parent.verticalCenter
                    }
                    Text {
                        text: row.name + (row.isSymlink ? "  ↪" : "")
                        color: row.isHidden ? Theme.textMuted : row.path === root.currentPath ? Theme.textStrong : Theme.text
                        font.pixelSize: Theme.fontSize
                        elide: Text.ElideRight
                        width: list.width - (14 + (row.depth + 1) * 14) - 60
                        anchors.verticalCenter: parent.verticalCenter
                    }
                }

                // Drag source: files and folders can be dropped on splits
                // (open), folders (move/copy), the terminal (insert path) and
                // other applications.
                Item { id: dragProxy }
                Drag.active: rowMouse.drag.active
                Drag.dragType: Drag.Automatic
                Drag.supportedActions: Qt.CopyAction | Qt.MoveAction
                Drag.proposedAction: Qt.MoveAction
                Drag.mimeData: ({ "text/uri-list": root.actions ? root.actions.uriList([row.path]) : "" })

                // Drop target: into this folder, or the folder of this file.
                DropArea {
                    anchors.fill: parent
                    keys: ["text/uri-list"]
                    onDropped: (drop) => root.actions.dropInto(row.isDir ? row.path : root.actions.ops.parentDir(row.path), drop)
                    Rectangle {
                        anchors.fill: parent
                        visible: parent.containsDrag && row.isDir
                        color: "transparent"
                        border.color: Theme.accent
                    }
                }

                MouseArea {
                    id: rowMouse
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
                    drag.target: dragProxy
                    drag.threshold: 8
                    onDoubleClicked: (mouse) => {
                        if (!row.isDir && mouse.button === Qt.LeftButton) root.pinRequested(row.path)
                    }
                    onClicked: (mouse) => {
                        root.selectedPath = row.path
                        list.forceActiveFocus()
                        if (mouse.button === Qt.RightButton) {
                            root.actions.showMenu([row.path], root.targetDir())
                            return
                        }
                        const newPane = mouse.button === Qt.MiddleButton
                            || (mouse.modifiers & Qt.ControlModifier)
                        // Folders expand on click; Ctrl/middle click opens
                        // them as file list in a new split. Ctrl+click on a
                        // file shows the quick look, middle click a split.
                        if (row.isDir && !newPane) root.model.toggle(row.index)
                        else if (!row.isDir && mouse.button === Qt.LeftButton && newPane) root.quickLook(row.path)
                        else root.openRequested(row.path, newPane)
                    }
                }

                IconButton {
                    anchors.right: parent.right
                    anchors.rightMargin: 6
                    anchors.verticalCenter: parent.verticalCenter
                    visible: row.isDir && hover.hovered
                    iconName: "list"
                    iconSize: 14
                    tip: qsTr("Als Dateiliste öffnen")
                    onClicked: root.openRequested(row.path, false)
                }
            }

            Label {
                anchors.centerIn: parent
                visible: !root.projectName
                width: parent.width - 32
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                color: Theme.textMuted
                text: qsTr("Noch kein Projektordner geöffnet.")
            }
        }
        // Momentum after touchpad swipes (#4).
        TouchpadFling {
            // Over the list, not a cell of the layout.
            parent: list
            anchors.fill: parent
            flickable: list
            active: !root.actions || !root.actions.app || root.actions.app.touchpadFling
            app: root.actions ? root.actions.app : null
        }

        Button {
            Layout.fillWidth: true
            Layout.margins: 10
            visible: !root.projectName
            text: qsTr("Projektordner öffnen…")
            onClicked: root.openProjectRequested()
        }
    }
}
