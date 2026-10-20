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

    // Size from the entries themselves: the list only estimates heights of
    // entries it has not created yet, and is not as wide as its widest one.
    function entriesSize() {
        let w = 0, h = 0
        for (let i = 0; i < control.count; ++i) {
            const item = control.itemAt(i)
            if (!item || !item.visible) continue
            w = Math.max(w, item.implicitWidth)
            h += item.implicitHeight
        }
        return Qt.size(w, h)
    }

    contentItem: ListView {
        readonly property size entries: control.count >= 0 ? control.entriesSize() : Qt.size(0, 0)
        implicitWidth: entries.width
        implicitHeight: Math.max(contentHeight, entries.height)
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
