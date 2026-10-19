// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Week view of the calendar: Monday to Sunday, an all-day row and an hour
// grid with the current time. Entries can be moved (drag) and resized
// (lower edge); dragging over free time or double-clicking creates an
// entry. Files dropped on an entry are linked, on free time they become a
// work block; tasks dropped here are planned.
Item {
    id: week
    required property var calendar

    readonly property real gutter: 52
    readonly property real hourHeight: 44
    readonly property real dayWidth: Math.max(40, (width - gutter - 10) / 7)
    readonly property date first: calendar.range[0]
    readonly property var days: [0, 1, 2, 3, 4, 5, 6].map(i => calendar.addDays(first, i))
    /// Snap for moving and resizing (minutes).
    readonly property int snap: 15

    // ---------------------------------------------------------------- layout

    /// Timed occurrences cut into per-day segments with overlap columns:
    /// {o, day, s, e (ms), col, cols}.
    readonly property var segments: {
        const out = []
        const timed = calendar.occs.filter(o => !o.allDay)
        for (let d = 0; d < 7; d++) {
            const ds = days[d].getTime(), de = calendar.addDays(days[d], 1).getTime()
            const segs = timed
                .map(o => ({ o: o, day: d, s: Math.max(calendar.parse(o.start).getTime(), ds), e: Math.min(calendar.parse(o.end).getTime(), de) }))
                .filter(x => x.s < de && x.e > ds && x.e > x.s)
                .sort((a, b) => a.s - b.s || b.e - a.e)
            let cluster = [], colsEnd = [], clusterEnd = 0
            const flush = () => { for (const x of cluster) x.cols = colsEnd.length; cluster = []; colsEnd = []; clusterEnd = 0 }
            for (const x of segs) {
                // Short entries (deadlines) still need room for their label.
                const visualEnd = Math.max(x.e, x.s + 25 * 60000)
                if (cluster.length && x.s >= clusterEnd) flush()
                let c = colsEnd.findIndex(end => end <= x.s)
                if (c < 0) { c = colsEnd.length; colsEnd.push(0) }
                colsEnd[c] = visualEnd
                x.col = c
                cluster.push(x)
                clusterEnd = Math.max(clusterEnd, visualEnd)
                out.push(x)
            }
            flush()
        }
        return out
    }

    /// All-day occurrences as bars over days: {o, from, to (day index), row}.
    readonly property var bars: {
        const out = []
        const rows = []
        const list = calendar.occs.filter(o => o.allDay).sort((a, b) => calendar.parse(a.start) - calendar.parse(b.start))
        const weekEnd = calendar.addDays(first, 7)
        for (const o of list) {
            const s = calendar.parse(o.start), e = calendar.parse(o.end)
            if (e <= first || s >= weekEnd) continue
            const from = Math.max(0, Math.round((calendar.dayStart(s) - first) / 86400000))
            const to = Math.min(6, Math.round((calendar.dayStart(new Date(e.getTime() - 1)) - first) / 86400000))
            let row = rows.findIndex(end => end < from)
            if (row < 0) { row = rows.length; rows.push(-1) }
            rows[row] = to
            out.push({ o: o, from: from, to: Math.max(from, to), row: row })
        }
        return out
    }
    readonly property int barRows: bars.reduce((m, b) => Math.max(m, b.row + 1), 0)

    function minutesAt(y) { return Math.max(0, Math.min(24 * 60 - snap, Math.round(y / hourHeight * 60 / snap) * snap)) }
    function dayAt(x) { return Math.max(0, Math.min(6, Math.floor((x - gutter) / dayWidth))) }
    function timeAt(x, y) { return calendar.addMinutes(days[dayAt(x)], minutesAt(y)) }
    function segmentAt(x, y) {
        const d = dayAt(x)
        const t = timeAt(x, y).getTime()
        return segments.find(sg => sg.day === d && sg.s <= t && Math.max(sg.e, sg.s + 25 * 60000) > t)
    }

    function scrollToMorning() {
        const now = new Date()
        const inWeek = now >= first && now < calendar.addDays(first, 7)
        grid.contentY = Math.max(0, Math.min(grid.contentHeight - grid.height, ((inWeek ? now.getHours() - 2 : 7)) * hourHeight))
    }
    Component.onCompleted: Qt.callLater(scrollToMorning)
    onFirstChanged: Qt.callLater(scrollToMorning)

    // Current time, refreshed every minute.
    property date now: new Date()
    Timer { interval: 60000; running: true; repeat: true; onTriggered: week.now = new Date() }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // Day names.
        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: 30
            Repeater {
                model: week.days
                Rectangle {
                    required property var modelData
                    required property int index
                    x: week.gutter + index * week.dayWidth
                    width: week.dayWidth
                    height: parent.height
                    color: week.calendar.sameDay(modelData, week.now) ? Qt.rgba(0, 0.47, 0.83, 0.15) : "transparent"
                    Label {
                        anchors.centerIn: parent
                        text: week.calendar.dayNames[index] + " " + parent.modelData.getDate() + "."
                        color: week.calendar.sameDay(parent.modelData, week.now) ? Theme.textStrong : Theme.textMuted
                        font.bold: week.calendar.sameDay(parent.modelData, week.now)
                        font.pixelSize: Theme.smallFontSize
                    }
                    MouseArea {
                        anchors.fill: parent
                        onDoubleClicked: week.calendar.newEntry({ kind: "event", allDay: true, start: week.calendar.isoDate(parent.modelData) })
                    }
                }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }

        // All-day row: deadlines, all-day appointments.
        Item {
            id: allDayRow
            Layout.fillWidth: true
            Layout.preferredHeight: Math.max(26, week.barRows * 22 + 6)
            Label {
                x: 4
                anchors.verticalCenter: parent.verticalCenter
                width: week.gutter - 8
                text: qsTr("ganztägig")
                color: Theme.textFaint
                font.pixelSize: 9
                horizontalAlignment: Text.AlignRight
            }
            Repeater {
                model: 7
                Rectangle {
                    required property int index
                    x: week.gutter + index * week.dayWidth
                    width: 1
                    height: parent.height
                    color: Theme.border
                }
            }
            MouseArea {
                x: week.gutter
                width: parent.width - week.gutter
                height: parent.height
                onClicked: week.calendar.select(null)
                onDoubleClicked: (mouse) => week.calendar.newEntry({ kind: "event", allDay: true, start: week.calendar.isoDate(week.days[week.dayAt(mouse.x + week.gutter)]) })
            }
            DropArea {
                anchors.fill: parent
                keys: ["text/uri-list", "text/x-atlas-task"]
                onDropped: (drop) => {
                    const day = week.days[week.dayAt(drop.x)]
                    if (drop.hasFormat("text/x-atlas-task")) {
                        week.calendar.scheduleTask(drop.getDataAsString("text/x-atlas-task"), week.calendar.addMinutes(day, 9 * 60))
                    } else {
                        const bar = week.bars.find(b => b.from <= week.dayAt(drop.x) && b.to >= week.dayAt(drop.x))
                        const paths = drop.urls.map(u => week.calendar.app.urlToPath(u.toString()))
                        if (bar) week.calendar.linkFiles(bar.o, paths)
                        else week.calendar.blockForFiles(paths, week.calendar.addMinutes(day, 9 * 60))
                    }
                    drop.accept(Qt.LinkAction)
                }
            }
            Repeater {
                model: week.bars
                Rectangle {
                    id: bar
                    required property var modelData
                    readonly property var o: modelData.o
                    property real dragX: 0
                    x: week.gutter + modelData.from * week.dayWidth + 2 + dragX
                    y: 3 + modelData.row * 22
                    width: (modelData.to - modelData.from + 1) * week.dayWidth - 4
                    height: 19
                    radius: 3
                    color: week.calendar.selected === o.uid ? week.calendar.colorOf(o) : Qt.darker(week.calendar.colorOf(o), 2.2)
                    border.color: week.calendar.colorOf(o)
                    opacity: o.status === "done" ? 0.55 : 1
                    z: barMouse.drag.active ? 10 : 1
                    Label {
                        anchors.fill: parent
                        anchors.leftMargin: 6
                        anchors.rightMargin: 4
                        verticalAlignment: Text.AlignVCenter
                        text: (bar.o.kind === "deadline" || bar.o.kind === "note" ? "⚑ " : "") + bar.o.title + (bar.o.recurring ? " ↻" : "")
                            + (bar.o.files && bar.o.files.length ? "  📎" + bar.o.files.length : "")
                        color: "white"
                        font.strikeout: bar.o.status === "done"
                        font.pixelSize: Theme.smallFontSize
                        elide: Text.ElideRight
                    }
                    MouseArea {
                        id: barMouse
                        anchors.fill: parent
                        cursorShape: bar.o.readOnly ? Qt.PointingHandCursor : Qt.SizeAllCursor
                        property real pressX: 0
                        onPressed: (mouse) => { pressX = mouse.x; week.calendar.select(bar.o) }
                        onPositionChanged: (mouse) => { if (!bar.o.readOnly && pressed) bar.dragX = mouse.x - pressX }
                        onReleased: {
                            const days = Math.round(bar.dragX / week.dayWidth)
                            bar.dragX = 0
                            if (days !== 0) {
                                const s = week.calendar.addDays(week.calendar.parse(bar.o.start), days)
                                const e = week.calendar.addDays(week.calendar.parse(bar.o.end), days)
                                week.calendar.move(bar.o, s, e, true)
                            }
                        }
                        onDoubleClicked: week.calendar.activate(bar.o)
                    }
                }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }

        Flickable {
            id: grid
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: width
            contentHeight: 24 * week.hourHeight
            boundsBehavior: Flickable.StopAtBounds
            interactive: false
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            // (All devices: Qt on Wayland reports wheels as a "touchpad";
            // touchpads may send pixel deltas only.)
            WheelHandler {
                acceptedDevices: PointerDevice.AllDevices
                acceptedPointerTypes: PointerDevice.AllPointerTypes
                onWheel: (event) => {
                    const dy = event.angleDelta.y ? event.angleDelta.y / 2 : event.pixelDelta.y
                    grid.contentY = Math.max(0, Math.min(grid.contentHeight - grid.height, grid.contentY - dy))
                }
            }

            Item {
                id: canvas
                width: grid.width
                height: grid.contentHeight

                // Hour lines and labels.
                Repeater {
                    model: 24
                    Item {
                        required property int index
                        y: index * week.hourHeight
                        width: canvas.width
                        height: week.hourHeight
                        Label {
                            x: 4
                            y: -7
                            width: week.gutter - 10
                            visible: parent.index > 0
                            text: (parent.index < 10 ? "0" : "") + parent.index + ":00"
                            color: Theme.textFaint
                            font.pixelSize: 10
                            horizontalAlignment: Text.AlignRight
                        }
                        Rectangle { x: week.gutter; width: parent.width - week.gutter; height: 1; color: Theme.border; opacity: 0.7 }
                        Rectangle { x: week.gutter; y: week.hourHeight / 2; width: parent.width - week.gutter; height: 1; color: Theme.border; opacity: 0.25 }
                    }
                }
                Repeater {
                    model: 7
                    Rectangle {
                        required property int index
                        x: week.gutter + index * week.dayWidth
                        width: week.dayWidth
                        height: canvas.height
                        color: week.calendar.sameDay(week.days[index], week.now) ? Qt.rgba(0, 0.47, 0.83, 0.05) : index >= 5 ? Qt.rgba(1, 1, 1, 0.015) : "transparent"
                        Rectangle { width: 1; height: parent.height; color: Theme.border }
                    }
                }

                // Free time: drag a range or double-click to create an entry.
                MouseArea {
                    id: freeArea
                    x: week.gutter
                    width: canvas.width - week.gutter
                    height: canvas.height
                    property var from: null
                    property var to: null
                    onPressed: (mouse) => { from = week.timeAt(mouse.x + week.gutter, mouse.y); to = null; week.calendar.forceActiveFocus() }
                    onPositionChanged: (mouse) => {
                        if (!pressed || !from) return
                        const t = week.calendar.addMinutes(week.timeAt(mouse.x + week.gutter, mouse.y), week.snap)
                        // Ranges stay within one day.
                        to = week.calendar.sameDay(t, from) || t.getHours() + t.getMinutes() === 0 ? t : null
                    }
                    onReleased: {
                        if (from && to && to > from) week.calendar.newEntry({ kind: "event", start: week.calendar.isoTime(from), end: week.calendar.isoTime(to) })
                        else week.calendar.select(null)
                        from = null
                        to = null
                    }
                    onDoubleClicked: (mouse) => {
                        const t = week.timeAt(mouse.x + week.gutter, mouse.y)
                        const s = new Date(t.getFullYear(), t.getMonth(), t.getDate(), t.getHours())
                        week.calendar.newEntry({ kind: "event", start: week.calendar.isoTime(s), end: week.calendar.isoTime(week.calendar.addMinutes(s, 60)) })
                    }
                }
                Rectangle {
                    visible: !!freeArea.from && !!freeArea.to
                    readonly property int dayIndex: freeArea.from ? Math.floor((week.calendar.dayStart(freeArea.from) - week.first) / 86400000 + 0.5) : 0
                    x: week.gutter + dayIndex * week.dayWidth + 2
                    width: week.dayWidth - 4
                    y: freeArea.from ? (freeArea.from.getHours() * 60 + freeArea.from.getMinutes()) / 60 * week.hourHeight : 0
                    height: freeArea.from && freeArea.to ? (freeArea.to - freeArea.from) / 3600000 * week.hourHeight : 0
                    color: Qt.rgba(0, 0.47, 0.83, 0.3)
                    border.color: Theme.accent
                    radius: 3
                    Label {
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.margins: 4
                        text: freeArea.from && freeArea.to ? week.calendar.fmtTime(freeArea.from) + "–" + week.calendar.fmtTime(freeArea.to) : ""
                        color: "white"
                        font.pixelSize: Theme.smallFontSize
                    }
                }

                // Files and tasks dropped on the grid.
                DropArea {
                    id: gridDrop
                    anchors.fill: parent
                    keys: ["text/uri-list", "text/x-atlas-task"]
                    property var hover: null
                    property bool overEntry: false
                    onPositionChanged: (drag) => {
                        hover = drag.x > week.gutter ? week.timeAt(drag.x, drag.y) : null
                        overEntry = !!week.segmentAt(drag.x, drag.y)
                    }
                    onExited: hover = null
                    onDropped: (drop) => {
                        hover = null
                        if (drop.x < week.gutter) return
                        const t = week.timeAt(drop.x, drop.y)
                        if (drop.hasFormat("text/x-atlas-task")) {
                            week.calendar.scheduleTask(drop.getDataAsString("text/x-atlas-task"), t)
                        } else {
                            const paths = drop.urls.map(u => week.calendar.app.urlToPath(u.toString())).filter(p => p)
                            const seg = week.segmentAt(drop.x, drop.y)
                            if (seg) week.calendar.linkFiles(seg.o, paths)
                            else week.calendar.blockForFiles(paths, t)
                        }
                        drop.accept(Qt.LinkAction)
                    }
                }
                Rectangle {
                    visible: !!gridDrop.hover && !gridDrop.overEntry
                    x: gridDrop.hover ? week.gutter + Math.floor((week.calendar.dayStart(gridDrop.hover) - week.first) / 86400000 + 0.5) * week.dayWidth + 2 : 0
                    y: gridDrop.hover ? (gridDrop.hover.getHours() * 60 + gridDrop.hover.getMinutes()) / 60 * week.hourHeight : 0
                    width: week.dayWidth - 4
                    height: week.hourHeight
                    radius: 3
                    color: Qt.rgba(0.18, 0.63, 0.26, 0.25)
                    border.color: Theme.ok
                    Label {
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.margins: 4
                        text: gridDrop.hover ? week.calendar.fmtTime(gridDrop.hover) + qsTr(" · Arbeitsblock") : ""
                        color: "white"
                        font.pixelSize: Theme.smallFontSize
                    }
                }

                // Entries.
                Repeater {
                    model: week.segments
                    Rectangle {
                        id: item
                        required property var modelData
                        readonly property var o: modelData.o
                        readonly property real baseX: week.gutter + modelData.day * week.dayWidth + 2 + modelData.col * (week.dayWidth - 4) / modelData.cols
                        readonly property real baseY: (modelData.s - week.days[modelData.day].getTime()) / 3600000 * week.hourHeight
                        readonly property real baseHeight: Math.max(20, (modelData.e - modelData.s) / 3600000 * week.hourHeight - 1)
                        property real dragDX: 0
                        property real dragDY: 0
                        property real extra: 0
                        readonly property bool point: modelData.e - modelData.s <= 60000
                        x: baseX + dragDX
                        y: baseY + dragDY
                        width: (week.dayWidth - 4) / modelData.cols - 2
                        height: baseHeight + extra
                        radius: 3
                        z: dragging ? 20 : 2
                        readonly property bool dragging: dragDX !== 0 || dragDY !== 0 || extra !== 0
                        readonly property color tone: week.calendar.colorOf(o)
                        color: week.calendar.selected === o.uid ? Qt.darker(tone, 1.3) : Qt.darker(tone, 2.4)
                        border.color: week.calendar.selected === o.uid ? "white" : tone
                        border.width: week.calendar.selected === o.uid ? 1 : 0
                        opacity: o.status === "done" ? 0.55 : 1
                        Rectangle { width: 3; height: parent.height; radius: 2; color: item.tone }
                        Column {
                            x: 7
                            y: 2
                            width: parent.width - 10
                            spacing: 0
                            Label {
                                width: parent.width
                                text: (item.o.kind === "deadline" || item.o.kind === "note" ? "⚑ " : item.o.kind === "block" ? "▣ " : "")
                                    + item.o.title + (item.o.recurring ? " ↻" : "")
                                color: "white"
                                font.pixelSize: Theme.smallFontSize
                                font.bold: true
                                font.strikeout: item.o.status === "done"
                                elide: Text.ElideRight
                                wrapMode: item.height > 40 ? Text.Wrap : Text.NoWrap
                                maximumLineCount: Math.max(1, Math.floor((item.height - 18) / 14))
                            }
                            Label {
                                width: parent.width
                                visible: item.height > 34
                                text: {
                                    const s = new Date(item.modelData.s + item.dragDY / week.hourHeight * 3600000)
                                    const e = new Date(item.modelData.e + (item.dragDY + item.extra) / week.hourHeight * 3600000)
                                    return week.calendar.fmtTime(s) + (item.point ? "" : "–" + week.calendar.fmtTime(e))
                                        + (item.o.files && item.o.files.length ? "  📎" + item.o.files.length : "")
                                        + (item.o.session ? "  ⧉" : "")
                                        + (item.o.checklist && item.o.checklist[1] ? "  ☑" + item.o.checklist[0] + "/" + item.o.checklist[1] : "")
                                }
                                color: Qt.rgba(1, 1, 1, 0.75)
                                font.pixelSize: 10
                                elide: Text.ElideRight
                            }
                        }
                        MouseArea {
                            anchors.fill: parent
                            anchors.bottomMargin: item.o.readOnly || item.point ? 0 : 6
                            cursorShape: item.o.readOnly ? Qt.PointingHandCursor : Qt.SizeAllCursor
                            property real px: 0
                            property real py: 0
                            onPressed: (mouse) => { px = mouse.x; py = mouse.y; week.calendar.select(item.o); week.calendar.forceActiveFocus() }
                            onPositionChanged: (mouse) => {
                                if (!pressed || item.o.readOnly) return
                                const dx = mouse.x - px, dy = mouse.y - py
                                if (Math.abs(dx) < 4 && Math.abs(dy) < 4 && !item.dragging) return
                                item.dragDX = Math.round(dx / week.dayWidth) * week.dayWidth
                                item.dragDY = Math.round(dy / week.hourHeight * 60 / week.snap) * week.snap / 60 * week.hourHeight
                            }
                            onReleased: {
                                const days = Math.round(item.dragDX / week.dayWidth)
                                const minutes = Math.round(item.dragDY / week.hourHeight * 60)
                                item.dragDX = 0
                                item.dragDY = 0
                                if (days === 0 && minutes === 0) return
                                const s = week.calendar.addMinutes(week.calendar.addDays(week.calendar.parse(item.o.start), days), minutes)
                                const e = week.calendar.addMinutes(week.calendar.addDays(week.calendar.parse(item.o.end), days), minutes)
                                week.calendar.move(item.o, s, e, false)
                            }
                            onDoubleClicked: week.calendar.activate(item.o)
                        }
                        // Lower edge: change the duration.
                        MouseArea {
                            visible: !item.o.readOnly && !item.point
                            anchors.bottom: parent.bottom
                            width: parent.width
                            height: 6
                            cursorShape: Qt.SizeVerCursor
                            property real py: 0
                            onPressed: (mouse) => { py = mouse.y; week.calendar.select(item.o) }
                            onPositionChanged: (mouse) => {
                                if (!pressed) return
                                const step = week.snap / 60 * week.hourHeight
                                item.extra = Math.max(step - item.baseHeight, Math.round((mouse.y - py) / step) * step)
                            }
                            onReleased: {
                                const minutes = Math.round(item.extra / week.hourHeight * 60)
                                item.extra = 0
                                if (minutes === 0) return
                                const s = week.calendar.parse(item.o.start)
                                const e = week.calendar.addMinutes(week.calendar.parse(item.o.end), minutes)
                                if (e > s) week.calendar.move(item.o, s, e, false)
                            }
                        }
                        ToolTip.visible: hover.hovered && !item.dragging && item.height < 34
                        ToolTip.delay: 500
                        ToolTip.text: o.title + " · " + week.calendar.fmtTime(week.calendar.parse(o.start)) + (point ? "" : "–" + week.calendar.fmtTime(week.calendar.parse(o.end)))
                        HoverHandler { id: hover }
                    }
                }

                // Now.
                Rectangle {
                    readonly property int day: Math.floor((week.calendar.dayStart(week.now) - week.first) / 86400000 + 0.5)
                    visible: day >= 0 && day < 7
                    x: week.gutter + day * week.dayWidth - 3
                    y: (week.now.getHours() * 60 + week.now.getMinutes()) / 60 * week.hourHeight - 1
                    width: week.dayWidth + 3
                    height: 2
                    color: Theme.error
                    z: 30
                    Rectangle { width: 7; height: 7; radius: 4; y: -2.5; color: Theme.error }
                }
            }
        }
    }
}
