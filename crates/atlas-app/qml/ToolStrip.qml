// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Layouts
import Atlas

// A strip of controls above or below a view (toolbars, location and status
// bars): header colour, a row for the content and a separating line.
// The content goes straight in, like in a RowLayout:
//
//     ToolStrip { IconButton { … } Text { Layout.fillWidth: true … } }
Rectangle {
    id: strip
    default property alias content: row.data
    property alias spacing: row.spacing
    property real leftMargin: 8
    property real rightMargin: 8
    /// The separating line: "bottom" (a bar above a view), "top" (below
    /// one) or "none".
    property string line: "bottom"

    Layout.fillWidth: true
    implicitHeight: 30
    color: Theme.header

    RowLayout {
        id: row
        anchors.fill: parent
        anchors.leftMargin: strip.leftMargin
        anchors.rightMargin: strip.rightMargin
        spacing: 4
    }
    Rectangle {
        visible: strip.line !== "none"
        y: strip.line === "top" ? 0 : parent.height - 1
        width: parent.width
        height: 1
        color: Theme.border
    }
}
