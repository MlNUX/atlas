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
    property bool active: false

    signal activated()
    signal emptied()
    signal closeRequested()
    signal splitRequested()
    /// Open in another split: `side` is "left", "right" or "" (default).
    signal openInSplit(string path, int line, string side)
    signal changed()

    color: Theme.editor
    border.color: active ? Theme.accent : temporary ? Theme.warn : "transparent"
    border.width: active || temporary ? 1 : 0

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
        tab.openRequested.connect((p, l, where) => {
            if (where === "split") group.openInSplit(p, l, "")
            else group.openTab(p, l, where)
        })
        tab.pinRequested.connect(() => { tab.preview = false; group.changed() })
        tab.pathChanged.connect(group.changed)
        const insertAt = current >= 0 ? current + 1 : tabs.length
        const list = tabs.slice()
        list.splice(insertAt, 0, tab)
        tabs = list
        current = insertAt
        changed()
        return tab
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

    function documents() { return tabs.map(t => t.path).filter(p => p.length > 0) }

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
        if (!path) return []
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

        // Tab strip.
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 32
            color: Theme.window
            RowLayout {
                anchors.fill: parent
                spacing: 0
                ListView {
                    id: tabStrip
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    orientation: ListView.Horizontal
                    clip: true
                    model: group.tabs.length
                    boundsBehavior: Flickable.StopAtBounds
                    delegate: Rectangle {
                        id: tabItem
                        required property int index
                        readonly property var tab: group.tabs[index] || null
                        readonly property bool isCurrent: index === group.current
                        width: Math.min(220, tabLabel.implicitWidth + 44)
                        height: tabStrip.height
                        color: isCurrent ? Theme.editor : tabHover.hovered ? Theme.hover : Theme.window
                        Rectangle { visible: tabItem.isCurrent && group.active; width: parent.width; height: 2; color: Theme.accent }
                        Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: Theme.border }
                        HoverHandler { id: tabHover }
                        Text {
                            id: tabLabel
                            anchors.verticalCenter: parent.verticalCenter
                            x: 12
                            width: parent.width - 40
                            text: tabItem.tab ? (tabItem.tab.kind === "dir" ? "📁 " : "") + tabItem.tab.title : ""
                            font.italic: tabItem.tab ? tabItem.tab.preview : false
                            font.pixelSize: Theme.fontSize
                            color: tabItem.isCurrent ? Theme.textStrong : Theme.textMuted
                            elide: Text.ElideRight
                        }
                        MouseArea {
                            anchors.fill: parent
                            acceptedButtons: Qt.LeftButton | Qt.MiddleButton
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
                            implicitWidth: 18
                            implicitHeight: 18
                            visible: tabItem.isCurrent || tabHover.hovered
                            glyph: "✕"
                            tip: qsTr("Tab schließen (Strg+W)")
                            onClicked: group.closeTab(tabItem.index)
                        }
                        ToolTip.visible: tabHover.hovered && !!tabItem.tab
                        ToolTip.delay: 800
                        ToolTip.text: tabItem.tab ? tabItem.tab.path + (tabItem.tab.preview ? qsTr("\n(Vorschau – Doppelklick zum Anheften)") : "") : ""
                    }
                }
                Label {
                    visible: group.temporary
                    text: qsTr("Temporär")
                    color: Theme.warn
                    font.pixelSize: Theme.smallFontSize
                    Layout.rightMargin: 4
                }
                IconButton {
                    visible: group.temporary
                    glyph: "📌"
                    tip: qsTr("Split anheften")
                    onClicked: { group.temporary = false; group.changed() }
                }
                IconButton { glyph: "◫"; tip: qsTr("Split rechts (Strg+\\)"); onClicked: group.splitRequested() }
                IconButton { glyph: "✕"; tip: qsTr("Split schließen"); onClicked: group.closeRequested() }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }

        // Navigation: back/forward and breadcrumbs of the current tab.
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 26
            color: Theme.editor
            visible: !!group.currentTab
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 4
                anchors.rightMargin: 8
                spacing: 2
                IconButton {
                    glyph: "←"
                    tip: qsTr("Zurück (Alt+←)")
                    enabled: group.currentTab ? group.currentTab.canGoBack : false
                    onClicked: group.currentTab.goBack()
                }
                IconButton {
                    glyph: "→"
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
    // opens a new split on that side.
    DropArea {
        id: drop
        anchors.fill: parent
        // Folder tabs take drops themselves (move/copy into the folder).
        enabled: !group.currentTab || (group.currentTab.kind !== "dir" && group.currentTab.kind !== "trash")
        keys: ["text/uri-list"]
        readonly property string side: !containsDrag ? "" : drag.x < width * 0.2 ? "left" : drag.x > width * 0.8 ? "right" : "center"
        onDropped: (event) => {
            const paths = event.urls.map(u => group.app.urlToPath(u.toString()))
            if (paths.length === 0) return
            if (side === "center") for (const p of paths) group.openTab(p, 0, "tab")
            else for (const p of paths) group.openInSplit(p, 0, side)
            event.accept(Qt.CopyAction)
            group.activated()
        }
        Rectangle {
            visible: drop.containsDrag
            color: Theme.accent
            opacity: 0.18
            x: drop.side === "right" ? parent.width * 0.5 : 0
            width: drop.side === "center" ? parent.width : parent.width * 0.5
            height: parent.height
        }
    }
}
