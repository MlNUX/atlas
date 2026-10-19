// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Templates as T

// Thin bar that appears while scrolling or hovered.
T.ScrollBar {
    id: control
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding)
    padding: 2
    visible: control.policy !== T.ScrollBar.AlwaysOff
    minimumSize: orientation === Qt.Horizontal ? height / width : width / height

    contentItem: Rectangle {
        implicitWidth: control.interactive ? 7 : 3
        implicitHeight: control.interactive ? 7 : 3
        radius: width / 2
        color: control.pressed ? "#4a6189" : control.hovered ? "#36496b" : "#26354f"
        opacity: control.policy === T.ScrollBar.AlwaysOn || (control.size < 1 && (control.active || control.hovered)) ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 160 } }
    }
}
