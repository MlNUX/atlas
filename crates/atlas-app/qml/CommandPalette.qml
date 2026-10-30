// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Universal bar (Ctrl+P): files, folders, notes and actions in one list.
//
//   text          fuzzy search over files, folders, note titles and actions
//   pfad:42       open a file at line 42
//   >text         actions only
//   #tag          notes with this tag (hierarchical, case-insensitive)
//   :42           line 42 in the current document
//   ?text         full text search in the project (search panel)
//   @text         PDF annotations and quotes (marked text, comments, bookmarks)
//   !text         calendar entries (also shown for longer queries)
//
// Enter opens (folders as file list), Ctrl+Enter in a new split.
Popup {
    id: bar
    required property AppController app
    /// [{ name, hint, run }]
    property var actions: []
    /// CalendarModel (entries are found as well).
    property var calendar: null
    property var results: []

    /// `state` goes to the viewer (`{annot: id}` for PDF annotations).
    signal openRequested(string path, int line, string where, var state)
    signal searchRequested(string text)
    signal lineRequested(int line)
    /// Show a calendar entry.
    signal calendarEntryRequested(string uid, string start)

    function show(prefix) {
        app.refreshFileList()
        input.text = prefix || ""
        open()
        input.forceActiveFocus()
        update()
    }

    function actionResults(query, limit) {
        const scored = []
        for (const a of actions) {
            const s = query ? app.fuzzyScore(query, a.name) : 0
            if (s >= 0) scored.push({ kind: "action", title: a.name, relative: a.hint || "", action: a, score: s })
        }
        scored.sort((x, y) => y.score - x.score)
        return scored.slice(0, limit)
    }

    /// PDF annotations as results (page as line, the ID for the viewer).
    function annotationResults(query, limit) {
        return JSON.parse(app.annotationSearch(query, limit)).map(a => ({
            kind: "annotation",
            title: a.kind === "bookmark" ? (a.title || qsTr("Lesezeichen")) : (a.text || a.comment || qsTr("Freihandmarkierung")),
            relative: qsTr("%1 · S. %2").arg(a.relative).arg(a.page) + (a.comment && a.text ? " · 💬 " + a.comment : ""),
            path: a.path,
            line: a.page,
            annot: a.id,
        }))
    }

    function calendarResults(query, limit) {
        if (!calendar) return []
        const kinds = { event: qsTr("Termin"), block: qsTr("Arbeitsblock"), deadline: qsTr("Frist"), task: qsTr("Aufgabe") }
        return JSON.parse(calendar.search(query, limit)).map(e => ({
            kind: "calendar",
            title: e.title,
            relative: (kinds[e.kind] || "") + (e.start ? " · " + e.start.replace("T", " ") : qsTr(" · nicht eingeplant")),
            uid: e.uid,
            start: e.start || "",
        }))
    }

    function update() {
        const text = input.text
        let list = []
        if (text.startsWith("@")) {
            list = annotationResults(text.slice(1).trim(), 50)
        } else if (text.startsWith("!")) {
            list = text.slice(1).trim() ? calendarResults(text.slice(1).trim(), 50) : []
        } else if (text.startsWith(">")) {
            list = actionResults(text.slice(1).trim(), 50)
        } else if (text.startsWith("#")) {
            const tag = text.slice(1).trim()
            const tags = JSON.parse(app.noteTags()).filter(t => !tag || t.key.indexOf(tag.toLowerCase()) >= 0)
            for (const t of tags.slice(0, 8))
                list.push({ kind: "tag", title: "#" + t.display, relative: qsTr("%n Notiz(en)", "", t.count), tag: t.key })
            // Notes of all matching tags (sub tags included).
            const seen = {}
            if (tag) for (const t of tags) for (const p of app.notesWithTag(t.key)) {
                if (seen[p]) continue
                seen[p] = true
                list.push({ kind: "note", title: app.fileName(p), relative: app.relativePath(p), path: p, line: 0 })
            }
        } else if (text.startsWith(":")) {
            const line = parseInt(text.slice(1))
            if (line > 0) list.push({ kind: "line", title: qsTr("Gehe zu Zeile %1").arg(line), relative: "", line: line })
        } else if (text.startsWith("?")) {
            if (text.length > 1) list.push({ kind: "search", title: qsTr("Im Projekt suchen nach „%1“").arg(text.slice(1).trim()), relative: "ripgrep", query: text.slice(1).trim() })
        } else {
            list = JSON.parse(app.quickOpen(text, 60)).map(r => ({
                kind: r.kind, title: r.title || app.fileName(r.path), relative: r.relative, path: r.path, line: r.line
            }))
            // Clearly matching actions (word starts, runs) follow the files.
            if (text.trim()) list = list.concat(actionResults(text.trim(), 3).filter(a => a.score >= text.trim().length * 6))
            // Quotes and annotations matching longer queries.
            if (text.trim().length >= 3) list = list.concat(annotationResults(text.trim(), 3))
            if (text.trim().length >= 3) list = list.concat(calendarResults(text.trim(), 3))
            if (!text.trim()) list = actionResults("", 0).concat(list)
            if (text.trim()) list.push({ kind: "search", title: qsTr("Im Projekt suchen nach „%1“").arg(text.trim()), relative: "ripgrep", query: text.trim() })
        }
        results = list
        view.currentIndex = 0
    }

    function activate(index, newSplit) {
        const r = results[index]
        if (!r) return
        if (r.kind === "tag") { input.text = "#" + r.tag + " "; update(); return }
        close()
        if (r.kind === "action") r.action.run()
        else if (r.kind === "search") searchRequested(r.query)
        else if (r.kind === "line") lineRequested(r.line)
        else if (r.kind === "calendar") calendarEntryRequested(r.uid, r.start)
        else openRequested(r.path, r.line || 0, newSplit ? "split" : "tab", r.annot ? { annot: r.annot } : {})
    }

    /// [icon, colour] of a result.
    function iconOf(r) {
        const fixed = { action: ["play", Theme.textMuted], tag: ["", ""], line: ["arrow-right", Theme.textMuted], search: ["search", Theme.textMuted],
                        annotation: ["quote", Theme.warn], calendar: ["calendar", Theme.accent] }[r.kind]
        if (fixed) return fixed
        return Theme.fileIcon(r.relative || r.title || "", r.kind === "dir")
    }

    parent: Overlay.overlay
    x: Math.round((parent.width - width) / 2)
    y: 48
    width: Math.min(720, parent.width - 40)
    height: Math.min(520, 70 + Math.max(1, results.length) * 38)
    modal: true
    focus: true
    padding: 8
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    background: Rectangle { color: Theme.popup; border.color: Theme.borderStrong; radius: 12 }

    ColumnLayout {
        anchors.fill: parent
        spacing: 6
        TextField {
            id: input
            Layout.fillWidth: true
            implicitHeight: 42
            leftPadding: 40
            font.pixelSize: 14
            Icon { name: "search"; size: 18; x: 13; anchors.verticalCenter: parent.verticalCenter }
            placeholderText: qsTr("Datei, Ordner, Notiz oder Aktion … (>Aktion, #Tag, @Zitat, !Termin, :Zeile, ?Volltext)")
            onTextEdited: bar.update()
            Keys.onDownPressed: view.incrementCurrentIndex()
            Keys.onUpPressed: view.decrementCurrentIndex()
            Keys.onPressed: (event) => {
                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    bar.activate(view.currentIndex, event.modifiers & Qt.ControlModifier)
                    event.accepted = true
                } else if (event.key === Qt.Key_PageDown) {
                    view.currentIndex = Math.min(view.count - 1, view.currentIndex + 10); event.accepted = true
                } else if (event.key === Qt.Key_PageUp) {
                    view.currentIndex = Math.max(0, view.currentIndex - 10); event.accepted = true
                }
            }
        }
        ListView {
            id: view
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: bar.results
            highlightMoveDuration: 0
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            delegate: ItemDelegate {
                id: resultRow
                required property var modelData
                required property int index
                width: ListView.view.width
                height: 38
                highlighted: ListView.isCurrentItem
                readonly property var resultIcon: bar.iconOf(modelData)
                background: Rectangle { radius: 7; color: parent.highlighted ? Theme.activeSelection : parent.hovered ? Theme.hover : "transparent" }
                contentItem: RowLayout {
                    spacing: 12
                    Item {
                        Layout.preferredWidth: 18
                        Layout.preferredHeight: 18
                        Icon { anchors.centerIn: parent; visible: !!resultRow.resultIcon[0]; name: resultRow.resultIcon[0]; color: resultRow.resultIcon[1]; size: 17 }
                        Label { anchors.centerIn: parent; visible: !resultRow.resultIcon[0]; text: "#"; color: Theme.accent; font.bold: true }
                    }
                    Label { text: modelData.title; color: Theme.textStrong; elide: Text.ElideRight; Layout.maximumWidth: parent.width * 0.5 }
                    Label {
                        Layout.fillWidth: true
                        text: modelData.relative + (modelData.line ? ":" + modelData.line : "")
                        color: Theme.textFaint
                        font.pixelSize: Theme.smallFontSize
                        elide: Text.ElideMiddle
                    }
                }
                onClicked: bar.activate(index, false)
            }
            Label {
                anchors.centerIn: parent
                visible: view.count === 0
                color: Theme.textFaint
                text: qsTr("Nichts gefunden")
            }
        }
    }
}
