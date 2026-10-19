// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Compact editor of a calendar entry: kind, title, target calendar, time,
// status, tags, description, checklist, linked files and session,
// estimate, reminder and a simple recurrence.
Dialog {
    id: dialog
    required property AppController app
    property var model: null
    /// The entry being edited (null: a new one).
    property var entry: null
    property string kind: "event"
    property var checklist: []
    property var files: []
    property bool rruleChanged: false
    property bool rruleSupported: true

    signal saved(string uid)

    anchors.centerIn: Overlay.overlay
    width: Math.min(560, Overlay.overlay ? Overlay.overlay.width - 40 : 560)
    height: Math.min(implicitHeight, Overlay.overlay ? Overlay.overlay.height - 40 : 700)
    modal: true
    title: entry ? qsTr("Eintrag bearbeiten") : qsTr("Neuer Eintrag")
    standardButtons: Dialog.Save | Dialog.Cancel

    readonly property var kinds: ["event", "block", "deadline", "task"]
    readonly property var kindLabels: [qsTr("Termin"), qsTr("Arbeitsblock"), qsTr("Frist"), qsTr("Aufgabe (ohne Zeit)")]
    readonly property var statuses: ["planned", "started", "done", "postponed"]
    readonly property var statusLabels: [qsTr("geplant"), qsTr("begonnen"), qsTr("erledigt"), qsTr("verschoben")]
    readonly property var reminderMinutes: [-1, 0, 5, 10, 15, 30, 60, 120, 1440]
    readonly property var reminderLabels: [qsTr("keine"), qsTr("zu Beginn"), qsTr("5 min vorher"), qsTr("10 min vorher"), qsTr("15 min vorher"),
        qsTr("30 min vorher"), qsTr("1 h vorher"), qsTr("2 h vorher"), qsTr("1 Tag vorher")]
    readonly property var freqs: ["none", "daily", "weekly", "monthly", "yearly"]
    readonly property var freqLabels: [qsTr("keine"), qsTr("täglich"), qsTr("wöchentlich"), qsTr("monatlich"), qsTr("jährlich")]
    property var calendarList: []

    function pad(n) { return (n < 10 ? "0" : "") + n }
    function datePart(iso) { return iso ? iso.slice(0, 10) : "" }
    function timePart(iso) { return iso && iso.length > 10 ? iso.slice(11, 16) : "" }
    function validDate(s) { return /^\d{4}-\d{2}-\d{2}$/.test(s) && !isNaN(new Date(s + "T00:00").getTime()) }
    function validTime(s) { return /^([01]?\d|2[0-3]):[0-5]\d$/.test(s) }
    function normTime(s) { return s.length === 4 ? "0" + s : s }

    function loadCalendars(current) {
        let list = []
        try { list = JSON.parse(model.calendars()) } catch (e) {}
        // Only calendars that take new entries (plus the current one).
        calendarList = list.filter(c => (c.visible && c.writable && !c.missing) || c.id === current)
        calendarBox.currentIndex = Math.max(0, calendarList.findIndex(c => c.id === current))
    }

    /// New entry; `fields` may set kind, start, end, allDay, files, title.
    function create(fields) {
        entry = null
        const now = new Date()
        const start = fields.start || (now.getFullYear() + "-" + pad(now.getMonth() + 1) + "-" + pad(now.getDate()) + "T" + pad(Math.min(23, now.getHours() + 1)) + ":00")
        fill({
            kind: fields.kind || "event", title: fields.title || "", description: "", start: start,
            end: fields.end || "", endDate: fields.allDay ? datePart(start) : "", allDay: !!fields.allDay, status: "planned",
            tags: [], checklist: [], files: fields.files || [], session: "", estimate: null, reminders: [], rrule: null,
            calendar: model ? model.defaultCalendar() : "local",
        })
    }

    function edit(e) {
        if (!e) return
        entry = e
        fill(e)
    }

    function fill(e) {
        kind = e.kind
        kindBox.currentIndex = Math.max(0, kinds.indexOf(e.kind))
        titleField.text = e.title || ""
        loadCalendars(e.calendar || "local")
        allDayBox.checked = !!e.allDay
        startDate.text = datePart(e.start)
        startTime.text = timePart(e.start) || "09:00"
        if (e.allDay) {
            endDate.text = e.endDate || datePart(e.start)
            endTime.text = ""
        } else {
            const end = e.end || ""
            endDate.text = datePart(end) || datePart(e.start)
            endTime.text = timePart(end) || (timePart(e.start) ? pad(Math.min(23, parseInt(timePart(e.start)) + 1)) + timePart(e.start).slice(2) : "10:00")
        }
        statusBox.currentIndex = Math.max(0, statuses.indexOf(e.status || "planned"))
        tagsField.text = (e.tags || []).join(", ")
        descriptionField.text = e.description || ""
        checklist = (e.checklist || []).map(c => ({ done: c.done, text: c.text }))
        files = (e.files || []).slice()
        sessionBox.editText = e.session || ""
        estimateField.text = e.estimate ? (e.estimate % 60 === 0 ? (e.estimate / 60) + "h" : e.estimate + "m") : ""
        const r = (e.reminders || [])[0]
        reminderBox.currentIndex = r === undefined ? 0 : Math.max(0, reminderMinutes.indexOf(r))
        rruleSupported = !e.rrule || e.rrule.supported !== false
        freqBox.currentIndex = e.rrule ? Math.max(0, freqs.indexOf(e.rrule.freq)) : 0
        intervalBox.value = e.rrule ? e.rrule.interval || 1 : 1
        untilField.text = e.rrule && e.rrule.until ? e.rrule.until : ""
        countBox.value = e.rrule && e.rrule.count ? e.rrule.count : 0
        rruleChanged = false
        problem.text = ""
        open()
        titleField.forceActiveFocus()
        titleField.selectAll()
    }

    function collect() {
        const k = kinds[kindBox.currentIndex]
        const allDay = allDayBox.checked || (k === "deadline" && !startTime.text)
        if (k !== "task") {
            if (!validDate(startDate.text)) return qsTr("Beginn: Datum als JJJJ-MM-TT angeben")
            if (!allDay && !validTime(startTime.text)) return qsTr("Beginn: Uhrzeit als HH:MM angeben")
            if (k !== "deadline" && !validDate(endDate.text)) return qsTr("Ende: Datum als JJJJ-MM-TT angeben")
            if (k !== "deadline" && !allDay && !validTime(endTime.text)) return qsTr("Ende: Uhrzeit als HH:MM angeben")
            if (untilField.text && !validDate(untilField.text)) return qsTr("Wiederholung bis: Datum als JJJJ-MM-TT angeben")
        }
        const start = allDay ? startDate.text : startDate.text + "T" + normTime(startTime.text)
        const end = allDay ? "" : endDate.text + "T" + normTime(endTime.text)
        if (k !== "task" && k !== "deadline" && !allDay && end < start) return qsTr("Das Ende liegt vor dem Beginn.")
        const minutes = reminderMinutes[reminderBox.currentIndex]
        return {
            uid: entry ? entry.uid : "",
            kind: k,
            title: titleField.text,
            calendar: calendarList[calendarBox.currentIndex] ? calendarList[calendarBox.currentIndex].id : "local",
            description: descriptionField.text,
            start: k === "task" ? "" : start,
            end: k === "task" ? "" : end,
            endDate: allDay ? (validDate(endDate.text) ? endDate.text : startDate.text) : "",
            allDay: allDay,
            status: statuses[statusBox.currentIndex],
            tags: tagsField.text.split(",").map(t => t.trim()).filter(t => t),
            checklist: checklist.filter(c => c.text.trim()),
            files: files,
            session: sessionBox.editText.trim(),
            estimate: estimateField.text.trim(),
            reminders: minutes >= 0 && k !== "task" ? [minutes] : [],
            rruleChanged: rruleChanged,
            rrule: { freq: freqs[freqBox.currentIndex], interval: intervalBox.value, count: countBox.value, until: untilField.text },
        }
    }

    onAccepted: {
        const v = collect()
        if (typeof v === "string") {
            problem.text = v
            open()
            return
        }
        const uid = model.save(JSON.stringify(v))
        if (uid) saved(uid)
        else {
            problem.text = model.error || qsTr("Eintrag konnte nicht gespeichert werden.")
            open()
        }
    }

    contentItem: ScrollView {
        id: scroll
        implicitHeight: form.implicitHeight
        contentWidth: availableWidth
        clip: true
        GridLayout {
            id: form
            width: scroll.availableWidth
            columns: 2
            columnSpacing: 10
            rowSpacing: 6

            Label { text: qsTr("Art"); color: Theme.textMuted }
            ComboBox {
                id: kindBox
                Layout.fillWidth: true
                model: dialog.kindLabels
                onActivated: dialog.kind = dialog.kinds[currentIndex]
            }

            Label { text: qsTr("Titel"); color: Theme.textMuted }
            TextField { id: titleField; Layout.fillWidth: true; placeholderText: qsTr("Worum geht es?"); onAccepted: dialog.accept() }

            Label { text: qsTr("Kalender"); color: Theme.textMuted }
            ComboBox {
                id: calendarBox
                Layout.fillWidth: true
                model: dialog.calendarList.map(c => c.name)
                delegate: ItemDelegate {
                    required property int index
                    required property string modelData
                    width: calendarBox.width
                    contentItem: RowLayout {
                        Rectangle { width: 10; height: 10; radius: 5; color: dialog.calendarList[index] ? dialog.calendarList[index].color : Theme.accent }
                        Label { Layout.fillWidth: true; text: modelData; color: Theme.text; elide: Text.ElideRight }
                    }
                }
                ToolTip.visible: hovered
                ToolTip.delay: 600
                ToolTip.text: qsTr("Zielkalender nur für diesen Eintrag; der Standard steht in den Kalender-Einstellungen.")
            }

            Label { text: qsTr("Beginn"); color: Theme.textMuted; visible: dialog.kind !== "task" }
            RowLayout {
                visible: dialog.kind !== "task"
                Layout.fillWidth: true
                TextField { id: startDate; Layout.preferredWidth: 110; placeholderText: "2026-10-19"; inputMethodHints: Qt.ImhDate
                    onEditingFinished: if (dialog.validDate(text) && (!dialog.validDate(endDate.text) || endDate.text < text)) endDate.text = text }
                TextField { id: startTime; Layout.preferredWidth: 64; placeholderText: "09:00"; visible: !allDayBox.checked }
                CheckBox { id: allDayBox; text: qsTr("ganztägig") }
            }

            Label { text: dialog.kind === "deadline" ? "" : qsTr("Ende"); color: Theme.textMuted; visible: dialog.kind !== "task" }
            RowLayout {
                visible: dialog.kind !== "task" && (dialog.kind !== "deadline" || allDayBox.checked)
                Layout.fillWidth: true
                TextField { id: endDate; Layout.preferredWidth: 110; placeholderText: "2026-10-19" }
                TextField { id: endTime; Layout.preferredWidth: 64; placeholderText: "10:00"; visible: !allDayBox.checked }
                Item { Layout.fillWidth: true }
            }

            Label { text: qsTr("Status"); color: Theme.textMuted }
            ComboBox { id: statusBox; Layout.fillWidth: true; model: dialog.statusLabels }

            Label { text: qsTr("Aufwand"); color: Theme.textMuted; visible: dialog.kind === "task" || dialog.kind === "block" }
            TextField { id: estimateField; visible: dialog.kind === "task" || dialog.kind === "block"; Layout.fillWidth: true; placeholderText: qsTr("z. B. 2h, 90m, 1h30m") }

            Label { text: qsTr("Tags"); color: Theme.textMuted }
            TextField { id: tagsField; Layout.fillWidth: true; placeholderText: qsTr("durch Komma getrennt") }

            Label { text: qsTr("Beschreibung"); color: Theme.textMuted; Layout.alignment: Qt.AlignTop }
            TextArea { id: descriptionField; Layout.fillWidth: true; Layout.preferredHeight: 60; wrapMode: TextEdit.Wrap }

            Label { text: qsTr("Checkliste"); color: Theme.textMuted; Layout.alignment: Qt.AlignTop }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Repeater {
                    model: dialog.checklist
                    RowLayout {
                        required property var modelData
                        required property int index
                        Layout.fillWidth: true
                        CheckBox { checked: modelData.done; onToggled: dialog.checklist[index].done = checked }
                        TextField { Layout.fillWidth: true; text: modelData.text; onTextEdited: dialog.checklist[index].text = text }
                        IconButton { glyph: "✕"; tip: qsTr("Entfernen"); onClicked: { const l = dialog.checklist.slice(); l.splice(index, 1); dialog.checklist = l } }
                    }
                }
                TextField {
                    Layout.fillWidth: true
                    placeholderText: qsTr("Punkt hinzufügen (Enter)")
                    onAccepted: { if (text.trim()) { dialog.checklist = dialog.checklist.concat([{ done: false, text: text.trim() }]); text = "" } }
                }
            }

            Label { text: qsTr("Dateien"); color: Theme.textMuted; Layout.alignment: Qt.AlignTop }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Repeater {
                    model: dialog.files
                    RowLayout {
                        required property string modelData
                        required property int index
                        Layout.fillWidth: true
                        Label { Layout.fillWidth: true; text: "📄 " + modelData; color: Theme.text; elide: Text.ElideMiddle }
                        IconButton { glyph: "✕"; tip: qsTr("Verknüpfung lösen"); onClicked: { const l = dialog.files.slice(); l.splice(index, 1); dialog.files = l } }
                    }
                }
                TextField {
                    Layout.fillWidth: true
                    placeholderText: qsTr("/pfad/im/projekt.md (Enter) – oder Datei auf den Eintrag ziehen")
                    onAccepted: {
                        const f = text.trim()
                        if (!f) return
                        dialog.files = dialog.files.concat([f.startsWith("/") ? f : "/" + f])
                        text = ""
                    }
                }
            }

            Label { text: qsTr("Session"); color: Theme.textMuted }
            ComboBox {
                id: sessionBox
                Layout.fillWidth: true
                editable: true
                model: [""].concat(dialog.app.sessionNames)
            }

            Label { text: qsTr("Erinnerung"); color: Theme.textMuted; visible: dialog.kind !== "task" }
            ComboBox { id: reminderBox; visible: dialog.kind !== "task"; Layout.fillWidth: true; model: dialog.reminderLabels }

            Label { text: qsTr("Wiederholung"); color: Theme.textMuted; visible: dialog.kind !== "task"; Layout.alignment: Qt.AlignTop }
            ColumnLayout {
                visible: dialog.kind !== "task"
                Layout.fillWidth: true
                spacing: 2
                Label {
                    visible: !dialog.rruleSupported
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    color: Theme.warn
                    font.pixelSize: Theme.smallFontSize
                    text: qsTr("Diese Regel (%1) nutzt Teile, die Atlas nicht bearbeiten kann. Sie bleibt unverändert, solange hier nichts geändert wird.")
                        .arg(dialog.entry && dialog.entry.rrule ? dialog.entry.rrule.text : "")
                }
                RowLayout {
                    ComboBox { id: freqBox; model: dialog.freqLabels; onActivated: dialog.rruleChanged = true }
                    Label { text: qsTr("alle"); visible: freqBox.currentIndex > 0; color: Theme.textMuted }
                    SpinBox { id: intervalBox; visible: freqBox.currentIndex > 0; from: 1; to: 99; editable: true; implicitWidth: 90; onValueModified: dialog.rruleChanged = true }
                }
                RowLayout {
                    visible: freqBox.currentIndex > 0
                    Label { text: qsTr("bis"); color: Theme.textMuted }
                    TextField { id: untilField; Layout.preferredWidth: 110; placeholderText: qsTr("offen"); onTextEdited: dialog.rruleChanged = true }
                    Label { text: qsTr("oder"); color: Theme.textMuted }
                    SpinBox { id: countBox; from: 0; to: 999; editable: true; implicitWidth: 90; onValueModified: dialog.rruleChanged = true }
                    Label { text: qsTr("mal"); color: Theme.textMuted }
                }
            }

            Label {
                id: problem
                Layout.columnSpan: 2
                Layout.fillWidth: true
                visible: text !== ""
                color: Theme.error
                wrapMode: Text.Wrap
            }
        }
    }
}
