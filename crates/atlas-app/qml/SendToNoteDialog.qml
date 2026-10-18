// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Puts terminal output as a code block into a note: an open Markdown tab
// (inserted into the editor, saved by autosave), any note of the project
// (appended on disk) or a new note.
Dialog {
    id: dialog
    required property AppController app
    required property var editor
    property var actions
    property string text
    property string cwd
    property var candidates: []
    property var filtered: []

    anchors.centerIn: Overlay.overlay
    width: 560
    height: 460
    modal: true
    title: qsTr("In Notiz übernehmen")
    standardButtons: Dialog.Cancel

    function ask(text, cwd) {
        dialog.text = text
        dialog.cwd = cwd
        // Open Markdown tabs first, then all notes of the project.
        const open = dialog.editor.documents().filter(p => dialog.app.fileKind(p) === "markdown")
        const all = dialog.app.noteList()
        const seen = {}
        dialog.candidates = open.concat(all).filter(p => !seen[p] && (seen[p] = true))
        filterField.text = ""
        updateFilter()
        list.currentIndex = 0
        open()
        filterField.forceActiveFocus()
    }
    function updateFilter() {
        const q = filterField.text.toLowerCase()
        filtered = candidates.filter(p => dialog.app.relativePath(p).toLowerCase().indexOf(q) >= 0)
    }

    /// Inserts the block into `path`.
    function sendTo(path) {
        const block = dialog.app.codeBlock(dialog.text, "console")
        const viewer = dialog.editor.viewerFor(path)
        const textEditor = viewer && viewer.editorItem ? viewer.editorItem : null
        if (textEditor && textEditor.insertText("\n" + block, true)) {
            textEditor.save()
        } else {
            const error = dialog.app.appendToNote(path, block)
            if (error) {
                dialog.actions.showMessage(qsTr("Übernehmen fehlgeschlagen: %1").arg(error))
                return
            }
        }
        dialog.actions.showMessage(qsTr("Ausgabe in %1 übernommen").arg(dialog.app.fileName(path)))
        close()
    }
    function newNote() {
        const inProject = dialog.app.projectRoot && (dialog.cwd + "/").startsWith(dialog.app.projectRoot + "/")
        const dir = inProject ? dialog.cwd : (dialog.app.projectRoot || dialog.cwd)
        const name = (newName.text.trim() || qsTr("Terminal-Ausgabe")).replace(/\.md$/, "") + ".md"
        const path = dialog.actions.ops.newFile(dir, name)
        if (path) sendTo(path)
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8
        Label {
            Layout.fillWidth: true
            text: qsTr("%n Zeile(n) als Codeblock anhängen an:", "", dialog.text.split("\n").length)
            color: Theme.textMuted
        }
        TextField {
            id: filterField
            Layout.fillWidth: true
            placeholderText: qsTr("Notiz suchen …")
            onTextChanged: { dialog.updateFilter(); list.currentIndex = 0 }
            Keys.onDownPressed: list.incrementCurrentIndex()
            Keys.onUpPressed: list.decrementCurrentIndex()
            onAccepted: if (list.currentIndex >= 0 && dialog.filtered.length) dialog.sendTo(dialog.filtered[list.currentIndex])
        }
        ListView {
            id: list
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: dialog.filtered
            highlightMoveDuration: 0
            ScrollBar.vertical: ScrollBar {}
            delegate: ItemDelegate {
                required property string modelData
                required property int index
                width: ListView.view.width
                highlighted: ListView.isCurrentItem
                background: Rectangle { color: parent.highlighted ? Theme.selection : parent.hovered ? Theme.hover : "transparent" }
                contentItem: ColumnLayout {
                    spacing: 0
                    Label { text: dialog.app.fileName(modelData); color: Theme.text }
                    Label { text: dialog.app.relativePath(modelData); color: Theme.textFaint; font.pixelSize: Theme.smallFontSize; elide: Text.ElideMiddle; Layout.fillWidth: true }
                }
                onClicked: dialog.sendTo(modelData)
            }
            Label {
                anchors.centerIn: parent
                visible: list.count === 0
                color: Theme.textFaint
                text: qsTr("Keine passende Notiz.")
            }
        }
        RowLayout {
            Layout.fillWidth: true
            TextField {
                id: newName
                Layout.fillWidth: true
                placeholderText: qsTr("Neue Notiz: Name")
                onAccepted: dialog.newNote()
            }
            Button { text: qsTr("Neue Notiz anlegen"); onClicked: dialog.newNote() }
        }
    }
}
