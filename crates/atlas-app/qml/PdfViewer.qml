// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Shapes
import QtQuick.Window
import Atlas

// PDF viewer: pages, thumbnails, outline, search, zoom and annotations.
// The last page and zoom of every document are remembered
// (`.atlas/metadata.sqlite`). Annotations – highlights, comments, freehand
// strokes, bookmarks – live in `.atlas/annotations/` and never change the
// PDF; an export writes a copy with embedded annotations.
Rectangle {
    id: root
    required property string source
    property var app: null
    /// FileActions (messages, quotes into notes); optional.
    property var actions: null
    /// 0 = fit width, -1 = fit page, otherwise the zoom factor.
    property real zoom: 0
    property int lastRenderMs: 0
    property bool sidebarVisible: false
    /// false: no toolbar (the quick look brings its own controls).
    property bool chrome: true
    readonly property int pageCount: doc.pageCount
    /// Size of page `index` in millimetres ({ w, h }), from PDF points.
    function pageSizeMm(index) {
        if (index < 0 || index >= doc.pageCount) return null
        return { w: Math.round(doc.pageWidth(index) * 25.4 / 72), h: Math.round(doc.pageHeight(index) * 25.4 / 72) }
    }
    /// 0 = thumbnails, 1 = outline, 2 = annotations.
    property int sidebarTab: 0
    /// Notes citing or linking to this PDF, and the local graph.
    property bool showContext: false

    /// Open another file (from backlinks or the graph).
    signal linkActivated(string path, int line, bool newPane, var state)
    readonly property int currentPage: Math.max(0, pages.indexAt(10, pages.contentY + pages.height / 3))

    // Search state.
    property string searchQuery: ""
    property var hits: []
    property var hitsByPage: ({})
    property int currentHit: -1
    property int searchMs: 0

    // Annotation tools: "select" (text), "ink" (freehand), "note" (comment pin).
    property string tool: "select"
    readonly property var markColors: ["#ffd400", "#7bd88f", "#5ac8fa", "#ff7eb6", "#ff6b6b"]
    property string markColor: markColors[0]
    property string inkColor: "#ff6b6b"
    property real inkWidth: 2

    // Annotations of this PDF (parsed from `notes.annotationsJson`).
    property var annots: []
    /// 1-based page → annotations.
    property var annotsByPage: ({})
    /// Annotation briefly outlined after a jump (e.g. from a quote).
    property string focusedAnnot: ""
    /// Annotation to show once document and annotations are loaded.
    property string pendingFocus: ""

    // Text selection: [{page (0-based), rects, text}].
    property var selection: []
    property var selectionByPage: ({})
    readonly property string selectionText: selection.map(s => s.text).join("\n")
    property var dragStart: null
    property var dragEnd: null
    property bool dragging: false
    property int selectStyle: 0
    property int selectRequest: 0
    property bool selectBusy: false
    property bool selectDirty: false
    /// Last pointer position in `pages` coordinates while dragging.
    property point dragViewport: Qt.point(0, 0)

    // Freehand stroke in progress: page (0-based) and points.
    property int strokePage: -1
    property var stroke: []

    color: Theme.pdfBackdrop
    focus: true

    function viewState() {
        return {
            zoom: root.zoom, page: currentPage, sidebar: sidebarVisible, sidebarTab: sidebarTab,
            context: showContext, contextTab: contextPanel.tab, graphDepth: contextPanel.depth,
        }
    }
    function applyState(state) {
        let s = state || {}
        if (s.annot) pendingFocus = s.annot
        // Only a jump to an annotation: keep zoom and position otherwise.
        if (s.annot && Object.keys(s).length === 1) { Qt.callLater(tryFocus); return }
        // Without a page from the session, continue where this document was
        // last read.
        if (s.page === undefined && root.app && root.source) {
            try { s = Object.assign(JSON.parse(root.app.documentState(root.source) || "{}"), s) } catch (e) {}
        }
        if (s.zoom !== undefined) root.zoom = s.zoom
        if (s.sidebar !== undefined) root.sidebarVisible = s.sidebar
        if (s.sidebarTab !== undefined) root.sidebarTab = s.sidebarTab
        if (s.context !== undefined) root.showContext = s.context
        if (s.contextTab !== undefined) contextPanel.tab = s.contextTab
        if (s.graphDepth !== undefined) contextPanel.depth = s.graphDepth
        restorePage = s.page || 0
        if (doc.status === "ready") Qt.callLater(root.restore)
    }
    /// Jump target from links (`datei.pdf#page=3`): 1-based page.
    function reveal(page) { if (page > 0) goToPage(page - 1) }

    property int restorePage: 0
    function restore() {
        if (restorePage > 0) goToPage(restorePage)
        restorePage = 0
        tryFocus()
    }
    function goToPage(index) {
        const i = Math.max(0, Math.min(doc.pageCount - 1, index))
        pages.positionViewAtIndex(i, ListView.Beginning)
    }
    /// Scrolls so that `y` (points) of page `index` is in the upper third.
    function showPoint(index, y) {
        pages.positionViewAtIndex(index, ListView.Beginning)
        pages.contentY = Math.max(pages.originY, pages.contentY + y * zoomScale - pages.height / 3)
    }

    // Remember page and zoom per document (debounced).
    Timer {
        id: rememberTimer
        interval: 800
        onTriggered: if (root.app && doc.status === "ready")
            root.app.saveDocumentState(root.source, JSON.stringify({ page: root.currentPage, zoom: root.zoom }))
    }
    onCurrentPageChanged: rememberTimer.restart()
    onZoomChanged: rememberTimer.restart()
    onSourceChanged: { clearSelection(); pendingFocus = "" }

    PdfDocument {
        id: doc
        source: root.source
        onStatusChanged: if (status === "ready") Qt.callLater(root.restore)
        onTextReady: (page, text, ms) => {
            textPopup.title = qsTr("Text von Seite %1 (%2 ms)").arg(page + 1).arg(ms)
            textPopup.text = text
            textPopup.open()
        }
        onSearchFinished: (query, hitsJson, ms) => {
            if (query !== root.searchQuery) return
            const list = JSON.parse(hitsJson)
            const byPage = {}
            list.forEach((h, i) => { h.index = i; (byPage[h.page] = byPage[h.page] || []).push(h) })
            root.hits = list
            root.hitsByPage = byPage
            root.searchMs = ms
            // Start with the first hit at or after the current page.
            const first = list.findIndex(h => h.page >= root.currentPage)
            root.showHit(list.length ? (first >= 0 ? first : 0) : -1)
        }
        onSelectionReady: (request, json) => {
            root.selectBusy = false
            if (request === root.selectRequest) root.setSelection(JSON.parse(json))
            if (root.selectDirty) root.requestSelection()
        }
    }

    // ------------------------------------------------------------ annotations

    PdfAnnotations {
        id: notes
        source: root.source
        projectRoot: root.app ? root.app.projectRoot : ""
        storeDir: root.app ? root.app.annotationsDir : ""
        onAnnotationsJsonChanged: root.parseAnnotations()
        onStatusChanged: {
            if (status === "moved") root.message(qsTr("Annotationen nach dem Verschieben der PDF wiedergefunden"))
            root.tryFocus()
        }
        onExportFinished: (ok, message) => {
            root.message(message)
            if (!ok) { exportError.text = message; exportError.open() }
        }
    }
    readonly property alias annotationStore: notes

    function message(text) { if (root.actions) root.actions.showMessage(text) }

    function parseAnnotations() {
        let list = []
        try { list = JSON.parse(notes.annotationsJson || "[]") } catch (e) {}
        const byPage = {}
        for (const a of list) (byPage[a.page] = byPage[a.page] || []).push(a)
        annots = list
        annotsByPage = byPage
    }
    function annotation(id) { return annots.find(a => a.id === id) || null }
    /// Sorted for the sidebar: by page, then from top to bottom.
    readonly property var sortedAnnots: annots.slice().sort((a, b) =>
        a.page - b.page || ((a.rects && a.rects.length ? a.rects[0][1] : 0) - (b.rects && b.rects.length ? b.rects[0][1] : 0)))

    function annotationLabel(a) {
        if (a.kind === "bookmark") return a.title || qsTr("Lesezeichen")
        if (a.text) return a.text
        if (a.comment) return a.comment
        return a.kind === "ink" ? qsTr("Freihandmarkierung") : qsTr("Notiz")
    }
    function kindGlyph(kind) {
        return ({ highlight: "▌", underline: "U̲", strikeout: "S̶", note: "🗨", ink: "✎", bookmark: "🔖" })[kind] || "•"
    }

    /// Scrolls to an annotation and outlines it briefly.
    function focusAnnotation(id) {
        const a = annotation(id)
        if (!a) return false
        const y = a.rects && a.rects.length ? a.rects[0][1] : a.ink && a.ink.length && a.ink[0].length ? a.ink[0][0][1] : 0
        showPoint(a.page - 1, y)
        focusedAnnot = id
        flashTimer.restart()
        return true
    }
    Timer { id: flashTimer; interval: 1800; onTriggered: root.focusedAnnot = "" }

    /// Shows the pending annotation once everything is loaded; the page of
    /// the link already serves as fallback.
    function tryFocus() {
        if (!pendingFocus || doc.status !== "ready" || notes.status === "loading" || notes.status === "") return
        const id = pendingFocus
        pendingFocus = ""
        if (!focusAnnotation(id)) message(qsTr("Die Annotation ist nicht mehr vorhanden – Seite des Links geöffnet"))
    }

    function newGroup() { return Date.now().toString(36) + Math.floor(Math.random() * 1e6).toString(36) }

    /// Turns the current selection into text markup; returns the first ID.
    function markSelection(kind, color) {
        if (!selection.length) return ""
        const group = selection.length > 1 ? newGroup() : ""
        let first = ""
        for (const part of selection) {
            const id = notes.add(JSON.stringify({
                kind: kind, page: part.page + 1, rects: part.rects, text: part.text, color: color || markColor, group: group
            }))
            if (!first) first = id
        }
        if (kind === "highlight" && color) markColor = color
        clearSelection()
        return first
    }

    /// Asks for a note and inserts the quote of annotation `id` there.
    function quote(id) {
        const a = annotation(id)
        if (!a || !root.actions) return
        const dir = root.source.slice(0, root.source.lastIndexOf("/"))
        root.actions.sendBlockToNote(qsTr("Zitat von Seite %1 übernehmen in:").arg(a.page),
                                     (note) => notes.quoteMarkdown(id, note), dir,
                                     qsTr("Zitat in %1 übernommen"), qsTr("Zitate"))
    }
    function quoteSelection() {
        const id = markSelection("highlight", markColor)
        if (id) quote(id)
    }
    function copySelection() {
        if (!selectionText) return
        if (root.app) root.app.copyToClipboard(selectionText)
        message(qsTr("Text kopiert"))
    }

    function toggleBookmark() {
        const page = currentPage + 1
        const existing = (annotsByPage[page] || []).find(a => a.kind === "bookmark")
        if (existing) {
            notes.remove(existing.id)
            message(qsTr("Lesezeichen auf Seite %1 entfernt").arg(page))
        } else {
            notes.add(JSON.stringify({ kind: "bookmark", page: page, title: qsTr("Seite %1").arg(page), color: "#ff6b6b" }))
            message(qsTr("Lesezeichen auf Seite %1").arg(page))
        }
    }
    readonly property bool pageBookmarked: (annotsByPage[currentPage + 1] || []).some(a => a.kind === "bookmark")

    function exportCopy() {
        exportDialog.selectedFile = "file://" + notes.defaultExportPath()
        exportDialog.open()
    }

    // ------------------------------------------------------------- selection

    function clearSelection() {
        selection = []
        selectionByPage = ({})
        dragStart = null
        dragEnd = null
        selectionMenu.close()
    }
    function setSelection(list) {
        const byPage = {}
        for (const s of list) byPage[s.page] = s.rects
        selection = list
        selectionByPage = byPage
    }
    function requestSelection() {
        if (!dragStart || !dragEnd) return
        if (selectBusy) { selectDirty = true; return }
        selectBusy = true
        selectDirty = false
        busyGuard.restart()
        selectRequest = doc.select(dragStart.page, dragStart.x, dragStart.y, dragEnd.page, dragEnd.x, dragEnd.y, selectStyle)
    }
    // Never stay blocked if a result got lost.
    Timer { id: busyGuard; interval: 2000; onTriggered: root.selectBusy = false }

    /// Page and position (points) under a point of the `pages` view.
    function pointAt(vx, vy) {
        const cy = vy + pages.contentY
        const cx = vx + pages.contentX
        let index = pages.indexAt(pages.contentWidth / 2, cy)
        if (index < 0) index = pages.indexAt(pages.contentWidth / 2, cy - pages.spacing)
        if (index < 0) return null
        const item = pages.itemAtIndex(index)
        if (!item) return null
        const local = item.paperItem.mapFromItem(pages.contentItem, cx, cy)
        const w = doc.pageWidth(index), h = doc.pageHeight(index)
        return {
            page: index,
            x: Math.max(0, Math.min(w, local.x / zoomScale)),
            y: Math.max(0, Math.min(h, local.y / zoomScale)),
        }
    }

    /// Annotation under a point of a page (topmost first).
    function annotationAt(page, x, y) {
        const list = (annotsByPage[page + 1] || []).slice().reverse()
        const inside = (r, m) => x >= r[0] - m && x <= r[0] + r[2] + m && y >= r[1] - m && y <= r[1] + r[3] + m
        for (const a of list) {
            if (a.kind === "ink") {
                for (const s of a.ink) for (const p of s)
                    if (Math.abs(p[0] - x) < 6 && Math.abs(p[1] - y) < 6) return a
            } else if (a.rects) {
                for (const r of a.rects) if (inside(r, a.kind === "note" ? 2 : 0)) return a
            }
        }
        return null
    }

    // Scrolls while a selection or stroke is dragged past the edge.
    Timer {
        id: autoScroll
        interval: 30
        repeat: true
        running: root.dragging
        onTriggered: {
            const vy = root.dragViewport.y
            const step = vy < 24 ? -Math.max(4, 24 - vy) : vy > pages.height - 24 ? Math.max(4, vy - pages.height + 24) : 0
            if (!step) return
            pages.contentY = Math.max(pages.originY, Math.min(pages.contentY + step, pages.contentHeight + pages.originY - pages.height))
            if (root.tool === "select") {
                const p = root.pointAt(root.dragViewport.x, root.dragViewport.y)
                if (p) { root.dragEnd = p; root.requestSelection() }
            }
        }
    }

    // ---------------------------------------------------------------- search

    function search(text) {
        searchQuery = text
        if (!text) { hits = []; hitsByPage = ({}); currentHit = -1; return }
        doc.search(text)
    }
    function showHit(i) {
        currentHit = i
        if (i < 0 || i >= hits.length) return
        showPoint(hits[i].page, hits[i].y)
    }
    function nextHit(step) {
        if (hits.length) showHit((currentHit + step + hits.length) % hits.length)
    }

    readonly property real fitScale: doc.maxPageWidth > 0 ? Math.max(0.1, (pages.width - 32) / doc.maxPageWidth) : 1
    readonly property real fitPageScale: doc.pageCount > 0
        ? Math.max(0.1, Math.min(fitScale, (pages.height - 24) / doc.pageHeight(currentPage))) : 1
    readonly property real zoomScale: zoom > 0 ? zoom : zoom < 0 ? fitPageScale : fitScale
    // Rendering follows the visible scale with a short delay so resizing a
    // split does not re-render every page on every frame.
    property real renderScale: zoomScale
    onZoomScaleChanged: renderTimer.restart()
    Timer { id: renderTimer; interval: 120; onTriggered: root.renderScale = root.zoomScale }

    /// Wheel notches of a wheel event, fractional for touchpads and
    /// high-resolution wheels (120 per notch; 50 px of pixel-only deltas).
    function wheelSteps(event) {
        const d = event.angleDelta.y || event.angleDelta.x
        if (d) return d / 120
        const p = event.pixelDelta.y || event.pixelDelta.x
        return p / 50
    }
    function zoomBy(factor) { zoom = Math.max(0.2, Math.min(6, zoomScale * factor)) }

    Keys.onPressed: (event) => {
        const ctrl = event.modifiers & Qt.ControlModifier
        if (event.key === Qt.Key_PageDown) goToPage(currentPage + 1)
        else if (event.key === Qt.Key_PageUp) goToPage(currentPage - 1)
        else if (event.key === Qt.Key_Home && ctrl) goToPage(0)
        else if (event.key === Qt.Key_End && ctrl) goToPage(doc.pageCount - 1)
        else if (ctrl && event.key === Qt.Key_F) { searchField.forceActiveFocus(); searchField.selectAll() }
        else if (event.key === Qt.Key_F3) nextHit(event.modifiers & Qt.ShiftModifier ? -1 : 1)
        else if (ctrl && (event.key === Qt.Key_Plus || event.key === Qt.Key_Equal)) zoomBy(1.2)
        else if (ctrl && event.key === Qt.Key_Minus) zoomBy(1 / 1.2)
        else if (ctrl && event.key === Qt.Key_0) zoom = 0
        else if (ctrl && event.key === Qt.Key_C && selection.length) copySelection()
        else if (ctrl && event.key === Qt.Key_D) toggleBookmark()
        else if (ctrl && event.key === Qt.Key_H && selection.length) markSelection("highlight", markColor)
        else if (ctrl && event.key === Qt.Key_Q && selection.length) quoteSelection()
        else if (event.key === Qt.Key_Escape && (selection.length || tool !== "select")) { clearSelection(); tool = "select" }
        else return
        event.accepted = true
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 32
            visible: root.chrome
            color: Theme.header
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 6
                anchors.rightMargin: 6
                spacing: 3
                IconButton {
                    iconName: "list"
                    tip: qsTr("Seitenleiste: Miniaturen, Inhalt und Annotationen")
                    checkable: true
                    checked: root.sidebarVisible
                    onClicked: root.sidebarVisible = !root.sidebarVisible
                }
                TextField {
                    id: pageField
                    Layout.preferredWidth: 44
                    implicitHeight: 24
                    horizontalAlignment: Text.AlignHCenter
                    font.pixelSize: Theme.smallFontSize
                    text: doc.pageCount > 0 ? String(root.currentPage + 1) : ""
                    validator: IntValidator { bottom: 1; top: Math.max(1, doc.pageCount) }
                    onAccepted: { root.goToPage(parseInt(text) - 1); root.forceActiveFocus() }
                }
                Label {
                    text: "/ " + doc.pageCount
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                }
                IconButton { iconName: "minus"; tip: qsTr("Verkleinern (Strg+−)"); onClicked: root.zoomBy(1 / 1.2) }
                Label {
                    text: Math.round(root.zoomScale * 100) + " %"
                    color: Theme.text
                    font.pixelSize: Theme.smallFontSize
                    Layout.preferredWidth: 40
                    horizontalAlignment: Text.AlignHCenter
                }
                IconButton { iconName: "plus"; tip: qsTr("Vergrößern (Strg++)"); onClicked: root.zoomBy(1.2) }
                IconButton { glyph: "↔"; tip: qsTr("Seitenbreite (Strg+0)"); checkable: true; checked: root.zoom === 0; onClicked: root.zoom = 0 }
                IconButton { glyph: "▯"; tip: qsTr("Ganze Seite"); checkable: true; checked: root.zoom < 0; onClicked: root.zoom = -1 }
                Rectangle { Layout.preferredWidth: 1; Layout.preferredHeight: 18; color: Theme.border }
                IconButton {
                    glyph: "I"
                    tip: qsTr("Text auswählen und markieren")
                    checkable: true
                    checked: root.tool === "select"
                    onClicked: root.tool = "select"
                }
                IconButton {
                    iconName: "pen"
                    tip: qsTr("Freihand zeichnen")
                    checkable: true
                    checked: root.tool === "ink"
                    onClicked: { root.tool = root.tool === "ink" ? "select" : "ink"; root.clearSelection() }
                }
                Rectangle {
                    visible: root.tool === "ink"
                    Layout.preferredWidth: 16
                    Layout.preferredHeight: 16
                    radius: 8
                    color: root.inkColor
                    border.color: Theme.border
                    MouseArea { anchors.fill: parent; onClicked: inkMenu.popup() }
                    Menu {
                        id: inkMenu
                        Repeater {
                            model: root.markColors
                            MenuItem {
                                required property string modelData
                                text: "●"
                                onTriggered: root.inkColor = modelData
                                contentItem: Text { text: "●  " + parent.modelData; color: parent.modelData }
                            }
                        }
                        MenuSeparator {}
                        MenuItem { text: qsTr("Dünn"); onTriggered: root.inkWidth = 1 }
                        MenuItem { text: qsTr("Mittel"); onTriggered: root.inkWidth = 2 }
                        MenuItem { text: qsTr("Breit"); onTriggered: root.inkWidth = 4 }
                    }
                }
                IconButton {
                    glyph: "🗨"
                    tip: qsTr("Notiz auf der Seite anheften")
                    checkable: true
                    checked: root.tool === "note"
                    onClicked: { root.tool = root.tool === "note" ? "select" : "note"; root.clearSelection() }
                }
                IconButton {
                    iconName: "bookmark"
                    tip: root.pageBookmarked ? qsTr("Lesezeichen entfernen (Strg+D)") : qsTr("Lesezeichen für diese Seite (Strg+D)")
                    checkable: true
                    checked: root.pageBookmarked
                    enabled: notes.status !== "loading" && notes.status !== "error"
                    onClicked: root.toggleBookmark()
                }
                Item { Layout.fillWidth: true }
                TextField {
                    id: searchField
                    Layout.preferredWidth: Math.min(220, root.width * 0.3)
                    implicitHeight: 24
                    font.pixelSize: Theme.smallFontSize
                    placeholderText: qsTr("Im Dokument suchen (Strg+F)")
                    onTextEdited: searchDebounce.restart()
                    onAccepted: { searchDebounce.stop(); if (text === root.searchQuery) root.nextHit(1); else root.search(text) }
                    Keys.onEscapePressed: { text = ""; root.search(""); root.forceActiveFocus() }
                    Timer { id: searchDebounce; interval: 300; onTriggered: root.search(searchField.text) }
                }
                Label {
                    visible: root.searchQuery.length > 0
                    text: root.hits.length ? qsTr("%1/%2").arg(root.currentHit + 1).arg(root.hits.length) : qsTr("0 Treffer")
                    color: root.hits.length ? Theme.textMuted : Theme.warn
                    font.pixelSize: Theme.smallFontSize
                }
                IconButton { glyph: "↑"; tip: qsTr("Vorheriger Treffer (Umschalt+F3)"); enabled: root.hits.length > 0; onClicked: root.nextHit(-1) }
                IconButton { glyph: "↓"; tip: qsTr("Nächster Treffer (F3)"); enabled: root.hits.length > 0; onClicked: root.nextHit(1) }
                IconButton {
                    glyph: "T"
                    tip: qsTr("Text der aktuellen Seite")
                    enabled: doc.status === "ready"
                    onClicked: doc.extractText(root.currentPage)
                }
                IconButton { iconName: "refresh"; tip: qsTr("Neu laden"); onClicked: { doc.reload(); notes.reload() } }
                Rectangle { Layout.preferredWidth: 1; Layout.preferredHeight: 18; color: Theme.border }
                IconButton {
                    iconName: "backlinks"; glyph: String(contextPanel.count)
                    tip: qsTr("Notizen, die auf diese PDF verweisen oder daraus zitieren")
                    checkable: true
                    checked: root.showContext && contextPanel.tab === 0
                    onClicked: { root.showContext = !(root.showContext && contextPanel.tab === 0); contextPanel.tab = 0 }
                }
                IconButton {
                    iconName: "graph"
                    tip: qsTr("Lokaler Graph: verknüpfte Notizen und Dateien")
                    checkable: true
                    checked: root.showContext && contextPanel.tab === 1
                    onClicked: { root.showContext = !(root.showContext && contextPanel.tab === 1); contextPanel.tab = 1 }
                }
            }
        }

        // The PDF changed since the annotations were made.
        Banner {
            Layout.fillWidth: true
            visible: notes.status === "changed"
            message: qsTr("Diese PDF wurde seit dem Anlegen der Annotationen verändert – Markierungen können verschoben sein.")
            actions: [ { text: qsTr("Verstanden"), run: () => notes.acknowledgeChange() } ]
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            // Thumbnails, outline and annotations.
            Rectangle {
                visible: root.sidebarVisible
                Layout.preferredWidth: 220
                Layout.fillHeight: true
                color: Theme.sidebar
                ColumnLayout {
                    anchors.fill: parent
                    spacing: 0
                    TabBar {
                        id: sideTabs
                        Layout.fillWidth: true
                        currentIndex: root.sidebarTab
                        onCurrentIndexChanged: root.sidebarTab = currentIndex
                        TabButton { text: qsTr("Seiten"); font.pixelSize: Theme.smallFontSize }
                        TabButton { text: qsTr("Inhalt"); font.pixelSize: Theme.smallFontSize }
                        TabButton { text: qsTr("Anmerkungen") + (notes.count ? " " + notes.count : ""); font.pixelSize: Theme.smallFontSize }
                    }
                    StackLayout {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        currentIndex: sideTabs.currentIndex
                        ListView {
                            id: thumbs
                            clip: true
                            model: doc.status === "ready" ? doc.pageCount : 0
                            spacing: 8
                            topMargin: 8
                            cacheBuffer: 400
                            ScrollBar.vertical: ScrollBar {}
                            delegate: Item {
                                id: thumb
                                required property int index
                                width: ListView.view.width
                                height: 110 * doc.pageHeight(index) / doc.pageWidth(index) + 22
                                Rectangle {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    width: 110
                                    height: parent.height - 22
                                    color: "white"
                                    border.width: thumb.index === root.currentPage ? 2 : 0
                                    border.color: Theme.accent
                                    PdfPage {
                                        anchors.fill: parent
                                        anchors.margins: parent.border.width
                                        source: doc.source
                                        page: thumb.index
                                        renderWidth: Math.round(110 * Screen.devicePixelRatio)
                                    }
                                    Text {
                                        visible: (root.annotsByPage[thumb.index + 1] || []).some(a => a.kind === "bookmark")
                                        anchors.right: parent.right
                                        anchors.top: parent.top
                                        text: "🔖"
                                        font.pixelSize: 12
                                    }
                                    MouseArea { anchors.fill: parent; onClicked: root.goToPage(thumb.index) }
                                }
                                Text {
                                    anchors.bottom: parent.bottom
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    text: thumb.index + 1
                                    color: thumb.index === root.currentPage ? Theme.textStrong : Theme.textMuted
                                    font.pixelSize: Theme.smallFontSize
                                }
                            }
                        }
                        ListView {
                            id: outlineList
                            clip: true
                            model: { try { return JSON.parse(doc.outlineJson || "[]") } catch (e) { return [] } }
                            ScrollBar.vertical: ScrollBar {}
                            delegate: Rectangle {
                                id: entry
                                required property var modelData
                                width: ListView.view.width
                                height: Theme.rowHeight
                                color: entryHover.hovered ? Theme.hover : "transparent"
                                HoverHandler { id: entryHover }
                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    x: 8 + entry.modelData.depth * 12
                                    width: parent.width - x - 30
                                    text: entry.modelData.title
                                    color: Theme.text
                                    font.pixelSize: Theme.smallFontSize
                                    elide: Text.ElideRight
                                }
                                Text {
                                    anchors.right: parent.right
                                    anchors.rightMargin: 6
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: entry.modelData.page >= 0 ? entry.modelData.page + 1 : ""
                                    color: Theme.textFaint
                                    font.pixelSize: Theme.smallFontSize
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    onClicked: if (entry.modelData.page >= 0) root.goToPage(entry.modelData.page)
                                }
                            }
                            Label {
                                anchors.centerIn: parent
                                visible: outlineList.count === 0
                                text: qsTr("Kein Inhaltsverzeichnis")
                                color: Theme.textFaint
                                font.pixelSize: Theme.smallFontSize
                            }
                        }
                        ColumnLayout {
                            spacing: 0
                            ListView {
                                id: annotList
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                clip: true
                                model: root.sortedAnnots
                                ScrollBar.vertical: ScrollBar {}
                                delegate: Rectangle {
                                    id: annotRow
                                    required property var modelData
                                    width: ListView.view.width
                                    height: annotColumn.implicitHeight + 10
                                    color: annotHover.hovered ? Theme.hover : root.focusedAnnot === modelData.id ? Theme.selection : "transparent"
                                    HoverHandler { id: annotHover }
                                    Rectangle {
                                        x: 0; width: 3; height: parent.height
                                        color: annotRow.modelData.color || Theme.accent
                                    }
                                    ColumnLayout {
                                        id: annotColumn
                                        x: 10
                                        y: 5
                                        width: parent.width - 34
                                        spacing: 2
                                        RowLayout {
                                            spacing: 4
                                            Text { text: root.kindGlyph(annotRow.modelData.kind); color: Theme.textMuted; font.pixelSize: Theme.smallFontSize }
                                            Text { text: qsTr("S. %1").arg(annotRow.modelData.page); color: Theme.textFaint; font.pixelSize: Theme.smallFontSize }
                                        }
                                        Text {
                                            Layout.fillWidth: true
                                            text: root.annotationLabel(annotRow.modelData)
                                            color: Theme.text
                                            font.pixelSize: Theme.smallFontSize
                                            wrapMode: Text.Wrap
                                            maximumLineCount: 3
                                            elide: Text.ElideRight
                                        }
                                        Text {
                                            Layout.fillWidth: true
                                            visible: !!annotRow.modelData.comment && !!annotRow.modelData.text
                                            text: "💬 " + (annotRow.modelData.comment || "")
                                            color: Theme.textMuted
                                            font.pixelSize: Theme.smallFontSize
                                            wrapMode: Text.Wrap
                                            maximumLineCount: 2
                                            elide: Text.ElideRight
                                        }
                                    }
                                    MouseArea {
                                        anchors.fill: parent
                                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                                        onClicked: (mouse) => {
                                            root.focusAnnotation(annotRow.modelData.id)
                                            if (mouse.button === Qt.RightButton) annotPopup.show(annotRow.modelData.id, null)
                                        }
                                        onDoubleClicked: annotPopup.show(annotRow.modelData.id, null)
                                    }
                                    IconButton {
                                        anchors.right: parent.right
                                        anchors.rightMargin: 4
                                        anchors.top: parent.top
                                        anchors.topMargin: 4
                                        visible: annotHover.hovered
                                        iconName: "close"
                                        tip: qsTr("Löschen")
                                        onClicked: notes.remove(annotRow.modelData.id)
                                    }
                                }
                                Label {
                                    anchors.centerIn: parent
                                    width: parent.width - 24
                                    visible: annotList.count === 0
                                    wrapMode: Text.Wrap
                                    horizontalAlignment: Text.AlignHCenter
                                    color: Theme.textFaint
                                    font.pixelSize: Theme.smallFontSize
                                    text: notes.status === "error" ? notes.error
                                        : qsTr("Noch keine Anmerkungen.\nText markieren, ✎ zeichnen, 🗨 anheften oder 🔖 setzen.")
                                }
                            }
                            Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }
                            Button {
                                Layout.fillWidth: true
                                Layout.margins: 6
                                flat: true
                                font.pixelSize: Theme.smallFontSize
                                text: qsTr("Als PDF-Kopie exportieren …")
                                enabled: notes.count > 0
                                onClicked: root.exportCopy()
                                ToolTip.visible: hovered
                                ToolTip.delay: 600
                                ToolTip.text: qsTr("Schreibt eine Kopie mit eingebetteten Annotationen; die Original-PDF bleibt unverändert.")
                            }
                        }
                    }
                }
            }

            ListView {
                id: pages
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: doc.status === "ready" ? doc.pageCount : 0
                spacing: 12
                topMargin: 12
                bottomMargin: 12
                cacheBuffer: Math.max(0, height)
                boundsBehavior: Flickable.StopAtBounds
                flickableDirection: Flickable.AutoFlickIfNeeded
                contentWidth: Math.max(width, doc.maxPageWidth * root.zoomScale + 32)
                ScrollBar.vertical: ScrollBar {}
                ScrollBar.horizontal: ScrollBar {}

                // Ctrl+wheel zooms (on the list itself: it would take every
                // wheel event for scrolling); the spot under the mouse stays.
                // Touchpads and high-resolution wheels (Wayland) send small or
                // pixel-only deltas; Ctrl+wheel never scrolls. Qt on Wayland
                // reports even mouse wheels as a "touchpad" with finger pointer
                // type: every device and pointer type counts.
                WheelHandler {
                    acceptedDevices: PointerDevice.AllDevices
                    acceptedPointerTypes: PointerDevice.AllPointerTypes
                    acceptedModifiers: Qt.ControlModifier
                    onWheel: (event) => {
                        event.accepted = true
                        const steps = root.wheelSteps(event)
                        if (!steps) return
                        const at = Qt.point(pages.contentX + event.x - pages.originX, pages.contentY + event.y - pages.originY)
                        const before = root.zoomScale
                        root.zoomBy(Math.pow(1.1, steps))
                        const k = root.zoomScale / before
                        Qt.callLater(() => {
                            pages.contentX = Math.max(0, at.x * k - event.x) + pages.originX
                            pages.contentY = Math.max(0, at.y * k - event.y) + pages.originY
                            pages.returnToBounds()
                        })
                        event.accepted = true
                    }
                }

                delegate: Item {
                    id: pageDelegate
                    required property int index
                    readonly property alias paperItem: paper
                    width: pages.contentWidth
                    height: doc.pageHeight(index) * root.zoomScale

                    Rectangle {
                        id: paper
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: doc.pageWidth(pageDelegate.index) * root.zoomScale
                        height: parent.height
                        color: "white"

                        PdfPage {
                            id: page
                            anchors.fill: parent
                            source: doc.source
                            page: pageDelegate.index
                            renderWidth: Math.round(doc.pageWidth(pageDelegate.index) * root.renderScale * Screen.devicePixelRatio)
                            onRenderMsChanged: root.lastRenderMs = renderMs
                        }

                        // Annotations of this page.
                        Repeater {
                            model: root.annotsByPage[pageDelegate.index + 1] || []
                            delegate: Item {
                                id: ann
                                required property var modelData
                                readonly property bool focused: root.focusedAnnot === modelData.id
                                readonly property real s: root.zoomScale
                                anchors.fill: parent

                                // Text markup: one bar per line.
                                Repeater {
                                    model: ["highlight", "underline", "strikeout"].indexOf(ann.modelData.kind) >= 0 ? ann.modelData.rects : []
                                    delegate: Rectangle {
                                        required property var modelData
                                        x: modelData[0] * ann.s
                                        y: modelData[1] * ann.s
                                        width: modelData[2] * ann.s
                                        height: modelData[3] * ann.s
                                        color: ann.modelData.kind === "highlight" ? Qt.alpha(ann.modelData.color, 0.38) : "transparent"
                                        border.width: ann.focused ? 2 : 0
                                        border.color: "#ff9632"
                                        Rectangle {
                                            visible: ann.modelData.kind !== "highlight"
                                            width: parent.width
                                            height: Math.max(1.5, 1.2 * ann.s)
                                            y: ann.modelData.kind === "underline" ? parent.height - height : (parent.height - height) / 2
                                            color: ann.modelData.color
                                        }
                                    }
                                }
                                // Markers for comments on text markup.
                                Text {
                                    visible: !!ann.modelData.comment && !!ann.modelData.rects && ann.modelData.rects.length > 0 && ann.modelData.kind !== "note"
                                    x: visible ? (ann.modelData.rects[0][0] + ann.modelData.rects[0][2]) * ann.s : 0
                                    y: visible ? ann.modelData.rects[0][1] * ann.s - height / 2 : 0
                                    text: "💬"
                                    font.pixelSize: Math.max(9, 9 * ann.s)
                                }
                                // Comment pinned to the page.
                                Rectangle {
                                    visible: ann.modelData.kind === "note"
                                    x: visible ? ann.modelData.rects[0][0] * ann.s : 0
                                    y: visible ? ann.modelData.rects[0][1] * ann.s : 0
                                    width: Math.max(16, 18 * ann.s)
                                    height: width
                                    radius: 3
                                    color: ann.modelData.color
                                    border.width: ann.focused ? 2 : 1
                                    border.color: ann.focused ? "#ff9632" : "#80000000"
                                    Text { anchors.centerIn: parent; text: "🗨"; font.pixelSize: parent.height * 0.6 }
                                    ToolTip.visible: noteHover.hovered && !!ann.modelData.comment
                                    ToolTip.text: ann.modelData.comment || ""
                                    HoverHandler { id: noteHover }
                                }
                                // Freehand strokes.
                                Shape {
                                    visible: ann.modelData.kind === "ink"
                                    anchors.fill: parent
                                    ShapePath {
                                        strokeColor: ann.focused ? "#ff9632" : ann.modelData.color
                                        strokeWidth: (ann.modelData.width || 2) * ann.s
                                        fillColor: "transparent"
                                        capStyle: ShapePath.RoundCap
                                        joinStyle: ShapePath.RoundJoin
                                        PathMultiline {
                                            paths: ann.modelData.kind === "ink"
                                                ? ann.modelData.ink.map(st => st.map(p => Qt.point(p[0] * ann.s, p[1] * ann.s))) : []
                                        }
                                    }
                                }
                                // Bookmark ribbon.
                                Text {
                                    visible: ann.modelData.kind === "bookmark"
                                    anchors.right: parent.right
                                    anchors.rightMargin: 8
                                    y: -4
                                    text: "🔖"
                                    font.pixelSize: Math.max(14, 16 * ann.s)
                                }
                            }
                        }

                        // Current text selection.
                        Repeater {
                            model: root.selectionByPage[pageDelegate.index] || []
                            delegate: Rectangle {
                                required property var modelData
                                x: modelData[0] * root.zoomScale
                                y: modelData[1] * root.zoomScale
                                width: modelData[2] * root.zoomScale
                                height: modelData[3] * root.zoomScale
                                color: "#5a3399ff"
                            }
                        }
                        // Freehand stroke being drawn.
                        Shape {
                            visible: root.strokePage === pageDelegate.index && root.stroke.length > 1
                            anchors.fill: parent
                            ShapePath {
                                strokeColor: root.inkColor
                                strokeWidth: root.inkWidth * root.zoomScale
                                fillColor: "transparent"
                                capStyle: ShapePath.RoundCap
                                joinStyle: ShapePath.RoundJoin
                                PathPolyline {
                                    path: root.strokePage === pageDelegate.index
                                        ? root.stroke.map(p => Qt.point(p[0] * root.zoomScale, p[1] * root.zoomScale)) : []
                                }
                            }
                        }
                        // Search hits on this page.
                        Repeater {
                            model: root.hitsByPage[pageDelegate.index] || []
                            delegate: Rectangle {
                                required property var modelData
                                x: modelData.x * root.zoomScale - 1
                                y: modelData.y * root.zoomScale - 1
                                width: modelData.w * root.zoomScale + 2
                                height: modelData.h * root.zoomScale + 2
                                color: modelData.index === root.currentHit ? "#80ff9632" : "#50ffd200"
                                border.color: modelData.index === root.currentHit ? "#ff9632" : "transparent"
                            }
                        }
                        Text {
                            anchors.centerIn: parent
                            visible: !page.rendered
                            text: page.error ? page.error : qsTr("Seite %1").arg(pageDelegate.index + 1)
                            color: page.error ? Theme.error : "#999999"
                            width: parent.width - 20
                            wrapMode: Text.Wrap
                            horizontalAlignment: Text.AlignHCenter
                        }

                        // Selecting, drawing and pinning notes.
                        MouseArea {
                            id: pageMouse
                            anchors.fill: parent
                            preventStealing: true
                            acceptedButtons: Qt.LeftButton | Qt.RightButton
                            hoverEnabled: true
                            cursorShape: root.tool === "ink" || root.tool === "note" ? Qt.CrossCursor
                                : hoveredAnnotation ? Qt.PointingHandCursor : Qt.IBeamCursor
                            property var pressPoint: null
                            property bool moved: false
                            property var hoveredAnnotation: null
                            function pointOf(mouse) {
                                return { page: pageDelegate.index, x: mouse.x / root.zoomScale, y: mouse.y / root.zoomScale }
                            }
                            onPositionChanged: (mouse) => {
                                const vp = mapToItem(pages, mouse.x, mouse.y)
                                if (!pressed) {
                                    if (root.tool === "select") hoveredAnnotation = root.annotationAt(pageDelegate.index, mouse.x / root.zoomScale, mouse.y / root.zoomScale)
                                    return
                                }
                                root.dragViewport = vp
                                if (!pressPoint) return
                                if (Math.abs(mouse.x - pressPoint.vx) + Math.abs(mouse.y - pressPoint.vy) > 3) moved = true
                                if (root.tool === "select" && moved) {
                                    root.dragging = true
                                    const p = root.pointAt(vp.x, vp.y)
                                    if (p) { root.dragEnd = p; root.requestSelection() }
                                } else if (root.tool === "ink") {
                                    root.dragging = true
                                    const p = root.pointAt(vp.x, vp.y)
                                    if (!p || p.page !== root.strokePage) return
                                    const last = root.stroke[root.stroke.length - 1]
                                    if (!last || Math.abs(last[0] - p.x) + Math.abs(last[1] - p.y) > 0.8)
                                        root.stroke = root.stroke.concat([[p.x, p.y]])
                                }
                            }
                            onPressed: (mouse) => {
                                root.forceActiveFocus()
                                const p = pointOf(mouse)
                                pressPoint = { vx: mouse.x, vy: mouse.y, page: p.page, x: p.x, y: p.y }
                                moved = false
                                if (mouse.button === Qt.RightButton) return
                                if (root.tool === "select") {
                                    selectionMenu.close()
                                    if (!(mouse.modifiers & Qt.ShiftModifier) || !root.dragStart) {
                                        root.selectStyle = 0
                                        root.dragStart = p
                                    }
                                } else if (root.tool === "ink") {
                                    root.strokePage = p.page
                                    root.stroke = [[p.x, p.y]]
                                }
                            }
                            onReleased: (mouse) => {
                                root.dragging = false
                                const p = pointOf(mouse)
                                if (mouse.button === Qt.RightButton) {
                                    const hit = root.annotationAt(p.page, p.x, p.y)
                                    if (hit) annotPopup.show(hit.id, mapToItem(root, mouse.x, mouse.y))
                                    else if (root.selection.length) selectionMenu.showAt(mapToItem(root, mouse.x, mouse.y))
                                    return
                                }
                                if (root.tool === "ink") {
                                    if (root.stroke.length > 1)
                                        notes.add(JSON.stringify({ kind: "ink", page: root.strokePage + 1, ink: [root.stroke], color: root.inkColor, width: root.inkWidth }))
                                    root.stroke = []
                                    root.strokePage = -1
                                    return
                                }
                                if (root.tool === "note") {
                                    const id = notes.add(JSON.stringify({ kind: "note", page: p.page + 1, rects: [[p.x - 9, p.y - 9, 18, 18]], color: "#ffd400" }))
                                    if (id) annotPopup.show(id, mapToItem(root, mouse.x, mouse.y), true)
                                    root.tool = "select"
                                    return
                                }
                                if (!moved && !(mouse.modifiers & Qt.ShiftModifier)) {
                                    // A click: open an annotation or drop the selection.
                                    const hit = root.annotationAt(p.page, p.x, p.y)
                                    root.clearSelection()
                                    if (hit) annotPopup.show(hit.id, mapToItem(root, mouse.x, mouse.y))
                                    return
                                }
                                if (mouse.modifiers & Qt.ShiftModifier) { root.dragEnd = root.pointAt(mapToItem(pages, mouse.x, mouse.y).x, mapToItem(pages, mouse.x, mouse.y).y) || p; root.requestSelection() }
                                selectionMenu.showAt(mapToItem(root, mouse.x, mouse.y))
                            }
                            onDoubleClicked: (mouse) => {
                                if (root.tool !== "select") return
                                const p = pointOf(mouse)
                                root.selectStyle = 1
                                root.dragStart = p
                                root.dragEnd = p
                                root.requestSelection()
                                selectionMenu.showAt(mapToItem(root, mouse.x, mouse.y))
                            }
                        }
                    }
                }

            }

            // Notes citing this PDF and the local graph.
            ContextPanel {
                id: contextPanel
                Layout.preferredWidth: 280
                Layout.fillHeight: true
                visible: root.showContext
                app: root.app
                path: root.source
                onOpenRequested: (path, line, newPane, state) => root.linkActivated(path, line, newPane, state)
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 20
            visible: root.chrome || doc.status === "error"
            color: Theme.header
            Label {
                anchors.fill: parent
                anchors.leftMargin: 8
                verticalAlignment: Text.AlignVCenter
                color: doc.status === "error" ? Theme.error : Theme.textFaint
                font.pixelSize: Theme.smallFontSize
                elide: Text.ElideRight
                text: doc.status === "error" ? doc.error
                    : doc.status === "loading" ? qsTr("Öffnen …")
                    : qsTr("Öffnen %1 ms · Rendern %2 ms%3%4").arg(doc.openMs).arg(root.lastRenderMs)
                        .arg(root.searchQuery ? qsTr(" · Suche %1 ms").arg(root.searchMs) : "")
                        .arg(notes.status === "error" ? qsTr(" · Anmerkungen: %1").arg(notes.error)
                            : notes.count ? qsTr(" · %n Anmerkung(en)", "", notes.count) : "")
            }
        }
    }

    Label {
        anchors.centerIn: parent
        visible: doc.status === "error"
        color: Theme.error
        text: doc.error
        width: parent.width - 40
        wrapMode: Text.Wrap
        horizontalAlignment: Text.AlignHCenter
    }

    // ---- actions for selected text
    Popup {
        id: selectionMenu
        padding: 4
        closePolicy: Popup.CloseOnEscape
        background: Rectangle { color: Theme.header; border.color: Theme.border; radius: 6 }
        function showAt(pos) {
            x = Math.max(4, Math.min(root.width - implicitWidth - 4, pos.x - implicitWidth / 2))
            y = Math.max(4, Math.min(root.height - implicitHeight - 4, pos.y + 14))
            open()
        }
        // Waiting for the selection result of the release is fine: the menu
        // only acts on what is selected when a button is pressed.
        RowLayout {
            spacing: 2
            Repeater {
                model: root.markColors
                AbstractButton {
                    required property string modelData
                    implicitWidth: 20
                    implicitHeight: 22
                    hoverEnabled: true
                    focusPolicy: Qt.NoFocus
                    contentItem: Rectangle { anchors.centerIn: parent; width: 14; height: 14; radius: 7; color: parent.modelData; border.color: parent.hovered ? "white" : "transparent" }
                    background: null
                    ToolTip.visible: hovered
                    ToolTip.delay: 600
                    ToolTip.text: qsTr("Markieren (Strg+H)")
                    onClicked: root.markSelection("highlight", modelData)
                }
            }
            IconButton { glyph: "U̲"; tip: qsTr("Unterstreichen"); onClicked: root.markSelection("underline", root.markColor) }
            IconButton { glyph: "S̶"; tip: qsTr("Durchstreichen"); onClicked: root.markSelection("strikeout", "#ff6b6b") }
            IconButton {
                glyph: "💬"
                tip: qsTr("Markieren und kommentieren")
                onClicked: {
                    const pos = Qt.point(selectionMenu.x + selectionMenu.width / 2, selectionMenu.y)
                    const id = root.markSelection("highlight", root.markColor)
                    if (id) annotPopup.show(id, pos, true)
                }
            }
            IconButton { glyph: qsTr("❝ Zitat"); tip: qsTr("Markieren und als Zitat in eine Notiz übernehmen (Strg+Q)"); onClicked: root.quoteSelection() }
            IconButton { glyph: "⧉"; tip: qsTr("Text kopieren (Strg+C)"); onClicked: { root.copySelection(); selectionMenu.close() } }
        }
    }

    // ---- one annotation: comment, color, quote, link, delete
    Popup {
        id: annotPopup
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
            const a = root.annotation(id)
            if (!a) return
            annotId = id
            info = a
            commentField.text = a.kind === "bookmark" ? (a.title || "") : (a.comment || "")
            if (pos) {
                x = Math.max(4, Math.min(root.width - width - 4, pos.x - width / 2))
                y = Math.max(4, Math.min(root.height - 220, pos.y + 12))
            } else {
                x = (root.width - width) / 2
                y = root.height / 4
            }
            open()
            if (edit) commentField.forceActiveFocus()
        }
        function commit() {
            if (!info || !root.annotation(annotId)) return
            const text = commentField.text.trim()
            if (info.kind === "bookmark") {
                if (text !== (info.title || "")) notes.update(annotId, JSON.stringify({ title: text }))
            } else if (text !== (info.comment || "")) {
                notes.update(annotId, JSON.stringify({ comment: text }))
            }
            // A pinned note without text is dropped.
            if (info.kind === "note" && !text) notes.remove(annotId)
        }
        onClosed: { commit(); root.forceActiveFocus() }

        ColumnLayout {
            anchors.left: parent.left
            anchors.right: parent.right
            spacing: 6
            RowLayout {
                Layout.fillWidth: true
                Label {
                    Layout.fillWidth: true
                    text: annotPopup.info ? root.kindGlyph(annotPopup.info.kind) + "  " + qsTr("Seite %1").arg(annotPopup.info.page) : ""
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                }
                Repeater {
                    model: annotPopup.info && annotPopup.info.kind !== "bookmark" ? root.markColors : []
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
                            notes.update(annotPopup.annotId, JSON.stringify({ color: modelData }))
                            annotPopup.info = root.annotation(annotPopup.annotId)
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
                    onClicked: { const id = annotPopup.annotId; annotPopup.close(); root.quote(id) }
                }
                Button {
                    text: qsTr("Link kopieren")
                    font.pixelSize: Theme.smallFontSize
                    onClicked: {
                        if (root.app) root.app.copyToClipboard(notes.linkFor(annotPopup.annotId))
                        root.message(qsTr("Link auf die Anmerkung kopiert"))
                    }
                }
                Item { Layout.fillWidth: true }
                Button {
                    text: qsTr("Löschen")
                    font.pixelSize: Theme.smallFontSize
                    onClicked: { const id = annotPopup.annotId; annotPopup.info = null; notes.remove(id); annotPopup.close() }
                }
            }
        }
    }

    FileDialog {
        id: exportDialog
        title: qsTr("Annotierte Kopie speichern")
        fileMode: FileDialog.SaveFile
        nameFilters: [qsTr("PDF-Dateien (*.pdf)")]
        onAccepted: {
            const path = root.app ? root.app.urlToPath(selectedFile.toString()) : selectedFile.toString().replace("file://", "")
            if (path === root.source) { exportError.text = qsTr("Die Original-PDF wird nicht überschrieben. Bitte einen anderen Namen wählen."); exportError.open(); return }
            notes.exportCopy(path)
        }
    }

    Dialog {
        id: exportError
        property alias text: exportErrorLabel.text
        anchors.centerIn: Overlay.overlay
        width: 420
        modal: true
        title: qsTr("Export")
        standardButtons: Dialog.Ok
        Label { id: exportErrorLabel; width: parent.width; wrapMode: Text.Wrap }
    }

    Popup {
        id: textPopup
        property string title
        property string text
        anchors.centerIn: Overlay.overlay
        width: Math.min(700, root.Window.width - 80)
        height: Math.min(520, root.Window.height - 80)
        modal: true
        background: Rectangle { color: Theme.editor; border.color: Theme.border; radius: 6 }
        ColumnLayout {
            anchors.fill: parent
            Label { text: textPopup.title; color: Theme.textStrong; font.bold: true }
            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                TextArea {
                    readOnly: true
                    text: textPopup.text
                    wrapMode: Text.Wrap
                    color: Theme.text
                    selectByMouse: true
                    background: null
                }
            }
        }
    }
}
