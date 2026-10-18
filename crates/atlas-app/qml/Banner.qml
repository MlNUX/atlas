// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Notice above a document with optional actions ({ text, run }).
Rectangle {
    id: root
    property string message
    property color tone: Theme.warn
    property var actions: []

    Layout.fillWidth: true
    Layout.preferredHeight: visible ? Math.max(32, row.implicitHeight + 8) : 0
    color: Qt.rgba(tone.r, tone.g, tone.b, 0.14)
    Rectangle { width: 3; height: parent.height; color: root.tone }

    RowLayout {
        id: row
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 8
        spacing: 6
        Label {
            Layout.fillWidth: true
            text: root.message
            color: Theme.textStrong
            wrapMode: Text.Wrap
            font.pixelSize: Theme.fontSize
        }
        Repeater {
            model: root.actions
            Button {
                required property var modelData
                text: modelData.text
                flat: true
                onClicked: modelData.run()
            }
        }
    }
}
