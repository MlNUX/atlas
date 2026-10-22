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
    property var actions: null
    property var groups: []
    property int currentIndex: 0
    readonly property var currentGroup: groups.length > 0 ? groups[Math.min(currentIndex, groups.length - 1)] : null
    readonly property int terminalCount: groups.reduce((n, g) => n + g.terminals.length, 0)

    signal hideRequested()

    color: Theme.terminal
    radius: Theme.radius
    border.color: Theme.border
    clip: true

    Component {
        id: groupComponent
        TerminalGroup {}
    }

    /// New terminal tab, in `cwd` or the project root.
    function newTerminal(cwd) {
        const group = groupComponent.createObject(stack, { workingDirectory: cwd || panel.workingDirectory, actions: panel.actions })
        group.empty.connect(() => panel.removeGroup(group))
        groups = groups.concat([group])
        currentIndex = groups.length - 1
        group.addTerminal()
    }

    /// Runs `command` in a new terminal tab (closed when it exits).
    function runCommand(command, cwd) {
        const group = groupComponent.createObject(stack, { workingDirectory: cwd || panel.workingDirectory, actions: panel.actions })
        group.empty.connect(() => panel.removeGroup(group))
        groups = groups.concat([group])
        currentIndex = groups.length - 1
        group.addTerminal(cwd, command)
    }

    /// Splits the active terminal of the current tab: the new one goes
    /// beside it (`vertical` false) or below it.
    function splitTerminal(vertical) {
        if (!currentGroup) return newTerminal()
        currentGroup.splitActive(vertical)
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
            const group = groupComponent.createObject(stack, { workingDirectory: panel.workingDirectory, actions: panel.actions })
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
        anchors.margins: 1
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 38
            Layout.leftMargin: 8
            Layout.rightMargin: 8
            spacing: 4

            Repeater {
                model: panel.groups
                delegate: AbstractButton {
                    id: tab
                    required property var modelData
                    required property int index
                    readonly property bool current: index === panel.currentIndex
                    implicitHeight: 30
                    implicitWidth: Math.min(280, tabRow.implicitWidth + 24)
                    hoverEnabled: true
                    onClicked: { panel.currentIndex = index; panel.focusTerminal() }
                    background: Rectangle {
                        radius: 7
                        color: tab.current ? Theme.raised : tab.hovered ? Theme.hover : "transparent"
                        border.color: tab.current ? Theme.border : "transparent"
                    }
                    contentItem: Item {
                        Row {
                            id: tabRow
                            anchors.verticalCenter: parent.verticalCenter
                            x: 10
                            spacing: 8
                            Icon { name: "terminal"; size: 15; color: tab.current ? Theme.text : Theme.textMuted; anchors.verticalCenter: parent.verticalCenter }
                            Text {
                                text: tab.modelData.title + (tab.modelData.terminals.length > 1 ? "  ⫴" + tab.modelData.terminals.length : "")
                                color: tab.current ? Theme.textStrong : Theme.textMuted
                                font.pixelSize: Theme.fontSize
                                width: Math.min(implicitWidth, 200)
                                elide: Text.ElideRight
                                anchors.verticalCenter: parent.verticalCenter
                            }
                        }
                    }
                }
            }
            IconButton { iconName: "plus"; tip: qsTr("Neues Terminal (Strg+Umschalt+`)"); onClicked: panel.newTerminal() }

            Item { Layout.fillWidth: true }

            Label {
                visible: !!panel.currentGroup
                text: panel.currentGroup ? panel.currentGroup.title : ""
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
                Layout.maximumWidth: 220
                elide: Text.ElideMiddle
                Layout.rightMargin: 8
            }
            IconButton { iconName: "split"; tip: qsTr("Terminal rechts teilen"); onClicked: panel.splitTerminal(false) }
            IconButton { iconName: "split-vertical"; tip: qsTr("Terminal unten teilen"); onClicked: panel.splitTerminal(true) }
            IconButton { iconName: "trash"; tip: qsTr("Terminal beenden"); enabled: panel.terminalCount > 0; onClicked: panel.killTerminal() }
            IconButton { iconName: "chevron-down"; tip: qsTr("Panel ausblenden (Strg+`)"); onClicked: panel.hideRequested() }
        }
        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }

        StackLayout {
            id: stack
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: panel.currentIndex
        }
    }
}
