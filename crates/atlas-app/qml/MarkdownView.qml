// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
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
    /// preview.html is loaded in the preview.
    property bool pageLoaded: false
    /// The preview can take content (loaded, and the view is there).
    readonly property bool pageReady: pageLoaded && web !== null

    // ---- memory of the preview (Chromium): created when first shown,
    // destroyed after a while out of sight (with its browser process),
    // created again when shown.
    readonly property bool previewShown: root.visible && root.mode !== "edit"
    property bool previewWanted: false
    /// Out of sight this long, the preview is destroyed.
    property int discardAfter: 5 * 60 * 1000
    // Decided a moment later: a new tab becomes visible before it applies
    // its state (a restored tab may be in edit mode).
    onPreviewShownChanged: Qt.callLater(updatePreview)
    function updatePreview() {
        if (previewShown) {
            previewWanted = true
            discardTimer.stop()
        } else if (previewWanted) {
            discardTimer.restart()
        }
    }
    /// Destroys a hidden preview (it is created again when shown).
    function discardPreview() {
        if (previewShown || !previewWanted) return false
        pageLoaded = false
        previewWanted = false
        return true
    }
    /// The preview (WebEngineView), null while there is none.
    readonly property var web: webLoader.item ? webLoader.item.webView : null
    Timer { id: discardTimer; interval: root.discardAfter; onTriggered: root.discardPreview() }
    property int previewLine: 1
    /// When the editor last moved because the preview was scrolled (no echo).
    property real syncedFromPreview: 0
    onModeChanged: if (mode === "split" && pageReady) followTimer.restart()
    // Resized (window, split divider): the middle moved, line up again.
    Connections {
        target: editor
        function onViewHeightChanged() { if (root.mode === "split" && root.pageReady) followTimer.restart() }
    }
    // Coalesces editor scroll steps into one message per frame.
    Timer {
        id: followTimer
        interval: 0
        // Lined up in the middle of the view, where one reads: the preview
        // sets type larger, so heights differ further up and down.
        onTriggered: {
            if (!root.web || !root.pageReady) return
            const mid = Math.round(Math.min(editor.viewHeight, web.height) / 2)
            web.runJavaScript("atlas.follow(" + editor.exactLineAt(mid) + "," + mid + ")")
        }
    }
    property real previewY: 0
    property var repairs: []
    /// FileActions (for "open in editor"); optional.
    property var actions: null

    /// Follow a link: `newPane` for Ctrl/middle click.
    signal linkActivated(string path, int line, bool newPane, var state)
    signal message(string text)
    signal savedAs(string path)

    readonly property alias editorItem: editor
    /// Keyboard focus: the editor when it is shown, else the view.
    function focusContent() { if (mode !== "preview") editor.focusText(); else root.forceActiveFocus() }
    // ---- inserting (menu "Einfügen" and command palette)
    /// Mermaid templates for "Diagramm einfügen".
    readonly property var diagramTemplates: [
        { name: qsTr("Flussdiagramm"), code: "flowchart TD\n    A[Start] --> B{Bedingung?}\n    B -->|ja| C[Schritt]\n    B -->|nein| D[Anderer Schritt]\n    C --> E[Ende]\n    D --> E" },
        { name: qsTr("Sequenzdiagramm"), code: "sequenceDiagram\n    participant A as Client\n    participant B as Server\n    A->>B: Anfrage\n    B-->>A: Antwort" },
        { name: qsTr("Klassendiagramm"), code: "classDiagram\n    class Tier {\n        +String name\n        +laut() void\n    }\n    class Hund\n    Tier <|-- Hund" },
        { name: qsTr("Zustandsdiagramm"), code: "stateDiagram-v2\n    [*] --> Bereit\n    Bereit --> Läuft: start\n    Läuft --> Bereit: stopp\n    Läuft --> [*]" },
        { name: qsTr("ER-Diagramm"), code: "erDiagram\n    STUDENT ||--o{ KURS : belegt\n    KURS }o--|| DOZENT : hält" },
        { name: qsTr("Gantt-Plan"), code: "gantt\n    title Projektplan\n    dateFormat YYYY-MM-DD\n    section Phase 1\n    Recherche :a1, 2026-11-01, 7d\n    Entwurf   :after a1, 5d\n    section Phase 2\n    Umsetzung :2026-11-14, 10d" },
        { name: qsTr("Mindmap"), code: "mindmap\n  root((Thema))\n    Idee A\n      Detail\n    Idee B\n    Idee C" },
        { name: qsTr("Kreisdiagramm"), code: "pie title Verteilung\n    \"A\" : 45\n    \"B\" : 30\n    \"C\" : 25" },
        { name: qsTr("Zeitstrahl"), code: "timeline\n    title Verlauf\n    2024 : Start\n    2025 : Ausbau\n    2026 : Abschluss" },
    ]
    /// The editor must be visible to insert at its cursor.
    function editorForInsert() {
        if (root.mode === "preview") root.mode = "split"
        return editor
    }
    function insertDiagram(index) {
        const t = diagramTemplates[index]
        if (t) editorForInsert().insertBlock("```mermaid\n" + t.code + "\n```")
    }
    /// Markdown table with `rows` body rows and `cols` columns.
    function tableMarkdown(rows, cols) {
        const cells = (f) => "| " + Array.from({ length: cols }, (_, i) => f(i)).join(" | ") + " |"
        const lines = [cells(i => qsTr("Spalte %1").arg(i + 1)), cells(() => "---")]
        for (let r = 0; r < rows; ++r) lines.push(cells(() => "   "))
        return lines.join("\n")
    }
    function insertTable(rows, cols) { editorForInsert().insertBlock(tableMarkdown(Math.max(1, rows), Math.max(1, cols))) }
    function askTable() { tableDialog.open() }
    function askImage() { imageDialog.open() }
    function askFolderTree() { treeDialog.open() }
    function insertFolderTree(path) {
        if (path) editorForInsert().insertBlock("```tree\n" + root.app.folderTree(path) + "```")
    }
    /// Runs a shell block of the preview in Atlas' terminal, in the folder of
    /// the note; the terminal stays open afterwards.
    function runCode(code) {
        if (!root.actions || !code.trim()) return
        const dir = root.source.slice(0, root.source.lastIndexOf("/")) || "/"
        const script = code + "\nstatus=$?\necho\necho \"── beendet (Exit $status) ──\"\nexec \"${SHELL:-bash}\""
        root.actions.runInTerminal(["bash", "-c", script], dir)
    }

    /// A picture of the preview in its own window. Local pictures come as
    /// `atlas://file/<path>` (served to the preview only): as file URL.
    function showImage(picture) {
        const src = picture.src.startsWith("atlas://file/")
            ? "file://" + picture.src.slice("atlas://file".length) : picture.src
        imageWindow.show(src, picture.alt)
    }
    readonly property alias imageWindowItem: imageWindow
    ImageWindow { id: imageWindow }

    readonly property var previewItem: root.web
    readonly property var previewRepaint: webLoader.item ? webLoader.item.repaint : null
    readonly property alias contextPanelItem: contextPanel

    color: Theme.editor

    function viewState() {
        return {
            mode: root.mode,
            backlinks: root.showBacklinks,
            contextTab: contextPanel.tab,
            line: root.mode === "edit" ? editor.topLine : root.previewLine,
            y: root.previewY,
            cursor: editor.viewState().cursor,
        }
    }
    /// Marks a hit of the project search in the preview (`{ text,
    /// caseSensitive, line }`) once the page shows the note.
    property var pendingHits: null
    function applyHits() {
        if (!root.pendingHits || !pageReady) return
        const f = root.pendingHits
        root.pendingHits = null
        web.runJavaScript("atlas.markHits(" + JSON.stringify(f.text || "") + ", " + !!f.caseSensitive + ", " + (f.line || 0) + ")")
    }
    function applyState(state) {
        if (state.find) {
            editor.applyState({ find: state.find })
            root.pendingHits = state.find
            // After a render that is already queued (new source).
            Qt.callLater(root.applyHits)
        }
        if (state.mode) root.mode = state.mode
        if (state.backlinks !== undefined) root.showBacklinks = state.backlinks
        if (state.contextTab !== undefined) contextPanel.tab = state.contextTab
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

    // The preview follows the design (diagrams are drawn again).
    Connections { target: Theme; function onLightChanged() { root.render() } }

    function render() {
        if (!pageReady) return
        const result = JSON.parse(root.app.renderMarkdown(editor.text, root.source))
        root.info = result
        web.runJavaScript("atlas.setLight(" + Theme.light + "); atlas.set(" + JSON.stringify(result.html) + ")")
        // New content: line up with the editor again.
        if (root.mode === "split") followTimer.restart()
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
    Component.onCompleted: {
        // After the tab applied its state (a restored tab may be in edit mode).
        Qt.callLater(updatePreview)
        Qt.callLater(checkAttention)
    }

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
                        color: Theme.link
                        font.pixelSize: Theme.smallFontSize
                        leftPadding: 4
                    }
                }
                Item { Layout.fillWidth: true }
                IconButton {
                    id: insertButton
                    glyph: qsTr("＋ Einfügen")
                    tip: qsTr("Bild, Tabelle, Diagramm oder Ordnerbaum an der Schreibmarke einfügen")
                    onClicked: insertMenu.popup(insertButton, 0, insertButton.height)
                    Menu {
                        id: insertMenu
                        MenuItem { text: qsTr("Bild …"); onTriggered: root.askImage() }
                        MenuItem { text: qsTr("Tabelle …"); onTriggered: root.askTable() }
                        Menu {
                            id: diagramMenu
                            title: qsTr("Diagramm")
                            Instantiator {
                                model: root.diagramTemplates
                                delegate: MenuItem {
                                    required property var modelData
                                    required property int index
                                    text: modelData.name
                                    onTriggered: root.insertDiagram(index)
                                }
                                onObjectAdded: (index, object) => diagramMenu.insertItem(index, object)
                                onObjectRemoved: (index, object) => diagramMenu.removeItem(object)
                            }
                        }
                        MenuItem { text: qsTr("Ordnerbaum …"); onTriggered: root.askFolderTree() }
                    }
                }
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
                    tip: qsTr("Graph: Notizen, PDFs und Dateien des Projekts")
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
                briskScrolling: true
                actions: root.actions
                onEdited: renderTimer.restart()
                onSynced: { root.render(); root.app.noteChanged(root.source) }
                onSavedAs: (path) => root.savedAs(path)
                // Split view: the preview follows smoothly (#7) – unless this
                // movement came from the preview itself.
                onScrollYChanged: if (root.mode === "split" && root.pageReady && Date.now() - root.syncedFromPreview > 200) followTimer.restart()
            }

            // Created only once the preview is shown (a tab in edit mode, or
            // one not looked at since a session restore, starts no browser).
            // The view comes from PreviewPool: after the first note, one waits
            // loaded, so switching to another note shows it at once.
            Loader {
                id: webLoader
                SplitView.fillWidth: true
                visible: root.mode !== "edit"
                active: root.previewWanted
                sourceComponent: Component {
                    Item {
                        id: webHost
                        readonly property alias repaint: webRepaint
                        /// The WebEngineView with preview.html (from the pool).
                        property var webView: null
                        Component.onCompleted: {
                            webView = PreviewPool.take(webHost)
                            webView.anchors.fill = webHost
                            // A spare was hidden: have Chromium paint it here.
                            webRepaint.request()
                            // Loaded already (the pool's spare): once the Loader
                            // has set its item, so `root.web` is there.
                            if (webView.ready) Qt.callLater(loaded)
                        }
                        Component.onDestruction: if (webView) webView.destroy()
                        function loaded() {
                            root.pageLoaded = true
                            root.render()
                            if (root.previewLine > 1) webView.runJavaScript("atlas.scrollToLine(" + root.previewLine + ")")
                            root.applyHits()
                        }
                        Connections {
                            target: webHost.webView
                            function onLoadingChanged(request) {
                                if (request.status === WebEngineLoadingInfo.LoadSucceededStatus) webHost.loaded()
                            }
                            // Links inside the page never navigate it.
                            function onNavigationRequested(request) {
                                if (request.url.toString() !== "atlas://app/web/preview.html") request.reject()
                            }
                            function onNewWindowRequested(request) { Qt.openUrlExternally(request.requestedUrl) }
                            function onJavaScriptConsoleMessage(level, text, lineNumber, sourceId) {
                                if (text.startsWith("atlas:link:")) root.followLink(JSON.parse(text.slice(11)))
                                else if (text.startsWith("atlas:copy:")) {
                                    root.app.copyToClipboard(JSON.parse(text.slice(11)).text)
                                    root.message(qsTr("Code kopiert"))
                                }
                                else if (text.startsWith("atlas:run:")) root.runCode(JSON.parse(text.slice(10)).code)
                                else if (text.startsWith("atlas:image:")) root.showImage(JSON.parse(text.slice(12)))
                                else if (text.startsWith("atlas:task:")) {
                                    const t = JSON.parse(text.slice(11))
                                    if (!editor.toggleTask(t.line, t.checked)) root.render()
                                }
                                else if (text.startsWith("atlas:scroll:")) {
                                    const s = JSON.parse(text.slice(13))
                                    pageScroll.track(s.y, s.height)
                                    if (s.own) return
                                    root.previewLine = s.line
                                    root.previewY = s.y
                                    // Split view: the editor follows the preview (#7).
                                    if (root.mode === "split" && s.exact) {
                                        root.syncedFromPreview = Date.now()
                                        editor.scrollToExactLine(s.exact, s.at || 0)
                                    }
                                } else if (level !== WebEngineView.InfoMessageLevel) console.warn("preview:", text)
                            }
                        }
                        // Never stays black after a workspace switch (#2).
                        WebRepaint { id: webRepaint; view: webHost.webView; z: 1 }

                        // Touchpad and wheel scroll like in the editor (TouchpadFling,
                        // brisk): it moves `pageScroll`, which keeps track of the
                        // page's position and height (CSS pixels) so that a glide
                        // ends at the top and bottom, and the page follows each
                        // step with scrollBy – relative, so a position reported
                        // late never makes it jump. Sideways steps stay with the
                        // page (wide code blocks and tables); Ctrl+wheel too.
                        Flickable {
                            id: pageScroll
                            visible: false
                            width: webHost.webView ? webHost.width / webHost.webView.zoomFactor : 0
                            height: webHost.webView ? webHost.height / webHost.webView.zoomFactor : 0
                            contentWidth: width
                            /// The page moved by itself (keys, scroll bar, links,
                            /// following the editor) or changed its height: taken
                            /// over while no swipe or glide is going on.
                            function track(y, pageHeight) {
                                contentHeight = Math.max(height, pageHeight || 0)
                                if (pageFling.gliding || Date.now() - pageFling.lastStep <= 250) return
                                contentY = y
                                pageFling.carry = 0
                            }
                        }
                        TouchpadFling {
                            id: pageFling
                            /// When the last step was handed to the page.
                            property real lastStep: 0
                            /// Movement not handed on yet: Chromium drops scrollBy steps
                            /// below half a pixel, so only whole pixels go to the page.
                            property real carry: 0
                            anchors.fill: parent
                            z: 2
                            flickable: pageScroll
                            brisk: true
                            horizontal: false
                            active: root.app.touchpadFling
                            app: root.app
                            scroller: (dx, dy) => {
                                const before = pageScroll.contentY
                                const max = Math.max(0, pageScroll.contentHeight - pageScroll.height)
                                pageScroll.contentY = Math.max(0, Math.min(max, before + dy / webHost.webView.zoomFactor))
                                pageFling.carry += pageScroll.contentY - before
                                pageFling.lastStep = Date.now()
                                const step = Math.round(pageFling.carry)
                                if (!step) return
                                pageFling.carry -= step
                                webHost.webView.runJavaScript("window.scrollBy(0, " + step + ")")
                            }
                        }
                    }
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

    FileDialog {
        id: imageDialog
        title: qsTr("Bild einfügen")
        fileMode: FileDialog.OpenFiles
        nameFilters: [qsTr("Bilder (*.png *.jpg *.jpeg *.gif *.webp *.svg *.bmp *.avif)"), qsTr("Alle Dateien (*)")]
        onAccepted: root.editorForInsert().insertAtCursor(
            root.app.importIntoNote(root.source, JSON.stringify(selectedFiles.map(u => u.toString()))))
    }
    FolderDialog {
        id: treeDialog
        title: qsTr("Ordnerbaum einfügen")
        currentFolder: "file://" + (root.app.projectRoot || "/")
        onAccepted: root.insertFolderTree(root.app.urlToPath(selectedFolder.toString()))
    }
    Dialog {
        id: tableDialog
        anchors.centerIn: parent
        modal: true
        title: qsTr("Tabelle einfügen")
        standardButtons: Dialog.Ok | Dialog.Cancel
        onAccepted: root.insertTable(rowsBox.value, colsBox.value)
        GridLayout {
            columns: 2
            Label { text: qsTr("Spalten") }
            SpinBox { id: colsBox; from: 1; to: 20; value: 3; editable: true }
            Label { text: qsTr("Zeilen") }
            SpinBox { id: rowsBox; from: 1; to: 100; value: 3; editable: true }
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
