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
    signal openProjectRequested()

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
            Layout.leftMargin: 12
            Layout.rightMargin: 6
            spacing: 2
            Label {
                Layout.fillWidth: true
                text: root.projectName ? root.projectName.toUpperCase() : qsTr("KEIN PROJEKT")
                color: Theme.text
                font.pixelSize: Theme.smallFontSize
                font.bold: true
                elide: Text.ElideRight
            }
            IconButton {
                glyph: "+"
                tip: qsTr("Neue Datei")
                enabled: !!root.projectName
                onClicked: root.actions.newFile(root.targetDir())
            }
            IconButton {
                glyph: "📁"
                tip: qsTr("Neuer Ordner")
                enabled: !!root.projectName
                onClicked: root.actions.newFolder(root.targetDir())
            }
            IconButton {
                glyph: "◌"
                tip: qsTr("Versteckte Dateien anzeigen")
                checkable: true
                checked: root.model.showHidden
                onToggled: root.model.showHidden = checked
            }
            IconButton {
                glyph: "⟳"
                tip: qsTr("Aktualisieren")
                onClicked: root.model.refresh()
            }
        }

        ListView {
            id: list
            Layout.fillWidth: true
            Layout.fillHeight: true
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
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    const item = list.itemAtIndex(row)
                    if (item && item.isDir) root.model.toggle(row)
                    else if (root.selectedPath) root.openRequested(root.selectedPath, ctrl)
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
                height: Theme.rowHeight
                color: path === root.selectedPath ? (list.activeFocus ? Theme.activeSelection : Theme.selection)
                     : path === root.currentPath ? Theme.selection
                     : hover.hovered ? Theme.hover : "transparent"

                HoverHandler { id: hover }

                Row {
                    anchors.verticalCenter: parent.verticalCenter
                    x: 8 + row.depth * 12
                    spacing: 4
                    Text {
                        width: 12
                        text: row.isDir ? (row.loading ? "…" : row.expanded ? "▾" : "▸") : ""
                        color: Theme.textMuted
                        font.pixelSize: 12
                    }
                    Text {
                        text: row.isDir ? (row.expanded ? "📂" : "📁") : fileGlyph(row.name)
                        font.pixelSize: 12
                        opacity: row.isHidden ? 0.6 : 1
                    }
                    Text {
                        text: row.name + (row.isSymlink ? "  ↪" : "")
                        color: row.isHidden ? Theme.textMuted : Theme.text
                        font.pixelSize: Theme.fontSize
                        elide: Text.ElideRight
                        width: list.width - (8 + row.depth * 12) - 44
                    }
                }

                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
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
                        // them as file list in a new split.
                        if (row.isDir && !newPane) root.model.toggle(row.index)
                        else root.openRequested(row.path, newPane)
                    }
                }

                IconButton {
                    anchors.right: parent.right
                    anchors.rightMargin: 6
                    anchors.verticalCenter: parent.verticalCenter
                    visible: row.isDir && hover.hovered
                    glyph: "☰"
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

        Button {
            Layout.fillWidth: true
            Layout.margins: 10
            visible: !root.projectName
            text: qsTr("Projektordner öffnen…")
            onClicked: root.openProjectRequested()
        }
    }

    function fileGlyph(name) {
        const ext = name.slice(name.lastIndexOf(".") + 1).toLowerCase()
        if (ext === "pdf") return "📕"
        if (ext === "md" || ext === "markdown") return "📝"
        if (["png", "jpg", "jpeg", "gif", "svg", "webp"].indexOf(ext) >= 0) return "🖼"
        if (["rs", "c", "h", "cpp", "py", "js", "ts", "qml", "nix", "sh", "toml", "json", "yaml", "yml"].indexOf(ext) >= 0) return "⌘"
        return "📄"
    }
}
