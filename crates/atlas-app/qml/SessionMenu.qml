// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Session button in the title bar ("Sitzung: …") and its panel. A session
// is a project folder (its tabs, splits and tree): search, all sessions
// with their folder and size, switch with a click; new (choose a folder),
// rename, save and remove from the list.
AbstractButton {
    id: root
    required property AppController app
    /// Tabs and splits of the active session as shown right now.
    property int activeTabs: 0
    property int activeSplits: 0

    /// Another session: open its project folder.
    signal openRequested(string projectRoot)
    /// A new session: Main asks for the project folder (and a name).
    signal newSessionRequested(string name)
    signal saveRequested()

    /// Asks for the project folder and name of a new session.
    function askNew() { root.newSessionRequested("") }

    enabled: !!app.projectRoot && app.persistSession
    hoverEnabled: true
    implicitHeight: 34
    implicitWidth: Math.min(280, row.implicitWidth + 24)
    onClicked: panel.visible ? panel.close() : panel.open()

    background: Rectangle {
        radius: 8
        color: root.down || panel.visible ? Theme.selection : root.hovered ? Theme.hover : Theme.header
        border.color: panel.visible ? Qt.rgba(0.24, 0.55, 0.99, 0.5) : Theme.border
    }
    contentItem: Item {
        implicitWidth: row.implicitWidth
        Row {
            id: row
            anchors.centerIn: parent
            spacing: 8
            Icon { name: "session"; size: 16; color: Theme.textMuted; anchors.verticalCenter: parent.verticalCenter }
            Text {
                id: label
                text: qsTr("Sitzung: %1").arg(root.app.activeSessionName || "–")
                color: root.enabled ? Theme.text : Theme.textFaint
                font.pixelSize: Theme.fontSize
                elide: Text.ElideRight
                width: Math.min(implicitWidth, 200)
                anchors.verticalCenter: parent.verticalCenter
            }
            Icon { name: panel.visible ? "chevron-left" : "chevron-down"; size: 14; color: Theme.textMuted; anchors.verticalCenter: parent.verticalCenter; rotation: panel.visible ? 90 : 0 }
        }
    }
    ToolTip.visible: hovered && !panel.visible
    ToolTip.delay: 600
    ToolTip.text: qsTr("Session wechseln oder verwalten")

    // ------------------------------------------------------------- panel

    function plural(n, one, many) { return n + " " + (n === 1 ? one : many) }
    function iconFor(kind) {
        return ({ pdf: "file-pdf", notes: "file-text", code: "file-code", terminal: "terminal", calendar: "calendar", mixed: "split", empty: "plus" })[kind] || "document"
    }
    function savedText(unix) {
        if (!unix) return ""
        const d = new Date(unix * 1000), now = new Date()
        const pad = (n) => (n < 10 ? "0" : "") + n
        if (d.toDateString() === now.toDateString()) return qsTr("Gespeichert %1").arg(pad(d.getHours()) + ":" + pad(d.getMinutes()))
        return qsTr("Gespeichert %1").arg(d.getDate() + "." + (d.getMonth() + 1) + ".")
    }

    property var sessions: []
    function isActive(s) { return s.root === app.projectRoot }
    function shortPath(path) {
        const home = app.placePaths.length ? app.placePaths[0] : ""
        return home && path.startsWith(home + "/") ? "~" + path.slice(home.length) : path
    }
    readonly property var shown: {
        const q = search.text.trim().toLowerCase()
        return sessions.filter(s => !q || s.name.toLowerCase().indexOf(q) >= 0 || s.root.toLowerCase().indexOf(q) >= 0)
    }

    Popup {
        id: panel
        parent: root
        x: root.width - width
        y: root.height + 6
        width: 470
        padding: 16
        focus: true
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutsideParent
        onAboutToShow: {
            search.text = ""
            try { root.sessions = JSON.parse(root.app.allSessions()) } catch (e) { root.sessions = [] }
            list.currentIndex = Math.max(0, root.shown.findIndex(s => root.isActive(s)))
            Qt.callLater(search.forceActiveFocus)
        }
        function pick(s) {
            panel.close()
            if (s && !root.isActive(s)) root.openRequested(s.root)
        }

        contentItem: ColumnLayout {
            spacing: 12
            Label {
                text: qsTr("Sessions")
                color: Theme.textStrong
                font.pixelSize: 19
                font.weight: Font.DemiBold
            }
            TextField {
                id: search
                Layout.fillWidth: true
                implicitHeight: 38
                leftPadding: 38
                placeholderText: qsTr("Sessions durchsuchen …")
                Icon { name: "search"; size: 17; x: 12; anchors.verticalCenter: parent.verticalCenter }
                Keys.onDownPressed: list.incrementCurrentIndex()
                Keys.onUpPressed: list.decrementCurrentIndex()
                onTextEdited: list.currentIndex = 0
                onAccepted: panel.pick(root.shown[list.currentIndex])
            }
            ListView {
                id: list
                Layout.fillWidth: true
                Layout.preferredHeight: Math.min(contentHeight, 6 * 62)
                clip: true
                spacing: 2
                model: root.shown
                boundsBehavior: Flickable.StopAtBounds
                highlightMoveDuration: 0
                ScrollBar.vertical: ScrollBar {}
                delegate: Rectangle {
                    id: entry
                    required property var modelData
                    required property int index
                    readonly property bool active: root.isActive(modelData)
                    // The active session counts what is open right now.
                    readonly property int tabs: active ? root.activeTabs : modelData.tabs
                    readonly property int splits: active ? root.activeSplits : modelData.splits
                    width: ListView.view.width
                    height: 60
                    radius: 8
                    color: active ? Theme.activeSelection : entryHover.hovered || ListView.isCurrentItem ? Theme.hover : "transparent"
                    HoverHandler { id: entryHover }
                    MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: panel.pick(entry.modelData) }
                    Icon {
                        visible: entry.active
                        x: 12
                        anchors.verticalCenter: parent.verticalCenter
                        name: "check"
                        size: 18
                        color: Theme.textStrong
                    }
                    Icon {
                        x: 42
                        anchors.verticalCenter: parent.verticalCenter
                        name: root.iconFor(entry.tabs === 0 ? "empty" : entry.modelData.kind)
                        size: 22
                        color: entry.active ? Theme.textStrong : Theme.text
                    }
                    Column {
                        x: 80
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - x - rightPart.width - 16
                        spacing: 3
                        Text {
                            width: parent.width
                            text: entry.modelData.name
                            color: Theme.textStrong
                            font.pixelSize: 14
                            font.weight: entry.active ? Font.DemiBold : Font.Normal
                            elide: Text.ElideRight
                        }
                        Text {
                            width: parent.width
                            text: root.shortPath(entry.modelData.root) + "  ·  " + root.plural(entry.tabs, qsTr("Tab"), qsTr("Tabs"))
                                + "  ·  " + root.plural(entry.splits, qsTr("Split"), qsTr("Splits"))
                            color: entry.active ? Theme.text : Theme.textMuted
                            font.pixelSize: Theme.smallFontSize + 1
                            elide: Text.ElideMiddle
                        }
                    }
                    Row {
                        id: rightPart
                        anchors.right: parent.right
                        anchors.rightMargin: 10
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 4
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: entry.active || !entryHover.hovered
                            text: entry.active ? qsTr("Automatisch gespeichert") : root.savedText(entry.modelData.saved)
                            color: entry.active ? Theme.text : Theme.textFaint
                            font.pixelSize: Theme.smallFontSize + 1
                        }
                        IconButton {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: entryHover.hovered
                            iconName: "more"
                            tip: entry.active ? qsTr("Umbenennen") : qsTr("Umbenennen oder entfernen")
                            onClicked: rowMenu.openFor(entry.modelData, this)
                        }
                    }
                }
                Label {
                    anchors.centerIn: parent
                    visible: list.count === 0
                    text: qsTr("Keine Session gefunden")
                    color: Theme.textFaint
                }
            }
            Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                PanelAction {
                    icon: "plus"
                    text: qsTr("Neue Session")
                    onTriggered: { panel.close(); root.newSessionRequested(search.text.trim()) }
                }
            }
        }
    }

    component PanelAction: Rectangle {
        id: action
        property string icon
        property string text
        signal triggered()
        Layout.fillWidth: true
        implicitHeight: 40
        radius: 8
        color: actionHover.hovered ? Theme.hover : "transparent"
        HoverHandler { id: actionHover }
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: action.triggered() }
        Row {
            x: 12
            anchors.verticalCenter: parent.verticalCenter
            spacing: 16
            Icon { name: action.icon; size: 20; color: Theme.text; anchors.verticalCenter: parent.verticalCenter }
            Text { text: action.text; color: Theme.textStrong; font.pixelSize: 14; anchors.verticalCenter: parent.verticalCenter }
        }
    }

    // A session (⋮ in its row): rename; other sessions also remove.
    Menu {
        id: rowMenu
        property var target: null
        function openFor(s, button) { target = s; popup(button, 0, button.height) }
        MenuItem { text: qsTr("Umbenennen …"); onTriggered: { panel.close(); nameDialog.ask(rowMenu.target.root, rowMenu.target.name) } }
        MenuItem {
            text: qsTr("Aus der Liste entfernen …")
            visible: !!rowMenu.target && rowMenu.target.root !== root.app.projectRoot
            height: visible ? implicitHeight : 0
            onTriggered: { panel.close(); confirm.ask(rowMenu.target.root, rowMenu.target.name) }
        }
    }

    Dialog {
        id: nameDialog
        /// Project folder of the session.
        property string targetRoot
        anchors.centerIn: Overlay.overlay
        parent: Overlay.overlay
        width: 400
        modal: true
        title: qsTr("Session umbenennen")
        standardButtons: Dialog.Ok | Dialog.Cancel

        function ask(projectRoot, initial) {
            targetRoot = projectRoot
            field.text = initial
            open()
            field.forceActiveFocus()
            field.selectAll()
        }
        onAccepted: {
            const name = field.text.trim()
            if (name) root.app.renameProjectSession(targetRoot, name)
        }
        ColumnLayout {
            anchors.fill: parent
            spacing: 10
            Label {
                text: root.shortPath(nameDialog.targetRoot)
                color: Theme.textMuted
                elide: Text.ElideMiddle
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
        property string targetRoot
        property string targetName
        anchors.centerIn: Overlay.overlay
        parent: Overlay.overlay
        width: 420
        modal: true
        title: qsTr("Session entfernen?")
        function ask(projectRoot, name) { targetRoot = projectRoot; targetName = name; open() }
        Label {
            width: parent.width
            wrapMode: Text.Wrap
            color: Theme.text
            text: qsTr("„%1“ verschwindet aus der Liste, ihre Tabs und Anordnung werden vergessen. Der Ordner %2 und seine Dateien bleiben unverändert.")
                .arg(confirm.targetName).arg(root.shortPath(confirm.targetRoot))
        }
        footer: DialogButtonBox {
            Button { text: qsTr("Abbrechen"); flat: true; DialogButtonBox.buttonRole: DialogButtonBox.RejectRole }
            Button { text: qsTr("Entfernen"); DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole }
        }
        onAccepted: root.app.forgetProject(targetRoot)
    }
}
