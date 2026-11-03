// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Calendar blocks of the context bar: „Heute“, „Diese Woche“ and
// „Aufgaben“ (see ContextProfiles.qml). This item holds their data; the
// views are components the bar puts into its blocks (ContextBlock.qml).
Item {
    id: modules
    required property AppController app
    /// FileActions: calendar model, messages, opening the calendar.
    required property var actions
    /// Which blocks are placed (set by the bar): only those load data.
    property bool showCalendar: true
    property bool showWeek: false
    property bool showTasks: true

    readonly property var model: actions ? actions.calendar : null
    visible: false

    // ------------------------------------------------------------- data

    function pad(n) { return (n < 10 ? "0" : "") + n }
    function isoDate(d) { return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate()) }
    function isoTime(d) { return isoDate(d) + "T" + pad(d.getHours()) + ":" + pad(d.getMinutes()) }
    function parse(s) {
        const m = /^(\d{4})-(\d{2})-(\d{2})(?:T(\d{2}):(\d{2}))?/.exec(s || "")
        return m ? new Date(+m[1], +m[2] - 1, +m[3], +(m[4] || 0), +(m[5] || 0)) : null
    }
    readonly property var weekdays: [qsTr("So"), qsTr("Mo"), qsTr("Di"), qsTr("Mi"), qsTr("Do"), qsTr("Fr"), qsTr("Sa")]
    function hm(d) { return pad(d.getHours()) + ":" + pad(d.getMinutes()) }
    readonly property var kindColors: ({ event: "#4fa3ff", block: "#2ea043", deadline: "#f85149", task: "#d7ba7d", note: "#c792ea" })

    /// Today's entries (also those already over, shown faded) with note
    /// deadlines of today; sorted, at most `limit`.
    property var today: []
    /// The next seven days after today: [{day, label, entries}].
    property var week: []
    property var openTasks: []
    readonly property int limit: 6
    function reload() {
        if (!model) return
        const now = new Date()
        const start = new Date(now.getFullYear(), now.getMonth(), now.getDate())
        const end = new Date(start.getTime() + 86400000)
        let list = []
        if (showCalendar) {
            try { list = JSON.parse(model.occurrences(isoTime(start), isoTime(end))) } catch (e) {}
            list = list.filter(o => o.status !== "done")
            let notes = []
            try { notes = JSON.parse(app.noteDeadlines()) } catch (e) {}
            for (const n of notes) {
                const due = parse(n.due)
                if (!due || n.status === "done" || due < start || due >= end) continue
                list.push({ uid: "note:" + n.path, kind: "note", title: n.title, start: n.due, allDay: n.due.length === 10, path: n.path })
            }
            // All-day entries first, then by time.
            list.sort((a, b) => (b.allDay ? 1 : 0) - (a.allDay ? 1 : 0) || parse(a.start) - parse(b.start))
        }
        today = list
        let days = []
        if (showWeek) {
            const weekEnd = new Date(end.getTime() + 7 * 86400000)
            let occ = []
            try { occ = JSON.parse(model.occurrences(isoTime(end), isoTime(weekEnd))) } catch (e) {}
            occ = occ.filter(o => o.status !== "done")
            let notes = []
            try { notes = JSON.parse(app.noteDeadlines()) } catch (e) {}
            for (const n of notes) {
                const due = parse(n.due)
                if (!due || n.status === "done" || due < end || due >= weekEnd) continue
                occ.push({ uid: "note:" + n.path, kind: "note", title: n.title, start: n.due, allDay: n.due.length === 10, path: n.path })
            }
            occ.sort((a, b) => parse(a.start) - parse(b.start) || (b.allDay ? 1 : 0) - (a.allDay ? 1 : 0))
            for (const o of occ) {
                const d = parse(o.start)
                const key = isoDate(d)
                let day = days.find(x => x.day === key)
                if (!day) { day = { day: key, label: weekdays[d.getDay()] + ", " + d.getDate() + "." + (d.getMonth() + 1) + ".", entries: [] }; days.push(day) }
                day.entries.push(o)
            }
        }
        week = days
        let tasks = []
        if (showTasks) try { tasks = JSON.parse(model.tasks()).filter(t => t.status !== "done") } catch (e) {}
        openTasks = tasks
    }
    Component.onCompleted: reload()
    onShowCalendarChanged: reload()
    onShowTasksChanged: reload()
    onShowWeekChanged: reload()
    Connections { target: modules.model; function onRevisionChanged() { modules.reload() } }
    Connections { target: modules.app; function onNotesRevisionChanged() { modules.reload() } }
    // Fades entries that are over and switches to the next day at midnight.
    property real now: Date.now()
    Timer { interval: 60000; running: modules.showCalendar || modules.showWeek; repeat: true; onTriggered: { modules.now = Date.now(); modules.reload() } }
    function isOver(o) { return !o.allDay && (parse(o.end) || parse(o.start)) < new Date(now) }
    readonly property string todayText: { const d = new Date(now); return weekdays[d.getDay()] + ", " + d.getDate() + "." + (d.getMonth() + 1) + "." }

    function openEntry(o) {
        if (o.kind === "note") { actions.openRequested(o.path, false); return }
        actions.calendarRequested({ mode: "week", date: (o.start || "").slice(0, 10) || isoDate(new Date()), selected: o.uid })
    }

    // ------------------------------------------------------------- views

    /// Number of entries for the block headers.
    readonly property int weekCount: week.reduce((n, d) => n + d.entries.length, 0)

    readonly property Component todayView: Component {
        ColumnLayout {
            x: 12
            width: parent ? parent.width - 24 : 0
            spacing: 2
            RowLayout {
                Layout.fillWidth: true
                Text { Layout.fillWidth: true; text: modules.todayText; color: Theme.textMuted; font.pixelSize: Theme.smallFontSize }
                IconButton { iconName: "chevron-right"; iconSize: 14; tip: qsTr("Kalender öffnen"); onClicked: modules.actions.calendarRequested({}) }
            }
            Repeater {
                model: modules.today.slice(0, modules.limit)
                delegate: EntryRow { }
            }
            Text {
                visible: modules.today.length === 0
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: qsTr("Heute keine Termine.")
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
            }
            Text {
                visible: modules.today.length > modules.limit
                text: qsTr("und %n weitere", "", modules.today.length - modules.limit)
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
            }
        }
    }

    readonly property Component weekView: Component {
        ColumnLayout {
            x: 12
            width: parent ? parent.width - 24 : 0
            spacing: 2
            Repeater {
                model: modules.week
                delegate: ColumnLayout {
                    id: dayColumn
                    required property var modelData
                    Layout.fillWidth: true
                    spacing: 0
                    Text { Layout.topMargin: 4; text: dayColumn.modelData.label; color: Theme.textMuted; font.pixelSize: Theme.smallFontSize; font.bold: true }
                    Repeater { model: dayColumn.modelData.entries; delegate: EntryRow { showDate: false } }
                }
            }
            Text {
                visible: modules.week.length === 0
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: qsTr("In den nächsten sieben Tagen nichts geplant.")
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
            }
        }
    }

    readonly property Component tasksView: Component {
        ColumnLayout {
            x: 12
            width: parent ? parent.width - 24 : 0
            spacing: 2
            Repeater {
                model: modules.openTasks.slice(0, 6)
                delegate: RowLayout {
                    id: task
                    required property var modelData
                    Layout.fillWidth: true
                    spacing: 4
                    CheckBox {
                        padding: 2
                        ToolTip.visible: hovered
                        ToolTip.text: qsTr("Erledigt")
                        onToggled: if (checked) modules.model.setStatus(task.modelData.uid, "done")
                    }
                    Text {
                        Layout.fillWidth: true
                        text: task.modelData.title
                        textFormat: Text.PlainText
                        color: Theme.text
                        font.pixelSize: Theme.fontSize
                        elide: Text.ElideRight
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: modules.actions.calendarRequested({ mode: "week", selected: task.modelData.uid })
                        }
                    }
                }
            }
            Text {
                visible: modules.openTasks.length > 6
                text: qsTr("und %n weitere im Kalender", "", modules.openTasks.length - 6)
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
            }
            TextField {
                Layout.fillWidth: true
                Layout.topMargin: 4
                implicitHeight: 28
                font.pixelSize: Theme.smallFontSize
                placeholderText: qsTr("Neue Aufgabe (Enter)")
                onAccepted: {
                    if (!text.trim() || !modules.model) return
                    modules.model.save(JSON.stringify({ kind: "task", title: text.trim() }))
                    text = ""
                }
            }
        }
    }

    /// An entry of the calendar: colour bar, title, time (or date).
    component EntryRow: Rectangle {
        id: entry
        required property var modelData
        property bool showDate: true
        readonly property var start: modules.parse(modelData.start)
        readonly property var end: modules.parse(modelData.end)
        Layout.fillWidth: true
        opacity: showDate && modules.now && modules.isOver(modelData) ? 0.5 : 1
        implicitHeight: 34
        radius: 6
        color: entryHover.hovered ? Theme.hover : "transparent"
        HoverHandler { id: entryHover }
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: modules.openEntry(entry.modelData) }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 4
            anchors.rightMargin: 4
            spacing: 8
            Rectangle {
                Layout.preferredWidth: 3
                Layout.preferredHeight: 22
                radius: 1.5
                color: entry.modelData.color || modules.kindColors[entry.modelData.kind] || Theme.accent
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0
                Text {
                    Layout.fillWidth: true
                    text: entry.modelData.title
                    textFormat: Text.PlainText
                    color: Theme.textStrong
                    font.pixelSize: Theme.fontSize
                    elide: Text.ElideRight
                }
                Text {
                    Layout.fillWidth: true
                    text: !entry.start ? ""
                        : entry.modelData.kind === "deadline" || entry.modelData.kind === "note" ? (entry.showDate ? qsTr("Frist heute") : qsTr("Frist"))
                        : entry.modelData.allDay ? qsTr("Ganztägig")
                        : modules.hm(entry.start) + (entry.end ? "–" + modules.hm(entry.end) : "")
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                }
            }
        }
    }
}
