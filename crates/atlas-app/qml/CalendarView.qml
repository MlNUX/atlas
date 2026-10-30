// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Project calendar (tab `calendar:`): week, month and agenda, unscheduled
// tasks on the left, details of the selected entry on the right. Files can
// be dropped on entries (link) or on free time (work block); tasks are
// planned by dragging them into the week.
Rectangle {
    id: cal
    required property AppController app
    /// FileActions; `actions.calendar` is the shared CalendarModel.
    property var actions: null
    readonly property var model: actions ? actions.calendar : null
    /// "week", "month" or "agenda".
    property string mode: "week"
    /// A day inside the visible period.
    property date anchor: new Date()
    property bool showTasks: true
    property bool showDetails: true
    /// Selected entry (UID; notes: "note:<path>").
    property string selected: ""
    property var occs: []
    property var tasks: []
    property var noteDeadlines: []
    /// The week, month or agenda view.
    readonly property alias bodyItem: body.item
    readonly property var selectedEntry: selected && model && model.revision >= 0 && !selected.startsWith("note:")
        ? (function () { try { return JSON.parse(model.entry(selected) || "null") } catch (e) { return null } })() : null

    signal openRequested(string path, int line, bool newPane, var state)
    /// Switch to the session with this name (saving the current one).
    signal sessionRequested(string name)

    color: Theme.editor

    // ------------------------------------------------------------- helpers

    readonly property var dayNames: ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"]
    readonly property var monthNames: ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"]
    readonly property var kindNames: ({ event: qsTr("Termin"), block: qsTr("Arbeitsblock"), deadline: qsTr("Frist"), task: qsTr("Aufgabe"), note: qsTr("Notiz-Frist") })
    readonly property var statusNames: ({ planned: qsTr("geplant"), started: qsTr("begonnen"), done: qsTr("erledigt"), postponed: qsTr("verschoben") })
    readonly property var kindColors: ({ event: "#4fa3ff", block: "#2ea043", deadline: "#f85149", task: "#d7ba7d", note: "#c792ea" })

    function pad(n) { return (n < 10 ? "0" : "") + n }
    function isoDate(d) { return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate()) }
    function isoTime(d) { return isoDate(d) + "T" + pad(d.getHours()) + ":" + pad(d.getMinutes()) }
    function parse(s) {
        const m = /^(\d{4})-(\d{2})-(\d{2})(?:T(\d{2}):(\d{2}))?/.exec(s || "")
        return m ? new Date(+m[1], +m[2] - 1, +m[3], +(m[4] || 0), +(m[5] || 0)) : null
    }
    function dayStart(d) { return new Date(d.getFullYear(), d.getMonth(), d.getDate()) }
    function addDays(d, n) { return new Date(d.getFullYear(), d.getMonth(), d.getDate() + n, d.getHours(), d.getMinutes()) }
    function addMinutes(d, n) { return new Date(d.getTime() + n * 60000) }
    function weekStart(d) { const x = dayStart(d); return addDays(x, -((x.getDay() + 6) % 7)) }
    function sameDay(a, b) { return a && b && a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate() }
    function fmtTime(d) { return pad(d.getHours()) + ":" + pad(d.getMinutes()) }
    function fmtDay(d) { return dayNames[(d.getDay() + 6) % 7] + " " + d.getDate() + "." + (d.getMonth() + 1) + "." }
    function fmtLong(d) { return dayNames[(d.getDay() + 6) % 7] + ", " + d.getDate() + ". " + monthNames[d.getMonth()] + " " + d.getFullYear() }
    function isoWeek(d) {
        const t = new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()))
        const day = t.getUTCDay() || 7
        t.setUTCDate(t.getUTCDate() + 4 - day)
        const yearStart = new Date(Date.UTC(t.getUTCFullYear(), 0, 1))
        return Math.ceil(((t - yearStart) / 86400000 + 1) / 7)
    }
    function duration(o) {
        const m = Math.round((parse(o.end) - parse(o.start)) / 60000)
        return m >= 60 ? (Math.floor(m / 60) + " h" + (m % 60 ? " " + (m % 60) + " min" : "")) : m + " min"
    }
    /// Nextcloud entries in their calendar's colour, project entries by kind;
    /// deadlines always stand out.
    function colorOf(o) {
        if (o.kind === "deadline" || o.kind === "note") return kindColors[o.kind]
        if (o.calendar && o.calendar !== "local" && o.color) return o.color
        return kindColors[o.kind] || Theme.accent
    }

    readonly property date today: new Date()
    readonly property var range: {
        if (mode === "week") { const s = weekStart(anchor); return [s, addDays(s, 7)] }
        if (mode === "month") { const s = weekStart(new Date(anchor.getFullYear(), anchor.getMonth(), 1)); return [s, addDays(s, 42)] }
        const s = dayStart(new Date())
        return [addDays(s, -30), addDays(s, 90)]
    }
    readonly property string title: mode === "week"
        ? qsTr("KW %1 · %2 %3").arg(isoWeek(range[0])).arg(monthNames[range[0].getMonth()]).arg(range[0].getFullYear())
        : mode === "month" ? monthNames[anchor.getMonth()] + " " + anchor.getFullYear()
        : qsTr("Agenda")

    function reload() {
        if (!model) return
        let list = []
        try { list = JSON.parse(model.occurrences(isoTime(range[0]), isoTime(range[1]))) } catch (e) {}
        // Notes with `due:` in their front matter (read-only).
        for (const n of noteDeadlines) {
            const due = parse(n.due)
            if (!due || due < range[0] || due >= range[1]) continue
            const allDay = n.due.length === 10
            list.push({ uid: "note:" + n.path, kind: "note", title: n.title, status: n.status === "done" ? "done" : "planned",
                        start: isoTime(due), end: isoTime(allDay ? addDays(due, 1) : addMinutes(due, 1)), allDay: allDay,
                        files: [n.relative], path: n.path, tags: [], checklist: [0, 0], readOnly: true })
        }
        occs = list
        try { tasks = JSON.parse(model.tasks()) } catch (e) { tasks = [] }
    }
    function reloadNotes() {
        try { noteDeadlines = JSON.parse(app.noteDeadlines()) } catch (e) { noteDeadlines = [] }
        reload()
    }
    onRangeChanged: reload()
    Connections { target: cal.model; function onRevisionChanged() { cal.reload() } }
    Connections { target: cal.app; function onNotesRevisionChanged() { cal.reloadNotes() } }
    Component.onCompleted: {
        if (model) {
            try {
                const s = JSON.parse(model.viewState() || "{}")
                if (s.mode) mode = s.mode
                if (s.tasks !== undefined) showTasks = s.tasks
                if (s.details !== undefined) showDetails = s.details
            } catch (e) {}
        }
        reloadNotes()
    }
    function remember() { if (model) model.setViewState(JSON.stringify({ mode: mode, tasks: showTasks, details: showDetails })) }
    onModeChanged: remember()
    onShowTasksChanged: remember()
    onShowDetailsChanged: remember()

    function viewState() { return { mode: mode, date: isoDate(anchor), selected: selected } }
    function applyState(s) {
        if (!s) return
        if (s.mode) mode = s.mode
        if (s.date) anchor = parse(s.date) || anchor
        if (s.selected !== undefined) selected = s.selected
    }

    function step(n) {
        if (mode === "week") anchor = addDays(anchor, 7 * n)
        else if (mode === "month") anchor = new Date(anchor.getFullYear(), anchor.getMonth() + n, 1)
    }
    function goToday() { anchor = new Date() }
    /// Shows an entry (e.g. from Ctrl+P): its week, selected.
    function reveal(uid, start) {
        const d = parse(start)
        if (d) anchor = d
        if (mode === "agenda" && d) mode = "week"
        selected = uid
        showDetails = true
    }

    // --------------------------------------------------------- operations

    function message(text) { if (actions) actions.showMessage(text) }
    function select(o) { selected = o ? o.uid : "" }
    function activate(o) {
        if (!o) return
        if (o.kind === "note") { openRequested(o.path, 0, false, {}); return }
        const e = JSON.parse(model.entry(o.uid) || "null")
        if (e && e.readOnly) { selected = o.uid; showDetails = true; message(qsTr("„%1“ liegt in einem schreibgeschützten Kalender").arg(e.title)); return }
        editDialog.edit(e)
    }
    function settings() { if (actions) actions.calendarSync.openSettings() }
    /// New entry with prefilled fields (kind, start, end, allDay, files).
    function newEntry(fields) { editDialog.create(fields || {}) }
    function move(o, start, end, allDay) {
        if (!o) return
        if (o.readOnly) { message(qsTr("„%1“ kann nicht verschoben werden").arg(o.title)); return }
        if (model.moveTo(o.uid, o.instance || "", isoTime(start), isoTime(end), !!allDay))
            message(o.recurring ? qsTr("Serie „%1“ verschoben").arg(o.title) : qsTr("„%1“ verschoben").arg(o.title))
    }
    function linkFiles(o, paths) {
        if (!o) return
        if (o.readOnly) { message(qsTr("„%1“ kann nicht geändert werden").arg(o.title)); return }
        let n = 0
        for (const p of paths) if (model.linkFile(o.uid, p)) n++
        if (n) message(qsTr("%n Datei(en) mit „%1“ verknüpft", "", n).arg(o.title))
    }
    function blockForFiles(paths, start) {
        for (const p of paths) model.blockForFile(p, isoTime(start), isoTime(addMinutes(start, 60)))
        if (paths.length) message(qsTr("Arbeitsblock für %1 angelegt").arg(app.fileName(paths[0])))
    }
    function scheduleTask(uid, start) {
        const t = tasks.find(x => x.uid === uid)
        const minutes = t && t.estimate ? t.estimate : 60
        if (model.schedule(uid, isoTime(start), isoTime(addMinutes(start, minutes))))
            message(qsTr("„%1“ eingeplant").arg(t ? t.title : ""))
    }
    function absolute(file) { return file.startsWith("/") && app.projectRoot ? app.projectRoot + file : file }

    Keys.onPressed: (event) => {
        if (event.key === Qt.Key_Delete && selectedEntry && !selectedEntry.readOnly) { deleteDialog.open(); event.accepted = true }
        else if (event.key === Qt.Key_Left && event.modifiers & Qt.AltModifier) { step(-1); event.accepted = true }
        else if (event.key === Qt.Key_Right && event.modifiers & Qt.AltModifier) { step(1); event.accepted = true }
        else if (event.key === Qt.Key_T && !event.modifiers) { goToday(); event.accepted = true }
        else if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && selected) { activate(occs.find(o => o.uid === selected)); event.accepted = true }
    }
    focus: true

    // ---------------------------------------------------------------- view

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 34
            color: Theme.header
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 8
                spacing: 4
                Repeater {
                    model: [{ m: "week", t: qsTr("Woche") }, { m: "month", t: qsTr("Monat") }, { m: "agenda", t: qsTr("Agenda") }]
                    IconButton {
                        required property var modelData
                        glyph: modelData.t
                        checkable: true
                        checked: cal.mode === modelData.m
                        onClicked: cal.mode = modelData.m
                    }
                }
                Rectangle { Layout.preferredWidth: 1; Layout.preferredHeight: 18; color: Theme.border }
                IconButton { iconName: "chevron-left"; tip: qsTr("Zurück (Alt+←)"); enabled: cal.mode !== "agenda"; onClicked: cal.step(-1) }
                IconButton { glyph: qsTr("Heute"); tip: qsTr("Heute (T)"); onClicked: cal.goToday() }
                IconButton { iconName: "chevron-right"; tip: qsTr("Weiter (Alt+→)"); enabled: cal.mode !== "agenda"; onClicked: cal.step(1) }
                Label {
                    text: cal.title
                    color: Theme.textStrong
                    font.bold: true
                    Layout.leftMargin: 8
                }
                Item { Layout.fillWidth: true }
                // Nextcloud: state of the synchronisation.
                IconButton {
                    visible: !!cal.model && cal.model.connected
                    glyph: !cal.model ? "" : cal.model.syncing ? "⟳ …" : !cal.model.unlocked ? "🔑" : cal.model.syncFailed ? "⚠ ⟳" : "⟳"
                        + (cal.model && cal.model.pending ? " " + cal.model.pending : "")
                    tip: cal.model ? cal.model.syncStatus + (cal.model.pending ? qsTr(" · %n Änderung(en) warten", "", cal.model.pending) : "")
                        + (cal.model.unlocked ? qsTr(" · Klick: jetzt synchronisieren") : cal.model.google ? qsTr(" · Klick: bei Google anmelden") : qsTr(" · Klick: App-Passwort eingeben")) : ""
                    onClicked: cal.model.unlocked ? cal.model.syncNow() : cal.actions.calendarSync.askPassword()
                }
                IconButton {
                    visible: !!cal.model && cal.model.conflictCount > 0
                    glyph: qsTr("⚠ %n Konflikt(e)", "", cal.model ? cal.model.conflictCount : 0)
                    tip: qsTr("In Nextcloud und Atlas geändert – Version wählen")
                    onClicked: cal.actions.calendarSync.showConflicts()
                }
                Label {
                    visible: cal.model && cal.model.error !== ""
                    textFormat: Text.PlainText
                    text: cal.model ? "⚠ " + cal.model.error : ""
                    color: Theme.error
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                    Layout.maximumWidth: 300
                }
                IconButton {
                    glyph: qsTr("+ Neu")
                    tip: qsTr("Neuer Termin, Arbeitsblock, Frist oder Aufgabe")
                    onClicked: newMenu.popup()
                    Menu {
                        id: newMenu
                        MenuItem { text: qsTr("Termin …"); onTriggered: cal.newEntry({ kind: "event" }) }
                        MenuItem { text: qsTr("Arbeitsblock …"); onTriggered: cal.newEntry({ kind: "block" }) }
                        MenuItem { text: qsTr("Frist …"); onTriggered: cal.newEntry({ kind: "deadline", allDay: true }) }
                        MenuItem { text: qsTr("Aufgabe …"); onTriggered: cal.newEntry({ kind: "task" }) }
                    }
                }
                IconButton { iconName: "check-square"; tip: qsTr("Noch nicht eingeplante Aufgaben"); checkable: true; checked: cal.showTasks; onClicked: cal.showTasks = !cal.showTasks }
                IconButton { iconName: "info"; tip: qsTr("Details und Wochenübersicht"); checkable: true; checked: cal.showDetails; onClicked: cal.showDetails = !cal.showDetails }
                IconButton { iconName: "settings"; tip: qsTr("Kalender-Einstellungen und Nextcloud"); onClicked: cal.settings() }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            // ---- unscheduled tasks
            Rectangle {
                visible: cal.showTasks
                Layout.preferredWidth: 220
                Layout.fillHeight: true
                color: Theme.sidebar
                // A file dropped here becomes a task ("Als Aufgabe einplanen").
                DropArea {
                    id: taskDrop
                    anchors.fill: parent
                    keys: ["text/uri-list"]
                    onDropped: (drop) => {
                        const paths = drop.urls.map(u => cal.app.urlToPath(u.toString())).filter(p => p)
                        for (const p of paths) cal.model.taskForFile(p)
                        if (paths.length) cal.message(qsTr("Aufgabe für %1 angelegt").arg(cal.app.fileName(paths[0])))
                        drop.accept(Qt.LinkAction)
                    }
                }
                Rectangle { anchors.fill: parent; visible: taskDrop.containsDrag; color: Qt.rgba(0.18, 0.63, 0.26, 0.15); border.color: Theme.ok; z: 5 }
                ColumnLayout {
                    anchors.fill: parent
                    spacing: 4
                    Label {
                        Layout.margins: 8
                        Layout.bottomMargin: 0
                        text: qsTr("NOCH NICHT EINGEPLANT")
                        color: Theme.text
                        font.bold: true
                        font.pixelSize: Theme.smallFontSize
                    }
                    TextField {
                        id: quickTask
                        Layout.fillWidth: true
                        Layout.leftMargin: 6
                        Layout.rightMargin: 6
                        implicitHeight: 26
                        font.pixelSize: Theme.smallFontSize
                        placeholderText: qsTr("Neue Aufgabe (Enter)")
                        onAccepted: {
                            if (!text.trim()) return
                            cal.model.save(JSON.stringify({ kind: "task", title: text.trim() }))
                            text = ""
                        }
                    }
                    ListView {
                        id: taskList
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        spacing: 4
                        model: cal.tasks.filter(t => t.status !== "done")
                        ScrollBar.vertical: ScrollBar {}
                        delegate: Rectangle {
                            id: taskItem
                            required property var modelData
                            /// Read by drop areas of the week and month view.
                            readonly property string taskUid: modelData.uid
                            width: ListView.view.width - 12
                            x: 6
                            height: taskColumn.implicitHeight + 10
                            radius: 4
                            color: cal.selected === modelData.uid ? Theme.selection : taskHover.hovered ? Theme.hover : Theme.editor
                            border.color: Theme.border
                            HoverHandler { id: taskHover }
                            // Dragged into the week or month to plan it.
                            Item { id: taskProxy }
                            Drag.active: taskMouse.drag.active
                            Drag.dragType: Drag.Automatic
                            Drag.supportedActions: Qt.LinkAction
                            Drag.mimeData: ({ "text/x-atlas-task": taskItem.taskUid })
                            ColumnLayout {
                                id: taskColumn
                                x: 8
                                y: 5
                                width: parent.width - 16
                                spacing: 2
                                Label {
                                    Layout.fillWidth: true
                                    text: taskItem.modelData.title
                                    color: Theme.text
                                    wrapMode: Text.Wrap
                                    font.pixelSize: Theme.smallFontSize
                                }
                                Label {
                                    Layout.fillWidth: true
                                    visible: text.length > 0
                                    text: [taskItem.modelData.estimate ? "⏱ " + taskItem.modelData.estimate + " min" : "",
                                           taskItem.modelData.files.length ? "📎 " + taskItem.modelData.files.length : "",
                                           taskItem.modelData.tags.map(t => "#" + t).join(" ")].filter(s => s).join("  ")
                                    color: Theme.textFaint
                                    font.pixelSize: 10
                                    elide: Text.ElideRight
                                }
                            }
                            Rectangle { width: 3; height: parent.height - 8; y: 4; x: 2; radius: 2; color: cal.colorOf(taskItem.modelData) }
                            MouseArea {
                                id: taskMouse
                                anchors.fill: parent
                                drag.target: taskProxy
                                drag.threshold: 6
                                onClicked: { cal.selected = taskItem.modelData.uid; cal.showDetails = true }
                                onDoubleClicked: cal.activate({ uid: taskItem.modelData.uid, kind: "task" })
                            }
                        }
                        Label {
                            anchors.centerIn: parent
                            width: parent.width - 24
                            visible: taskList.count === 0
                            wrapMode: Text.Wrap
                            horizontalAlignment: Text.AlignHCenter
                            color: Theme.textFaint
                            font.pixelSize: Theme.smallFontSize
                            text: qsTr("Keine offenen Aufgaben.\nAufgaben in die Woche ziehen, um sie einzuplanen.")
                        }
                    }
                }
            }

            Loader {
                id: body
                Layout.fillWidth: true
                Layout.fillHeight: true
                sourceComponent: cal.mode === "month" ? monthComponent : cal.mode === "agenda" ? agendaComponent : weekComponent
            }

            // ---- details of the selection, otherwise the week at a glance
            Rectangle {
                visible: cal.showDetails
                Layout.preferredWidth: 280
                Layout.fillHeight: true
                color: Theme.sidebar
                ScrollView {
                    anchors.fill: parent
                    contentWidth: availableWidth
                    ColumnLayout {
                        width: parent.width
                        spacing: 6
                        Item { Layout.preferredHeight: 4 }
                        // Selected entry.
                        ColumnLayout {
                            visible: !!cal.selectedEntry
                            Layout.fillWidth: true
                            Layout.margins: 10
                            spacing: 6
                            readonly property var e: cal.selectedEntry || ({})
                            RowLayout {
                                Rectangle { width: 10; height: 10; radius: 5; color: cal.colorOf(parent.parent.e) }
                                Label { text: cal.kindNames[parent.parent.e.kind] || ""; color: Theme.textMuted; font.pixelSize: Theme.smallFontSize }
                                Label {
                                    Layout.fillWidth: true
                                    text: (parent.parent.e.calendarName || "") + (parent.parent.e.readOnly ? " 🔒" : "") + (parent.parent.e.pending ? qsTr(" · ⇡ wartet") : "")
                                    color: Theme.textFaint
                                    font.pixelSize: Theme.smallFontSize
                                    elide: Text.ElideRight
                                    horizontalAlignment: Text.AlignRight
                                }
                            }
                            Label {
                                Layout.fillWidth: true
                                text: parent.e.title || ""
                                color: Theme.textStrong
                                font.bold: true
                                font.pixelSize: 15
                                wrapMode: Text.Wrap
                            }
                            Label {
                                Layout.fillWidth: true
                                visible: !!parent.e.start
                                color: Theme.text
                                wrapMode: Text.Wrap
                                text: {
                                    const e = parent.e
                                    if (!e.start) return ""
                                    const s = cal.parse(e.start)
                                    if (e.allDay) return cal.fmtLong(s) + (e.endDate && e.endDate !== cal.isoDate(s) ? " – " + cal.fmtLong(cal.parse(e.endDate)) : "") + qsTr(" · ganztägig")
                                    if (!e.end) return cal.fmtLong(s) + ", " + cal.fmtTime(s)
                                    return cal.fmtLong(s) + ", " + cal.fmtTime(s) + "–" + cal.fmtTime(cal.parse(e.end)) + " (" + cal.duration(e) + ")"
                                }
                            }
                            Label {
                                Layout.fillWidth: true
                                visible: !!parent.e.rrule
                                text: parent.e.rrule ? "↻ " + parent.e.rrule.text : ""
                                color: Theme.textMuted
                                font.pixelSize: Theme.smallFontSize
                            }
                            RowLayout {
                                Label { text: qsTr("Status"); color: Theme.textMuted; font.pixelSize: Theme.smallFontSize }
                                ComboBox {
                                    Layout.fillWidth: true
                                    implicitHeight: 26
                                    font.pixelSize: Theme.smallFontSize
                                    model: ["planned", "started", "done", "postponed"].map(s => cal.statusNames[s])
                                    currentIndex: ["planned", "started", "done", "postponed"].indexOf(parent.parent.e.status || "planned")
                                    enabled: !parent.parent.e.readOnly
                                    onActivated: (index) => cal.model.setStatus(cal.selected, ["planned", "started", "done", "postponed"][index])
                                }
                            }
                            Flow {
                                Layout.fillWidth: true
                                visible: (parent.e.tags || []).length > 0
                                spacing: 4
                                Repeater {
                                    model: parent.parent.e.tags || []
                                    Label { required property string modelData; text: "#" + modelData; color: Theme.link; font.pixelSize: Theme.smallFontSize }
                                }
                            }
                            Label {
                                Layout.fillWidth: true
                                visible: !!parent.e.description
                                text: parent.e.description || ""
                                color: Theme.text
                                wrapMode: Text.Wrap
                                font.pixelSize: Theme.smallFontSize
                            }
                            // Checklist.
                            Repeater {
                                model: parent.e.checklist || []
                                CheckBox {
                                    required property var modelData
                                    required property int index
                                    Layout.fillWidth: true
                                    text: modelData.text
                                    checked: modelData.done
                                    font.pixelSize: Theme.smallFontSize
                                    onToggled: cal.model.toggleCheck(cal.selected, index)
                                }
                            }
                            // Linked files.
                            Label {
                                visible: (parent.e.files || []).length > 0
                                text: qsTr("DATEIEN")
                                color: Theme.textMuted
                                font.bold: true
                                font.pixelSize: 10
                            }
                            Repeater {
                                model: parent.e.files || []
                                RowLayout {
                                    required property string modelData
                                    Layout.fillWidth: true
                                    Label {
                                        Layout.fillWidth: true
                                        text: "📄 " + modelData
                                        color: cal.app.exists(cal.absolute(modelData)) ? Theme.accent : Theme.error
                                        elide: Text.ElideMiddle
                                        font.pixelSize: Theme.smallFontSize
                                        MouseArea {
                                            anchors.fill: parent
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: (mouse) => cal.openRequested(cal.absolute(parent.parent.modelData), 0, !!(mouse.modifiers & Qt.ControlModifier), {})
                                        }
                                    }
                                    IconButton { iconName: "close"; tip: qsTr("Verknüpfung lösen"); onClicked: cal.model.unlinkFile(cal.selected, parent.modelData) }
                                }
                            }
                            Button {
                                Layout.fillWidth: true
                                visible: !!parent.e.session
                                text: qsTr("In Session wechseln: %1").arg(parent.e.session || "")
                                onClicked: cal.sessionRequested(parent.e.session)
                            }
                            RowLayout {
                                visible: !parent.e.readOnly
                                Button { text: qsTr("Bearbeiten"); onClicked: cal.editDialog.edit(cal.selectedEntry) }
                                Button { text: qsTr("Löschen"); flat: true; onClicked: deleteDialog.open() }
                            }
                            Button {
                                Layout.fillWidth: true
                                visible: !parent.e.readOnly && parent.e.kind === "block" && !parent.e.rrule
                                flat: true
                                text: qsTr("Zurück zu „Noch nicht eingeplant“")
                                onClicked: cal.model.unschedule(cal.selected)
                            }
                            Label {
                                Layout.fillWidth: true
                                wrapMode: Text.Wrap
                                color: Theme.textFaint
                                font.pixelSize: 10
                                text: qsTr("Dateien aus dem Explorer hierher oder auf den Eintrag ziehen, um sie zu verknüpfen.")
                            }
                            DropArea {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 40
                                keys: ["text/uri-list"]
                                onDropped: (drop) => {
                                    cal.linkFiles({ uid: cal.selected, title: cal.selectedEntry.title }, drop.urls.map(u => cal.app.urlToPath(u.toString())))
                                    drop.accept(Qt.LinkAction)
                                }
                                Rectangle { anchors.fill: parent; radius: 4; color: parent.containsDrag ? Theme.hover : "transparent"; border.color: Theme.border }
                            }
                        }
                        // Week at a glance.
                        ColumnLayout {
                            visible: !cal.selectedEntry
                            Layout.fillWidth: true
                            Layout.margins: 10
                            spacing: 4
                            readonly property var weekOccs: {
                                const s = cal.weekStart(cal.mode === "week" ? cal.anchor : new Date())
                                const e = cal.addDays(s, 7)
                                return cal.occs.filter(o => cal.parse(o.start) < e && cal.parse(o.end) > s)
                            }
                            readonly property real plannedHours: weekOccs.filter(o => !o.allDay && (o.kind === "event" || o.kind === "block"))
                                .reduce((sum, o) => sum + (cal.parse(o.end) - cal.parse(o.start)) / 3600000, 0)
                            Label { text: qsTr("DIESE WOCHE"); color: Theme.text; font.bold: true; font.pixelSize: Theme.smallFontSize }
                            Label { text: qsTr("%1 h verplant").arg(Math.round(parent.plannedHours * 10) / 10); color: Theme.textStrong; font.pixelSize: 15 }
                            Label {
                                text: qsTr("%1 Termine · %2 Arbeitsblöcke · %3 Fristen")
                                    .arg(parent.weekOccs.filter(o => o.kind === "event").length)
                                    .arg(parent.weekOccs.filter(o => o.kind === "block").length)
                                    .arg(parent.weekOccs.filter(o => o.kind === "deadline" || o.kind === "note").length)
                                color: Theme.textMuted
                                font.pixelSize: Theme.smallFontSize
                            }
                            Label { text: qsTr("%n offene Aufgabe(n)", "", cal.tasks.filter(t => t.status !== "done").length); color: Theme.textMuted; font.pixelSize: Theme.smallFontSize }
                            Item { Layout.preferredHeight: 6 }
                            Label { text: qsTr("FRISTEN"); color: Theme.text; font.bold: true; font.pixelSize: 10; visible: deadlineRepeater.count > 0 }
                            Repeater {
                                id: deadlineRepeater
                                model: parent.weekOccs.filter(o => o.kind === "deadline" || o.kind === "note")
                                Label {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    text: "⚑ " + cal.fmtDay(cal.parse(modelData.start)) + "  " + modelData.title
                                    color: modelData.status === "done" ? Theme.textFaint : Theme.text
                                    font.strikeout: modelData.status === "done"
                                    font.pixelSize: Theme.smallFontSize
                                    elide: Text.ElideRight
                                    MouseArea { anchors.fill: parent; onClicked: cal.selected = parent.modelData.uid; onDoubleClicked: cal.activate(parent.modelData) }
                                }
                            }
                            Item { Layout.preferredHeight: 10 }
                            Label {
                                Layout.fillWidth: true
                                wrapMode: Text.Wrap
                                color: Theme.textFaint
                                font.pixelSize: 10
                                text: qsTr("Doppelklick in die Woche legt einen Termin an, Ziehen über freie Zeit einen Zeitraum. Dateien auf freie Zeit ziehen: Arbeitsblock. Zeitzone: %1").arg(cal.model ? cal.model.zone : "")
                            }
                        }
                    }
                }
            }
        }
    }

    Component {
        id: weekComponent
        CalendarWeek { calendar: cal }
    }
    Component {
        id: monthComponent
        CalendarMonth { calendar: cal }
    }
    Component {
        id: agendaComponent
        ListView {
            id: agenda
            clip: true
            ScrollBar.vertical: ScrollBar {}
            readonly property date now: new Date()
            function group(o) {
                const s = cal.parse(o.start), today = cal.dayStart(now)
                if (cal.parse(o.end) <= now) return (o.kind === "deadline" || o.kind === "note") && o.status !== "done" ? qsTr("Überfällig") : ""
                if (s < cal.addDays(today, 1)) return qsTr("Heute")
                if (s < cal.addDays(today, 2)) return qsTr("Morgen")
                if (s < cal.weekStart(cal.addDays(today, 7))) return qsTr("Diese Woche")
                return qsTr("Später")
            }
            model: {
                const out = []
                let last = ""
                for (const o of cal.occs) {
                    const g = group(o)
                    if (!g) continue
                    if (g !== last) { out.push({ header: g }); last = g }
                    out.push(o)
                }
                return out
            }
            delegate: Rectangle {
                id: row
                required property var modelData
                width: ListView.view.width
                height: modelData.header ? 30 : 40
                color: !modelData.header && cal.selected === modelData.uid ? Theme.selection : rowHover.hovered && !modelData.header ? Theme.hover : "transparent"
                HoverHandler { id: rowHover }
                Label {
                    visible: !!row.modelData.header
                    anchors.verticalCenter: parent.verticalCenter
                    x: 16
                    text: (row.modelData.header || "").toUpperCase()
                    color: row.modelData.header === qsTr("Überfällig") ? Theme.error : Theme.text
                    font.bold: true
                    font.pixelSize: Theme.smallFontSize
                }
                RowLayout {
                    visible: !row.modelData.header
                    anchors.fill: parent
                    anchors.leftMargin: 16
                    anchors.rightMargin: 16
                    spacing: 10
                    Rectangle { width: 4; Layout.fillHeight: true; Layout.margins: 6; color: row.modelData.header ? "transparent" : cal.colorOf(row.modelData) }
                    Label {
                        Layout.preferredWidth: 130
                        text: row.modelData.header ? "" : cal.fmtDay(cal.parse(row.modelData.start)) + "  " + (row.modelData.allDay ? qsTr("ganztägig") : cal.fmtTime(cal.parse(row.modelData.start)))
                        color: Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                    }
                    Label {
                        Layout.fillWidth: true
                        text: row.modelData.header ? "" : (row.modelData.kind === "deadline" || row.modelData.kind === "note" ? "⚑ " : "") + row.modelData.title
                            + (row.modelData.recurring ? "  ↻" : "") + (row.modelData.files && row.modelData.files.length ? "  📎" + row.modelData.files.length : "")
                        color: row.modelData.status === "done" ? Theme.textFaint : Theme.text
                        font.strikeout: row.modelData.status === "done"
                        elide: Text.ElideRight
                    }
                    Label {
                        text: row.modelData.header ? "" : cal.kindNames[row.modelData.kind]
                        color: Theme.textFaint
                        font.pixelSize: Theme.smallFontSize
                    }
                }
                MouseArea {
                    anchors.fill: parent
                    enabled: !row.modelData.header
                    onClicked: cal.select(row.modelData)
                    onDoubleClicked: cal.activate(row.modelData)
                }
            }
            Label {
                anchors.centerIn: parent
                visible: agenda.count === 0
                color: Theme.textFaint
                text: qsTr("Keine Einträge in den nächsten Wochen.")
            }
        }
    }

    readonly property alias editDialog: editDialog
    EntryDialog {
        id: editDialog
        app: cal.app
        model: cal.model
        onSaved: (uid) => { cal.selected = uid }
    }

    Dialog {
        id: deleteDialog
        anchors.centerIn: Overlay.overlay
        width: 400
        modal: true
        title: qsTr("Eintrag löschen?")
        standardButtons: Dialog.Cancel
        Label {
            width: parent.width
            wrapMode: Text.Wrap
            text: cal.selectedEntry
                ? qsTr("„%1“ wird gelöscht.").arg(cal.selectedEntry.title) + (cal.selectedEntry.rrule ? qsTr(" Alle Wiederholungen werden entfernt.") : "")
                : ""
        }
        footer: DialogButtonBox {
            Button { text: qsTr("Abbrechen"); DialogButtonBox.buttonRole: DialogButtonBox.RejectRole }
            Button {
                text: qsTr("Löschen")
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                onClicked: { cal.model.remove(cal.selected); cal.selected = "" }
            }
        }
    }
}
