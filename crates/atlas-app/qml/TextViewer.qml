// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Read-only text/Markdown view. Markdown is rendered with Qt's built-in
// CommonMark/GFM support (tables, code blocks); syntax highlighting, LaTeX
// and Mermaid follow in Phase 1.
Rectangle {
    id: root
    required property string source
    required property AppController app
    property bool markdown: false
    property bool showSource: false
    property int line: 0
    property string content: ""

    color: Theme.editor

    function reload() {
        content = root.app.readTextFile(root.source)
        Qt.callLater(scrollToLine)
    }
    function scrollToLine() {
        if (root.line <= 0 || (root.markdown && !root.showSource)) return
        let pos = 0
        for (let i = 1; i < root.line && pos >= 0; ++i) pos = content.indexOf("\n", pos) + 1
        if (pos < 0) return
        area.cursorPosition = pos
        const rect = area.positionToRectangle(pos)
        flick.contentY = Math.max(0, Math.min(rect.y - flick.height / 3, area.height - flick.height))
    }
    function viewState() { return { showSource: root.showSource } }
    function applyState(state) { if (state.showSource !== undefined) root.showSource = state.showSource }

    onSourceChanged: reload()
    onLineChanged: scrollToLine()
    Component.onCompleted: reload()

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 30
            color: Theme.header
            visible: root.markdown
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 8
                IconButton { glyph: qsTr("Vorschau"); checkable: true; checked: !root.showSource; onClicked: root.showSource = false }
                IconButton { glyph: qsTr("Quelltext"); checkable: true; checked: root.showSource; onClicked: root.showSource = true }
                Item { Layout.fillWidth: true }
                IconButton { glyph: "⟳"; tip: qsTr("Neu laden"); onClicked: root.reload() }
            }
        }

        Flickable {
            id: flick
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: width
            contentHeight: area.height
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}

            TextEdit {
                id: area
                width: flick.width
                readOnly: true
                selectByMouse: true
                persistentSelection: true
                padding: root.markdown && !root.showSource ? 24 : 12
                wrapMode: TextEdit.Wrap
                color: Theme.text
                selectionColor: Theme.activeSelection
                selectedTextColor: Theme.textStrong
                textFormat: root.markdown && !root.showSource ? TextEdit.MarkdownText : TextEdit.PlainText
                font.family: root.markdown && !root.showSource ? Qt.application.font.family : Theme.monoFont
                font.pixelSize: root.markdown && !root.showSource ? 15 : 13
                text: root.content
                onLinkActivated: (link) => Qt.openUrlExternally(link)
            }
        }
    }
}
