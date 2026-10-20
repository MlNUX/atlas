// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Templates as T

// Menu entry; text after a tab (`"Kopieren\tStrg+C"`) is shown as
// shortcut on the right.
T.MenuItem {
    id: control
    readonly property var parts: text.split("\t")
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, implicitContentHeight + topPadding + bottomPadding, implicitIndicatorHeight + topPadding + bottomPadding)
    padding: 6
    leftPadding: 12 + (control.checkable ? 22 : 0)
    rightPadding: 12 + (control.subMenu ? 18 : 0)
    spacing: 8
    font.pixelSize: Colors.fontSize
    hoverEnabled: true

    contentItem: Item {
        implicitWidth: label.implicitWidth + (shortcut.visible ? shortcut.implicitWidth + 28 : 0)
        implicitHeight: label.implicitHeight
        Text {
            id: label
            width: parent.width - (shortcut.visible ? shortcut.implicitWidth + 16 : 0)
            anchors.verticalCenter: parent.verticalCenter
            text: control.parts[0]
            font: control.font
            color: !control.enabled ? Colors.textMuted : control.highlighted ? Colors.textStrong : Colors.text
            elide: Text.ElideRight
        }
        Text {
            id: shortcut
            visible: control.parts.length > 1
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            text: control.parts.slice(1).join(" ")
            font.pixelSize: Colors.fontSize - 1
            color: control.highlighted ? Colors.text : Colors.textMuted
        }
    }

    indicator: Image {
        visible: control.checkable && control.checked
        x: 10
        y: control.topPadding + (control.availableHeight - height) / 2
        width: 15
        height: 15
        sourceSize: Qt.size(30, 30)
        source: Colors.icon("<path d='m5 12.5 4.5 4.5L19 7.5'/>", Colors.accent)
    }

    arrow: Image {
        visible: !!control.subMenu
        x: control.width - width - 10
        y: control.topPadding + (control.availableHeight - height) / 2
        width: 14
        height: 14
        sourceSize: Qt.size(28, 28)
        source: Colors.icon("<path d='m9 6 6 6-6 6'/>", Colors.textMuted)
    }

    background: Rectangle {
        implicitWidth: 200
        implicitHeight: 32
        radius: 6
        color: control.highlighted && control.enabled ? Colors.hover : "transparent"
    }
}
