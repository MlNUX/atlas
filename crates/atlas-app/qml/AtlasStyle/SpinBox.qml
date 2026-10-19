// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Templates as T

T.SpinBox {
    id: control
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, contentItem.implicitWidth + 2 * padding + up.implicitIndicatorWidth + down.implicitIndicatorWidth)
    implicitHeight: Math.max(implicitContentHeight + topPadding + bottomPadding, implicitBackgroundHeight, up.implicitIndicatorHeight, down.implicitIndicatorHeight)
    padding: 4
    leftPadding: padding + (control.mirrored ? (up.indicator ? up.indicator.width : 0) : (down.indicator ? down.indicator.width : 0))
    rightPadding: padding + (control.mirrored ? (down.indicator ? down.indicator.width : 0) : (up.indicator ? up.indicator.width : 0))
    font.pixelSize: Colors.fontSize
    editable: true

    validator: IntValidator {
        locale: control.locale.name
        bottom: Math.min(control.from, control.to)
        top: Math.max(control.from, control.to)
    }

    contentItem: TextInput {
        z: 2
        text: control.displayText
        font: control.font
        color: Colors.textStrong
        selectionColor: Colors.activeSelection
        selectedTextColor: "white"
        horizontalAlignment: Qt.AlignHCenter
        verticalAlignment: Qt.AlignVCenter
        readOnly: !control.editable
        validator: control.validator
        inputMethodHints: control.inputMethodHints
        clip: width < implicitWidth
    }

    up.indicator: Rectangle {
        x: control.mirrored ? 2 : control.width - width - 2
        y: 2
        implicitWidth: 26
        implicitHeight: 28
        height: control.height - 4
        radius: 6
        color: control.up.pressed ? Colors.selection : control.up.hovered ? Colors.hover : "transparent"
        Text { anchors.centerIn: parent; text: "+"; color: control.up.indicator.enabled ? Colors.text : Colors.textFaint; font.pixelSize: 15 }
    }
    down.indicator: Rectangle {
        x: control.mirrored ? control.width - width - 2 : 2
        y: 2
        implicitWidth: 26
        implicitHeight: 28
        height: control.height - 4
        radius: 6
        color: control.down.pressed ? Colors.selection : control.down.hovered ? Colors.hover : "transparent"
        Text { anchors.centerIn: parent; text: "−"; color: control.down.indicator.enabled ? Colors.text : Colors.textFaint; font.pixelSize: 15 }
    }
    background: Rectangle {
        implicitWidth: 110
        implicitHeight: 32
        radius: Colors.radius
        color: Colors.field
        border.color: control.activeFocus ? Colors.accent : Colors.border
    }
}
