// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls

// Small flat toolbar button with a text glyph and a tooltip.
AbstractButton {
    id: control
    property string glyph
    property string tip
    property bool checkedLook: checkable && checked
    implicitWidth: Math.max(22, label.implicitWidth + 10)
    implicitHeight: 22
    hoverEnabled: true
    focusPolicy: Qt.NoFocus

    ToolTip.visible: hovered && tip.length > 0
    ToolTip.delay: 600
    ToolTip.text: tip

    background: Rectangle {
        radius: 4
        color: control.down ? "#3c3f41" : control.checkedLook ? "#2f3f52" : control.hovered ? Theme.hover : "transparent"
        border.color: control.checkedLook ? Theme.accent : "transparent"
    }
    contentItem: Text {
        id: label
        text: control.glyph
        color: control.enabled ? Theme.text : Theme.textFaint
        font.pixelSize: 13
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }
}
