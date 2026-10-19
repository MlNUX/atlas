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
//
// Enter opens (folders as file list), Ctrl+Enter in a new split.
Popup {
    id: bar
    required property AppController app
    /// [{ name, hint, run }]
    property var actions: []
    property var results: []

    signal openRequested(string path, int line, string where)
    signal searchRequested(string text)
    signal lineRequested(int line)

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

    function update() {
        const text = input.text
        let list = []
        if (text.startsWith(">")) {
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
        else openRequested(r.path, r.line || 0, newSplit ? "split" : "tab")
    }

    function glyph(kind) {
        return ({ action: "›", dir: "📁", note: "📝", file: "📄", tag: "#", line: "↧", search: "🔍" })[kind] || "•"
    }

    parent: Overlay.overlay
    x: Math.round((parent.width - width) / 2)
    y: 48
    width: Math.min(680, parent.width - 40)
    height: Math.min(480, 54 + Math.max(1, results.length) * 34)
    modal: true
    focus: true
    padding: 6
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    background: Rectangle { color: Theme.header; border.color: Theme.border; radius: 8 }

    ColumnLayout {
        anchors.fill: parent
        spacing: 6
        TextField {
            id: input
            Layout.fillWidth: true
            placeholderText: qsTr("Datei, Ordner, Notiz oder Aktion … (>Aktion, #Tag, :Zeile, ?Volltext)")
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
                required property var modelData
                required property int index
                width: ListView.view.width
                height: 34
                highlighted: ListView.isCurrentItem
                background: Rectangle { radius: 4; color: parent.highlighted ? Theme.activeSelection : parent.hovered ? Theme.hover : "transparent" }
                contentItem: RowLayout {
                    spacing: 10
                    Label { text: bar.glyph(modelData.kind); color: Theme.textMuted; Layout.preferredWidth: 18; horizontalAlignment: Text.AlignHCenter }
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
