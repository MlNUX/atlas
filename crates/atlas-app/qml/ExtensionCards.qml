// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Cards of extensions in the „Aktionen“ tab of the context bar („In der
// Kontextleiste anzeigen“ in the settings): a header with the initial, the
// title and the description, then a row per view or command with its
// keyboard shortcut.
ColumnLayout {
    id: cards
    required property AppController app
    /// Extensions with a card: read again when the settings or the
    /// installed extensions change.
    readonly property var extensions: {
        app.contextModules; app.untrustedExtensions
        try { return JSON.parse(app.contextExtensions()).filter(e => e.shown && e.entries.length > 0) } catch (e) { return [] }
    }
    /// A view or command of an extension: {id, name} (see Main.runKeyAction).
    signal keyActionRequested(var binding)
    visible: extensions.length > 0
    spacing: 8

    Repeater {
        model: cards.extensions
        delegate: Rectangle {
            id: card
            required property var modelData
            Layout.fillWidth: true
            Layout.leftMargin: 10
            Layout.rightMargin: 10
            implicitHeight: column.implicitHeight
            radius: Theme.radius
            color: Theme.header
            border.color: cardHover.hovered ? Theme.borderStrong : Theme.border
            HoverHandler { id: cardHover }

            Column {
                id: column
                width: parent.width

                // ---- header: initial, title, description, hide button
                Item {
                    width: parent.width
                    height: Math.max(52, headerText.implicitHeight + 20)
                    Rectangle {
                        id: badge
                        x: 12
                        anchors.verticalCenter: parent.verticalCenter
                        width: 30
                        height: 30
                        radius: 8
                        color: Theme.accentSoft
                        Text {
                            anchors.centerIn: parent
                            text: (card.modelData.title || card.modelData.name || "?").charAt(0).toUpperCase()
                            color: Theme.accent
                            font.pixelSize: 15
                            font.bold: true
                        }
                    }
                    Column {
                        id: headerText
                        anchors.left: badge.right
                        anchors.leftMargin: 10
                        anchors.right: hide.left
                        anchors.rightMargin: 4
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 1
                        Text {
                            width: parent.width
                            text: card.modelData.title || card.modelData.name
                            textFormat: Text.PlainText
                            color: Theme.textStrong
                            font.pixelSize: Theme.fontSize
                            font.bold: true
                            elide: Text.ElideRight
                        }
                        Text {
                            visible: text !== ""
                            width: parent.width
                            text: card.modelData.description || ""
                            textFormat: Text.PlainText
                            color: Theme.textMuted
                            font.pixelSize: Theme.smallFontSize
                            wrapMode: Text.Wrap
                            maximumLineCount: 2
                            elide: Text.ElideRight
                        }
                    }
                    IconButton {
                        id: hide
                        anchors.right: parent.right
                        anchors.rightMargin: 6
                        anchors.verticalCenter: parent.verticalCenter
                        opacity: cardHover.hovered ? 1 : 0
                        Behavior on opacity { NumberAnimation { duration: 120 } }
                        iconName: "close"
                        iconSize: 12
                        tip: qsTr("Aus der Kontextleiste entfernen (Einstellungen → Kontextleiste)")
                        onClicked: cards.app.setContextModule("ext:" + card.modelData.name, 0)
                    }
                }
                Rectangle { x: 12; width: parent.width - 24; height: 1; color: Theme.border }

                // ---- views and commands
                Item { width: 1; height: 4 }
                Repeater {
                    model: card.modelData.entries
                    delegate: Item {
                        id: entry
                        required property var modelData
                        readonly property bool view: modelData.kind === "view"
                        width: column.width
                        height: 36
                        Rectangle {
                            anchors.fill: parent
                            anchors.leftMargin: 4
                            anchors.rightMargin: 4
                            radius: 7
                            color: entryHover.hovered ? Theme.hover : "transparent"
                        }
                        HoverHandler { id: entryHover }
                        Icon {
                            id: entryIcon
                            x: 18
                            anchors.verticalCenter: parent.verticalCenter
                            name: entry.view ? "external" : "play"
                            size: 15
                            color: Theme.accent
                        }
                        Text {
                            anchors.left: entryIcon.right
                            anchors.leftMargin: 10
                            anchors.right: key.visible ? key.left : parent.right
                            anchors.rightMargin: 10
                            anchors.verticalCenter: parent.verticalCenter
                            text: entry.modelData.name
                            textFormat: Text.PlainText
                            color: entryHover.hovered ? Theme.textStrong : Theme.text
                            font.pixelSize: Theme.fontSize
                            elide: Text.ElideRight
                        }
                        // Keyboard shortcut as a small key cap.
                        Rectangle {
                            id: key
                            visible: !!entry.modelData.keybinding
                            anchors.right: parent.right
                            anchors.rightMargin: 14
                            anchors.verticalCenter: parent.verticalCenter
                            width: keyText.implicitWidth + 10
                            height: 18
                            radius: 4
                            color: Theme.raised
                            border.color: Theme.border
                            Text {
                                id: keyText
                                anchors.centerIn: parent
                                text: entry.modelData.keybinding || ""
                                color: Theme.textMuted
                                font.pixelSize: Theme.smallFontSize - 1
                                font.family: Theme.monoFont
                            }
                        }
                        ToolTip.visible: entryHover.hovered
                        ToolTip.delay: 600
                        ToolTip.text: entry.view ? qsTr("Als Tab öffnen") : qsTr("Ausführen")
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: cards.keyActionRequested({ id: entry.modelData.id, name: entry.modelData.name })
                        }
                    }
                }
                Item { width: 1; height: 4 }
            }
        }
    }
}
