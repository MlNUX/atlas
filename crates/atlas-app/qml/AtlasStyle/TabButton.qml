// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Templates as T

// Text tab with an accent underline (like the context bar).
T.TabButton {
    id: control
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding)
    padding: 8
    horizontalPadding: 12
    font.pixelSize: Colors.fontSize
    hoverEnabled: true

    contentItem: Text {
        text: control.text
        font: control.font
        color: control.checked ? Colors.accent : control.hovered ? Colors.textStrong : Colors.text
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
    background: Item {
        implicitHeight: 36
        Rectangle {
            anchors.bottom: parent.bottom
            width: parent.width
            height: 2
            radius: 1
            color: Colors.accent
            visible: control.checked
        }
    }
}
