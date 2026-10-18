// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Large temporary preview (Space): no tab, no split. Arrow keys move through
// the list it was opened from, Enter opens a permanent tab, Space/Esc close.
Popup {
    id: root
    required property AppController app
    property var actions
    property string path
    /// `step => path` provided by the list the preview was opened from.
    property var navigate: null

    signal openRequested(string path)

    function show(path, navigate) {
        root.path = path
        root.navigate = navigate
        open()
        view.forceActiveFocus()
    }
    function step(delta) {
        if (!navigate) return
        const next = navigate(delta)
        if (next) root.path = next
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: parent ? parent.width * 0.86 : 800
    height: parent ? parent.height * 0.88 : 600
    modal: true
    focus: true
    padding: 0
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    background: Rectangle { color: Theme.editor; border.color: Theme.border; radius: 8 }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 34
            color: Theme.window
            radius: 8
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 8
                Label {
                    text: root.app.fileName(root.path)
                    color: Theme.textStrong
                    font.bold: true
                }
                Label {
                    Layout.fillWidth: true
                    text: root.app.relativePath(root.path)
                    color: Theme.textFaint
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideMiddle
                }
                Label {
                    text: qsTr("←/→ blättern · Enter öffnen · Leertaste schließen")
                    color: Theme.textFaint
                    font.pixelSize: Theme.smallFontSize
                }
                IconButton { glyph: "↗"; tip: qsTr("Als Tab öffnen (Enter)"); onClicked: { root.openRequested(root.path); root.close() } }
                IconButton { glyph: "✕"; tip: qsTr("Schließen"); onClicked: root.close() }
            }
        }
        DocumentView {
            id: view
            Layout.fillWidth: true
            Layout.fillHeight: true
            app: root.app
            actions: root.actions
            path: root.visible ? root.path : ""
            focus: true
            onOpenRequested: (p) => { root.openRequested(p); root.close() }
            // Keys go to the preview first; the shortcuts below only see
            // what it does not use.
            Keys.priority: Keys.AfterItem
            Keys.onPressed: (event) => {
                if (event.key === Qt.Key_Space || event.key === Qt.Key_Escape) root.close()
                else if (event.key === Qt.Key_Right || event.key === Qt.Key_Down) root.step(1)
                else if (event.key === Qt.Key_Left || event.key === Qt.Key_Up) root.step(-1)
                else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) { root.openRequested(root.path); root.close() }
                else return
                event.accepted = true
            }
        }
    }
}
