// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Bottom terminal panel: tabs ("groups"), each with split terminals.
// Processes keep running while the panel is hidden.
Rectangle {
    id: panel
    property string workingDirectory
    property var groups: []
    property int currentIndex: 0
    readonly property var currentGroup: groups.length > 0 ? groups[Math.min(currentIndex, groups.length - 1)] : null
    readonly property int terminalCount: groups.reduce((n, g) => n + g.terminals.length, 0)

    signal hideRequested()

    color: Theme.panel

    Component {
        id: groupComponent
        TerminalGroup {}
    }

    function newTerminal() {
        const group = groupComponent.createObject(stack, { workingDirectory: panel.workingDirectory })
        group.empty.connect(() => panel.removeGroup(group))
        groups = groups.concat([group])
        currentIndex = groups.length - 1
        group.addTerminal()
    }

    function splitTerminal() {
        if (!currentGroup) return newTerminal()
        currentGroup.addTerminal()
    }

    function killTerminal() {
        if (currentGroup && currentGroup.activeTerminal) currentGroup.removeTerminal(currentGroup.activeTerminal)
    }

    function removeGroup(group) {
        const index = groups.indexOf(group)
        if (index < 0) return
        const list = groups.slice()
        list.splice(index, 1)
        groups = list
        group.destroy()
        currentIndex = Math.max(0, Math.min(currentIndex, groups.length - 1))
        if (groups.length === 0) panel.hideRequested()
    }

    /// Layout for the session: terminals per tab and their folders.
    function serialize() {
        return { current: currentIndex, groups: groups.map(g => g.serialize()) }
    }

    /// Recreates the saved layout (processes are started fresh).
    function restore(state) {
        const saved = state && state.groups && state.groups.length ? state.groups : [{ terminals: [{}] }]
        for (const gs of saved) {
            const group = groupComponent.createObject(stack, { workingDirectory: panel.workingDirectory })
            group.empty.connect(() => panel.removeGroup(group))
            groups = groups.concat([group])
            group.restore(gs)
        }
        currentIndex = Math.max(0, Math.min(state && state.current || 0, groups.length - 1))
    }

    function focusTerminal() {
        if (!currentGroup) newTerminal()
        else if (currentGroup.activeTerminal) currentGroup.activeTerminal.focusTerminal()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }

        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 30
            Layout.leftMargin: 12
            Layout.rightMargin: 6
            spacing: 2

            Label {
                text: qsTr("TERMINAL")
                color: Theme.textStrong
                font.pixelSize: Theme.smallFontSize
                font.bold: true
                Layout.rightMargin: 12
            }

            Repeater {
                model: panel.groups
                delegate: AbstractButton {
                    id: tab
                    required property var modelData
                    required property int index
                    implicitHeight: 24
                    implicitWidth: Math.min(260, tabLabel.implicitWidth + 24)
                    hoverEnabled: true
                    onClicked: { panel.currentIndex = index; panel.focusTerminal() }
                    background: Rectangle {
                        radius: 4
                        color: tab.index === panel.currentIndex ? Theme.selection : tab.hovered ? Theme.hover : "transparent"
                    }
                    contentItem: Text {
                        id: tabLabel
                        text: "›_ " + tab.modelData.title + (tab.modelData.terminals.length > 1 ? "  ⫴" + tab.modelData.terminals.length : "")
                        color: tab.index === panel.currentIndex ? Theme.textStrong : Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                        elide: Text.ElideRight
                    }
                }
            }

            Item { Layout.fillWidth: true }

            IconButton { glyph: "+"; tip: qsTr("Neues Terminal (Strg+Umschalt+`)"); onClicked: panel.newTerminal() }
            IconButton { glyph: "◫"; tip: qsTr("Terminal teilen"); onClicked: panel.splitTerminal() }
            IconButton { glyph: "🗑"; tip: qsTr("Terminal beenden"); enabled: panel.terminalCount > 0; onClicked: panel.killTerminal() }
            IconButton { glyph: "⌄"; tip: qsTr("Panel ausblenden (Strg+`)"); onClicked: panel.hideRequested() }
        }

        StackLayout {
            id: stack
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: panel.currentIndex
        }
    }
}
