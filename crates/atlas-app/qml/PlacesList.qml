// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Atlas

// Fixed system places above the project tree (no favourites, see CONCEPT.md).
Column {
    id: root
    required property var names
    required property var paths
    required property var icons
    property string currentPath
    property bool expanded: true

    signal openRequested(string path, bool newPane)

    function glyph(icon) {
        return { home: "🏠", desktop: "🖥", documents: "📄", downloads: "⬇", trash: "🗑" }[icon] || "📁"
    }

    Item {
        width: root.width
        height: 26
        Text {
            anchors.verticalCenter: parent.verticalCenter
            x: 8
            text: (root.expanded ? "▾  " : "▸  ") + qsTr("ORTE")
            color: Theme.text
            font.pixelSize: Theme.smallFontSize
            font.bold: true
        }
        MouseArea { anchors.fill: parent; onClicked: root.expanded = !root.expanded }
    }

    Repeater {
        model: root.expanded ? root.names.length : 0
        delegate: Rectangle {
            id: place
            required property int index
            readonly property string path: root.paths[index]
            width: root.width
            height: Theme.rowHeight
            color: path === root.currentPath ? Theme.activeSelection : hover.hovered ? Theme.hover : "transparent"
            HoverHandler { id: hover }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                x: 20
                width: parent.width - 28
                text: root.glyph(root.icons[place.index]) + "  " + root.names[place.index]
                color: Theme.text
                font.pixelSize: Theme.fontSize
                elide: Text.ElideRight
            }
            ToolTip.visible: hover.hovered
            ToolTip.delay: 800
            ToolTip.text: place.path
            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                onClicked: (mouse) => root.openRequested(place.path,
                    mouse.button === Qt.MiddleButton || (mouse.modifiers & Qt.ControlModifier))
            }
        }
    }
}
