// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Templates as T

T.ItemDelegate {
    id: control
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding)
    padding: 6
    horizontalPadding: 10
    font.pixelSize: Colors.fontSize
    hoverEnabled: true

    contentItem: Text {
        text: control.text
        font: control.font
        color: control.highlighted ? Colors.textStrong : control.enabled ? Colors.text : Colors.textFaint
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
    }
    background: Rectangle {
        implicitWidth: 100
        implicitHeight: 32
        radius: 6
        color: control.highlighted ? Colors.activeSelection : control.down ? Colors.selection : control.hovered || control.visualFocus ? Colors.hover : "transparent"
    }
}
