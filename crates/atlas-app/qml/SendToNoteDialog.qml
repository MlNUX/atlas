// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Puts a Markdown block – terminal output as a code block, a PDF quote –
// into a note: an open Markdown tab (inserted into the editor, saved by
// autosave), any note of the project (appended on disk) or a new note.
// The last target comes first, so repeated quotes take one Enter.
Dialog {
    id: dialog
    required property AppController app
    required property var editor
    property var actions
    property string heading
    /// `function(notePath) -> Markdown block` (links depend on the note).
    property var makeBlock: null
    property string doneText
    property string cwd
    property string newNoteName
    property var candidates: []
    property var filtered: []
    /// Note that received the last block.
    property string lastTarget

    anchors.centerIn: Overlay.overlay
    width: 560
    height: 460
    modal: true
    title: qsTr("In Notiz übernehmen")
    standardButtons: Dialog.Cancel

    /// Terminal output as a `console` code block.
    function ask(text, cwd) {
        askBlock(qsTr("%n Zeile(n) als Codeblock anhängen an:", "", text.split("\n").length),
                 () => dialog.app.codeBlock(text, "console"), cwd, qsTr("Ausgabe in %1 übernommen"), qsTr("Terminal-Ausgabe"))
    }

    /// Generic form: `doneText` contains %1 for the note name.
    function askBlock(heading, makeBlock, cwd, doneText, newNoteName) {
        dialog.heading = heading
        dialog.makeBlock = makeBlock
        dialog.cwd = cwd || dialog.app.projectRoot
        dialog.doneText = doneText || qsTr("In %1 übernommen")
        dialog.newNoteName = newNoteName || qsTr("Notiz")
        // The last target, open Markdown tabs, then all notes of the project.
        const open = dialog.editor.documents().filter(p => dialog.app.fileKind(p) === "markdown")
        const all = dialog.app.noteList()
        const first = dialog.lastTarget && dialog.app.exists(dialog.lastTarget) ? [dialog.lastTarget] : []
        const seen = {}
        dialog.candidates = first.concat(open, all).filter(p => !seen[p] && (seen[p] = true))
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

    /// Inserts the block at the end of `path`, separated by a blank line.
    function sendTo(path) {
        const block = dialog.makeBlock(path)
        if (!block) return
        const viewer = dialog.editor.viewerFor(path)
        const textEditor = viewer && viewer.editorItem ? viewer.editorItem : null
        if (textEditor && !textEditor.readOnly) {
            const text = textEditor.text
            const gap = text.length === 0 || text.endsWith("\n\n") ? "" : text.endsWith("\n") ? "\n" : "\n\n"
            textEditor.insertText(gap + block, true)
            textEditor.save()
        } else {
            const error = dialog.app.appendToNote(path, block)
            if (error) {
                dialog.actions.showMessage(qsTr("Übernehmen fehlgeschlagen: %1").arg(error))
                return
            }
        }
        dialog.lastTarget = path
        dialog.actions.showMessage(dialog.doneText.arg(dialog.app.fileName(path)))
        close()
    }
    function newNote() {
        const inProject = dialog.app.projectRoot && (dialog.cwd + "/").startsWith(dialog.app.projectRoot + "/")
        const dir = inProject ? dialog.cwd : (dialog.app.projectRoot || dialog.cwd)
        const name = (newName.text.trim() || dialog.newNoteName).replace(/\.md$/, "") + ".md"
        const path = dialog.actions.ops.newFile(dir, name)
        if (path) sendTo(path)
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8
        Label {
            Layout.fillWidth: true
            text: dialog.heading
            color: Theme.textMuted
            elide: Text.ElideRight
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
                    Label {
                        text: dialog.app.fileName(modelData) + (modelData === dialog.lastTarget ? qsTr("  · zuletzt") : "")
                        color: Theme.text
                    }
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
