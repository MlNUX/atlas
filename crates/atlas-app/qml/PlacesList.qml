// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Atlas

// System places above the project tree: Atlas' fixed ones, then (if
// switched on) those of Dolphin/KDE and GTK file managers.
Column {
    id: root
    required property var names
    required property var paths
    required property var icons
    /// The first `fixedCount` places are Atlas' own; a divider follows.
    property int fixedCount: names.length
    property string currentPath
    property bool expanded: true

    signal openRequested(string path, bool newPane)

    Item {
        width: root.width
        height: 34
        Text {
            anchors.verticalCenter: parent.verticalCenter
            x: 16
            text: qsTr("Orte")
            color: Theme.textStrong
            font.pixelSize: Theme.fontSize
            font.bold: true
        }
        Icon {
            anchors.verticalCenter: parent.verticalCenter
            anchors.right: parent.right
            anchors.rightMargin: 14
            visible: placesHover.hovered
            name: root.expanded ? "chevron-down" : "chevron-right"
            size: 14
        }
        HoverHandler { id: placesHover }
        MouseArea { anchors.fill: parent; onClicked: root.expanded = !root.expanded }
    }

    Repeater {
        model: root.expanded ? root.names.length : 0
        delegate: Item {
            id: place
            required property int index
            // (The lists change one after another when the places change.)
            readonly property string path: root.paths[index] || ""
            readonly property bool current: path === root.currentPath
            readonly property bool firstExtra: index === root.fixedCount && index > 0
            width: root.width
            height: 32 + (firstExtra ? 26 : 0)
            Item {
                visible: place.firstExtra
                width: parent.width
                height: 26
                Rectangle { x: 16; y: 8; width: parent.width - 32; height: 1; color: Theme.border }
                Text {
                    x: 16
                    y: 13
                    text: qsTr("DATEIMANAGER")
                    color: Theme.textFaint
                    font.pixelSize: 10
                    font.letterSpacing: 0.6
                }
            }
            // The row below the optional divider.
            Item {
                anchors.fill: parent
                anchors.topMargin: place.firstExtra ? 26 : 0
                Rectangle {
                    anchors.fill: parent
                    anchors.leftMargin: 8
                    anchors.rightMargin: 8
                    anchors.topMargin: 1
                    anchors.bottomMargin: 1
                    radius: 6
                    color: place.current ? Theme.activeSelection : hover.hovered ? Theme.hover : "transparent"
                }
                HoverHandler { id: hover }
                Row {
                    anchors.verticalCenter: parent.verticalCenter
                    x: 18
                    spacing: 12
                    Icon {
                        name: root.icons[place.index] || "folder"
                        size: 18
                        color: place.current ? Theme.accent : Theme.textMuted
                        anchors.verticalCenter: parent.verticalCenter
                    }
                    Text {
                        width: root.width - 64
                        text: root.names[place.index] || ""
                        color: place.current ? Theme.textStrong : Theme.text
                        font.pixelSize: Theme.fontSize
                        elide: Text.ElideRight
                        anchors.verticalCenter: parent.verticalCenter
                    }
                }
                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                    onClicked: (mouse) => root.openRequested(place.path,
                        mouse.button === Qt.MiddleButton || (mouse.modifiers & Qt.ControlModifier))
                }
            }
            ToolTip.visible: hover.hovered
            ToolTip.delay: 800
            ToolTip.text: place.path
        }
    }
}
