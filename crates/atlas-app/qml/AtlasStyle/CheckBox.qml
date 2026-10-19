// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Templates as T

T.CheckBox {
    id: control
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding, implicitIndicatorHeight + topPadding + bottomPadding)
    padding: 4
    spacing: 8
    font.pixelSize: Colors.fontSize
    hoverEnabled: true

    indicator: Rectangle {
        implicitWidth: 18
        implicitHeight: 18
        x: control.leftPadding
        y: control.topPadding + (control.availableHeight - height) / 2
        radius: 5
        color: control.checkState !== Qt.Unchecked ? (control.down ? Colors.accentDown : Colors.accent) : control.hovered ? Colors.hover : Colors.field
        border.color: control.checkState !== Qt.Unchecked ? "transparent" : control.visualFocus ? Colors.accent : Colors.borderStrong
        opacity: control.enabled ? 1 : 0.5
        Image {
            anchors.centerIn: parent
            width: 14
            height: 14
            sourceSize: Qt.size(28, 28)
            visible: control.checkState === Qt.Checked
            source: Colors.icon("<path d='m5 12.5 4.5 4.5L19 7.5'/>", "white")
        }
        Rectangle {
            anchors.centerIn: parent
            width: 8
            height: 2
            radius: 1
            color: "white"
            visible: control.checkState === Qt.PartiallyChecked
        }
    }
    contentItem: Text {
        leftPadding: control.indicator && !control.mirrored ? control.indicator.width + control.spacing : 0
        rightPadding: control.indicator && control.mirrored ? control.indicator.width + control.spacing : 0
        text: control.text
        font: control.font
        color: control.enabled ? Colors.text : Colors.textFaint
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
        wrapMode: Text.Wrap
    }
}
