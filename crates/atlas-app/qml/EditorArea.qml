// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Atlas

// Editor area: up to four splits ("groups"), each with tabs.
SplitView {
    id: area
    required property AppController app
    property var actions
    readonly property int maxGroups: 4
    property var groups: []
    property int activeIndex: 0
    readonly property var activeGroup: groups.length > 0 ? groups[Math.min(activeIndex, groups.length - 1)] : null
    readonly property var activeTab: activeGroup ? activeGroup.currentTab : null

    signal layoutChanged()
    /// "+" in a tab strip.
    signal quickOpenRequested()

    orientation: Qt.Horizontal

    // Splits are cards with a gap between them.
    handle: Item {
        implicitWidth: Theme.gap
        implicitHeight: Theme.gap
        Rectangle {
            anchors.centerIn: parent
            width: area.orientation === Qt.Horizontal ? 2 : 36
            height: area.orientation === Qt.Horizontal ? 36 : 2
            radius: 1
            color: Theme.accent
            opacity: parent.SplitHandle.pressed ? 1 : parent.SplitHandle.hovered ? 0.6 : 0
        }
    }

    Component {
        id: groupComponent
        EditorGroup {
            app: area.app
            actions: area.actions
            SplitView.minimumWidth: 220
            SplitView.minimumHeight: 140
        }
    }

    function addGroup(atIndex, temporary) {
        if (groups.length >= maxGroups) return null
        const group = groupComponent.createObject(null, { temporary: !!temporary })
        const index = atIndex === undefined || atIndex < 0 ? groups.length : Math.min(atIndex, groups.length)
        area.insertItem(index, group)
        const list = groups.slice()
        list.splice(index, 0, group)
        groups = list
        group.activated.connect(() => { area.activeIndex = area.groups.indexOf(group) })
        group.emptied.connect(() => { if (area.groups.length > 1) area.closeGroup(group) })
        group.closeRequested.connect(() => area.closeGroup(group))
        group.splitRequested.connect(() => area.splitGroup(group))
        group.openInSplit.connect((p, l, side, s) => area.openInSplit(group, p, l, side, s))
        group.changed.connect(area.layoutChanged)
        group.quickOpenRequested.connect(area.quickOpenRequested)
        activeIndex = index
        equalize()
        layoutChanged()
        return group
    }

    function closeGroup(group) {
        const index = groups.indexOf(group)
        if (index < 0) return
        area.removeItem(group)
        const list = groups.slice()
        list.splice(index, 1)
        groups = list
        group.destroy()
        if (groups.length === 0) addGroup()
        activeIndex = Math.max(0, Math.min(activeIndex, groups.length - 1))
        equalize()
        layoutChanged()
    }

    /// Duplicates the current tab of `group` into a new split to its right.
    function splitGroup(group) {
        const index = groups.indexOf(group)
        const tab = group.currentTab
        const target = addGroup(index + 1)
        if (target && tab && tab.path) target.openTab(tab.path, 0, "tab", tab.viewState())
    }

    /// Opens next to `from`: on the given side, or in the next split.
    function openInSplit(from, path, line, side, state) {
        const index = groups.indexOf(from)
        let target = null
        if (side === "left" || side === "right") target = addGroup(side === "left" ? index : index + 1)
        if (!target) target = groups.length < maxGroups ? addGroup(index + 1) : groups[(index + 1) % groups.length]
        target.openTab(path, line, "tab", state)
        activeIndex = groups.indexOf(target)
    }

    /// Opens a file. `where`:
    ///   "preview"   – preview tab in the active split (single click)
    ///   "tab"       – permanent tab in the active split
    ///   "split"     – permanent tab in a new split
    ///   "temporary" – the temporary split (created or reused)
    function open(path, line, where, state) {
        if (!path) return
        if (where === "split") {
            const g = groups.length < maxGroups ? addGroup(activeIndex + 1) : activeGroup
            return g.openTab(path, line, "tab", state)
        }
        if (where === "temporary") {
            let g = groups.find(x => x.temporary)
            if (!g) g = groups.length < maxGroups ? addGroup(groups.length, true) : activeGroup
            const tab = g.openTab(path, line, "preview", state)
            activeIndex = groups.indexOf(g)
            return tab
        }
        if (!activeGroup) addGroup()
        return activeGroup.openTab(path, line, where === "tab" ? "tab" : "preview", state)
    }

    /// Compatibility helper: `newPane` opens in a new split, otherwise preview.
    function openFile(path, line, newPane) {
        return open(path, line, newPane ? "split" : "preview")
    }

    function clear() {
        for (const g of groups) {
            area.removeItem(g)
            g.destroy()
        }
        groups = []
    }

    function serialize() {
        return {
            orientation: area.orientation === Qt.Horizontal ? "horizontal" : "vertical",
            active: activeIndex,
            groups: groups.map(g => g.serialize())
        }
    }

    function restore(state) {
        clear()
        if (state && state.orientation) area.orientation = state.orientation === "vertical" ? Qt.Vertical : Qt.Horizontal
        // Phase 0 layouts had one document per split ("panes").
        const groupStates = state && state.groups ? state.groups
            : state && state.panes ? state.panes.map(p => ({ tabs: [p], current: 0 })) : []
        for (const gs of groupStates) {
            const g = addGroup(groups.length, gs.temporary)
            for (const t of gs.tabs || []) {
                const path = !t.path ? "" : t.path.startsWith("terminal:") || t.path.startsWith("calendar:") ? t.path : area.app.resolveDocument(t.path)
                if (!path || t.transient) continue
                g.openTab(path, t.line || 0, t.preview ? "preview" : "tab", t)
            }
            if (g.tabs.length === 0 && groupStates.length > 1) { closeGroup(g); continue }
            g.current = Math.max(0, Math.min(gs.current || 0, g.tabs.length - 1))
        }
        if (groups.length === 0) addGroup()
        activeIndex = state && state.active !== undefined ? Math.max(0, Math.min(state.active, groups.length - 1)) : 0
        equalize()
    }

    /// Viewer of an open tab showing `path` (null if none).
    function viewerFor(path) {
        for (const g of groups) for (const t of g.tabs) if (t.path === path) return t.viewerItem
        return null
    }

    function documents() {
        return groups.reduce((all, g) => all.concat(g.documents()), [])
    }

    /// Tabs follow files and folders that Atlas moved or renamed.
    function pathMoved(from, to) {
        for (const g of groups) g.pathMoved(from, to)
        layoutChanged()
    }

    /// Tabs showing deleted items show a notice instead of stale content.
    function pathsRemoved(paths) {
        for (const g of groups) g.pathsRemoved(paths)
    }

    /// Gives all groups the same share of the available space.
    function equalize() {
        const n = Math.max(1, groups.length)
        for (const g of groups) {
            g.SplitView.preferredWidth = area.width / n
            g.SplitView.preferredHeight = area.height / n
        }
    }
    onOrientationChanged: equalize()
    property bool sized: false
    onWidthChanged: if (!sized && width > 0) { sized = true; equalize() }

    onActiveIndexChanged: {
        for (let i = 0; i < groups.length; ++i) groups[i].active = (i === activeIndex)
        layoutChanged()
    }
    onGroupsChanged: { for (let i = 0; i < groups.length; ++i) groups[i].active = (i === activeIndex) }
}
