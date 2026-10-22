// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Optional modules at the top of the context bar: a pomodoro timer for
// learning, the next calendar entries and the open tasks. Each can be
// switched off in the settings (`[context]` in config.toml).
ColumnLayout {
    id: modules
    required property AppController app
    /// FileActions: calendar model, messages, opening the calendar.
    required property var actions

    readonly property var settings: { try { return JSON.parse(app.contextModules || "{}") } catch (e) { return {} } }
    readonly property bool showPomodoro: settings.pomodoro !== false
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
    /// "Heute", "Morgen" or "Mi 24.10.".
    function dayLabel(d) {
        const today = new Date(); today.setHours(0, 0, 0, 0)
        const day = new Date(d); day.setHours(0, 0, 0, 0)
        const diff = Math.round((day - today) / 86400000)
        if (diff === 0) return qsTr("Heute")
        if (diff === 1) return qsTr("Morgen")
        return weekdays[d.getDay()] + " " + d.getDate() + "." + (d.getMonth() + 1) + "."
    }
    readonly property var kindColors: ({ event: "#4fa3ff", block: "#2ea043", deadline: "#f85149", task: "#d7ba7d", note: "#c792ea" })

    /// Entries of the next seven days that are not over or done, with note
    /// deadlines; sorted, at most `limit`.
    property var upcoming: []
    property var openTasks: []
    readonly property int limit: 5
    function reload() {
        if (!model) return
        const now = new Date()
        const until = new Date(now.getTime() + 7 * 86400000)
        let list = []
        if (showCalendar) {
            try { list = JSON.parse(model.occurrences(isoTime(new Date(now.getFullYear(), now.getMonth(), now.getDate())), isoTime(until))) } catch (e) {}
            list = list.filter(o => o.status !== "done" && (parse(o.end) || parse(o.start)) > now)
            let notes = []
            try { notes = JSON.parse(app.noteDeadlines()) } catch (e) {}
            for (const n of notes) {
                const due = parse(n.due)
                if (!due || n.status === "done" || due > until || (n.due.length === 10 ? due.getTime() + 86400000 : due.getTime()) < now.getTime()) continue
                list.push({ uid: "note:" + n.path, kind: "note", title: n.title, start: n.due, allDay: n.due.length === 10, path: n.path })
            }
            list.sort((a, b) => parse(a.start) - parse(b.start))
        }
        upcoming = list
        let tasks = []
        if (showTasks) try { tasks = JSON.parse(model.tasks()).filter(t => t.status !== "done") } catch (e) {}
        openTasks = tasks
    }
    Component.onCompleted: reload()
    onShowCalendarChanged: reload()
    onShowTasksChanged: reload()
    Connections { target: modules.model; function onRevisionChanged() { modules.reload() } }
    Connections { target: modules.app; function onNotesRevisionChanged() { modules.reload() } }
    // Past entries drop out as time goes by.
    Timer { interval: 60000; running: modules.showCalendar; repeat: true; onTriggered: modules.reload() }

    function openEntry(o) {
        if (o.kind === "note") { actions.openRequested(o.path, false); return }
        actions.calendarRequested({ mode: "week", date: (o.start || "").slice(0, 10) || isoDate(new Date()), selected: o.uid })
    }

    // ---------------------------------------------------------- pomodoro

    /// "work", "break" or "long".
    property string phase: "work"
    property bool running: false
    /// Remaining seconds while paused, and the end time while running.
    property int remaining: minutesOf("work") * 60
    property real endsAt: 0
    /// Finished work rounds; every fourth is followed by a long break.
    property int rounds: 0
    function minutesOf(p) {
        return p === "work" ? (settings.pomodoro_work || 25) : p === "break" ? (settings.pomodoro_break || 5) : (settings.pomodoro_long_break || 15)
    }
    function phaseText(p) { return p === "work" ? qsTr("Lernen") : p === "break" ? qsTr("Kurze Pause") : qsTr("Lange Pause") }
    function startPause() {
        if (running) {
            remaining = Math.max(0, Math.round((endsAt - Date.now()) / 1000))
            running = false
        } else {
            endsAt = Date.now() + remaining * 1000
            running = true
        }
    }
    function resetTimer() {
        running = false
        remaining = minutesOf(phase) * 60
    }
    /// Ends the phase (also by hand: „Überspringen“) and prepares the next.
    function nextPhase(finished) {
        if (phase === "work") {
            if (finished) rounds += 1
            phase = rounds > 0 && rounds % 4 === 0 && finished ? "long" : "break"
        } else {
            phase = "work"
        }
        running = false
        remaining = minutesOf(phase) * 60
        if (finished && actions)
            actions.showMessage(phase === "work" ? qsTr("Pause vorbei – weiter geht's") : qsTr("Pomodoro geschafft – Zeit für eine %1").arg(phaseText(phase)))
    }
    // A new length in the settings applies at once while the timer stands.
    readonly property int phaseMinutes: minutesOf(phase)
    onPhaseMinutesChanged: if (!running) remaining = phaseMinutes * 60
    Timer {
        interval: 250
        running: modules.running
        repeat: true
        onTriggered: {
            const left = Math.max(0, Math.round((modules.endsAt - Date.now()) / 1000))
            modules.remaining = left
            if (left === 0) modules.nextPhase(true)
        }
    }
    readonly property string clock: Math.floor(remaining / 60) + ":" + pad(remaining % 60)

    // ------------------------------------------------------------- view

    Card {
        visible: modules.showPomodoro
        implicitHeight: pomodoroColumn.implicitHeight + 20
        ColumnLayout {
            id: pomodoroColumn
            x: 12; y: 10
            width: parent.width - 24
            spacing: 6
            RowLayout {
                Layout.fillWidth: true
                Header { Layout.fillWidth: true; icon: "focus"; text: qsTr("Lernzeit") }
                Row {
                    spacing: 3
                    Repeater {
                        model: 4
                        Rectangle {
                            required property int index
                            width: 7; height: 7; radius: 3.5
                            color: index < modules.rounds % 4 || (modules.rounds > 0 && modules.rounds % 4 === 0 && modules.phase === "long") ? Theme.accent : Theme.border
                        }
                    }
                }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 10
                Text {
                    text: modules.clock
                    color: modules.phase === "work" ? Theme.textStrong : Theme.ok
                    font.pixelSize: 28
                    font.weight: Font.DemiBold
                    font.family: Theme.monoFont
                }
                Text {
                    Layout.fillWidth: true
                    text: modules.phaseText(modules.phase) + (modules.running ? "" : qsTr(" · angehalten"))
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                }
            }
            // Progress of the phase.
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 3
                radius: 1.5
                color: Theme.border
                Rectangle {
                    width: parent.width * (1 - modules.remaining / Math.max(1, modules.minutesOf(modules.phase) * 60))
                    height: parent.height
                    radius: 1.5
                    color: modules.phase === "work" ? Theme.accent : Theme.ok
                }
            }
            Flow {
                Layout.fillWidth: true
                spacing: 4
                Button {
                    text: modules.running ? qsTr("Anhalten") : qsTr("Start")
                    highlighted: !modules.running
                    onClicked: modules.startPause()
                }
                Button { flat: true; text: qsTr("Zurücksetzen"); onClicked: modules.resetTimer() }
                Button { flat: true; text: qsTr("Überspringen"); onClicked: modules.nextPhase(false) }
            }
        }
    }

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
                Header { Layout.fillWidth: true; icon: "calendar"; text: qsTr("Demnächst") }
                IconButton { iconName: "chevron-right"; iconSize: 14; tip: qsTr("Kalender öffnen"); onClicked: modules.actions.calendarRequested({}) }
            }
            Repeater {
                model: modules.upcoming.slice(0, modules.limit)
                delegate: Rectangle {
                    id: entry
                    required property var modelData
                    readonly property var start: modules.parse(modelData.start)
                    Layout.fillWidth: true
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
                                text: !entry.start ? "" : modules.dayLabel(entry.start)
                                      + (entry.modelData.allDay ? (entry.modelData.kind === "deadline" || entry.modelData.kind === "note" ? qsTr(" · Frist") : "")
                                                                : " · " + modules.pad(entry.start.getHours()) + ":" + modules.pad(entry.start.getMinutes()))
                                color: Theme.textMuted
                                font.pixelSize: Theme.smallFontSize
                                elide: Text.ElideRight
                            }
                        }
                    }
                }
            }
            Text {
                visible: modules.upcoming.length === 0
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: qsTr("Nichts in den nächsten sieben Tagen.")
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
            }
            Text {
                visible: modules.upcoming.length > modules.limit
                text: qsTr("und %n weitere", "", modules.upcoming.length - modules.limit)
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
