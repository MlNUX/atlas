// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Actions for the text selected in a PDF (mark, quote, copy, formula).
Popup {
    id: selectionMenu
    required property var viewer
    required property var annotationPopup
    padding: 4
    closePolicy: Popup.CloseOnEscape
    background: Rectangle { color: Theme.header; border.color: Theme.border; radius: 6 }
    function showAt(pos) {
        x = Math.max(4, Math.min(viewer.width - implicitWidth - 4, pos.x - implicitWidth / 2))
        y = Math.max(4, Math.min(viewer.height - implicitHeight - 4, pos.y + 14))
        open()
    }
    // Waiting for the selection result of the release is fine: the menu
    // only acts on what is selected when a button is pressed.
    RowLayout {
        spacing: 2
        Repeater {
            model: viewer.markColors
            AbstractButton {
                required property string modelData
                implicitWidth: 20
                implicitHeight: 22
                hoverEnabled: true
                focusPolicy: Qt.NoFocus
                contentItem: Rectangle { anchors.centerIn: parent; width: 14; height: 14; radius: 7; color: parent.modelData; border.color: parent.hovered ? "white" : "transparent" }
                background: null
                ToolTip.visible: hovered
                ToolTip.delay: 600
                ToolTip.text: qsTr("Markieren (Strg+H)")
                onClicked: viewer.markSelection("highlight", modelData)
            }
        }
        IconButton { glyph: "U̲"; tip: qsTr("Unterstreichen"); onClicked: viewer.markSelection("underline", viewer.markColor) }
        IconButton { glyph: "S̶"; tip: qsTr("Durchstreichen"); onClicked: viewer.markSelection("strikeout", "#ff6b6b") }
        IconButton {
            glyph: "💬"
            tip: qsTr("Markieren und kommentieren")
            onClicked: {
                const pos = Qt.point(selectionMenu.x + selectionMenu.width / 2, selectionMenu.y)
                const id = viewer.markSelection("highlight", viewer.markColor)
                if (id) annotationPopup.show(id, pos, true)
            }
        }
        IconButton { glyph: qsTr("❝ Zitat"); tip: qsTr("Markieren und als Zitat in eine Notiz übernehmen (Strg+Q)"); onClicked: viewer.quoteSelection() }
        IconButton { glyph: "⧉"; tip: qsTr("Text kopieren (Strg+C)"); onClicked: { viewer.copySelection(); selectionMenu.close() } }
        IconButton { glyph: "∑"; tip: qsTr("Als LaTeX erkennen und kopieren"); onClicked: { viewer.recognizeSelection(); selectionMenu.close() } }
    }
}
