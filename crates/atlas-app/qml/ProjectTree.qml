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

    signal openRequested(string path, bool newPane)
    signal openProjectRequested()

    color: Theme.sidebar

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
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

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
                color: path === root.currentPath ? Theme.activeSelection
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
                    acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                    onClicked: (mouse) => {
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
