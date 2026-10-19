// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Project wide text search on top of ripgrep (respects .gitignore).
Rectangle {
    id: root
    required property SearchModel model
    required property string projectRoot

    signal openRequested(string path, int line, bool newPane)

    color: Theme.sidebar

    function focusQuery() {
        query.forceActiveFocus()
        query.selectAll()
    }

    function viewState() {
        return { query: query.text, regex: regex.checked, caseSensitive: caseSensitive.checked, globs: globs.text, buildDirs: buildDirs.checked }
    }
    function applyState(state) {
        query.text = state.query || ""
        regex.checked = !!state.regex
        caseSensitive.checked = !!state.caseSensitive
        globs.text = state.globs || ""
        buildDirs.checked = !!state.buildDirs
        if (query.text) root.run()
        else root.model.clear()
    }

    function setQuery(text) {
        query.text = text
        root.run()
    }

    /// Finds TODO and FIXME markers in the project.
    function searchTodos() {
        query.text = "\\b(TODO|FIXME)\\b"
        regex.checked = true
        caseSensitive.checked = true
        root.run()
    }

    function run() {
        root.model.search(root.projectRoot, query.text, regex.checked, caseSensitive.checked, globs.text, buildDirs.checked)
    }

    Timer {
        id: debounce
        interval: 250
        onTriggered: root.run()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 6

        RowLayout {
            Layout.fillWidth: true
            Label {
                Layout.fillWidth: true
                text: qsTr("SUCHE")
                color: Theme.text
                font.pixelSize: Theme.smallFontSize
                font.bold: true
                Layout.leftMargin: 4
            }
            IconButton { glyph: "TODO"; tip: qsTr("TODO und FIXME im Projekt finden"); onClicked: root.searchTodos() }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 2
            TextField {
                id: query
                Layout.fillWidth: true
                placeholderText: qsTr("Suchen")
                selectByMouse: true
                onTextEdited: debounce.restart()
                onAccepted: { debounce.stop(); root.run() }
                Keys.onEscapePressed: root.model.cancel()
            }
            IconButton { id: caseSensitive; glyph: "Aa"; tip: qsTr("Groß-/Kleinschreibung beachten"); checkable: true; onToggled: debounce.restart() }
            IconButton { id: regex; glyph: ".*"; tip: qsTr("Regulärer Ausdruck"); checkable: true; onToggled: debounce.restart() }
            IconButton { id: buildDirs; glyph: "⚙"; tip: qsTr("Build- und Abhängigkeitsordner einbeziehen (target, node_modules, …)"); checkable: true; onToggled: debounce.restart() }
        }

        TextField {
            id: globs
            Layout.fillWidth: true
            placeholderText: qsTr("Dateimuster, z. B. *.md, !docs/**")
            selectByMouse: true
            font.pixelSize: Theme.smallFontSize
            onTextEdited: debounce.restart()
        }

        Label {
            Layout.fillWidth: true
            Layout.leftMargin: 4
            color: root.model.error ? Theme.error : Theme.textMuted
            font.pixelSize: Theme.smallFontSize
            elide: Text.ElideRight
            text: root.model.error ? root.model.error
                : root.model.running ? qsTr("Suche läuft … %1 Treffer").arg(root.model.count)
                : query.text.length === 0 ? qsTr("ripgrep · .gitignore wird berücksichtigt")
                : qsTr("%1 Treffer in %2 ms%3").arg(root.model.count).arg(root.model.elapsedMs)
                    .arg(root.model.truncated ? qsTr(" (gekürzt)") : "")
        }

        ListView {
            id: results
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: root.model
            reuseItems: true
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}

            delegate: Column {
                id: hit
                required property string path
                required property string relativePath
                required property int line
                required property string text
                required property int matchStart
                required property int matchEnd
                required property bool firstInFile
                width: ListView.view.width

                Text {
                    visible: hit.firstInFile
                    width: parent.width
                    height: visible ? 24 : 0
                    verticalAlignment: Text.AlignBottom
                    text: hit.relativePath
                    color: Theme.textStrong
                    font.pixelSize: Theme.fontSize
                    font.bold: true
                    elide: Text.ElideMiddle
                }
                Rectangle {
                    width: parent.width
                    height: 20
                    color: area.containsMouse ? Theme.hover : "transparent"
                    Text {
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        verticalAlignment: Text.AlignVCenter
                        textFormat: Text.StyledText
                        elide: Text.ElideRight
                        color: Theme.text
                        font.pixelSize: Theme.fontSize
                        text: "<font color='" + Theme.textFaint + "'>" + hit.line + "</font>  " + root.highlight(hit.text, hit.matchStart, hit.matchEnd)
                    }
                    MouseArea {
                        id: area
                        anchors.fill: parent
                        hoverEnabled: true
                        acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                        onClicked: (mouse) => root.openRequested(hit.path, hit.line,
                            mouse.button === Qt.MiddleButton || (mouse.modifiers & Qt.ControlModifier))
                    }
                }
            }
        }
    }

    function escapeHtml(s) {
        return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    }

    // Shows the match with some leading context and a highlight.
    function highlight(text, start, end) {
        let from = Math.max(0, start - 24)
        const lead = text.slice(from, start).replace(/^\s+/, "")
        return (from > 0 ? "…" : "") + escapeHtml(lead)
            + "<b><font color='" + Theme.matchText + "'>" + escapeHtml(text.slice(start, end)) + "</font></b>"
            + escapeHtml(text.slice(end, end + 160))
    }
}
