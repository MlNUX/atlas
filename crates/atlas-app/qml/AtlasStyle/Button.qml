// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Templates as T

// Rounded button; `highlighted` (and the accept button of dialogs) in the
// accent colour, `flat` without frame.
T.Button {
    id: control
    readonly property bool primary: highlighted || T.DialogButtonBox.buttonRole === T.DialogButtonBox.AcceptRole
        || T.DialogButtonBox.buttonRole === T.DialogButtonBox.YesRole || T.DialogButtonBox.buttonRole === T.DialogButtonBox.ApplyRole

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding)
    padding: 6
    horizontalPadding: 14
    spacing: 6
    font.pixelSize: Colors.fontSize
    hoverEnabled: true

    contentItem: Text {
        text: control.text
        font: control.font
        color: !control.enabled ? Colors.textFaint : control.primary && !control.flat ? "white" : control.flat && !control.hovered ? Colors.text : Colors.textStrong
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
    background: Rectangle {
        implicitWidth: 72
        implicitHeight: 32
        radius: Colors.radius
        opacity: control.enabled ? 1 : 0.55
        color: control.primary && !control.flat
            ? (control.down ? Colors.accentDown : control.hovered ? Colors.accentHover : Colors.accent)
            : control.flat ? (control.down ? Colors.selection : control.hovered || control.visualFocus ? Colors.hover : "transparent")
            : (control.down ? Colors.selection : control.hovered ? Colors.hover : Colors.raised)
        border.color: control.visualFocus ? Colors.accent : control.flat || control.primary ? "transparent" : Colors.border
    }
}
