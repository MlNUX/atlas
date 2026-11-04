// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// The formula recognized as LaTeX: to check, edit and copy.
Popup {
    id: latexPopup
    required property var viewer
    property bool busy: false
    property string error: ""
    property bool firstStart: false
    /// Download of the model (first use).
    property string progress: ""
    readonly property alias latex: latexField.text
    function start(warm) {
        busy = true
        error = ""
        progress = ""
        firstStart = !warm
        latexField.text = ""
        open()
    }
    function finish(text, err) {
        busy = false
        error = err
        latexField.text = text
    }
    x: Math.round((viewer.width - width) / 2)
    y: 56
    width: Math.min(viewer.width - 40, 560)
    padding: 12
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    background: Rectangle { color: Theme.header; border.color: Theme.border; radius: 8 }
    contentItem: ColumnLayout {
        spacing: 8
        RowLayout {
            spacing: 8
            Text { text: "∑"; color: Theme.accent; font.pixelSize: Theme.fontSize + 2 }
            Text {
                Layout.fillWidth: true
                text: latexPopup.busy
                    ? (latexPopup.progress || (latexPopup.firstStart ? qsTr("Erkenne Formel … (das Modell wird geladen)") : qsTr("Erkenne Formel …")))
                    : latexPopup.error ? qsTr("Keine Formel erkannt") : qsTr("LaTeX – kopiert, hier noch änderbar")
                color: Theme.textStrong
                font.pixelSize: Theme.fontSize
                elide: Text.ElideRight
            }
            BusyIndicator { running: latexPopup.busy; visible: running; Layout.preferredWidth: 20; Layout.preferredHeight: 20 }
        }
        Text {
            visible: latexPopup.error !== ""
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            color: Theme.error
            font.pixelSize: Theme.smallFontSize
            text: latexPopup.error + (/Download|github|ONNX Runtime/i.test(latexPopup.error)
                ? "\n" + qsTr("Beim ersten Mal lädt Atlas das Formelmodell aus dem Netz (etwa 170 MB) – mehr in docs/latex-ocr.md") : "")
        }
        TextArea {
            id: latexField
            visible: !latexPopup.busy && !latexPopup.error
            Layout.fillWidth: true
            wrapMode: TextEdit.Wrap
            selectByMouse: true
            font.family: "monospace"
            font.pixelSize: Theme.fontSize
        }
        RowLayout {
            visible: !latexPopup.busy && !latexPopup.error
            spacing: 6
            Item { Layout.fillWidth: true }
            Button { text: qsTr("$…$ kopieren"); flat: true; onClicked: { viewer.app.copyToClipboard("$" + latexField.text + "$"); viewer.message(qsTr("Mit $…$ kopiert")) } }
            Button { text: qsTr("$$…$$ kopieren"); flat: true; onClicked: { viewer.app.copyToClipboard("$$\n" + latexField.text + "\n$$"); viewer.message(qsTr("Mit $$…$$ kopiert")) } }
            Button { text: qsTr("Kopieren"); highlighted: true; onClicked: { viewer.app.copyToClipboard(latexField.text); latexPopup.close(); viewer.message(qsTr("LaTeX kopiert")) } }
        }
    }
}
