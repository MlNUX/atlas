// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtWebEngine
import Atlas

// Markdown file: editor, live preview or both side by side, with
// backlinks and repair of broken links. The same view serves READMEs,
// documentation and personal notes – there is no separate note library.
Rectangle {
    id: root
    required property AppController app
    required property string source
    property int line: 0
    /// "edit", "split" or "preview".
    property string mode: "preview"
    property bool showBacklinks: false
    property var info: ({ tags: [], headings: [], broken: 0 })
    property bool pageReady: false
    property int previewLine: 1
    property real previewY: 0
    property var repairs: []
    /// FileActions (for "open in editor"); optional.
    property var actions: null

    /// Follow a link: `newPane` for Ctrl/middle click.
    signal linkActivated(string path, int line, bool newPane, var state)
    signal message(string text)
    signal savedAs(string path)

    readonly property alias editorItem: editor
    readonly property alias previewItem: web

    color: Theme.editor

    function viewState() {
        return {
            mode: root.mode,
            backlinks: root.showBacklinks,
            contextTab: contextPanel.tab,
            graphDepth: contextPanel.depth,
            line: root.mode === "edit" ? editor.topLine : root.previewLine,
            y: root.previewY,
            cursor: editor.viewState().cursor,
        }
    }
    function applyState(state) {
        if (state.mode) root.mode = state.mode
        if (state.backlinks !== undefined) root.showBacklinks = state.backlinks
        if (state.contextTab !== undefined) contextPanel.tab = state.contextTab
        if (state.graphDepth !== undefined) contextPanel.depth = state.graphDepth
        if (state.cursor !== undefined) editor.applyState({ cursor: state.cursor })
        if (state.line) {
            root.previewLine = state.line
            if (root.mode === "edit") editor.applyState({ line: state.line })
            else if (pageReady) web.runJavaScript("atlas.scrollToLine(" + state.line + ")")
        }
    }
    function reveal(newLine) {
        if (newLine <= 0) return
        if (root.mode !== "preview") editor.reveal(newLine)
        if (root.mode !== "edit") {
            root.previewLine = newLine
            if (pageReady) web.runJavaScript("atlas.scrollToLine(" + newLine + ")")
        }
    }

    function render() {
        if (!pageReady) return
        const result = JSON.parse(root.app.renderMarkdown(editor.text, root.source))
        root.info = result
        web.runJavaScript("atlas.set(" + JSON.stringify(result.html) + ")")
    }

    function followLink(link) {
        if (link.external) { Qt.openUrlExternally(link.external); return }
        if (!link.path) return
        if (link.broken) {
            root.message(qsTr("Link-Ziel nicht gefunden: %1").arg(root.app.relativePath(link.path)))
            root.app.findLinkRepairs(root.source)
            return
        }
        let target = link.line || link.page
        if (!target && link.anchor && root.app.fileKind(link.path) === "markdown")
            target = root.app.headingLine(link.path, link.anchor)
        root.linkActivated(link.path, target, link.newTab, link.annot ? { annot: link.annot } : {})
    }

    onSourceChanged: Qt.callLater(render)
    onLineChanged: reveal(line)
    Component.onCompleted: Qt.callLater(checkAttention)

    Connections {
        target: root.app
        function onLinkRepairsFound(path, json) {
            if (path !== root.source) return
            root.repairs = JSON.parse(json)
            if (root.repairs.length) repairDialog.open()
            else root.message(qsTr("Keine eindeutigen Reparaturvorschläge gefunden."))
        }
    }

    // Recovery copies and conflicts need a decision: show the editor with
    // its banners even when only the preview was visible.
    Connections {
        target: editor.document
        function onStateChanged() { root.checkAttention() }
    }

    function checkAttention() {
        const d = editor.document
        if ((d.hasRecovery || d.conflict) && root.mode === "preview") root.mode = "split"
    }

    Timer { id: renderTimer; interval: 150; onTriggered: root.render() }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ---- toolbar
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 30
            color: Theme.header
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 8
                spacing: 2
                Repeater {
                    model: [
                        { mode: "edit", text: qsTr("Bearbeiten") },
                        { mode: "split", text: qsTr("Geteilt") },
                        { mode: "preview", text: qsTr("Vorschau") },
                    ]
                    IconButton {
                        required property var modelData
                        glyph: modelData.text
                        checkable: true
                        checked: root.mode === modelData.mode
                        onClicked: {
                            const top = root.mode === "edit" ? editor.topLine : root.previewLine
                            root.mode = modelData.mode
                            Qt.callLater(root.applyState, { line: top })
                        }
                    }
                }
                Item { Layout.preferredWidth: 10 }
                Repeater {
                    model: root.info.tags || []
                    Label {
                        required property string modelData
                        text: "#" + modelData
                        color: "#9dd1ff"
                        font.pixelSize: Theme.smallFontSize
                        leftPadding: 4
                    }
                }
                Item { Layout.fillWidth: true }
                IconButton {
                    visible: root.info.broken > 0
                    glyph: "⚠ " + root.info.broken
                    tip: qsTr("%1 defekte Links – Reparatur suchen").arg(root.info.broken)
                    onClicked: root.app.findLinkRepairs(root.source)
                }
                IconButton {
                    iconName: "backlinks"; glyph: String(contextPanel.count)
                    tip: qsTr("Rückverweise")
                    checkable: true
                    checked: root.showBacklinks && contextPanel.tab === 0
                    onClicked: {
                        root.showBacklinks = !(root.showBacklinks && contextPanel.tab === 0)
                        contextPanel.tab = 0
                    }
                }
                IconButton {
                    iconName: "graph"
                    tip: qsTr("Lokaler Graph: verknüpfte Notizen, PDFs und Dateien")
                    checkable: true
                    checked: root.showBacklinks && contextPanel.tab === 1
                    onClicked: {
                        root.showBacklinks = !(root.showBacklinks && contextPanel.tab === 1)
                        contextPanel.tab = 1
                    }
                }
            }
        }

        SplitView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Horizontal

            TextEditor {
                id: editor
                SplitView.fillWidth: root.mode === "edit"
                SplitView.preferredWidth: root.mode === "split" ? parent.width / 2 : parent.width
                visible: root.mode !== "preview"
                app: root.app
                source: root.source
                wrap: true
                wikiCompletion: true
                actions: root.actions
                onEdited: renderTimer.restart()
                onSynced: { root.render(); root.app.noteChanged(root.source) }
                onSavedAs: (path) => root.savedAs(path)
                onTopLineChanged: if (root.mode === "split" && root.pageReady)
                    web.runJavaScript("atlas.scrollToLine(" + editor.topLine + ")")
            }

            WebEngineView {
                id: web
                SplitView.fillWidth: true
                visible: root.mode !== "edit"
                url: "atlas://app/web/preview.html"
                backgroundColor: Theme.editor
                settings.localContentCanAccessFileUrls: true
                settings.localContentCanAccessRemoteUrls: false
                settings.javascriptCanOpenWindows: false
                settings.focusOnNavigationEnabled: false
                onLoadingChanged: (request) => {
                    if (request.status !== WebEngineLoadingInfo.LoadSucceededStatus) return
                    root.pageReady = true
                    root.render()
                    if (root.previewLine > 1) web.runJavaScript("atlas.scrollToLine(" + root.previewLine + ")")
                }
                // Links inside the page never navigate it.
                onNavigationRequested: (request) => {
                    if (request.url.toString() !== "atlas://app/web/preview.html") request.reject()
                }
                onNewWindowRequested: (request) => Qt.openUrlExternally(request.requestedUrl)
                onJavaScriptConsoleMessage: (level, text, lineNumber, sourceId) => {
                    if (text.startsWith("atlas:link:")) root.followLink(JSON.parse(text.slice(11)))
                    else if (text.startsWith("atlas:scroll:")) {
                        const s = JSON.parse(text.slice(13))
                        root.previewLine = s.line
                        root.previewY = s.y
                    } else if (level !== WebEngineView.InfoMessageLevel) console.warn("preview:", text)
                }
            }

            // ---- backlinks and local graph
            ContextPanel {
                id: contextPanel
                SplitView.preferredWidth: 280
                SplitView.minimumWidth: 180
                visible: root.showBacklinks
                app: root.app
                path: root.source
                onOpenRequested: (path, line, newPane, state) => root.linkActivated(path, line, newPane, state)
            }
        }
    }

    // ---- confirm link repairs (files are never changed without asking)
    Dialog {
        id: repairDialog
        anchors.centerIn: parent
        width: Math.min(parent.width - 40, 640)
        modal: true
        title: qsTr("Defekte Links reparieren")
        standardButtons: Dialog.Apply | Dialog.Cancel
        property var chosen: ({})
        onAboutToShow: {
            const c = {}
            for (const r of root.repairs) c[r.broken] = true
            chosen = c
        }
        ColumnLayout {
            anchors.fill: parent
            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                color: Theme.textMuted
                text: qsTr("Atlas hat die Ziele folgender Links an neuer Stelle gefunden. Ausgewählte Links werden in der Datei angepasst.")
            }
            Repeater {
                model: root.repairs
                CheckBox {
                    required property var modelData
                    Layout.fillWidth: true
                    checked: true
                    text: qsTr("Zeile %1: %2 → %3").arg(modelData.line).arg(modelData.raw).arg(modelData.label)
                    onToggled: repairDialog.chosen[modelData.broken] = checked
                }
            }
        }
        onApplied: {
            // Unsaved edits go to disk first; the repair is based on them.
            editor.save()
            root.app.flushSession()
            const fixes = root.repairs.filter((r) => repairDialog.chosen[r.broken])
            if (fixes.length && root.app.applyLinkRepairs(root.source, JSON.stringify(fixes))) editor.document.checkDisk(false)
            close()
        }
    }
}
