// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Atlas

// One terminal tab: one or more terminals, split like in VS Code. Splitting
// divides the active terminal beside or below; splits nest (a column inside
// a row …), and the parts of a split share its space evenly.
Item {
    id: group
    property string workingDirectory
    property var actions: null
    /// All terminals of the tab, in the order they were opened.
    property var terminals: []
    property int activeIndex: 0
    readonly property var activeTerminal: terminals.length > 0 ? terminals[Math.min(activeIndex, terminals.length - 1)] : null
    readonly property string title: activeTerminal && activeTerminal.title ? activeTerminal.title : shellName()
    /// Direction of the outermost split.
    property alias orientation: rootSplit.orientation

    signal empty()

    SplitView {
        id: rootSplit
        anchors.fill: parent
        orientation: Qt.Horizontal
        handle: splitHandle
    }

    Component {
        id: splitHandle
        Rectangle {
            implicitWidth: 3
            implicitHeight: 3
            color: SplitHandle.pressed ? Theme.accent : Theme.border
        }
    }

    // A nested split; `container` is the split it sits in.
    Component {
        id: splitComponent
        SplitView {
            property var container: null
            handle: splitHandle
            SplitView.preferredWidth: container ? container.width / Math.max(1, container.count) : 0
            SplitView.preferredHeight: container ? container.height / Math.max(1, container.count) : 0
            SplitView.minimumWidth: 120
            SplitView.minimumHeight: 60
        }
    }

    Component {
        id: terminalComponent
        TerminalPane {
            property var container: null
            SplitView.preferredWidth: container ? container.width / Math.max(1, container.count) : 0
            SplitView.preferredHeight: container ? container.height / Math.max(1, container.count) : 0
            SplitView.minimumWidth: 120
            SplitView.minimumHeight: 60
            actions: group.actions
        }
    }

    function shellName() {
        const command = activeTerminal ? activeTerminal.command : []
        return command.length ? command[0].slice(command[0].lastIndexOf("/") + 1) : "terminal"
    }

    function indexIn(container, item) {
        for (let i = 0; i < container.count; ++i) if (container.itemAt(i) === item) return i
        return -1
    }

    function makePane(cwd, command, container) {
        const pane = terminalComponent.createObject(null, { workingDirectory: cwd || group.workingDirectory, command: command || [], container: container })
        terminals = terminals.concat([pane])
        pane.closed.connect(() => group.removeTerminal(pane))
        pane.activated.connect(() => { group.activeIndex = group.terminals.indexOf(pane) })
        return pane
    }

    function activate(pane) {
        activeIndex = terminals.indexOf(pane)
        Qt.callLater(pane.focusTerminal)
    }

    /// A terminal at the end of the outermost split (a tab's first one).
    function addTerminal(cwd, command) {
        const pane = makePane(cwd, command, rootSplit)
        rootSplit.addItem(pane)
        activate(pane)
        return pane
    }

    /// Divides the active terminal: the new one goes beside it (`vertical`
    /// false) or below it.
    function splitActive(vertical, cwd) {
        const target = activeTerminal
        if (!target) return addTerminal(cwd)
        const direction = vertical ? Qt.Vertical : Qt.Horizontal
        const container = target.container
        const at = indexIn(container, target)
        let pane
        if (container.orientation === direction || container.count === 1) {
            container.orientation = direction
            pane = makePane(cwd || target.cwd(), [], container)
            container.insertItem(at + 1, pane)
        } else {
            // The other direction: a new split in the active terminal's place.
            const split = splitComponent.createObject(null, { orientation: direction, container: container })
            container.takeItem(at)
            container.insertItem(at, split)
            split.addItem(target)
            target.container = split
            pane = makePane(cwd || target.cwd(), [], split)
            split.addItem(pane)
        }
        activate(pane)
        return pane
    }

    function removeTerminal(pane) {
        const index = terminals.indexOf(pane)
        if (index < 0) return
        const container = pane.container
        container.takeItem(indexIn(container, pane))
        const list = terminals.slice()
        list.splice(index, 1)
        terminals = list
        pane.destroy()
        collapse(container)
        if (terminals.length === 0) {
            group.empty()
            return
        }
        activeIndex = Math.min(index, terminals.length - 1)
        Qt.callLater(() => { if (group.activeTerminal) group.activeTerminal.focusTerminal() })
    }

    /// A nested split with one part left is replaced by that part; the
    /// outermost split takes over a lone nested split's direction.
    function collapse(split) {
        if (split !== rootSplit && split.count === 1) {
            const outer = split.container
            const rest = split.takeItem(0)
            const at = indexIn(outer, split)
            outer.takeItem(at)
            outer.insertItem(at, rest)
            rest.container = outer
            split.destroy()
        } else if (split === rootSplit && split.count === 1 && isSplit(split.itemAt(0))) {
            const inner = rootSplit.takeItem(0)
            rootSplit.orientation = inner.orientation
            while (inner.count > 0) {
                const item = inner.takeItem(0)
                rootSplit.addItem(item)
                item.container = rootSplit
            }
            inner.destroy()
        }
    }

    function isSplit(item) { return item.count !== undefined && item.itemAt !== undefined }

    /// The layout as a tree: {orientation, items: [{cwd} | {orientation,
    /// items}]}; `terminals` lists them flat (read by older versions).
    function serialize() {
        const node = (split) => {
            const items = []
            for (let i = 0; i < split.count; ++i) {
                const item = split.itemAt(i)
                if (isSplit(item)) items.push(node(item))
                // Programs started by Atlas (editors) are not restored.
                else if (item.command.length === 0) items.push({ cwd: item.cwd() })
            }
            return { orientation: split.orientation === Qt.Vertical ? "vertical" : "horizontal", items: items }
        }
        const tree = node(rootSplit)
        tree.terminals = terminals.filter(t => t.command.length === 0).map(t => ({ cwd: t.cwd() }))
        return tree
    }

    function restore(state) {
        const build = (split, node) => {
            split.orientation = node.orientation === "vertical" ? Qt.Vertical : Qt.Horizontal
            for (const item of node.items) {
                if (item.items) {
                    if (!item.items.length) continue
                    const inner = splitComponent.createObject(null, { container: split })
                    split.addItem(inner)
                    build(inner, item)
                    if (inner.count === 0) { split.takeItem(indexIn(split, inner)); inner.destroy() }
                } else {
                    split.addItem(makePane(item.cwd, [], split))
                }
            }
        }
        if (state.items && state.items.length) build(rootSplit, state)
        if (terminals.length === 0) {
            // Older sessions: one direction, a flat list.
            if (state.orientation === "vertical") rootSplit.orientation = Qt.Vertical
            const list = state.terminals && state.terminals.length ? state.terminals : [{}]
            for (const t of list) rootSplit.addItem(makePane(t.cwd, [], rootSplit))
        }
        activate(terminals[terminals.length - 1])
    }

    onActiveIndexChanged: updateActive()
    onTerminalsChanged: updateActive()

    function updateActive() {
        for (let i = 0; i < terminals.length; ++i) terminals[i].active = (i === activeIndex && terminals.length > 1)
    }
}
