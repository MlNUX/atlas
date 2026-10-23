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
    /// Tab being dragged: {group, tab} (null otherwise).
    property var draggedTab: null

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

    // Documents belong to the area, not to a split: a tab can move to another
    // split (and outlive the one it came from) without being recreated.
    Component {
        id: documentComponent
        DocumentView { anchors.fill: parent }
    }
    // Owner of all documents. Not an Item: children of the SplitView itself
    // would become (hidden) splits with their own handles.
    QtObject { id: documentOwner }
    /// A document shown in `visualParent` (the content of a split) but
    /// owned by the area.
    function createDocument(props, visualParent) {
        const doc = documentComponent.createObject(visualParent, props)
        if (doc) area.app.setOwner(doc, documentOwner)
        return doc
    }

    Component {
        id: groupComponent
        EditorGroup {
            editorArea: area
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

    /// A split without tabs (reused instead of opening another split).
    function emptyGroup() { return groups.find(g => g.tabs.length === 0) || null }

    /// Closes empty splits as soon as another split shows something: an
    /// empty window never stays beside a document.
    function pruneEmpty() {
        if (!groups.some(g => g.tabs.length > 0)) return
        for (const g of groups.slice()) if (g.tabs.length === 0 && groups.length > 1) closeGroup(g)
    }

    /// Duplicates the current tab of `group` into a new split to its right.
    function splitGroup(group) {
        const index = groups.indexOf(group)
        const tab = group.currentTab
        if (!tab) return
        const target = addGroup(index + 1)
        if (target && tab && tab.path) target.openTab(tab.path, 0, "tab", tab.viewState())
    }

    /// Moves the dragged tab to `target` at index `at` (-1: end), or with
    /// `side` "left"/"right" into a new split beside `target`. The tab keeps
    /// its state; a split left empty closes.
    function moveTab(target, at, side) {
        const d = draggedTab
        draggedTab = null
        if (!d || !d.tab || !target) return
        const source = d.group
        const from = source.tabs.indexOf(d.tab)
        if (from < 0) return
        if (side === "left" || side === "right") {
            // Alone in its split: moving it beside itself changes nothing.
            if (source === target && source.tabs.length === 1) return
            const index = groups.indexOf(target)
            // At most four splits: then the tab goes into `target` itself.
            const created = addGroup(side === "left" ? index : index + 1)
            if (created) {
                target = created
                at = 0
            } else {
                at = -1
            }
        }
        if (source === target) {
            if (at < 0) at = target.tabs.length
            if (at === from || at === from + 1) { target.current = from; return }
            source.takeTab(from)
            target.insertTab(d.tab, at > from ? at - 1 : at)
        } else {
            source.takeTab(from)
            target.adoptTab(d.tab, at < 0 ? target.tabs.length : at)
            // A tab with the same file may already be open there: keep one.
            const twin = target.tabs.findIndex(t => t !== d.tab && t.path === d.tab.path && !t.path.startsWith("terminal:"))
            if (twin >= 0) target.closeTab(twin)
            if (source.tabs.length === 0 && groups.length > 1) closeGroup(source)
        }
        activeIndex = groups.indexOf(target)
        layoutChanged()
    }

    /// Opens next to `from`: on the given side, or in the next split.
    function openInSplit(from, path, line, side, state) {
        const index = groups.indexOf(from)
        let target = emptyGroup()
        if (!target && (side === "left" || side === "right")) target = addGroup(side === "left" ? index : index + 1)
        if (!target) target = groups.length < maxGroups ? addGroup(index + 1) : groups[(index + 1) % groups.length]
        target.openTab(path, line, "tab", state)
        activeIndex = groups.indexOf(target)
        pruneEmpty()
    }

    /// Opens a file. `where`:
    ///   "tab"       – tab in the active split ("preview" means the same;
    ///                 there are no preview tabs outside the temporary split)
    ///   "split"     – permanent tab in a new split
    ///   "temporary" – the temporary split (created or reused)
    function open(path, line, where, state) {
        if (!path) return
        let tab = null
        if (where === "split" || where === "temporary") {
            // An empty split is used instead of opening another one.
            let g = emptyGroup() || (where === "temporary" ? groups.find(x => x.temporary) : null)
            if (!g) g = groups.length < maxGroups ? addGroup(where === "split" ? activeIndex + 1 : groups.length, where === "temporary") : activeGroup
            tab = g.openTab(path, line, where === "split" ? "tab" : "preview", state)
            activeIndex = groups.indexOf(g)
        } else {
            if (!activeGroup) addGroup()
            tab = activeGroup.openTab(path, line, where === "tab" ? "tab" : "preview", state)
        }
        pruneEmpty()
        return tab
    }

    /// Compatibility helper: `newPane` opens in a new split, otherwise a tab.
    function openFile(path, line, newPane) {
        return open(path, line, newPane ? "split" : "tab")
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
                g.openTab(path, t.line || 0, "tab", t)
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

    /// Share of each split in the available space; kept when the area
    /// changes size (window, sidebar, context bar), so the splits always
    /// fill it.
    property var shares: []

    /// Gives all groups the same share of the available space.
    function equalize() {
        shares = groups.map(() => 1)
        relayout()
    }

    /// Sizes the splits by their shares; the last one takes what rounding
    /// leaves.
    function relayout() {
        const n = groups.length
        if (!n) return
        const horizontal = orientation === Qt.Horizontal
        const total = (horizontal ? width : height) - (n - 1) * Theme.gap
        if (total <= 0) return
        if (shares.length !== n) shares = groups.map(() => 1)
        const sum = shares.reduce((a, b) => a + b, 0) || n
        for (let i = 0; i < n; ++i) {
            const g = groups[i]
            const size = Math.max(140, total * shares[i] / sum)
            g.SplitView.fillWidth = horizontal && i === n - 1
            g.SplitView.fillHeight = !horizontal && i === n - 1
            if (horizontal) g.SplitView.preferredWidth = size
            else g.SplitView.preferredHeight = size
        }
    }

    // A split resized by hand keeps its new share.
    onResizingChanged: {
        if (resizing || !groups.length) return
        const horizontal = orientation === Qt.Horizontal
        shares = groups.map(g => Math.max(1, horizontal ? g.width : g.height))
        relayout()
    }
    onOrientationChanged: relayout()
    onWidthChanged: relayout()
    onHeightChanged: relayout()

    onActiveIndexChanged: {
        for (let i = 0; i < groups.length; ++i) groups[i].active = (i === activeIndex)
        layoutChanged()
    }
    onGroupsChanged: { for (let i = 0; i < groups.length; ++i) groups[i].active = (i === activeIndex) }
}
