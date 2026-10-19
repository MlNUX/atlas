// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Templates as T
import QtQuick.Window

T.Menu {
    id: control
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, contentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, contentHeight + topPadding + bottomPadding)
    margins: 0
    padding: 6
    overlap: 4
    font.pixelSize: Colors.fontSize

    delegate: MenuItem {}

    contentItem: ListView {
        implicitHeight: contentHeight
        model: control.contentModel
        interactive: Window.window ? contentHeight + control.topPadding + control.bottomPadding > Window.window.height : false
        clip: true
        currentIndex: control.currentIndex
        T.ScrollBar.vertical: ScrollBar {}
    }

    background: Rectangle {
        implicitWidth: 230
        implicitHeight: 32
        radius: 10
        color: Colors.popup
        border.color: Colors.borderStrong
    }

    enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 90 } }
    exit: Transition { NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 70 } }

    T.Overlay.modal: Rectangle { color: "transparent" }
    T.Overlay.modeless: Rectangle { color: "transparent" }
}
