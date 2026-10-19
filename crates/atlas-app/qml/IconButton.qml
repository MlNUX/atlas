// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Atlas

// Small flat toolbar button: a line icon (`icon`), a text glyph, or both,
// with a tooltip.
AbstractButton {
    id: control
    property string glyph
    /// Name of an `Icon` (shown before the glyph).
    property string iconName
    property int iconSize: 16
    property string tip
    property bool checkedLook: checkable && checked
    readonly property color foreground: !enabled ? Theme.textFaint : checkedLook ? Theme.textStrong : hovered ? Theme.textStrong : Theme.text

    implicitWidth: Math.max(26, row.implicitWidth + (glyph ? 12 : 8))
    implicitHeight: 26
    hoverEnabled: true
    focusPolicy: Qt.NoFocus
    ToolTip.visible: hovered && tip.length > 0
    ToolTip.delay: 600
    ToolTip.text: tip

    background: Rectangle {
        radius: 6
        color: control.down ? Theme.selection : control.checkedLook ? Theme.accentSoft : control.hovered ? Theme.hover : "transparent"
        border.color: control.checkedLook ? Qt.rgba(0.24, 0.55, 0.99, 0.5) : "transparent"
    }
    contentItem: Item {
        implicitWidth: row.implicitWidth
        implicitHeight: row.implicitHeight
        Row {
            id: row
            anchors.centerIn: parent
            spacing: 6
            Icon {
                visible: control.iconName !== ""
                name: control.iconName
                size: control.iconSize
                color: control.checkedLook ? Theme.accent : control.foreground
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                id: label
                visible: control.glyph !== ""
                text: control.glyph
                color: control.foreground
                font.pixelSize: 13
                anchors.verticalCenter: parent.verticalCenter
            }
        }
    }
}
