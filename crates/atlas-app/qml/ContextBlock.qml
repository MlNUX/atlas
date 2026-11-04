// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// A block of the context bar (see ContextProfiles.qml): a header with an
// arrow to collapse it, icon, name, a count and – on hover – a ⋯ menu;
// below it the content. Click on the header collapses; with the keyboard:
// Tab reaches the headers, Space collapses, Alt+↑/↓ move, Shift+F10 opens
// the menu.
Rectangle {
    id: block
    required property string blockId
    /// {name, icon, letter?} from ContextProfiles.info().
    required property var info
    /// The content (its implicitHeight counts).
    property Component content: null
    /// Next to the name: a number, a date …; "leer" for empty blocks.
    property string count: ""
    property bool empty: false
    property bool collapsed: false
    /// Where it stands: "overview", "pinned" or a tab (page).
    property string area: "overview"
    readonly property bool inPage: area !== "overview" && area !== "pinned"
    /// Areas it can move to: [{id, title}] (overview and tabs).
    property var targets: []
    property bool canMoveUp: false
    property bool canMoveDown: false
    /// Small buttons in the header (left of ⋯).
    property alias tools: toolRow.data
    /// Name of the profile changes go to, and whether that is the file
    /// type's own profile (for „Gilt für“).
    property string profileTitle: ""
    property bool ownProfile: false
    property bool canChooseScope: false

    signal toggleCollapsed()
    signal moveBy(int delta)
    signal toTab()
    signal toPinned()
    signal toOverview()
    signal hide()
    signal customize()
    /// true: only the file type's profile, false: „Allgemein“.
    signal scopeChosen(bool own)
    /// Move to area `target` ("" = a new tab).
    signal moveTo(string target)

    readonly property bool open: !collapsed
    readonly property alias header: header
    readonly property alias menu: menu

    Layout.fillWidth: true
    Layout.leftMargin: 10
    Layout.rightMargin: 10
    implicitHeight: header.height + (open && loader.item ? loader.item.implicitHeight + 8 : 0)
    radius: 10
    color: Theme.header
    border.color: header.activeFocus ? Theme.accent : Theme.border

    function openMenu() { menu.popup(more, 0, more.height) }

    Item {
        id: header
        width: parent.width
        height: 38
        activeFocusOnTab: true
        Accessible.role: Accessible.Button
        Accessible.name: block.info.name
        HoverHandler { id: headerHover }
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            cursorShape: Qt.PointingHandCursor
            onClicked: (mouse) => {
                if (mouse.button === Qt.RightButton) block.openMenu()
                else block.toggleCollapsed()
            }
        }
        Keys.onPressed: (event) => {
            if (event.key === Qt.Key_Space || event.key === Qt.Key_Return) { block.toggleCollapsed(); event.accepted = true }
            else if ((event.modifiers & Qt.AltModifier) && event.key === Qt.Key_Up) { block.moveBy(-1); event.accepted = true }
            else if ((event.modifiers & Qt.AltModifier) && event.key === Qt.Key_Down) { block.moveBy(1); event.accepted = true }
            else if (((event.modifiers & Qt.ShiftModifier) && event.key === Qt.Key_F10) || event.key === Qt.Key_Menu) { block.openMenu(); event.accepted = true }
        }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 8
            anchors.rightMargin: 6
            spacing: 6
            Icon { name: block.open ? "chevron-down" : "chevron-right"; size: 12; color: Theme.textMuted }
            Rectangle {
                visible: !!block.info.letter
                Layout.preferredWidth: 18
                Layout.preferredHeight: 18
                radius: 5
                color: Theme.accentSoft
                Text { anchors.centerIn: parent; text: block.info.letter || ""; color: Theme.accent; font.pixelSize: 11; font.bold: true }
            }
            Icon { visible: !block.info.letter; name: block.info.icon || "grid"; size: 14; color: Theme.accent }
            Text {
                Layout.fillWidth: true
                text: block.info.name
                textFormat: Text.PlainText
                color: Theme.textStrong
                font.pixelSize: Theme.fontSize
                font.bold: true
                elide: Text.ElideRight
            }
            Text {
                visible: text !== ""
                text: block.empty ? qsTr("leer") : block.count
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
            }
            Row { id: toolRow; spacing: 0 }
            IconButton {
                id: more
                iconName: "more-horizontal"
                iconSize: 14
                tip: qsTr("Baustein: einklappen, verschieben, ausblenden …")
                opacity: headerHover.hovered || menu.visible || header.activeFocus ? 1 : 0
                Behavior on opacity { NumberAnimation { duration: 120 } }
                onClicked: block.openMenu()
            }
        }
    }

    Loader {
        id: loader
        y: header.height
        width: parent.width
        active: block.open && !!block.content
        sourceComponent: block.content
    }

    Menu {
        id: menu
        MenuItem { text: block.open ? qsTr("Einklappen") : qsTr("Ausklappen"); onTriggered: block.toggleCollapsed() }
        MenuItem { text: qsTr("Nach oben (Alt+↑)"); enabled: block.canMoveUp; onTriggered: block.moveBy(-1) }
        MenuItem { text: qsTr("Nach unten (Alt+↓)"); enabled: block.canMoveDown; onTriggered: block.moveBy(1) }
        MenuSeparator {}
        Menu {
            id: moveMenu
            title: qsTr("Verschieben nach")
            Instantiator {
                model: block.targets
                delegate: MenuItem {
                    required property var modelData
                    text: modelData.title
                    onTriggered: block.moveTo(modelData.id)
                }
                onObjectAdded: (index, object) => moveMenu.insertItem(index, object)
                onObjectRemoved: (index, object) => moveMenu.removeItem(object)
            }
            MenuSeparator {}
            MenuItem { text: qsTr("Neuer Reiter …"); onTriggered: block.moveTo("") }
        }
        MenuItem {
            text: qsTr("Als eigenen Reiter zeigen")
            visible: !block.inPage && !!(ContextProfiles.catalog[block.blockId] || {}).tab
            height: visible ? implicitHeight : 0
            onTriggered: block.toTab()
        }
        MenuItem {
            text: block.area === "pinned" ? qsTr("In die Übersicht") : qsTr("Unten anheften")
            visible: block.area === "pinned" || (!block.inPage && !!(ContextProfiles.catalog[block.blockId] || {}).pin)
            height: visible ? implicitHeight : 0
            onTriggered: block.area === "pinned" ? block.toOverview() : block.toPinned()
        }
        MenuSeparator { visible: block.canChooseScope; height: visible ? implicitHeight : 0 }
        MenuItem {
            text: qsTr("Gilt für: nur %1").arg(block.profileTitle)
            visible: block.canChooseScope
            height: visible ? implicitHeight : 0
            checkable: true
            checked: block.ownProfile
            onTriggered: block.scopeChosen(true)
        }
        MenuItem {
            text: qsTr("Gilt für: alle Dateitypen")
            visible: block.canChooseScope
            height: visible ? implicitHeight : 0
            checkable: true
            checked: !block.ownProfile
            onTriggered: block.scopeChosen(false)
        }
        MenuSeparator {}
        MenuItem { text: block.inPage ? qsTr("Aus dem Reiter entfernen") : qsTr("Ausblenden"); onTriggered: block.hide() }
        MenuItem { text: qsTr("Leiste anpassen …"); onTriggered: block.customize() }
    }
}
