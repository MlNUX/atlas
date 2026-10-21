// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// "Verknüpfen mit …": relates a file with another file of the project
// (e.g. two PDFs). The relation lives in `.atlas/relations.json`, shows in
// the graph and under "Verknüpfungen"; the files stay unchanged.
Dialog {
    id: dialog
    required property AppController app
    /// The file to relate.
    property string source
    property var related: []
    property var results: []

    anchors.centerIn: Overlay.overlay
    width: 560
    height: 480
    modal: true
    title: qsTr("Verknüpfen mit …")
    standardButtons: Dialog.Cancel

    function ask(path) {
        source = path
        try { related = JSON.parse(app.relatedFiles(path)).map(r => r.path) } catch (e) { related = [] }
        field.text = ""
        search()
        open()
        field.forceActiveFocus()
    }
    /// Files of the project (no folders, no hidden files, not the file itself).
    function search() {
        let found = []
        try { found = JSON.parse(app.quickOpen(field.text, 200)) } catch (e) {}
        results = found.filter(r => r.kind !== "dir" && r.path !== source && !/(^|\/)\./.test(r.relative)).slice(0, 80)
        list.currentIndex = 0
    }
    function pick(r) {
        if (!r) return
        if (related.indexOf(r.path) < 0) app.relateFiles(source, r.path)
        close()
    }

    contentItem: ColumnLayout {
        spacing: 10
        Label {
            Layout.fillWidth: true
            text: qsTr("%1 verknüpfen mit:").arg(dialog.app.relativePath(dialog.source))
            color: Theme.textMuted
            elide: Text.ElideMiddle
        }
        TextField {
            id: field
            Layout.fillWidth: true
            placeholderText: qsTr("Datei suchen …")
            onTextEdited: dialog.search()
            Keys.onDownPressed: list.incrementCurrentIndex()
            Keys.onUpPressed: list.decrementCurrentIndex()
            onAccepted: dialog.pick(dialog.results[list.currentIndex])
        }
        ListView {
            id: list
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: dialog.results
            highlightMoveDuration: 0
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                id: row
                required property var modelData
                required property int index
                readonly property bool done: dialog.related.indexOf(modelData.path) >= 0
                width: ListView.view.width
                height: 34
                radius: 6
                color: ListView.isCurrentItem ? Theme.activeSelection : rowHover.hovered ? Theme.hover : "transparent"
                HoverHandler { id: rowHover }
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: dialog.pick(row.modelData) }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 10
                    anchors.rightMargin: 10
                    spacing: 10
                    Icon {
                        readonly property var look: Theme.fileIcon(row.modelData.relative, false)
                        name: look[0]
                        color: look[1]
                        size: 16
                    }
                    Text {
                        Layout.fillWidth: true
                        text: row.modelData.relative
                        color: Theme.textStrong
                        font.pixelSize: Theme.fontSize
                        elide: Text.ElideMiddle
                    }
                    Text {
                        visible: row.done
                        text: qsTr("verknüpft")
                        color: Theme.accent
                        font.pixelSize: Theme.smallFontSize
                    }
                }
            }
            Label {
                anchors.centerIn: parent
                visible: list.count === 0
                text: qsTr("Keine passende Datei")
                color: Theme.textFaint
            }
        }
    }
}
