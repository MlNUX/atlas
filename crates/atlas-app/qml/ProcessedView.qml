// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtWebEngine
import Atlas

// A file shown through an extension's processor: the command's output as
// Markdown preview, plain text or PDF. Read-only; results are cached in
// `.atlas/cache/processed/` per file content.
Rectangle {
    id: root
    required property AppController app
    required property string source
    property var actions: null
    /// Processor ID ("" = the first automatic one for this file).
    property string processor: ""
    property var choices: []
    /// "markdown", "text" or "pdf" once converted.
    property string kind: ""
    property string content: ""
    property string error: ""
    property bool busy: false
    readonly property var current: choices.find(c => c.id === processor) || null

    signal linkActivated(string path, int line, bool newPane, var state)

    color: Theme.editor

    function viewState() { return { processor: root.processor } }
    function applyState(state) { if (state && state.processor) root.processor = state.processor }

    function run() {
        choices = JSON.parse(app.processorsFor(source))
        if (!processor || !choices.some(c => c.id === processor)) {
            const auto = choices.find(c => c.auto) || choices[0]
            processor = auto ? auto.id : ""
        }
        kind = ""
        content = ""
        error = processor ? "" : qsTr("Keine aktive Erweiterung kann diese Datei anzeigen.")
        busy = !!processor
        if (processor) app.processFile(processor, source)
    }
    // Several property changes at creation lead to one conversion.
    Timer { id: runTimer; interval: 0; onTriggered: root.run() }
    onSourceChanged: runTimer.restart()
    onProcessorChanged: runTimer.restart()
    Component.onCompleted: runTimer.restart()

    Connections {
        target: root.app
        function onFileProcessed(path, id, kind, content, error) {
            if (path !== root.source || id !== root.processor) return
            root.busy = false
            root.kind = kind
            root.content = content
            root.error = error
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 30
            color: Theme.header
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 8
                spacing: 6
                Label {
                    text: root.current ? qsTr("Angezeigt mit „%1“ (Erweiterung %2) – nur lesen").arg(root.current.name).arg(root.current.extension)
                                       : qsTr("Anzeige über Erweiterung")
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                ComboBox {
                    visible: root.choices.length > 1
                    implicitHeight: 24
                    font.pixelSize: Theme.smallFontSize
                    model: root.choices.map(c => c.name)
                    currentIndex: Math.max(0, root.choices.findIndex(c => c.id === root.processor))
                    onActivated: (index) => root.processor = root.choices[index].id
                }
                IconButton { glyph: "⟳"; tip: qsTr("Erneut umwandeln"); enabled: !root.busy; onClicked: root.run() }
                IconButton {
                    glyph: "↗"
                    tip: qsTr("Mit Standardprogramm öffnen")
                    onClicked: Qt.openUrlExternally("file://" + encodeURI(root.source))
                }
            }
        }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            BusyIndicator { anchors.centerIn: parent; running: root.busy; visible: root.busy }
            ScrollView {
                anchors.fill: parent
                anchors.margins: 16
                visible: root.error !== ""
                TextArea {
                    readOnly: true
                    selectByMouse: true
                    wrapMode: Text.Wrap
                    color: Theme.error
                    background: null
                    text: root.error
                }
            }
            Loader {
                anchors.fill: parent
                active: !root.busy && root.error === "" && root.kind !== ""
                sourceComponent: root.kind === "pdf" ? pdfView : root.kind === "markdown" ? markdownView : textView
            }
        }
    }

    Component {
        id: textView
        ScrollView {
            TextArea {
                readOnly: true
                selectByMouse: true
                text: root.content
                color: Theme.text
                font.family: Theme.monoFont
                font.pixelSize: 13
                wrapMode: TextEdit.NoWrap
                background: null
            }
        }
    }
    Component {
        id: pdfView
        PdfViewer { source: root.content; app: root.app; actions: root.actions }
    }
    Component {
        id: markdownView
        WebEngineView {
            id: web
            url: "atlas://app/web/preview.html"
            backgroundColor: Theme.editor
            settings.localContentCanAccessFileUrls: true
            settings.localContentCanAccessRemoteUrls: false
            settings.javascriptCanOpenWindows: false
            settings.focusOnNavigationEnabled: false
            onLoadingChanged: (request) => {
                if (request.status !== WebEngineLoadingInfo.LoadSucceededStatus) return
                // Links resolve relative to the original file.
                const result = JSON.parse(root.app.renderMarkdown(root.content, root.source))
                web.runJavaScript("atlas.set(" + JSON.stringify(result.html) + ")")
            }
            onNavigationRequested: (request) => { if (request.url.toString() !== "atlas://app/web/preview.html") request.reject() }
            onJavaScriptConsoleMessage: (level, text) => {
                if (!text.startsWith("atlas:link:")) return
                const link = JSON.parse(text.slice(11))
                if (link.external) Qt.openUrlExternally(link.external)
                else if (link.path && !link.broken) root.linkActivated(link.path, link.line || link.page, link.newTab, link.annot ? { annot: link.annot } : {})
            }
            // Never stays black after a workspace switch (#2).
            WebRepaint { id: webRepaint }
        }
    }
}
