// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Templates as T

// Card on a dimmed window: title, content, buttons on the right.
T.Dialog {
    id: control
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, contentWidth + leftPadding + rightPadding, implicitHeaderWidth, implicitFooterWidth)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             contentHeight + topPadding + bottomPadding
                             + (implicitHeaderHeight > 0 ? implicitHeaderHeight + spacing : 0)
                             + (implicitFooterHeight > 0 ? implicitFooterHeight + spacing : 0))
    padding: 20
    topPadding: control.title ? 6 : 20
    spacing: 4
    font.pixelSize: Colors.fontSize

    background: Rectangle {
        radius: 14
        color: Colors.surface
        border.color: Colors.borderStrong
    }

    header: Text {
        text: control.title
        visible: control.title
        elide: Text.ElideRight
        font.pixelSize: 16
        font.weight: Font.DemiBold
        color: Colors.textStrong
        leftPadding: 20
        rightPadding: 20
        topPadding: 18
        bottomPadding: 6
    }

    footer: DialogButtonBox {
        visible: count > 0
    }

    enter: Transition {
        NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 120; easing.type: Easing.OutCubic }
        NumberAnimation { property: "scale"; from: 0.97; to: 1; duration: 120; easing.type: Easing.OutCubic }
    }
    exit: Transition { NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 80 } }

    T.Overlay.modal: Rectangle { color: Colors.dim }
    T.Overlay.modeless: Rectangle { color: Qt.rgba(0, 0, 0, 0.15) }
}
