// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Session dropdown in the title bar: shows the active session; switch,
// create, duplicate; rename, save and delete in an overflow menu.
AbstractButton {
    id: root
    required property AppController app

    signal switchRequested(string id)
    signal createRequested(string name, bool duplicate)
    signal saveRequested()
    signal deleteRequested(string id)

    /// Asks for the name of a new, empty session.
    function askNew() { nameDialog.ask(qsTr("Neue Session"), "", false) }

    enabled: !!app.projectRoot && app.persistSession
    hoverEnabled: true
    implicitHeight: 26
    implicitWidth: Math.min(260, label.implicitWidth + 28)
    onClicked: menu.popup(0, height)

    background: Rectangle {
        radius: 4
        color: root.down || menu.visible ? "#3c3f41" : root.hovered ? Theme.hover : "transparent"
        border.color: Theme.border
    }
    contentItem: Text {
        id: label
        leftPadding: 8
        text: "◷  " + (root.app.activeSessionName || qsTr("Session")) + "  ▾"
        color: root.enabled ? Theme.text : Theme.textFaint
        font.pixelSize: Theme.smallFontSize
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
    ToolTip.visible: hovered && !menu.visible
    ToolTip.delay: 600
    ToolTip.text: qsTr("Session wechseln oder verwalten")

    Menu {
        id: menu
        width: 260

        Repeater {
            model: root.app.sessionIds.length
            delegate: MenuItem {
                required property int index
                readonly property string sessionId: root.app.sessionIds[index]
                text: root.app.sessionNames[index]
                checkable: true
                checked: sessionId === root.app.activeSession
                onTriggered: root.switchRequested(sessionId)
            }
        }
        MenuSeparator {}
        MenuItem { text: qsTr("Neue Session …"); onTriggered: nameDialog.ask(qsTr("Neue Session"), "", false) }
        MenuItem {
            text: qsTr("Session duplizieren …")
            onTriggered: nameDialog.ask(qsTr("Session duplizieren"), qsTr("%1 (Kopie)").arg(root.app.activeSessionName), true)
        }
        Menu {
            title: qsTr("Weitere Aktionen")
            MenuItem {
                text: qsTr("Umbenennen …")
                onTriggered: nameDialog.ask(qsTr("Session umbenennen"), root.app.activeSessionName, false, true)
            }
            MenuItem { text: qsTr("Jetzt speichern"); onTriggered: root.saveRequested() }
            MenuSeparator {}
            MenuItem {
                text: qsTr("Session löschen …")
                onTriggered: confirm.open()
            }
        }
    }

    Dialog {
        id: nameDialog
        property bool duplicate: false
        property bool rename: false
        anchors.centerIn: Overlay.overlay
        parent: Overlay.overlay
        width: 380
        modal: true
        standardButtons: Dialog.Ok | Dialog.Cancel

        function ask(title, initial, duplicate, rename) {
            nameDialog.title = title
            nameDialog.duplicate = !!duplicate
            nameDialog.rename = !!rename
            field.text = initial
            open()
            field.forceActiveFocus()
            field.selectAll()
        }
        onAccepted: {
            const name = field.text.trim()
            if (!name) return
            if (rename) root.app.renameSession(root.app.activeSession, name)
            else root.createRequested(name, duplicate)
        }
        ColumnLayout {
            anchors.fill: parent
            Label {
                text: nameDialog.duplicate ? qsTr("Übernimmt Tabs, Splits und Positionen der aktuellen Session.")
                    : nameDialog.rename ? "" : qsTr("Beginnt mit einer leeren Arbeitsanordnung.")
                visible: text.length > 0
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            TextField {
                id: field
                Layout.fillWidth: true
                placeholderText: qsTr("Name der Session")
                onAccepted: nameDialog.accept()
            }
        }
    }

    Dialog {
        id: confirm
        anchors.centerIn: Overlay.overlay
        parent: Overlay.overlay
        width: 400
        modal: true
        title: qsTr("Session löschen?")
        standardButtons: Dialog.Yes | Dialog.No
        Label {
            width: parent.width
            wrapMode: Text.Wrap
            text: root.app.sessionIds.length > 1
                ? qsTr("„%1“ wird gelöscht. Atlas wechselt vorher zu einer anderen Session. Projektdateien bleiben unverändert.").arg(root.app.activeSessionName)
                : qsTr("„%1“ ist die einzige Session. Atlas legt vorher eine neue, leere Session an. Projektdateien bleiben unverändert.").arg(root.app.activeSessionName)
        }
        onAccepted: root.deleteRequested(root.app.activeSession)
    }
}
