// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import Atlas

// The head of a dialog: icon tile, title, a line below it and a separating
// line. Controls given as content (close button, zoom …) go on the right.
ColumnLayout {
    id: header
    property string icon
    property string title
    property string subtitle
    /// The subtitle breaks into lines instead of being cut off.
    property bool wrap: false
    property int elide: Text.ElideRight
    property real margins: 18
    property real bottomMargin: 14
    default property alias actions: row.data

    Layout.fillWidth: true
    spacing: 0

    RowLayout {
        id: row
        Layout.fillWidth: true
        Layout.margins: header.margins
        Layout.bottomMargin: header.bottomMargin
        spacing: 12
        Rectangle {
            Layout.preferredWidth: 36
            Layout.preferredHeight: 36
            Layout.alignment: header.wrap ? Qt.AlignTop : Qt.AlignVCenter
            radius: 10
            color: Theme.accentSoft
            Icon { anchors.centerIn: parent; name: header.icon; size: 20; color: Theme.accent }
        }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2
            Text {
                Layout.fillWidth: true
                text: header.title
                color: Theme.textStrong
                font.pixelSize: 16
                font.weight: Font.DemiBold
                elide: Text.ElideRight
            }
            Text {
                Layout.fillWidth: true
                visible: text !== ""
                text: header.subtitle
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
                wrapMode: header.wrap ? Text.Wrap : Text.NoWrap
                elide: header.wrap ? Text.ElideNone : header.elide
            }
        }
    }
    Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }
}
