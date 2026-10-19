// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Month view: six weeks; deadlines and all-day entries first, a bar shows
// how much of the day is planned. Entries are moved to another day by
// dragging; tasks and files dropped on a day are planned at 09:00.
Item {
    id: month
    required property var calendar

    readonly property date first: calendar.range[0]
    readonly property real cellWidth: width / 7
    readonly property real cellHeight: (height - 26) / 6
    readonly property int maxItems: Math.max(1, Math.floor((cellHeight - 24) / 17))

    /// Occurrences per day index (0..41), all-day and deadlines first.
    readonly property var perDay: {
        const out = []
        for (let i = 0; i < 42; i++) out.push([])
        for (const o of calendar.occs) {
            const s = calendar.parse(o.start), e = calendar.parse(o.end)
            let from = Math.max(0, Math.round((calendar.dayStart(s) - first) / 86400000))
            let to = Math.min(41, Math.round((calendar.dayStart(new Date(e.getTime() - 1)) - first) / 86400000))
            for (let d = from; d <= Math.max(from, to); d++) if (d >= 0 && d < 42) out[d].push(o)
        }
        const rank = (o) => o.kind === "deadline" || o.kind === "note" ? 0 : o.allDay ? 1 : 2
        for (const list of out) list.sort((a, b) => rank(a) - rank(b) || calendar.parse(a.start) - calendar.parse(b.start))
        return out
    }
    /// Planned hours of a day (appointments and work blocks).
    function load(list) {
        return list.filter(o => !o.allDay && (o.kind === "event" || o.kind === "block"))
            .reduce((sum, o) => sum + (calendar.parse(o.end) - calendar.parse(o.start)) / 3600000, 0)
    }

    property int dragFrom: -1
    property int dragTo: -1
    property var dragged: null

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        Row {
            Layout.preferredHeight: 26
            Repeater {
                model: month.calendar.dayNames
                Label {
                    required property string modelData
                    width: month.cellWidth
                    height: 26
                    text: modelData
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                }
            }
        }
        Grid {
            id: cells
            Layout.fillWidth: true
            Layout.fillHeight: true
            columns: 7
            Repeater {
                model: 42
                Rectangle {
                    id: cell
                    required property int index
                    readonly property date day: month.calendar.addDays(month.first, index)
                    readonly property bool inMonth: day.getMonth() === month.calendar.anchor.getMonth()
                    readonly property bool isToday: month.calendar.sameDay(day, new Date())
                    readonly property var items: month.perDay[index] || []
                    readonly property real hours: month.load(items)
                    width: month.cellWidth
                    height: month.cellHeight
                    color: month.dragTo === index ? Theme.hover : drop.containsDrag ? Qt.rgba(0.18, 0.63, 0.26, 0.2) : inMonth ? "transparent" : Qt.rgba(0, 0, 0, 0.18)
                    border.color: Theme.border
                    border.width: 0.5
                    MouseArea {
                        anchors.fill: parent
                        onClicked: month.calendar.select(null)
                        onDoubleClicked: month.calendar.newEntry({ kind: "event", allDay: true, start: month.calendar.isoDate(cell.day) })
                    }
                    RowLayout {
                        x: 6
                        y: 3
                        width: parent.width - 12
                        Label {
                            text: cell.day.getDate()
                            color: cell.isToday ? "white" : cell.inMonth ? Theme.text : Theme.textFaint
                            font.bold: cell.isToday
                            font.pixelSize: Theme.smallFontSize
                            background: Rectangle { visible: cell.isToday; radius: 8; color: Theme.accent; anchors.fill: parent; anchors.margins: -2 }
                        }
                        Item { Layout.fillWidth: true }
                        // Load of the day (8 h = full).
                        Rectangle {
                            visible: cell.hours > 0
                            Layout.preferredWidth: 30
                            Layout.preferredHeight: 4
                            radius: 2
                            color: Theme.border
                            ToolTip.visible: loadHover.hovered
                            ToolTip.text: qsTr("%1 h verplant").arg(Math.round(cell.hours * 10) / 10)
                            HoverHandler { id: loadHover }
                            Rectangle {
                                width: parent.width * Math.min(1, cell.hours / 8)
                                height: parent.height
                                radius: 2
                                color: cell.hours > 8 ? Theme.error : cell.hours > 6 ? Theme.warn : Theme.ok
                            }
                        }
                    }
                    Column {
                        x: 3
                        y: 22
                        width: parent.width - 6
                        spacing: 1
                        Repeater {
                            model: cell.items.slice(0, cell.items.length > month.maxItems ? month.maxItems - 1 : month.maxItems)
                            Rectangle {
                                id: chip
                                required property var modelData
                                width: parent.width
                                height: 16
                                radius: 3
                                readonly property bool flag: modelData.kind === "deadline" || modelData.kind === "note"
                                color: month.calendar.selected === modelData.uid ? Theme.selection : modelData.allDay || flag ? Qt.darker(month.calendar.colorOf(modelData), 2.2) : "transparent"
                                opacity: modelData.status === "done" ? 0.55 : 1
                                Row {
                                    x: 3
                                    spacing: 4
                                    anchors.verticalCenter: parent.verticalCenter
                                    Rectangle { visible: !chip.modelData.allDay && !chip.flag; width: 6; height: 6; radius: 3; color: month.calendar.colorOf(chip.modelData); anchors.verticalCenter: parent.verticalCenter }
                                    Label {
                                        width: chip.width - 14
                                        text: (chip.flag ? "⚑ " : chip.modelData.allDay ? "" : month.calendar.fmtTime(month.calendar.parse(chip.modelData.start)) + " ") + chip.modelData.title
                                        color: Theme.textStrong
                                        font.pixelSize: 10
                                        font.strikeout: chip.modelData.status === "done"
                                        elide: Text.ElideRight
                                    }
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    onPressed: { month.calendar.select(chip.modelData); month.dragged = chip.modelData; month.dragFrom = cell.index; month.dragTo = -1 }
                                    onPositionChanged: (mouse) => {
                                        if (!pressed || chip.modelData.readOnly) return
                                        const p = mapToItem(cells, mouse.x, mouse.y)
                                        const target = Math.floor(p.y / month.cellHeight) * 7 + Math.floor(p.x / month.cellWidth)
                                        month.dragTo = target >= 0 && target < 42 && target !== month.dragFrom ? target : -1
                                    }
                                    onReleased: {
                                        if (month.dragTo >= 0 && month.dragged) {
                                            const days = month.dragTo - month.dragFrom
                                            const o = month.dragged
                                            month.calendar.move(o, month.calendar.addDays(month.calendar.parse(o.start), days), month.calendar.addDays(month.calendar.parse(o.end), days), o.allDay)
                                        }
                                        month.dragTo = -1
                                        month.dragged = null
                                    }
                                    onDoubleClicked: month.calendar.activate(chip.modelData)
                                }
                            }
                        }
                        Label {
                            visible: cell.items.length > month.maxItems
                            text: qsTr("+ %1 weitere").arg(cell.items.length - month.maxItems + 1)
                            color: Theme.textMuted
                            font.pixelSize: 10
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: { month.calendar.anchor = cell.day; month.calendar.mode = "week" }
                            }
                        }
                    }
                    DropArea {
                        id: drop
                        anchors.fill: parent
                        keys: ["text/uri-list", "text/x-atlas-task"]
                        onDropped: (event) => {
                            const at = month.calendar.addMinutes(cell.day, 9 * 60)
                            if (event.hasFormat("text/x-atlas-task")) {
                                month.calendar.scheduleTask(event.getDataAsString("text/x-atlas-task"), at)
                            } else {
                                const paths = event.urls.map(u => month.calendar.app.urlToPath(u.toString())).filter(p => p)
                                // On an entry: link; otherwise a work block at 09:00.
                                const row = Math.floor((event.y - 22) / 17)
                                const target = row >= 0 && row < Math.min(cell.items.length, month.maxItems) ? cell.items[row] : null
                                if (target) month.calendar.linkFiles(target, paths)
                                else month.calendar.blockForFiles(paths, at)
                            }
                            event.accept(Qt.LinkAction)
                        }
                    }
                }
            }
        }
    }
}
