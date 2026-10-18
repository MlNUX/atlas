// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Atlas

// Editor area with up to four document panes in a split view.
SplitView {
    id: area
    required property AppController app
    readonly property int maxPanes: 4
    property var panes: []
    property int activeIndex: 0
    readonly property var activePane: panes.length > 0 ? panes[Math.min(activeIndex, panes.length - 1)] : null

    signal layoutChanged()

    orientation: Qt.Horizontal

    handle: Rectangle {
        implicitWidth: 4
        implicitHeight: 4
        color: SplitHandle.pressed ? Theme.accent : SplitHandle.hovered ? Theme.border : Theme.window
    }

    Component {
        id: paneComponent
        DocumentPane {
            app: area.app
            SplitView.minimumWidth: 200
            SplitView.minimumHeight: 120
        }
    }

    function addPane(path, line, state, atIndex) {
        if (panes.length >= maxPanes) return activePane
        const pane = paneComponent.createObject(null, { path: path || "", line: line || 0 })
        if (state) pane.applyState(state)
        const index = atIndex === undefined ? panes.length : atIndex
        area.insertItem(index, pane)
        const list = panes.slice()
        list.splice(index, 0, pane)
        panes = list
        pane.activated.connect(() => { area.activeIndex = area.panes.indexOf(pane) })
        pane.closeRequested.connect(() => area.closePane(pane))
        pane.splitRequested.connect(() => area.splitPane(pane))
        pane.pathChanged.connect(area.layoutChanged)
        activeIndex = index
        equalize()
        layoutChanged()
        return pane
    }

    function closePane(pane) {
        const index = panes.indexOf(pane)
        if (index < 0) return
        area.removeItem(pane)
        const list = panes.slice()
        list.splice(index, 1)
        panes = list
        pane.destroy()
        if (panes.length === 0) addPane("")
        equalize()
        activeIndex = Math.max(0, Math.min(activeIndex, panes.length - 1))
        layoutChanged()
    }

    function splitPane(pane) {
        const index = panes.indexOf(pane)
        addPane(pane.path, 0, pane.viewState(), index + 1)
    }

    /// Opens a file in the active pane, or in a new split.
    function openFile(path, line, newPane) {
        if (newPane && panes.length < maxPanes) {
            addPane(path, line)
            return
        }
        if (!activePane) addPane("")
        activePane.open(path, line)
        layoutChanged()
    }

    function clear() {
        for (const pane of panes) {
            area.removeItem(pane)
            pane.destroy()
        }
        panes = []
    }

    function serialize() {
        return {
            orientation: area.orientation === Qt.Horizontal ? "horizontal" : "vertical",
            active: activeIndex,
            // Paths relative to the project, so the session survives moving
            // or syncing the project folder.
            panes: panes.map(p => Object.assign(p.viewState(), { path: p.path ? area.app.relativePath(p.path) : "" }))
        }
    }

    function restore(state) {
        clear()
        if (state && state.orientation) area.orientation = state.orientation === "vertical" ? Qt.Vertical : Qt.Horizontal
        const list = state && state.panes ? state.panes : []
        for (const p of list) {
            const path = p.path ? area.app.resolveDocument(p.path) : ""
            if (p.path && !path) continue  // gone for good
            addPane(path, 0, p)
        }
        if (panes.length === 0) addPane("")
        activeIndex = state && state.active !== undefined ? Math.min(state.active, panes.length - 1) : 0
    }

    /// Gives all panes the same share of the available space.
    function equalize() {
        const n = Math.max(1, panes.length)
        for (const p of panes) {
            p.SplitView.preferredWidth = area.width / n
            p.SplitView.preferredHeight = area.height / n
        }
    }
    onOrientationChanged: equalize()
    // Panes created before the first layout pass get their share once the
    // area has a size.
    property bool sized: false
    onWidthChanged: if (!sized && width > 0) { sized = true; equalize() }

    function documents() {
        return panes.map(p => p.path).filter(p => p.length > 0)
    }

    onActiveIndexChanged: {
        for (let i = 0; i < panes.length; ++i) panes[i].active = (i === activeIndex)
        layoutChanged()
    }
    onPanesChanged: {
        for (let i = 0; i < panes.length; ++i) panes[i].active = (i === activeIndex)
    }
}
