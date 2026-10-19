// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Templates as T

T.Popup {
    id: control
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, contentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, contentHeight + topPadding + bottomPadding)
    padding: 12
    font.pixelSize: Colors.fontSize

    background: Rectangle {
        radius: 12
        color: Colors.popup
        border.color: Colors.borderStrong
    }

    T.Overlay.modal: Rectangle { color: Colors.dim }
    T.Overlay.modeless: Rectangle { color: "transparent" }
}
