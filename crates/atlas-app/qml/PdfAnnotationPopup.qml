// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// One annotation of a PDF: comment, colour, quote, link, delete.
Popup {
    id: annotPopup
    required property var viewer
    required property var annotations
    property string annotId
    property var info: null
    width: 320
    padding: 10
    modal: false
    focus: true
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    background: Rectangle { color: Theme.header; border.color: Theme.border; radius: 6 }

    /// `pos` in viewer coordinates (null: centered); `edit` focuses the comment.
    function show(id, pos, edit) {
        const a = viewer.annotation(id)
        if (!a) return
        annotId = id
        info = a
        commentField.text = a.kind === "bookmark" ? (a.title || "") : (a.comment || "")
        if (pos) {
            x = Math.max(4, Math.min(viewer.width - width - 4, pos.x - width / 2))
            y = Math.max(4, Math.min(viewer.height - 220, pos.y + 12))
        } else {
            x = (viewer.width - width) / 2
            y = viewer.height / 4
        }
        open()
        if (edit) commentField.forceActiveFocus()
    }
    function commit() {
        if (!info || !viewer.annotation(annotId)) return
        const text = commentField.text.trim()
        if (info.kind === "bookmark") {
            if (text !== (info.title || "")) viewer.updateAnnot(annotId, JSON.stringify({ title: text }))
        } else if (text !== (info.comment || "")) {
            viewer.updateAnnot(annotId, JSON.stringify({ comment: text }))
        }
        // A pinned note without text is dropped.
        if (info.kind === "note" && !text) viewer.removeAnnot(annotId)
    }
    onClosed: { commit(); viewer.forceActiveFocus() }

    ColumnLayout {
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: 6
        RowLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                text: annotPopup.info ? viewer.kindGlyph(annotPopup.info.kind) + "  " + qsTr("Seite %1").arg(annotPopup.info.page) : ""
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
            }
            Repeater {
                model: annotPopup.info && annotPopup.info.kind !== "bookmark" ? viewer.markColors : []
                AbstractButton {
                    required property string modelData
                    implicitWidth: 18
                    implicitHeight: 18
                    focusPolicy: Qt.NoFocus
                    background: null
                    contentItem: Rectangle {
                        anchors.centerIn: parent
                        width: 12; height: 12; radius: 6
                        color: parent.modelData
                        border.color: annotPopup.info && annotPopup.info.color === parent.modelData ? "white" : "transparent"
                    }
                    onClicked: {
                        viewer.updateAnnot(annotPopup.annotId, JSON.stringify({ color: modelData }))
                        annotPopup.info = viewer.annotation(annotPopup.annotId)
                    }
                }
            }
        }
        Label {
            Layout.fillWidth: true
            visible: !!annotPopup.info && !!annotPopup.info.text
            text: annotPopup.info ? "„" + annotPopup.info.text + "“" : ""
            color: Theme.text
            wrapMode: Text.Wrap
            maximumLineCount: 4
            elide: Text.ElideRight
            font.italic: true
        }
        TextArea {
            id: commentField
            Layout.fillWidth: true
            Layout.preferredHeight: Math.max(60, contentHeight + 16)
            placeholderText: annotPopup.info && annotPopup.info.kind === "bookmark" ? qsTr("Titel des Lesezeichens") : qsTr("Kommentar …")
            wrapMode: TextEdit.Wrap
            Keys.onPressed: (event) => {
                if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && (event.modifiers & Qt.ControlModifier)) {
                    annotPopup.close()
                    event.accepted = true
                }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 4
            Button {
                visible: !!annotPopup.info && ["highlight", "underline", "strikeout"].indexOf(annotPopup.info.kind) >= 0
                text: qsTr("❝ Zitat in Notiz")
                font.pixelSize: Theme.smallFontSize
                onClicked: { const id = annotPopup.annotId; annotPopup.close(); viewer.quote(id) }
            }
            Button {
                text: qsTr("Link kopieren")
                font.pixelSize: Theme.smallFontSize
                onClicked: {
                    if (viewer.app) viewer.app.copyToClipboard(annotations.linkFor(annotPopup.annotId))
                    viewer.message(qsTr("Link auf die Anmerkung kopiert"))
                }
            }
            Item { Layout.fillWidth: true }
            Button {
                text: qsTr("Löschen")
                font.pixelSize: Theme.smallFontSize
                onClicked: { const id = annotPopup.annotId; annotPopup.info = null; viewer.removeAnnot(id); annotPopup.close() }
            }
        }
    }
}
