// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Atlas

// Clickable path (breadcrumbs, like Dolphin's location bar). A click on the
// free space or Ctrl+L turns it into a text field.
Rectangle {
    id: root
    /// AppController (optional): folders in the project or below a place
    /// start there.
    property var app: null
    property string path
    property bool editing: false

    signal navigateRequested(string path)

    /// Segments [{ name, path, icon? }] from the root (or place) down.
    function segments(path) {
        const out = [{ name: "/", path: "/" }]
        let acc = ""
        for (const part of path.split("/").filter(p => p)) {
            acc += "/" + part
            out.push({ name: part, path: acc })
        }
        // The project and the places are roots of their own (longest match).
        const roots = []
        if (app && app.projectRoot) roots.push({ name: app.projectName || app.fileName(app.projectRoot), path: app.projectRoot, icon: "logo" })
        if (app) app.placePaths.forEach((p, i) => roots.push({ name: app.placeNames[i], path: p, icon: app.placeIcons[i] }))
        let best = null
        for (const r of roots) {
            if (r.path !== "/" && (path === r.path || path.startsWith(r.path + "/")) && (!best || r.path.length > best.path.length)) best = r
        }
        if (!best) return out
        return [best].concat(out.slice(best.path.split("/").filter(x => x).length + 1))
    }
    function startEditing() {
        field.text = root.path
        editing = true
        field.forceActiveFocus()
        field.selectAll()
    }

    implicitHeight: 32
    radius: 8
    color: Theme.editor
    border.color: editing ? Theme.accent : Theme.border
    clip: true

    // The free space right of the segments edits the path. The list is not
    // interactive, so clicks besides the segments fall through to here.
    MouseArea {
        anchors.fill: parent
        visible: !root.editing
        cursorShape: Qt.IBeamCursor
        onClicked: root.startEditing()
    }

    ListView {
        id: crumbs
        visible: !root.editing
        anchors.fill: parent
        anchors.leftMargin: 6
        anchors.rightMargin: 6
        orientation: ListView.Horizontal
        interactive: false
        model: root.segments(root.path)
        onCountChanged: Qt.callLater(positionViewAtEnd)
        onWidthChanged: Qt.callLater(positionViewAtEnd)
        delegate: Row {
            id: crumb
            required property var modelData
            required property int index
            height: crumbs.height
            spacing: 0
            Icon {
                visible: crumb.index > 0
                anchors.verticalCenter: parent.verticalCenter
                name: "chevron-right"
                size: 12
                color: Theme.textFaint
            }
            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: crumbRow.width + 12
                height: 24
                radius: 6
                color: crumbHover.hovered ? Theme.hover : "transparent"
                Row {
                    id: crumbRow
                    anchors.centerIn: parent
                    spacing: 6
                    Icon {
                        visible: !!crumb.modelData.icon
                        anchors.verticalCenter: parent.verticalCenter
                        name: crumb.modelData.icon || "folder"
                        size: 14
                        color: Theme.textMuted
                    }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: crumb.modelData.name
                        color: crumb.index === crumbs.count - 1 ? Theme.textStrong : Theme.text
                        font.pixelSize: Theme.fontSize
                        font.weight: crumb.index === crumbs.count - 1 ? Font.DemiBold : Font.Normal
                    }
                }
                HoverHandler { id: crumbHover }
                MouseArea { anchors.fill: parent; onClicked: root.navigateRequested(crumb.modelData.path) }
            }
        }
        // Some free space after the last segment, also when the path is
        // longer than the bar (clicks there reach the area below).
        footer: Item { width: 40; height: crumbs.height }
    }
    // Leading segments scrolled out of view.
    Rectangle {
        visible: !root.editing && crumbs.contentX > crumbs.originX + 1
        x: 1
        y: 1
        width: 28
        height: parent.height - 2
        radius: 8
        color: Theme.editor
        Text {
            anchors.centerIn: parent
            text: "…"
            color: Theme.textMuted
            font.pixelSize: Theme.fontSize
        }
        MouseArea { anchors.fill: parent; onClicked: root.startEditing() }
    }
    TextField {
        id: field
        visible: root.editing
        anchors.fill: parent
        background: null
        font.pixelSize: Theme.fontSize
        onAccepted: { root.editing = false; root.navigateRequested(text.trim()) }
        Keys.onEscapePressed: (event) => { root.editing = false; event.accepted = true }
        onActiveFocusChanged: if (!activeFocus) root.editing = false
    }
}
