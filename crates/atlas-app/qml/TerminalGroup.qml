// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Atlas

// One terminal tab: one or more terminals side by side (VS Code "split").
SplitView {
    id: group
    property string workingDirectory
    property var terminals: []
    property int activeIndex: 0
    readonly property var activeTerminal: terminals.length > 0 ? terminals[Math.min(activeIndex, terminals.length - 1)] : null
    readonly property string title: activeTerminal && activeTerminal.title ? activeTerminal.title : shellName()

    signal empty()

    orientation: Qt.Horizontal
    handle: Rectangle {
        implicitWidth: 3
        color: SplitHandle.pressed ? Theme.accent : Theme.border
    }

    Component {
        id: terminalComponent
        TerminalPane {
            SplitView.preferredWidth: group.width / Math.max(1, group.terminals.length + 1)
            SplitView.minimumWidth: 120
        }
    }

    function shellName() {
        return "terminal"
    }

    function addTerminal() {
        const pane = terminalComponent.createObject(null, { workingDirectory: group.workingDirectory })
        group.addItem(pane)
        terminals = terminals.concat([pane])
        pane.closed.connect(() => group.removeTerminal(pane))
        pane.activated.connect(() => { group.activeIndex = group.terminals.indexOf(pane) })
        activeIndex = terminals.length - 1
        Qt.callLater(pane.focusTerminal)
        return pane
    }

    function removeTerminal(pane) {
        const index = terminals.indexOf(pane)
        if (index < 0) return
        group.removeItem(pane)
        const list = terminals.slice()
        list.splice(index, 1)
        terminals = list
        pane.destroy()
        if (terminals.length === 0) {
            group.empty()
            return
        }
        activeIndex = Math.min(index, terminals.length - 1)
        Qt.callLater(() => { if (group.activeTerminal) group.activeTerminal.focusTerminal() })
    }

    onActiveIndexChanged: updateActive()
    onTerminalsChanged: updateActive()

    function updateActive() {
        for (let i = 0; i < terminals.length; ++i) terminals[i].active = (i === activeIndex && terminals.length > 1)
    }
}
