// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// One split of the editor area: a header and a viewer chosen by file type.
Rectangle {
    id: pane
    required property AppController app
    property string path
    property string kind: path ? app.fileKind(path) : ""
    property bool active: false
    property int line: 0
    property var pendingState: ({})

    signal activated()
    signal closeRequested()
    signal splitRequested()

    color: Theme.editor
    border.color: active ? Theme.accent : "transparent"
    border.width: active ? 1 : 0

    function open(newPath, newLine) {
        pane.line = newLine || 0
        pane.path = newPath
    }
    function viewState() {
        const s = { path: pane.path, kind: pane.kind }
        if (viewer.item && viewer.item.viewState) Object.assign(s, viewer.item.viewState())
        return s
    }
    function applyState(state) {
        pane.pendingState = state
        if (viewer.item && viewer.item.applyState) viewer.item.applyState(state)
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: pane.border.width
        spacing: 0

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 32
            color: Theme.window
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 10
                anchors.rightMargin: 4
                spacing: 6
                Label {
                    text: pane.path ? pane.app.fileName(pane.path) : qsTr("Leer")
                    color: pane.active ? Theme.textStrong : Theme.textMuted
                    font.pixelSize: Theme.fontSize
                }
                Label {
                    Layout.fillWidth: true
                    text: pane.path ? pane.app.relativePath(pane.path) : ""
                    color: Theme.textFaint
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideMiddle
                }
                IconButton { glyph: "◫"; tip: qsTr("Rechts teilen"); onClicked: pane.splitRequested() }
                IconButton { glyph: "✕"; tip: qsTr("Schließen"); onClicked: pane.closeRequested() }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }

        Loader {
            id: viewer
            Layout.fillWidth: true
            Layout.fillHeight: true
            sourceComponent: pane.kind === "pdf" ? pdfComponent
                : pane.kind === "markdown" || pane.kind === "text" ? textComponent
                : pane.kind === "image" ? imageComponent
                : pane.path ? unsupportedComponent : emptyComponent
            onLoaded: if (item.applyState) item.applyState(pane.pendingState)
        }
    }

    // Clicking anywhere activates the pane without stealing the event.
    TapHandler {
        gesturePolicy: TapHandler.DragThreshold
        grabPermissions: PointerHandler.TakeOverForbidden
        onTapped: pane.activated()
    }

    Component {
        id: pdfComponent
        PdfViewer { source: pane.path }
    }
    Component {
        id: textComponent
        TextViewer {
            source: pane.path
            app: pane.app
            markdown: pane.kind === "markdown"
            line: pane.line
        }
    }
    Component {
        id: imageComponent
        Rectangle {
            color: Theme.pdfBackdrop
            Image {
                anchors.fill: parent
                anchors.margins: 16
                source: "file://" + pane.path
                fillMode: Image.PreserveAspectFit
                asynchronous: true
            }
        }
    }
    Component {
        id: unsupportedComponent
        Label {
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            color: Theme.textMuted
            text: qsTr("Keine Vorschau für diesen Dateityp.")
        }
    }
    Component {
        id: emptyComponent
        Label {
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            color: Theme.textFaint
            wrapMode: Text.Wrap
            text: qsTr("Datei im Explorer wählen.\nStrg+Klick öffnet sie in einem neuen Split.")
        }
    }
}
