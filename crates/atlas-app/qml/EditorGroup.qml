// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// One split of the editor area: tabs, navigation bar with breadcrumbs, and
// the documents of the tabs.
Rectangle {
    id: group
    required property AppController app
    property var actions
    property var tabs: []
    property int current: -1
    readonly property var currentTab: current >= 0 && current < tabs.length ? tabs[current] : null
    /// A temporary split is replaced by the next temporary open until pinned.
    property bool temporary: false
    /// The EditorArea (shared drag state for moving tabs between splits).
    property var editorArea: null
    property bool active: false

    signal activated()
    signal emptied()
    signal closeRequested()
    signal splitRequested()
    /// Open in another split: `side` is "left", "right" or "" (default).
    signal openInSplit(string path, int line, string side, var state)
    signal changed()

    /// "+" in the tab strip: find a file to open.
    signal quickOpenRequested()

    color: Theme.editor
    radius: Theme.radius
    clip: true
    border.color: temporary ? Theme.warn : active ? Qt.rgba(0.24, 0.55, 0.99, 0.55) : Theme.border
    border.width: 1

    Component {
        id: tabComponent
        DocumentView {
            app: group.app
            actions: group.actions
            anchors.fill: parent
        }
    }

    function indexOfPath(path) { return tabs.findIndex(t => t.path === path) }

    /// Opens `path` in this group. `mode`: "preview" replaces the current
    /// preview tab, "tab" opens a permanent tab.
    function openTab(path, line, mode, state) {
        let index = indexOfPath(path)
        if (index >= 0) {
            if (mode !== "preview") tabs[index].preview = false
            if (line) tabs[index].reveal(line)
            if (state && state.annot) tabs[index].applyState({ annot: state.annot })
            if (state && state.processor) tabs[index].applyState({ processor: state.processor })
            if (state && path.startsWith("calendar:") && Object.keys(state).length) tabs[index].applyState(state)
            current = index
            changed()
            return tabs[index]
        }
        const previewIndex = tabs.findIndex(t => t.preview)
        if (mode === "preview" && previewIndex >= 0) {
            const tab = tabs[previewIndex]
            tab.backStack = []
            tab.forwardStack = []
            tab.show(path, line || 0, state || {})
            current = previewIndex
            changed()
            return tab
        }
        const tab = tabComponent.createObject(content, { path: path, line: line || 0, preview: mode === "preview" })
        if (state) tab.applyState(state)
        // Connections go through `ownerGroup`, so a tab moved to another
        // split talks to its new group.
        tab.ownerGroup = group
        tab.openRequested.connect((p, l, where, s) => {
            const g = tab.ownerGroup
            if (where === "split") g.openInSplit(p, l, "", s || {})
            else g.openTab(p, l, where, s)
        })
        tab.pinRequested.connect(() => { tab.preview = false; tab.ownerGroup.changed() })
        tab.closeRequested.connect(() => tab.ownerGroup.closeTab(tab.ownerGroup.tabs.indexOf(tab)))
        tab.pathChanged.connect(() => tab.ownerGroup.changed())
        insertTab(tab, current >= 0 ? current + 1 : tabs.length)
        return tab
    }

    function insertTab(tab, at) {
        const insertAt = Math.max(0, Math.min(at, tabs.length))
        const list = tabs.slice()
        list.splice(insertAt, 0, tab)
        tabs = list
        current = insertAt
        changed()
    }

    /// Removes tab `index` without closing it (it moves elsewhere).
    function takeTab(index) {
        const tab = tabs[index]
        if (!tab) return null
        const list = tabs.slice()
        list.splice(index, 1)
        tabs = list
        if (current >= tabs.length) current = tabs.length - 1
        else if (index < current) current--
        changed()
        return tab
    }

    /// Takes over `tab` from another split (it keeps its state, terminals
    /// keep running) and inserts it at `at`.
    function adoptTab(tab, at) {
        tab.parent = content
        tab.ownerGroup = group
        tab.preview = false
        insertTab(tab, at)
        group.activated()
    }

    function closeTab(index) {
        if (index < 0 || index >= tabs.length) return
        const tab = tabs[index]
        const list = tabs.slice()
        list.splice(index, 1)
        tabs = list
        tab.destroy()
        if (current >= tabs.length) current = tabs.length - 1
        else if (index < current) current--
        changed()
        if (tabs.length === 0) emptied()
    }

    function closeCurrent() { closeTab(current) }
    function nextTab(step) { if (tabs.length) current = (current + step + tabs.length) % tabs.length }

    function serialize() {
        return {
            temporary: group.temporary,
            current: group.current,
            tabs: tabs.map(t => Object.assign(t.viewState(),
                { path: t.path ? group.app.relativePath(t.path) : "", preview: t.preview, line: t.line }))
        }
    }

    function documents() { return tabs.map(t => t.path).filter(p => p.length > 0 && !p.startsWith("terminal:") && !p.startsWith("calendar:")) }

    function pathMoved(from, to) {
        for (const t of tabs) {
            if (t.path === from) t.path = to
            else if (t.path.startsWith(from + "/")) t.path = to + t.path.slice(from.length)
        }
    }
    function pathsRemoved(paths) {
        for (const t of tabs) if (paths.some(r => t.path === r || t.path.startsWith(r + "/"))) t.refresh()
    }

    /// Path segments for the breadcrumbs: inside the project relative to its
    /// root, otherwise from "/".
    function crumbs(path) {
        if (!path || path.startsWith("terminal:") || path.startsWith("calendar:")) return []
        const root = group.app.projectRoot
        const inside = root && (path === root || path.startsWith(root + "/"))
        const base = inside ? root : ""
        const rest = inside ? path.slice(root.length) : path
        const parts = rest.split("/").filter(s => s.length > 0)
        const out = [{ label: inside ? group.app.projectName : "/", path: inside ? root : "/" }]
        let acc = base
        for (const p of parts) {
            acc += "/" + p
            out.push({ label: p, path: acc })
        }
        return out
    }

    TapHandler {
        gesturePolicy: TapHandler.DragThreshold
        grabPermissions: PointerHandler.TakeOverForbidden
        onTapped: group.activated()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: group.border.width
        spacing: 0

        // Tab strip: tabs as small cards, "+" to open another file.
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 44
            color: Theme.header
            radius: Theme.radius
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: Theme.radius; color: Theme.header }
            // A tab dropped on the strip goes to that position.
            DropArea {
                id: stripDrop
                anchors.fill: parent
                keys: ["text/x-atlas-tab"]
                // Position among the tabs (the strip's coordinates).
                readonly property int at: {
                    if (!containsDrag) return -1
                    const x = mapToItem(tabStrip.contentItem, drag.x, 0).x
                    for (let i = 0; i < tabStrip.count; ++i) {
                        const it = tabStrip.itemAtIndex(i)
                        if (it && x < it.x + it.width / 2) return i
                    }
                    return tabStrip.count
                }
                onDropped: (drop) => {
                    if (group.editorArea && group.editorArea.draggedTab) group.editorArea.moveTab(group, at, "")
                    drop.accept(Qt.MoveAction)
                }
            }
            Rectangle {
                visible: stripDrop.containsDrag && stripDrop.at >= 0
                readonly property var neighbour: tabStrip.itemAtIndex(Math.min(stripDrop.at, tabStrip.count - 1))
                x: !neighbour ? 8 : tabStrip.x + (stripDrop.at < tabStrip.count ? neighbour.x : neighbour.x + neighbour.width) - tabStrip.contentX - 1
                y: 8
                width: 2
                height: parent.height - 16
                radius: 1
                color: Theme.accent
            }
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 8
                spacing: 2
                ListView {
                    id: tabStrip
                    Layout.preferredWidth: Math.min(contentWidth, parent.width - toolsRow.width - 50)
                    Layout.fillHeight: true
                    orientation: ListView.Horizontal
                    clip: true
                    spacing: 4
                    model: group.tabs.length
                    boundsBehavior: Flickable.StopAtBounds
                    delegate: Item {
                        id: tabItem
                        required property int index
                        readonly property var tab: group.tabs[index] || null
                        readonly property bool isCurrent: index === group.current
                        readonly property var fileIcon: !tab ? ["document", Theme.textMuted]
                            : tab.kind === "calendar" ? ["calendar", Theme.accent]
                            : tab.kind === "terminal" ? ["terminal", Theme.textMuted]
                            : Theme.fileIcon(tab.title, tab.kind === "dir" || tab.kind === "trash")
                        width: Math.min(240, tabLabel.implicitWidth + 72)
                        height: tabStrip.height
                        Rectangle {
                            anchors.fill: parent
                            anchors.topMargin: 6
                            anchors.bottomMargin: 6
                            radius: 8
                            color: tabItem.isCurrent ? Theme.raised : tabHover.hovered ? Theme.hover : "transparent"
                            border.color: tabItem.isCurrent ? (group.active ? Qt.rgba(0.24, 0.55, 0.99, 0.4) : Theme.border) : "transparent"
                        }
                        HoverHandler { id: tabHover }
                        // Drag source: to another position, another split, or
                        // (as a file) onto the terminal or the calendar.
                        Item { id: tabDragProxy }
                        Drag.active: tabMouse.drag.active
                        Drag.dragType: Drag.Automatic
                        Drag.supportedActions: Qt.MoveAction | Qt.CopyAction | Qt.LinkAction
                        Drag.proposedAction: Qt.MoveAction
                        Drag.mimeData: {
                            const data = { "text/x-atlas-tab": tabItem.tab ? tabItem.tab.path : "" }
                            if (tabItem.tab && group.actions && ["terminal", "calendar", "trash", ""].indexOf(tabItem.tab.kind) < 0)
                                data["text/uri-list"] = group.actions.uriList([tabItem.tab.path])
                            return data
                        }
                        Connections {
                            target: tabMouse.drag
                            function onActiveChanged() {
                                if (!group.editorArea) return
                                if (tabMouse.drag.active) group.editorArea.draggedTab = { group: group, tab: tabItem.tab }
                                else Qt.callLater(() => { if (group.editorArea) group.editorArea.draggedTab = null })
                            }
                        }
                        Icon {
                            x: 12
                            anchors.verticalCenter: parent.verticalCenter
                            name: tabItem.fileIcon[0]
                            color: tabItem.fileIcon[1]
                            size: 16
                        }
                        Text {
                            id: tabLabel
                            anchors.verticalCenter: parent.verticalCenter
                            x: 36
                            width: parent.width - 64
                            text: tabItem.tab ? tabItem.tab.title : ""
                            font.italic: tabItem.tab ? tabItem.tab.preview : false
                            font.pixelSize: Theme.fontSize
                            color: tabItem.isCurrent ? Theme.textStrong : Theme.textMuted
                            elide: Text.ElideRight
                        }
                        MouseArea {
                            id: tabMouse
                            anchors.fill: parent
                            acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                            drag.target: tabDragProxy
                            drag.threshold: 10
                            onClicked: (mouse) => {
                                if (mouse.button === Qt.MiddleButton) group.closeTab(tabItem.index)
                                else { group.current = tabItem.index; group.activated() }
                            }
                            onDoubleClicked: if (tabItem.tab) { tabItem.tab.preview = false; group.changed() }
                        }
                        IconButton {
                            anchors.right: parent.right
                            anchors.rightMargin: 6
                            anchors.verticalCenter: parent.verticalCenter
                            implicitWidth: 20
                            implicitHeight: 20
                            visible: tabItem.isCurrent || tabHover.hovered
                            iconName: "close"
                            iconSize: 13
                            tip: qsTr("Tab schließen (Strg+W)")
                            onClicked: group.closeTab(tabItem.index)
                        }
                        ToolTip.visible: tabHover.hovered && !!tabItem.tab
                        ToolTip.delay: 800
                        ToolTip.text: tabItem.tab ? tabItem.tab.path + (tabItem.tab.preview ? qsTr("\n(Vorschau – Doppelklick zum Anheften)") : "") : ""
                    }
                }
                IconButton { iconName: "plus"; tip: qsTr("Datei öffnen (Strg+P)"); onClicked: group.quickOpenRequested() }
                Item { Layout.fillWidth: true }
                Row {
                id: toolsRow
                spacing: 2
                Label {
                    visible: group.temporary
                    anchors.verticalCenter: parent.verticalCenter
                    text: qsTr("Temporär")
                    color: Theme.warn
                    font.pixelSize: Theme.smallFontSize
                    Layout.rightMargin: 4
                }
                IconButton {
                    visible: group.temporary
                    iconName: "bookmark"
                    tip: qsTr("Split anheften")
                    onClicked: { group.temporary = false; group.changed() }
                }
                IconButton { iconName: "split"; tip: qsTr("Split rechts (Strg+\\)"); onClicked: group.splitRequested() }
                IconButton { iconName: "close"; tip: qsTr("Split schließen"); onClicked: group.closeRequested() }
                }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }

        // Navigation: back/forward and breadcrumbs of the current tab.
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 28
            color: Theme.header
            visible: !!group.currentTab && group.currentTab.kind !== "calendar"
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 4
                anchors.rightMargin: 8
                spacing: 2
                IconButton {
                    iconName: "arrow-left"
                    iconSize: 14
                    tip: qsTr("Zurück (Alt+←)")
                    enabled: group.currentTab ? group.currentTab.canGoBack : false
                    onClicked: group.currentTab.goBack()
                }
                IconButton {
                    iconName: "arrow-right"
                    iconSize: 14
                    tip: qsTr("Vor (Alt+→)")
                    enabled: group.currentTab ? group.currentTab.canGoForward : false
                    onClicked: group.currentTab.goForward()
                }
                ListView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    orientation: ListView.Horizontal
                    clip: true
                    interactive: contentWidth > width
                    model: group.currentTab ? group.crumbs(group.currentTab.path) : []
                    onCountChanged: positionViewAtEnd()
                    delegate: Row {
                        id: crumb
                        required property var modelData
                        required property int index
                        height: ListView.view.height
                        Text {
                            visible: crumb.index > 0
                            anchors.verticalCenter: parent.verticalCenter
                            text: "  ›  "
                            color: Theme.textFaint
                            font.pixelSize: Theme.smallFontSize
                        }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: crumb.modelData.label
                            color: crumbArea.containsMouse ? Theme.textStrong : Theme.textMuted
                            font.pixelSize: Theme.smallFontSize
                            MouseArea {
                                id: crumbArea
                                anchors.fill: parent
                                hoverEnabled: true
                                acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                                cursorShape: Qt.PointingHandCursor
                                onClicked: (mouse) => {
                                    if (crumb.modelData.path === group.currentTab.path) return
                                    if (mouse.button === Qt.MiddleButton) group.openTab(crumb.modelData.path, 0, "tab")
                                    else group.currentTab.navigate(crumb.modelData.path, 0)
                                }
                            }
                        }
                    }
                }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }

        Item {
            id: content
            Layout.fillWidth: true
            Layout.fillHeight: true
            // Only the current tab is visible; the others keep their state.
        }
    }

    onCurrentChanged: { for (let i = 0; i < tabs.length; ++i) tabs[i].visible = (i === current) }
    onTabsChanged: { for (let i = 0; i < tabs.length; ++i) tabs[i].visible = (i === current) }

    // Back/forward with the extra mouse buttons.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.BackButton | Qt.ForwardButton
        onClicked: (mouse) => {
            if (!group.currentTab) return
            if (mouse.button === Qt.BackButton) group.currentTab.goBack()
            else group.currentTab.goForward()
        }
    }

    // Drag-and-drop: drop in the middle opens here, on the left/right edge
    // opens a new split on that side. Tabs dragged from a tab strip move.
    DropArea {
        id: drop
        anchors.fill: parent
        anchors.topMargin: 44
        keys: ["text/uri-list", "text/x-atlas-tab"]
        readonly property bool isTab: containsDrag && drag.hasFormat("text/x-atlas-tab") && !!group.editorArea && !!group.editorArea.draggedTab
        readonly property string side: !containsDrag ? "" : drag.x < width * 0.2 ? "left" : drag.x > width * 0.8 ? "right" : "center"
        // Folder tabs take file drops themselves (move/copy into the
        // folder), terminal tabs insert the paths, the calendar links them.
        onEntered: (event) => {
            const tabMove = event.hasFormat("text/x-atlas-tab") && !!group.editorArea && !!group.editorArea.draggedTab
            if (!tabMove && group.currentTab && ["dir", "trash", "terminal", "calendar"].indexOf(group.currentTab.kind) >= 0) event.accepted = false
        }
        onDropped: (event) => {
            if (isTab) {
                group.editorArea.moveTab(group, side === "center" ? group.tabs.length : -1, side === "center" ? "" : side)
                event.accept(Qt.MoveAction)
                return
            }
            const paths = event.urls.map(u => group.app.urlToPath(u.toString()))
            if (paths.length === 0) return
            if (side === "center") for (const p of paths) group.openTab(p, 0, "tab")
            else for (const p of paths) group.openInSplit(p, 0, side, {})
            event.accept(Qt.CopyAction)
            group.activated()
        }
        Rectangle {
            visible: drop.containsDrag
            color: Theme.accent
            opacity: 0.18
            radius: Theme.radius
            x: drop.side === "right" ? parent.width * 0.5 : 0
            width: drop.side === "center" ? parent.width : parent.width * 0.5
            height: parent.height
        }
    }
}
