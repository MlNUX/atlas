// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Templates as T

T.ToolTip {
    id: control
    x: parent ? (parent.width - implicitWidth) / 2 : 0
    y: -implicitHeight - 4
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, contentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, contentHeight + topPadding + bottomPadding)
    margins: 6
    padding: 6
    horizontalPadding: 10
    font.pixelSize: 12
    closePolicy: T.Popup.CloseOnEscape | T.Popup.CloseOnPressOutsideParent | T.Popup.CloseOnReleaseOutsideParent

    contentItem: Text {
        // (Tips can carry server messages; never render them as HTML.)
        textFormat: Text.PlainText
        text: control.text
        font: control.font
        wrapMode: Text.Wrap
        color: Colors.textStrong
    }
    background: Rectangle {
        radius: 7
        color: Colors.tooltip
        border.color: Colors.borderStrong
    }
}
