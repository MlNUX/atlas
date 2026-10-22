// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Optional modules at the top of the context bar: today's calendar entries
// and the open tasks (the pomodoro timer sits at the bottom edge, see
// PomodoroTimer.qml). Each can be switched off in the settings (`[context]`
// in config.toml).
ColumnLayout {
    id: modules
    required property AppController app
    /// FileActions: calendar model, messages, opening the calendar.
    required property var actions

    readonly property var settings: { try { return JSON.parse(app.contextModules || "{}") } catch (e) { return {} } }
    readonly property bool showCalendar: settings.calendar !== false
    readonly property bool showTasks: settings.tasks !== false
    readonly property var model: actions ? actions.calendar : null
    spacing: 10

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
        let tasks = []
        if (showTasks) try { tasks = JSON.parse(model.tasks()).filter(t => t.status !== "done") } catch (e) {}
        openTasks = tasks
    }
    Component.onCompleted: reload()
    onShowCalendarChanged: reload()
    onShowTasksChanged: reload()
    Connections { target: modules.model; function onRevisionChanged() { modules.reload() } }
    Connections { target: modules.app; function onNotesRevisionChanged() { modules.reload() } }
    // Fades entries that are over and switches to the next day at midnight.
    property real now: Date.now()
    Timer { interval: 60000; running: modules.showCalendar; repeat: true; onTriggered: { modules.now = Date.now(); modules.reload() } }
    function isOver(o) { return !o.allDay && (parse(o.end) || parse(o.start)) < new Date(now) }
    readonly property string todayText: { const d = new Date(now); return weekdays[d.getDay()] + ", " + d.getDate() + "." + (d.getMonth() + 1) + "." }

    function openEntry(o) {
        if (o.kind === "note") { actions.openRequested(o.path, false); return }
        actions.calendarRequested({ mode: "week", date: (o.start || "").slice(0, 10) || isoDate(new Date()), selected: o.uid })
    }

    // ------------------------------------------------------------- view

    Card {
        visible: modules.showCalendar
        implicitHeight: calendarColumn.implicitHeight + 20
        ColumnLayout {
            id: calendarColumn
            x: 12; y: 10
            width: parent.width - 24
            spacing: 4
            RowLayout {
                Layout.fillWidth: true
                Header { Layout.fillWidth: true; icon: "calendar"; text: qsTr("Heute") }
                Text { text: modules.todayText; color: Theme.textMuted; font.pixelSize: Theme.smallFontSize }
                IconButton { iconName: "chevron-right"; iconSize: 14; tip: qsTr("Kalender öffnen"); onClicked: modules.actions.calendarRequested({}) }
            }
            Repeater {
                model: modules.today.slice(0, modules.limit)
                delegate: Rectangle {
                    id: entry
                    required property var modelData
                    readonly property var start: modules.parse(modelData.start)
                    readonly property var end: modules.parse(modelData.end)
                    Layout.fillWidth: true
                    opacity: modules.now && modules.isOver(modelData) ? 0.5 : 1
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
                                    : entry.modelData.kind === "deadline" || entry.modelData.kind === "note" ? qsTr("Frist heute")
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

    Card {
        visible: modules.showTasks
        implicitHeight: tasksColumn.implicitHeight + 20
        ColumnLayout {
            id: tasksColumn
            x: 12; y: 10
            width: parent.width - 24
            spacing: 2
            Header { Layout.fillWidth: true; icon: "check-square"; text: qsTr("Aufgaben") + (modules.openTasks.length ? "  " + modules.openTasks.length : "") }
            Repeater {
                model: modules.openTasks.slice(0, 6)
                delegate: RowLayout {
                    id: task
                    required property var modelData
                    Layout.fillWidth: true
                    spacing: 4
                    CheckBox {
                        id: done
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
                id: newTask
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

    component Card: Rectangle {
        Layout.fillWidth: true
        Layout.leftMargin: 10
        Layout.rightMargin: 10
        radius: 10
        color: Theme.header
        border.color: Theme.border
    }
    component Header: RowLayout {
        property string icon
        property string text
        spacing: 6
        Icon { name: parent.icon; size: 14; color: Theme.accent }
        Text {
            Layout.fillWidth: true
            text: parent.text
            color: Theme.textStrong
            font.pixelSize: Theme.fontSize
            font.bold: true
            elide: Text.ElideRight
        }
    }
}
