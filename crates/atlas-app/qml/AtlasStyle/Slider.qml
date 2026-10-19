// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Templates as T

// Thin track with an accent fill and a round handle.
T.Slider {
    id: control
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitHandleWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitHandleHeight + topPadding + bottomPadding)
    padding: 4

    handle: Rectangle {
        x: control.leftPadding + (control.horizontal ? control.visualPosition * (control.availableWidth - width) : (control.availableWidth - width) / 2)
        y: control.topPadding + (control.horizontal ? (control.availableHeight - height) / 2 : control.visualPosition * (control.availableHeight - height))
        implicitWidth: 14
        implicitHeight: 14
        radius: 7
        color: control.pressed ? Colors.accentDown : control.hovered ? Colors.accentHover : Colors.accent
        border.color: Colors.surface
        border.width: 2
    }

    background: Item {
        x: control.leftPadding
        y: control.topPadding
        implicitWidth: control.horizontal ? 120 : 14
        implicitHeight: control.horizontal ? 14 : 120
        width: control.availableWidth
        height: control.availableHeight
        Rectangle {
            x: control.horizontal ? 0 : (parent.width - width) / 2
            y: control.horizontal ? (parent.height - height) / 2 : 0
            width: control.horizontal ? parent.width : 4
            height: control.horizontal ? 4 : parent.height
            radius: 2
            color: Colors.borderStrong
            Rectangle {
                width: control.horizontal ? control.position * parent.width : parent.width
                height: control.horizontal ? parent.height : (1 - control.position) * parent.height
                y: control.horizontal ? 0 : control.visualPosition * parent.height
                radius: 2
                color: Colors.accent
            }
        }
    }
}
