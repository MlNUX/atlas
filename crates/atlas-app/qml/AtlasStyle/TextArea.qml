// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls.impl
import QtQuick.Templates as T

T.TextArea {
    id: control
    implicitWidth: Math.max(contentWidth + leftPadding + rightPadding, implicitBackgroundWidth + leftInset + rightInset, placeholder.implicitWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(contentHeight + topPadding + bottomPadding, implicitBackgroundHeight + topInset + bottomInset, placeholder.implicitHeight + topPadding + bottomPadding)
    padding: 8
    leftPadding: 10
    rightPadding: 10
    font.pixelSize: Colors.fontSize
    color: enabled ? Colors.textStrong : Colors.textFaint
    selectionColor: Colors.activeSelection
    selectedTextColor: "white"
    placeholderTextColor: Colors.textFaint
    selectByMouse: true

    PlaceholderText {
        id: placeholder
        x: control.leftPadding
        y: control.topPadding
        width: control.width - (control.leftPadding + control.rightPadding)
        height: control.height - (control.topPadding + control.bottomPadding)
        text: control.placeholderText
        font: control.font
        color: control.placeholderTextColor
        verticalAlignment: control.verticalAlignment
        visible: !control.length && !control.preeditText && (!control.activeFocus || control.horizontalAlignment !== Qt.AlignHCenter)
        elide: Text.ElideRight
        renderType: control.renderType
    }
    background: Rectangle {
        implicitWidth: 200
        implicitHeight: 60
        radius: Colors.radius
        color: control.enabled ? Colors.field : "transparent"
        border.width: control.activeFocus ? 1.5 : 1
        border.color: control.activeFocus ? Colors.accent : control.hovered ? Colors.borderStrong : Colors.border
    }
}
