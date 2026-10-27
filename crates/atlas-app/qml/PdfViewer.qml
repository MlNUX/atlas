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
    readonly property alias fling: pagesFling
    /// Size of page `index` in millimetres ({ w, h }), from PDF points.
    function pageSizeMm(index) {
        if (index < 0 || index >= doc.pageCount) return null
        return { w: Math.round(doc.pageWidth(index) * 25.4 / 72), h: Math.round(doc.pageHeight(index) * 25.4 / 72) }
    }
    /// Dark mode for all PDFs (setting of the app): inverted brightness.
    readonly property bool dark: !!root.app && root.app.pdfDark
    readonly property color paperColor: dark ? "#262626" : "white"
    /// 0 = thumbnails, 1 = outline, 2 = annotations.
    property int sidebarTab: 0
    /// Notes citing or linking to this PDF, and the local graph.
    property bool showContext: false

    /// Open another file (from backlinks or the graph).
    signal linkActivated(string path, int line, bool newPane, var state)
    // Probed in the middle: with "whole page" the page is narrower than the
    // view and centred; between two pages the one above counts.
    readonly property int currentPage: {
        // (Not contentWidth: that follows the zoom, which follows the page.)
        const x = pages.contentX - pages.originX + pages.width / 2
        const y = pages.contentY + pages.height / 3
        const at = pages.indexAt(x, y)
        return Math.max(0, at >= 0 ? at : pages.indexAt(x, y - pages.spacing))
    }

    // Search state.
    property string searchQuery: ""
    property var hits: []
    property var hitsByPage: ({})
    property int currentHit: -1
    property int searchMs: 0

    // Annotation tools: "select" (text), "ink" (freehand), "erase" (removes
    // whole strokes), "note" (comment pin).
    property string tool: "select"
    readonly property var markColors: ["#ffd400", "#7bd88f", "#5ac8fa", "#ff7eb6", "#ff6b6b"]
    property string markColor: markColors[0]
    property string inkColor: "#ff6b6b"
    property real inkWidth: 2

    // Annotations of this PDF (read page by page, see `readAnnotationPages`).
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

    color: presenting ? "black" : Theme.pdfBackdrop
    focus: true

    function viewState() {
        return {
            zoom: root.zoom, page: currentPage, sidebar: sidebarVisible, sidebarTab: sidebarTab,
            context: showContext, contextTab: contextPanel.tab,
        }
    }
    function applyState(state) {
        let s = state || {}
        if (s.find) {
            applyFind(s.find)
            // Only the search (the tab is open already): keep the view.
            if (Object.keys(s).length === 1) return
        }
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
        runPendingFind()
    }
    function goToPage(index) {
        const i = Math.max(0, Math.min(doc.pageCount - 1, index))
        // (After a zoom change the pages must be laid out anew first.)
        pages.forceLayout()
        pages.positionViewAtIndex(i, ListView.Beginning)
    }
    /// Links of the document by 0-based page (from `doc.linksJson`).
    readonly property var linksByPage: {
        const by = {}
        try {
            for (const l of JSON.parse(doc.linksJson || "[]")) (by[l.page] = by[l.page] || []).push(l)
        } catch (e) {}
        return by
    }
    function linkAt(page, x, y) {
        return (linksByPage[page] || []).find(l => x >= l.rect[0] && x <= l.rect[0] + l.rect[2]
                                                && y >= l.rect[1] && y <= l.rect[1] + l.rect[3]) || null
    }
    /// Follows a link: jumps inside the document or opens a web address in
    /// the browser (only web and mail addresses, nothing to run).
    function followLink(link) {
        if (link.target >= 0) {
            if (link.top >= 0) showPoint(link.target, link.top)
            else goToPage(link.target)
        } else if (/^(https?|mailto|ftp):/i.test(link.uri)) {
            Qt.openUrlExternally(link.uri)
        } else if (root.actions) {
            root.actions.showMessage(qsTr("Link nicht geöffnet: %1").arg(link.uri))
        }
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
    onZoomChanged: { rememberTimer.restart(); if (zoom < 0) fitPageIndex = currentPage }
    onSourceChanged: { clearSelection(); pendingFocus = "" }

    PdfDocument {
        id: doc
        source: root.source
        onStatusChanged: if (status === "ready") Qt.callLater(root.restore)
        onTextReady: (page, text, ms, formulas) => {
            textPopup.page = page
            textPopup.title = qsTr("Text von Seite %1 (%2 ms)").arg(page + 1).arg(ms)
            textPopup.text = text
            textPopup.busy = formulas > 0
            textPopup.status = formulas > 0
                ? qsTr("%n Formel(n) gefunden – werden als LaTeX erkannt …", "", formulas) : ""
            textPopup.error = ""
            textPopup.open()
        }
        onFormulaProgress: (page, text) => {
            if (page === -1) root.message(text)
            else if (page === textPopup.page && textPopup.busy) textPopup.status = text
        }
        onSelectionLatexReady: (request, text, formulas) => root.finishQuote(request, text, formulas)
        onFormulaText: (page, text, error) => {
            if (page !== textPopup.page || !textPopup.busy) return
            textPopup.busy = false
            textPopup.text = text
            textPopup.error = error
            textPopup.status = error ? "" : qsTr("Formeln als LaTeX ($…$) – bitte prüfen")
        }
        onSearchFinished: (query, hitsJson, ms) => {
            if (query !== root.searchQuery) return
            const list = JSON.parse(hitsJson)
            const byPage = {}
            list.forEach((h, i) => { h.index = i; (byPage[h.page] = byPage[h.page] || []).push(h) })
            root.hits = list
            root.hitsByPage = byPage
            root.searchMs = ms
            // Start with the first hit at or after the current page (or the
            // page of the project search hit).
            const from = root.searchFromPage >= 0 ? root.searchFromPage : root.currentPage
            root.searchFromPage = -1
            const first = list.findIndex(h => h.page >= from)
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
        onPagesChanged: (pages) => root.readAnnotationPages(JSON.parse(pages))
        onStatusChanged: {
            if (status === "moved") root.message(qsTr("Annotationen nach dem Verschieben der PDF wiedergefunden"))
            if (status === "changed" && count === 0) Qt.callLater(acknowledgeChange)
            root.tryFocus()
        }
        onExportFinished: (ok, message) => {
            root.message(message)
            if (!ok) { exportError.text = message; exportError.open() }
        }
    }
    readonly property alias annotationStore: notes
    readonly property alias pagesView: pages
    readonly property bool changeNotice: notes.status === "changed" && notes.count > 0

    function message(text) { if (root.actions) root.actions.showMessage(text) }

    /// Reads the annotations of the pages that changed (page numbers from
    /// 1); the other pages keep theirs, nothing is built anew there.
    function readAnnotationPages(changed) {
        const byPage = Object.assign({}, annotsByPage)
        for (const page of changed) {
            let list = []
            try { list = JSON.parse(notes.pageJson(page)) } catch (e) {}
            if (list.length) byPage[page] = list
            else delete byPage[page]
        }
        annotsByPage = byPage
        annots = [].concat(...Object.keys(byPage).map(Number).sort((x, y) => x - y).map(page => byPage[page]))
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

    // ------------------------------------------------------------ undo
    // Every change of annotations goes through addAnnot, updateAnnot and
    // removeAnnot and is remembered as {type, before, after} with
    // snapshots (JSON lists of the annotation and its group): undo puts the
    // old state back, redo the new one (#6).

    property var undoStack: []
    property var redoStack: []
    readonly property bool canUndo: undoStack.length > 0
    readonly property bool canRedo: redoStack.length > 0
    /// Changes of the same annotation within this time count as one step
    /// (e.g. typing a comment).
    readonly property int mergeMs: 1500
    /// Removals with the same batch (one eraser drag) are one step.
    property string eraseBatch: ""

    /// A step: {type, parts: [{id, before, after}], group, batch, time}.
    function record(step) {
        const last = undoStack[undoStack.length - 1]
        const part = step.parts[0]
        if (step.type === "update" && last && last.type === "update" && last.parts[0].id === part.id && step.time - last.time < mergeMs) {
            last.parts[0].after = part.after
            last.time = step.time
            undoStack = undoStack.slice()
        } else if (step.type === "add" && step.group && last && last.type === "add" && last.group === step.group) {
            // Markup over several pages: one step.
            last.parts[0].after = part.after
            undoStack = undoStack.slice()
        } else if (step.type === "remove" && step.batch && last && last.type === "remove" && last.batch === step.batch) {
            last.parts.push(part)
            undoStack = undoStack.slice()
        } else {
            undoStack = undoStack.concat([step]).slice(-200)
        }
        redoStack = []
    }
    function addAnnot(json) {
        const id = notes.add(json)
        if (id) record({ type: "add", group: JSON.parse(json).group || "", time: Date.now(),
                         parts: [{ id: id, before: "[]", after: notes.snapshot(id) }] })
        return id
    }
    function updateAnnot(id, patch) {
        const before = notes.snapshot(id)
        const ok = notes.update(id, patch)
        if (ok) record({ type: "update", time: Date.now(), parts: [{ id: id, before: before, after: notes.snapshot(id) }] })
        return ok
    }
    function removeAnnot(id) {
        const before = notes.snapshot(id)
        const ok = notes.remove(id)
        if (ok) record({ type: "remove", batch: eraseBatch, time: Date.now(), parts: [{ id: id, before: before, after: "[]" }] })
        return ok
    }
    /// Brings the state `snapshot` back; "[]" means the annotation is gone.
    function applySnapshot(id, snapshot, other) {
        if (snapshot === "[]") notes.remove(id)
        else {
            // Parts of a group that exist only in the other state go first.
            const keep = JSON.parse(snapshot).map(a => a.id)
            for (const a of JSON.parse(other)) if (keep.indexOf(a.id) < 0) notes.remove(a.id)
            notes.restore(snapshot)
        }
    }
    function undo() {
        if (!canUndo) return
        const step = undoStack[undoStack.length - 1]
        undoStack = undoStack.slice(0, -1)
        for (const p of step.parts.slice().reverse()) applySnapshot(p.id, p.before, p.after)
        redoStack = redoStack.concat([step])
        message(step.type === "add" ? qsTr("Rückgängig: hinzugefügt") : step.type === "remove" ? qsTr("Rückgängig: gelöscht") : qsTr("Rückgängig: geändert"))
    }
    function redo() {
        if (!canRedo) return
        const step = redoStack[redoStack.length - 1]
        redoStack = redoStack.slice(0, -1)
        for (const p of step.parts) applySnapshot(p.id, p.after, p.before)
        undoStack = undoStack.concat([step])
    }

    // ---------------------------------------------------------- eraser
    // The eraser removes whole strokes: every ink annotation it touches
    // on its way. One drag is one undo step.

    /// Eraser radius in points.
    readonly property real eraserRadius: 6
    function distToSegment(px, py, ax, ay, bx, by) {
        const dx = bx - ax, dy = by - ay
        const len = dx * dx + dy * dy
        const t = len > 0 ? Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / len)) : 0
        return Math.hypot(px - ax - t * dx, py - ay - t * dy)
    }
    /// Distance between the segments a–b and c–d (0 if they cross).
    function segmentDistance(ax, ay, bx, by, cx, cy, dx, dy) {
        const cross = (px, py, qx, qy, rx, ry) => (qx - px) * (ry - py) - (qy - py) * (rx - px)
        const d1 = cross(ax, ay, bx, by, cx, cy), d2 = cross(ax, ay, bx, by, dx, dy)
        const d3 = cross(cx, cy, dx, dy, ax, ay), d4 = cross(cx, cy, dx, dy, bx, by)
        if (((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) && ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0))) return 0
        return Math.min(distToSegment(ax, ay, cx, cy, dx, dy), distToSegment(bx, by, cx, cy, dx, dy),
                        distToSegment(cx, cy, ax, ay, bx, by), distToSegment(dx, dy, ax, ay, bx, by))
    }
    /// Whether the eraser moving from (x0, y0) to (x, y) touches stroke `a`.
    function eraserHits(a, x0, y0, x, y) {
        const r = eraserRadius + (a.width || 2) / 2
        for (const s of a.ink) {
            if (s.length === 1 && distToSegment(s[0][0], s[0][1], x0, y0, x, y) <= r) return true
            for (let i = 1; i < s.length; ++i)
                if (segmentDistance(x0, y0, x, y, s[i - 1][0], s[i - 1][1], s[i][0], s[i][1]) <= r) return true
        }
        return false
    }
    /// Erases the strokes on page `page` (0-based) along the eraser's way.
    function eraseAlong(page, x0, y0, x, y) {
        let n = 0
        for (const a of (annotsByPage[page + 1] || []))
            if (a.kind === "ink" && eraserHits(a, x0, y0, x, y) && removeAnnot(a.id)) n++
        return n
    }

    /// Turns the current selection into text markup; returns the first ID.
    function markSelection(kind, color) {
        const id = markParts(kind, color, selection)
        clearSelection()
        return id
    }
    /// Marks the parts of a selection ({page, rects, text}); the first ID.
    function markParts(kind, color, parts) {
        if (!parts.length) return ""
        const group = parts.length > 1 ? newGroup() : ""
        let first = ""
        for (const part of parts) {
            const id = addAnnot(JSON.stringify({
                kind: kind, page: part.page + 1, rects: part.rects, text: part.text, color: color || markColor, group: group
            }))
            if (!first) first = id
        }
        if (kind === "highlight" && color) markColor = color
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
    /// "In Notiz übernehmen": formulas in the selected text are recognised
    /// first and go into the quote as LaTeX (`$…$`).
    property int quoteRequest: 0
    property var quoteJob: null
    function quoteSelection() {
        if (!selection.length) return
        if (!root.app) { const id = markSelection("highlight", markColor); if (id) quote(id); return }
        quoteRequest++
        const parts = selection.map(p => Object.assign({}, p))
        quoteJob = { request: quoteRequest, parts: parts, waiting: parts.length }
        parts.forEach((part, i) => doc.selectionLatex(quoteRequest * 100 + i, part.page,
            JSON.stringify(part.rects.map(r => [r[0], r[1], r[0] + r[2], r[1] + r[3]]))))
    }
    function finishQuote(request, text, formulas) {
        const job = quoteJob
        if (!job || Math.floor(request / 100) !== job.request) return
        if (formulas > 0 && text) job.parts[request % 100].text = text
        if (--job.waiting > 0) return
        quoteJob = null
        const id = markParts("highlight", markColor, job.parts)
        clearSelection()
        if (id) quote(id)
    }
    /// A part of the page as picture into a note, with the page as source.
    function quoteFigure(page, x0, y0, x1, y1) {
        if (!root.actions || !root.app) return
        const area = JSON.stringify([Math.min(x0, x1), Math.min(y0, y1), Math.max(x0, x1), Math.max(y0, y1)])
        const dir = root.source.slice(0, root.source.lastIndexOf("/"))
        root.actions.sendBlockToNote(qsTr("Bild von Seite %1 übernehmen in:").arg(page + 1),
                                     (note) => root.app.pdfAreaToNote(root.source, page + 1, area, note), dir,
                                     qsTr("Bild in %1 übernommen"), qsTr("Zitate"))
    }
    function copySelection() {
        if (!selectionText) return
        if (root.app) root.app.copyToClipboard(selectionText)
        message(qsTr("Text kopiert"))
    }

    // ------------------------------------------------- formula as LaTeX
    // A box dragged over a formula (tool "latex") or the selected text goes
    // as a picture to the built-in pix2tex model (atlas_core::latex_ocr, on
    // this machine); the LaTeX is copied and shown to check and edit.
    /// The box being dragged: {page, x0, y0, x1, y1} (points) or null.
    property var latexBox: null
    property int latexRequest: 0
    /// The result window's state (for the smoke test).
    readonly property bool latexShown: latexPopup.visible
    readonly property bool latexBusy: latexPopup.busy
    readonly property string latexText: latexPopup.latex
    readonly property string latexError: latexPopup.error
    function recognizeLatex(page, x0, y0, x1, y1) {
        if (!root.app) return
        latexRequest++
        latexPopup.start(root.app.latexOcrWarm())
        root.app.latexFromPdf(latexRequest, root.source, page, JSON.stringify([x0, y0, x1, y1]))
    }
    /// The selected text's box on its first page, with a little margin.
    function recognizeSelection() {
        const s = selection[0]
        if (!s || !s.rects.length) return
        let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity
        for (const r of s.rects) {
            x0 = Math.min(x0, r[0]); y0 = Math.min(y0, r[1])
            x1 = Math.max(x1, r[0] + r[2]); y1 = Math.max(y1, r[1] + r[3])
        }
        recognizeLatex(s.page, x0 - 3, y0 - 3, x1 + 3, y1 + 3)
    }
    Connections {
        target: root.app
        function onLatexProgress(request, text) {
            if (request === root.latexRequest) latexPopup.progress = text
        }
        function onLatexRecognized(request, latex, error) {
            if (request !== root.latexRequest) return
            latexPopup.finish(latex, error)
            if (latex) {
                root.app.copyToClipboard(latex)
                root.message(qsTr("LaTeX kopiert"))
            }
        }
    }

    function toggleBookmark() {
        const page = currentPage + 1
        const existing = (annotsByPage[page] || []).find(a => a.kind === "bookmark")
        if (existing) {
            removeAnnot(existing.id)
            message(qsTr("Lesezeichen auf Seite %1 entfernt").arg(page))
        } else {
            addAnnot(JSON.stringify({ kind: "bookmark", page: page, title: qsTr("Seite %1").arg(page), color: "#ff6b6b" }))
            message(qsTr("Lesezeichen auf Seite %1").arg(page))
        }
    }
    readonly property bool pageBookmarked: (annotsByPage[currentPage + 1] || []).some(a => a.kind === "bookmark")

    function exportCopy() {
        if (notes.count === 0) return
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

    /// Text of the project search to look for once the document is ready
    /// (`find`: `{ text, page }`, page 1-based): its hits are marked, the
    /// first one shown is on that page.
    property string pendingFind: ""
    property int searchFromPage: -1
    function applyFind(find) {
        pendingFind = find.text || ""
        searchFromPage = Math.max(0, (find.page || 1) - 1)
        if (doc.status === "ready") Qt.callLater(runPendingFind)
    }
    function runPendingFind() {
        if (!pendingFind || doc.status !== "ready") return
        const text = pendingFind
        pendingFind = ""
        searchField.text = text
        search(text)
    }

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
    // "Whole page" fits the page shown when it was chosen (following the
    // current page would loop: the scale moves the pages, which changes it).
    property int fitPageIndex: 0
    readonly property real fitPageScale: doc.pageCount > 0
        ? Math.max(0.1, Math.min(fitScale, (pages.height - 24) / doc.pageHeight(Math.min(fitPageIndex, doc.pageCount - 1)))) : 1
    readonly property real zoomScale: presenting ? slideScale : zoom > 0 ? zoom : zoom < 0 ? fitPageScale : fitScale
    /// The slide as large as the screen allows (presentation).
    readonly property real slideScale: doc.pageCount > 0
        ? Math.max(0.1, Math.min(pages.width / doc.pageWidth(slide), pages.height / doc.pageHeight(slide))) : 1
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
    /// Pixels per wheel notch and per arrow key.
    readonly property int wheelStep: 160
    readonly property int arrowStep: 80
    /// Touchpad movement not yet shown (applied once per frame).
    property point pendingScroll: Qt.point(0, 0)
    function flushScroll() {
        const p = pendingScroll
        pendingScroll = Qt.point(0, 0)
        if (p.x || p.y) scrollBy(-p.x, -p.y)
    }
    /// Scrolls the pages by (dx, dy) pixels, within bounds.
    function scrollBy(dx, dy) {
        pages.cancelFlick()
        if (dy) {
            const minY = pages.originY - pages.topMargin
            const maxY = Math.max(minY, pages.originY + pages.contentHeight + pages.bottomMargin - pages.height)
            pages.contentY = Math.max(minY, Math.min(maxY, pages.contentY + dy))
        }
        if (dx) {
            const maxX = Math.max(0, pages.contentWidth - pages.width)
            pages.contentX = Math.max(0, Math.min(maxX, pages.contentX - pages.originX + dx)) + pages.originX
        }
    }
    function zoomBy(factor) { zoom = Math.max(0.2, Math.min(6, zoomScale * factor)) }

    // Presentation (like PowerPoint): the viewer itself moves into a full
    // screen window of its own (annotations stay visible) and shows one page
    // at a time on black. Click, arrows, space, wheel: next/previous page;
    // number + Enter: that page; B/W: black/white screen; Esc: back to the
    // tab, at the page shown last.
    property bool presenting: false
    /// The page shown in the presentation.
    property int slide: 0
    /// "" or the colour of a blanked screen ("black", "white").
    property string blank: ""
    /// Digits typed for "number + Enter".
    property string typedSlide: ""
    readonly property bool toolbarShown: toolbar.visible
    property var normalParent: null
    property var normalState: null
    function startPresentation(fromStart) {
        if (presenting || doc.status !== "ready") return
        const page = fromStart ? 0 : currentPage
        normalState = { zoom: zoom, sidebar: sidebarVisible, context: showContext, tool: tool }
        normalParent = root.parent
        clearSelection()
        tool = "select"
        sidebarVisible = false
        showContext = false
        blank = ""
        typedSlide = ""
        slide = page
        presenting = true
        presentationWindow.showFullScreen()
        body.parent = presentationWindow.contentItem
        presentationHint.restart()
        cursorTimer.restart()
        presentationWindow.requestActivate()
        body.forceActiveFocus()
        Qt.callLater(() => showSlide(page))
    }
    function endPresentation() {
        if (!presenting) return
        const page = slide
        body.parent = root
        presenting = false
        blank = ""
        presentationWindow.hide()
        if (normalState) {
            zoom = normalState.zoom
            sidebarVisible = normalState.sidebar
            showContext = normalState.context
            tool = normalState.tool
        }
        if (root.Window.window) root.Window.window.requestActivate()
        root.forceActiveFocus()
        Qt.callLater(() => goToPage(page))
    }
    /// Shows page `index` alone, centred (the gaps between pages are a
    /// screen high, so no neighbour peeks in).
    function showSlide(index) {
        if (!presenting) return
        slide = Math.max(0, Math.min(doc.pageCount - 1, index))
        pages.forceLayout()
        pages.positionViewAtIndex(slide, ListView.Center)
    }
    function nextSlide(step) {
        if (blank) { blank = ""; return }
        typedSlide = ""
        showSlide(slide + step)
    }
    function presentationKey(event) {
        const k = event.key
        if (k === Qt.Key_Escape || k === Qt.Key_F11 || k === Qt.Key_F5) { endPresentation(); return }
        if (k >= Qt.Key_0 && k <= Qt.Key_9) { typedSlide += String(k - Qt.Key_0); return }
        if ((k === Qt.Key_Return || k === Qt.Key_Enter) && typedSlide) {
            const n = parseInt(typedSlide)
            typedSlide = ""
            blank = ""
            showSlide(n - 1)
            return
        }
        if (k === Qt.Key_B || k === Qt.Key_Period) { blank = blank === "black" ? "" : "black"; return }
        if (k === Qt.Key_W || k === Qt.Key_Comma) { blank = blank === "white" ? "" : "white"; return }
        if ([Qt.Key_Right, Qt.Key_Down, Qt.Key_Space, Qt.Key_PageDown, Qt.Key_Return, Qt.Key_Enter, Qt.Key_N].indexOf(k) >= 0) nextSlide(1)
        else if ([Qt.Key_Left, Qt.Key_Up, Qt.Key_Backspace, Qt.Key_PageUp, Qt.Key_P].indexOf(k) >= 0) nextSlide(-1)
        else if (k === Qt.Key_Home) { blank = ""; showSlide(0) }
        else if (k === Qt.Key_End) { blank = ""; showSlide(doc.pageCount - 1) }
    }
    // Slides of another size change the scale: keep the slide centred.
    onSlideScaleChanged: if (presenting) Qt.callLater(() => showSlide(slide))
    /// The presentation window (for the smoke test).
    readonly property alias presentationWindowItem: presentationWindow
    readonly property alias bodyItem: body
    readonly property bool statusShown: statusBar.visible
    Component.onDestruction: if (presenting) presentationWindow.hide()


    Window {
        id: presentationWindow
        // A window of its own: Qt would take the window of the viewer as
        // transient parent – in full screen that is this window itself.
        transientParent: null
        title: root.source.split("/").pop()
        color: "black"
        // Ends only by Esc, the button or closing the window: on Wayland
        // the compositor reports "windowed" for a moment while it is
        // setting up full screen, which must not end it again.
        onClosing: (close) => { close.accepted = false; root.endPresentation() }

        Timer { id: presentationHint; interval: 3000 }
        Rectangle {
            z: 110
            anchors.top: parent.top
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.topMargin: 16
            width: hintRow.implicitWidth + 24
            height: 34
            radius: 17
            color: "#d0202020"
            opacity: presentationHint.running || hintArea.hovered || exitButton.hovered || root.typedSlide ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: 200 } }
            visible: opacity > 0
            RowLayout {
                id: hintRow
                anchors.centerIn: parent
                spacing: 8
                Label {
                    text: root.typedSlide
                        ? qsTr("Gehe zu Seite %1 … (Enter)").arg(root.typedSlide)
                        : qsTr("Seite %1 / %2 · Klick, ← → oder Mausrad blättern · B schwarz · Esc beendet").arg(root.slide + 1).arg(doc.pageCount)
                    color: "white"
                    font.pixelSize: Theme.smallFontSize
                }
                IconButton { id: exitButton; iconName: "close"; tip: qsTr("Präsentation beenden (Esc)"); onClicked: root.endPresentation() }
            }
        }
        // The hint comes back when the mouse goes to the top edge.
        Item {
            z: 109
            anchors.top: parent.top
            width: parent.width
            height: 60
            HoverHandler { id: hintArea }
        }
    }

    Keys.onPressed: (event) => handleKey(event)
    function handleKey(event) {
        if (presenting) { presentationKey(event); event.accepted = true; return }
        const ctrl = event.modifiers & Qt.ControlModifier
        // Undo and redo of annotation changes.
        if (ctrl && event.key === Qt.Key_Z && !(event.modifiers & Qt.ShiftModifier)) { undo(); event.accepted = true; return }
        if (ctrl && (event.key === Qt.Key_Y || (event.key === Qt.Key_Z && (event.modifiers & Qt.ShiftModifier)))) { redo(); event.accepted = true; return }
        // Arrows: up/down scroll, left/right turn pages (like other viewers).
        if (!ctrl && event.key === Qt.Key_Down) { scrollBy(0, arrowStep); event.accepted = true; return }
        if (!ctrl && event.key === Qt.Key_Up) { scrollBy(0, -arrowStep); event.accepted = true; return }
        if (!ctrl && event.key === Qt.Key_Right) {
            if (pages.contentWidth > pages.width + 1) scrollBy(arrowStep, 0); else goToPage(currentPage + 1)
            event.accepted = true; return
        }
        if (!ctrl && event.key === Qt.Key_Left) {
            if (pages.contentWidth > pages.width + 1) scrollBy(-arrowStep, 0); else goToPage(currentPage - 1)
            event.accepted = true; return
        }
        if (event.key === Qt.Key_F5 || event.key === Qt.Key_F11) {
            startPresentation(event.key === Qt.Key_F5 && !(event.modifiers & Qt.ShiftModifier))
            event.accepted = true; return
        }
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

    // Everything visible. In a presentation it moves into the presentation
    // window and fills it; the viewer itself stays in its tab (the tab
    // keeps sizing it there, which must not reach the presentation).
    Rectangle {
        id: body
        anchors.fill: parent
        color: root.color
        focus: true
        Keys.onPressed: (event) => root.handleKey(event)

        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            Rectangle {
                Layout.fillWidth: true
                id: toolbar
                Layout.preferredHeight: 32
                visible: root.chrome && !root.presenting
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
                    IconButton {
                        iconName: "moon"
                        tip: root.dark ? qsTr("Dunkelmodus aus: Originalfarben") : qsTr("Dunkelmodus: Farben umkehren (für alle PDFs)")
                        checkable: true
                        checked: root.dark
                        enabled: !!root.app
                        onClicked: root.app.pdfDark = !root.app.pdfDark
                    }
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
                        iconName: "eraser"
                        tip: qsTr("Radierer: ganze Striche löschen (über sie fahren)")
                        checkable: true
                        checked: root.tool === "erase"
                        onClicked: { root.tool = root.tool === "erase" ? "select" : "erase"; root.clearSelection() }
                    }
                    IconButton {
                        glyph: "🗨"
                        tip: qsTr("Notiz auf der Seite anheften")
                        checkable: true
                        checked: root.tool === "note"
                        onClicked: { root.tool = root.tool === "note" ? "select" : "note"; root.clearSelection() }
                    }
                    IconButton {
                        iconName: "image"
                        tip: qsTr("Bild als Zitat: Rahmen um Abbildung oder Tabelle ziehen, dann Notiz wählen")
                        checkable: true
                        checked: root.tool === "figure"
                        onClicked: { root.tool = root.tool === "figure" ? "select" : "figure"; root.clearSelection() }
                    }
                    IconButton {
                        glyph: "∑"
                        tip: qsTr("Formel als LaTeX: Rahmen um die Formel ziehen (läuft auf diesem Rechner)")
                        checkable: true
                        checked: root.tool === "latex"
                        onClicked: { root.tool = root.tool === "latex" ? "select" : "latex"; root.clearSelection() }
                    }
                    IconButton {
                        iconName: "undo"
                        tip: qsTr("Rückgängig (Strg+Z)")
                        enabled: root.canUndo
                        onClicked: root.undo()
                    }
                    IconButton {
                        iconName: "redo"
                        tip: qsTr("Wiederholen (Strg+Umschalt+Z)")
                        enabled: root.canRedo
                        onClicked: root.redo()
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
                        tip: qsTr("Text der aktuellen Seite, Formeln als LaTeX")
                        enabled: doc.status === "ready"
                        onClicked: doc.extractText(root.currentPage)
                    }
                    IconButton { iconName: "refresh"; tip: qsTr("Neu laden"); onClicked: { doc.reload(); notes.reload() } }
                    IconButton {
                        iconName: "play"
                        tip: qsTr("Präsentieren ab dieser Seite (Umschalt+F5; F5: von vorn)")
                        onClicked: root.startPresentation(false)
                    }
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
                        tip: qsTr("Graph: Notizen, PDFs und Dateien des Projekts")
                        checkable: true
                        checked: root.showContext && contextPanel.tab === 1
                        onClicked: { root.showContext = !(root.showContext && contextPanel.tab === 1); contextPanel.tab = 1 }
                    }
                }
            }

            // The PDF changed since the annotations were made (only matters if
            // there are any; without, the new version is taken silently).
            Banner {
                Layout.fillWidth: true
                visible: root.changeNotice
                message: qsTr("Diese PDF wurde seit dem Anlegen der %n Anmerkung(en) verändert – Markierungen können verschoben sein.", "", notes.count)
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
                                        color: root.paperColor
                                        border.width: thumb.index === root.currentPage ? 2 : 0
                                        border.color: Theme.accent
                                        PdfPage {
                                            anchors.fill: parent
                                            anchors.margins: parent.border.width
                                            source: doc.source
                                            page: thumb.index
                                            inverted: root.dark
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
                                    // Built only while the list is shown.
                                    model: root.sidebarVisible && root.sidebarTab === 2 ? root.sortedAnnots : []
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
                                            onClicked: root.removeAnnot(annotRow.modelData.id)
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
                    // Arrow keys are handled by the viewer (scroll, turn pages).
                    keyNavigationEnabled: false
                    model: doc.status === "ready" ? doc.pageCount : 0
                    // Presenting: a screen of space between pages, so only one
                    // shows; scrolling is the presentation's (page by page).
                    spacing: root.presenting ? height : 12
                    topMargin: root.presenting ? 0 : 12
                    bottomMargin: root.presenting ? 0 : 12
                    interactive: !root.presenting
                    cacheBuffer: Math.max(0, height)
                    boundsBehavior: Flickable.StopAtBounds
                    flickableDirection: Flickable.AutoFlickIfNeeded
                    contentWidth: root.presenting ? width : Math.max(width, doc.maxPageWidth * root.zoomScale + 32)
                    ScrollBar.vertical: ScrollBar { policy: root.presenting ? ScrollBar.AlwaysOff : ScrollBar.AsNeeded }
                    ScrollBar.horizontal: ScrollBar { policy: root.presenting ? ScrollBar.AlwaysOff : ScrollBar.AsNeeded }

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
                            // The spot under the mouse stays: remembered as page and
                            // place on that page – not as a distance from the top of
                            // the document, which drifts (the gaps between pages do
                            // not grow with the zoom, and the list only estimates the
                            // positions of pages it has not laid out). Pages are
                            // centred in the content, so a page narrower than the
                            // view must not move sideways either.
                            root.flushScroll()
                            pagesFling.stop()
                            pages.cancelFlick()
                            // The handler lives in the list's content: its event
                            // position includes the scroll position. In view
                            // coordinates (counting the scroll position twice made
                            // the pages run away – the further down, the more).
                            const at = pages.mapFromItem(pages.contentItem, event.x, event.y)
                            const before = root.zoomScale
                            const offset = (z) => (pages.contentWidth - doc.maxPageWidth * z) / 2
                            const px = (pages.contentX - pages.originX + at.x - offset(before)) / before
                            const cy = pages.contentY + at.y
                            let index = pages.indexAt(pages.contentWidth / 2, cy)
                            if (index < 0) index = pages.indexAt(pages.contentWidth / 2, cy - pages.spacing)
                            if (index < 0) index = root.currentPage
                            const item = pages.itemAtIndex(index)
                            // Place on the page as a fraction of its height (in the gap
                            // below a page: a little over 1).
                            const along = item && item.height > 0 ? (cy - item.y) / item.height : 0
                            root.zoomBy(Math.pow(1.1, steps))
                            const place = () => {
                                const maxX = Math.max(0, pages.contentWidth - pages.width)
                                const x = offset(root.zoomScale) + px * root.zoomScale - at.x
                                pages.contentX = Math.max(0, Math.min(maxX, x)) + pages.originX
                                // The pages at the new size, laid out now.
                                pages.forceLayout()
                                const now = pages.itemAtIndex(index)
                                if (!now) return
                                const y = now.y + along * now.height - at.y
                                const minY = pages.originY - pages.topMargin
                                const maxY = Math.max(minY, pages.originY + pages.contentHeight + pages.bottomMargin - pages.height)
                                pages.contentY = Math.max(minY, Math.min(maxY, y))
                            }
                            // At once (no frame at the old position), and again after
                            // the list settled (pages above may have been measured anew).
                            place()
                            Qt.callLater(place)
                            event.accepted = true
                        }
                    }
                    // Momentum after touchpad swipes (#4); the wheel handler below
                    // scrolls and reports the steps.
                    TouchpadFling {
                        id: pagesFling
                        parent: pages
                        flickable: pages
                        handleWheel: false
                        active: !root.app || root.app.touchpadFling
                        app: root.app
                    }
                    // Plain wheel: scroll faster than the list would (a few lines
                    // per notch felt sluggish on long documents). Touchpads keep
                    // their own pixel deltas, a bit amplified.
                    WheelHandler {
                        acceptedDevices: PointerDevice.AllDevices
                        acceptedPointerTypes: PointerDevice.AllPointerTypes
                        acceptedModifiers: Qt.NoModifier
                        onWheel: (event) => {
                            event.accepted = true
                            // Fingers lifted: the momentum starts at once (after
                            // the movement still waiting for its frame).
                            if (event.phase === Qt.ScrollEnd) root.flushScroll()
                            if (pagesFling.phase(event)) return
                            const delta = (angle, pixel) => pixel !== 0 && (angle === 0 || angle % 120 !== 0)
                                ? pixel * 1.5 : angle / 120 * root.wheelStep
                            const dx = delta(event.angleDelta.x, event.pixelDelta.x)
                            const dy = delta(event.angleDelta.y, event.pixelDelta.y)
                            // Touchpad: measured for the momentum after the swipe,
                            // and moved once per frame (it sends several steps per
                            // frame; each would lay out the pages anew).
                            if (pagesFling.isTouchpad(event)) {
                                pagesFling.sample(dx, dy)
                                root.pendingScroll = Qt.point(root.pendingScroll.x + dx, root.pendingScroll.y + dy)
                            } else {
                                root.scrollBy(-dx, -dy)
                            }
                        }
                    }
                    FrameAnimation {
                        running: root.pendingScroll.x !== 0 || root.pendingScroll.y !== 0
                        onTriggered: root.flushScroll()
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
                            color: root.paperColor

                            PdfPage {
                                id: page
                                anchors.fill: parent
                                source: doc.source
                                page: pageDelegate.index
                                inverted: root.dark
                                renderWidth: Math.round(doc.pageWidth(pageDelegate.index) * root.renderScale * Screen.devicePixelRatio)
                                onRenderMsChanged: root.lastRenderMs = renderMs
                            }

                            // Everything drawn over the page, in page points: the
                            // layer is only scaled as a whole, so zooming does not
                            // recompute every annotation, stroke point and hit.
                            Item {
                                id: overlay
                                readonly property real s: root.zoomScale
                                width: doc.pageWidth(pageDelegate.index)
                                height: doc.pageHeight(pageDelegate.index)
                                transformOrigin: Item.TopLeft
                                scale: s

                                // Annotations of this page.
                                Repeater {
                                    model: root.annotsByPage[pageDelegate.index + 1] || []
                                    delegate: Item {
                                        id: ann
                                        required property var modelData
                                        readonly property bool focused: root.focusedAnnot === modelData.id
                                        anchors.fill: parent

                                        // Text markup: one bar per line.
                                        Repeater {
                                            model: ["highlight", "underline", "strikeout"].indexOf(ann.modelData.kind) >= 0 ? ann.modelData.rects : []
                                            delegate: Rectangle {
                                                required property var modelData
                                                x: modelData[0]
                                                y: modelData[1]
                                                width: modelData[2]
                                                height: modelData[3]
                                                color: ann.modelData.kind === "highlight" ? Qt.alpha(ann.modelData.color, 0.38) : "transparent"
                                                border.width: ann.focused ? 2 / overlay.s : 0
                                                border.color: "#ff9632"
                                                Rectangle {
                                                    visible: ann.modelData.kind !== "highlight"
                                                    width: parent.width
                                                    height: Math.max(1.5 / overlay.s, 1.2)
                                                    y: ann.modelData.kind === "underline" ? parent.height - height : (parent.height - height) / 2
                                                    color: ann.modelData.color
                                                }
                                            }
                                        }
                                        // Markers for comments on text markup.
                                        Text {
                                            visible: !!ann.modelData.comment && !!ann.modelData.rects && ann.modelData.rects.length > 0 && ann.modelData.kind !== "note"
                                            x: visible ? ann.modelData.rects[0][0] + ann.modelData.rects[0][2] : 0
                                            y: visible ? ann.modelData.rects[0][1] - height / 2 : 0
                                            text: "💬"
                                            font.pixelSize: Math.max(9 / overlay.s, 9)
                                        }
                                        // Comment pinned to the page.
                                        Rectangle {
                                            visible: ann.modelData.kind === "note"
                                            x: visible ? ann.modelData.rects[0][0] : 0
                                            y: visible ? ann.modelData.rects[0][1] : 0
                                            width: Math.max(16 / overlay.s, 18)
                                            height: width
                                            radius: 3 / overlay.s
                                            color: ann.modelData.color
                                            border.width: (ann.focused ? 2 : 1) / overlay.s
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
                                                strokeWidth: ann.modelData.width || 2
                                                fillColor: "transparent"
                                                capStyle: ShapePath.RoundCap
                                                joinStyle: ShapePath.RoundJoin
                                                PathMultiline {
                                                    paths: ann.modelData.kind === "ink"
                                                        ? ann.modelData.ink.map(st => st.map(p => Qt.point(p[0], p[1]))) : []
                                                }
                                            }
                                        }
                                        // Bookmark ribbon.
                                        Text {
                                            visible: ann.modelData.kind === "bookmark"
                                            anchors.right: parent.right
                                            anchors.rightMargin: 8 / overlay.s
                                            y: -4 / overlay.s
                                            text: "🔖"
                                            font.pixelSize: Math.max(14 / overlay.s, 16)
                                        }
                                    }
                                }

                                // Current text selection.
                                Repeater {
                                    model: root.selectionByPage[pageDelegate.index] || []
                                    delegate: Rectangle {
                                        required property var modelData
                                        x: modelData[0]
                                        y: modelData[1]
                                        width: modelData[2]
                                        height: modelData[3]
                                        color: "#5a3399ff"
                                    }
                                }
                                // The box for "formula as LaTeX".
                                Rectangle {
                                    readonly property var b: root.latexBox
                                    visible: !!b && b.page === pageDelegate.index
                                    x: b ? Math.min(b.x0, b.x1) : 0
                                    y: b ? Math.min(b.y0, b.y1) : 0
                                    width: b ? Math.abs(b.x1 - b.x0) : 0
                                    height: b ? Math.abs(b.y1 - b.y0) : 0
                                    color: "#223399ff"
                                    border.color: Theme.accent
                                    border.width: 1.5 / overlay.s
                                }
                                // Freehand stroke being drawn.
                                Shape {
                                    visible: root.strokePage === pageDelegate.index && root.stroke.length > 1
                                    anchors.fill: parent
                                    ShapePath {
                                        strokeColor: root.inkColor
                                        strokeWidth: root.inkWidth
                                        fillColor: "transparent"
                                        capStyle: ShapePath.RoundCap
                                        joinStyle: ShapePath.RoundJoin
                                        PathPolyline {
                                            path: root.strokePage === pageDelegate.index
                                                ? root.stroke.map(p => Qt.point(p[0], p[1])) : []
                                        }
                                    }
                                }
                                // Search hits on this page.
                                Repeater {
                                    model: root.hitsByPage[pageDelegate.index] || []
                                    delegate: Rectangle {
                                        required property var modelData
                                        x: modelData.x - 1 / overlay.s
                                        y: modelData.y - 1 / overlay.s
                                        width: modelData.w + 2 / overlay.s
                                        height: modelData.h + 2 / overlay.s
                                        color: modelData.index === root.currentHit ? "#80ff9632" : "#50ffd200"
                                        border.color: modelData.index === root.currentHit ? "#ff9632" : "transparent"
                                        border.width: 1 / overlay.s
                                    }
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
                                cursorShape: root.tool === "erase" ? Qt.BlankCursor
                                    : root.tool === "ink" || root.tool === "note" || root.tool === "latex" || root.tool === "figure" ? Qt.CrossCursor
                                    : hoveredAnnotation || hoveredLink ? Qt.PointingHandCursor : Qt.IBeamCursor
                                property var pressPoint: null
                                property bool moved: false
                                property var hoveredAnnotation: null
                                property var hoveredLink: null
                                /// Last eraser position (points) and strokes erased in this drag.
                                property var erasePoint: null
                                property int erased: 0
                                /// Pointer position for the eraser circle.
                                property point hoverPos: Qt.point(-100, -100)
                                ToolTip.visible: !!hoveredLink && !!hoveredLink.uri && !pressed
                                ToolTip.text: hoveredLink ? hoveredLink.uri : ""
                                ToolTip.delay: 400
                                function pointOf(mouse) {
                                    return { page: pageDelegate.index, x: mouse.x / root.zoomScale, y: mouse.y / root.zoomScale }
                                }
                                onPositionChanged: (mouse) => {
                                    const vp = mapToItem(pages, mouse.x, mouse.y)
                                    hoverPos = Qt.point(mouse.x, mouse.y)
                                    if (root.tool === "erase") {
                                        if (pressed && erasePoint) {
                                            const p = pointOf(mouse)
                                            erased += root.eraseAlong(pageDelegate.index, erasePoint.x, erasePoint.y, p.x, p.y)
                                            erasePoint = p
                                        }
                                        return
                                    }
                                    if (!pressed) {
                                        if (root.tool === "select") {
                                            hoveredAnnotation = root.annotationAt(pageDelegate.index, mouse.x / root.zoomScale, mouse.y / root.zoomScale)
                                            hoveredLink = hoveredAnnotation ? null : root.linkAt(pageDelegate.index, mouse.x / root.zoomScale, mouse.y / root.zoomScale)
                                        }
                                        return
                                    }
                                    root.dragViewport = vp
                                    if (!pressPoint) return
                                    if (Math.abs(mouse.x - pressPoint.vx) + Math.abs(mouse.y - pressPoint.vy) > 3) moved = true
                                    if (root.tool === "select" && moved) {
                                        root.dragging = true
                                        const p = root.pointAt(vp.x, vp.y)
                                        if (p) { root.dragEnd = p; root.requestSelection() }
                                    } else if ((root.tool === "latex" || root.tool === "figure") && root.latexBox) {
                                        const p = pointOf(mouse)
                                        root.latexBox = Object.assign({}, root.latexBox, { x1: p.x, y1: p.y })
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
                                    if (root.tool === "erase") {
                                        root.eraseBatch = Date.now().toString(36)
                                        erasePoint = p
                                        erased = root.eraseAlong(p.page, p.x, p.y, p.x, p.y)
                                        return
                                    }
                                    if (root.tool === "select") {
                                        selectionMenu.close()
                                        if (!(mouse.modifiers & Qt.ShiftModifier) || !root.dragStart) {
                                            root.selectStyle = 0
                                            root.dragStart = p
                                        }
                                    } else if (root.tool === "ink") {
                                        root.strokePage = p.page
                                        root.stroke = [[p.x, p.y]]
                                    } else if (root.tool === "latex" || root.tool === "figure") {
                                        root.latexBox = { page: p.page, x0: p.x, y0: p.y, x1: p.x, y1: p.y }
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
                                    if (root.tool === "erase") {
                                        root.eraseBatch = ""
                                        erasePoint = null
                                        if (erased > 0) root.message(qsTr("%n Strich(e) gelöscht – Strg+Z holt sie zurück", "", erased))
                                        erased = 0
                                        return
                                    }
                                    if (root.tool === "ink") {
                                        if (root.stroke.length > 1)
                                            root.addAnnot(JSON.stringify({ kind: "ink", page: root.strokePage + 1, ink: [root.stroke], color: root.inkColor, width: root.inkWidth }))
                                        root.stroke = []
                                        root.strokePage = -1
                                        return
                                    }
                                    if (root.tool === "latex" || root.tool === "figure") {
                                        const b = root.latexBox
                                        root.latexBox = null
                                        if (b && Math.abs(b.x1 - b.x0) > 4 && Math.abs(b.y1 - b.y0) > 4) {
                                            if (root.tool === "latex") root.recognizeLatex(b.page, b.x0, b.y0, b.x1, b.y1)
                                            else root.quoteFigure(b.page, b.x0, b.y0, b.x1, b.y1)
                                        }
                                        return
                                    }
                                    if (root.tool === "note") {
                                        const id = root.addAnnot(JSON.stringify({ kind: "note", page: p.page + 1, rects: [[p.x - 9, p.y - 9, 18, 18]], color: "#ffd400" }))
                                        if (id) annotPopup.show(id, mapToItem(root, mouse.x, mouse.y), true)
                                        root.tool = "select"
                                        return
                                    }
                                    if (!moved && !(mouse.modifiers & Qt.ShiftModifier)) {
                                        // A click: open an annotation, follow a link or drop
                                        // the selection.
                                        const hit = root.annotationAt(p.page, p.x, p.y)
                                        root.clearSelection()
                                        if (hit) annotPopup.show(hit.id, mapToItem(root, mouse.x, mouse.y))
                                        else {
                                            const link = root.linkAt(p.page, p.x, p.y)
                                            if (link) root.followLink(link)
                                        }
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
                            // The eraser under the pointer.
                            Rectangle {
                                readonly property real size: Math.max(10, 2 * root.eraserRadius * root.zoomScale)
                                visible: root.tool === "erase" && pageMouse.containsMouse
                                x: pageMouse.hoverPos.x - size / 2
                                y: pageMouse.hoverPos.y - size / 2
                                width: size
                                height: size
                                radius: size / 2
                                color: Qt.rgba(1, 1, 1, pageMouse.pressed ? 0.35 : 0.2)
                                border.color: "#80000000"
                                border.width: 1
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
                id: statusBar
                Layout.preferredHeight: 20
                visible: (root.chrome || doc.status === "error") && !root.presenting
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

        // Click: next page, right click: previous, wheel: one page per notch;
        // the pointer hides after two seconds without moving.
        MouseArea {
            id: presentationMouse
            z: 100
            anchors.fill: parent
            visible: root.presenting
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            hoverEnabled: true
            cursorShape: cursorTimer.running ? Qt.ArrowCursor : Qt.BlankCursor
            property real wheelRest: 0
            onPositionChanged: cursorTimer.restart()
            onClicked: (mouse) => root.nextSlide(mouse.button === Qt.RightButton ? -1 : 1)
            onWheel: (wheel) => {
                const d = wheel.angleDelta.y !== 0 ? wheel.angleDelta.y : wheel.pixelDelta.y * 2
                wheelRest += d
                if (Math.abs(wheelRest) >= 120) {
                    root.nextSlide(wheelRest < 0 ? 1 : -1)
                    wheelRest = 0
                }
            }
            Timer { id: cursorTimer; interval: 2000 }
        }
        Rectangle {
            z: 99
            anchors.fill: parent
            visible: root.presenting && root.blank !== ""
            color: root.blank || "black"
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
            IconButton { glyph: "∑"; tip: qsTr("Als LaTeX erkennen und kopieren"); onClicked: { root.recognizeSelection(); selectionMenu.close() } }
        }
    }

    // ---- the recognized LaTeX: to check, edit and copy
    Popup {
        id: latexPopup
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
        x: Math.round((root.width - width) / 2)
        y: 56
        width: Math.min(root.width - 40, 560)
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
                Button { text: qsTr("$…$ kopieren"); flat: true; onClicked: { root.app.copyToClipboard("$" + latexField.text + "$"); root.message(qsTr("Mit $…$ kopiert")) } }
                Button { text: qsTr("$$…$$ kopieren"); flat: true; onClicked: { root.app.copyToClipboard("$$\n" + latexField.text + "\n$$"); root.message(qsTr("Mit $$…$$ kopiert")) } }
                Button { text: qsTr("Kopieren"); highlighted: true; onClicked: { root.app.copyToClipboard(latexField.text); latexPopup.close(); root.message(qsTr("LaTeX kopiert")) } }
            }
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
                if (text !== (info.title || "")) root.updateAnnot(annotId, JSON.stringify({ title: text }))
            } else if (text !== (info.comment || "")) {
                root.updateAnnot(annotId, JSON.stringify({ comment: text }))
            }
            // A pinned note without text is dropped.
            if (info.kind === "note" && !text) root.removeAnnot(annotId)
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
                            root.updateAnnot(annotPopup.annotId, JSON.stringify({ color: modelData }))
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
                    onClicked: { const id = annotPopup.annotId; annotPopup.info = null; root.removeAnnot(id); annotPopup.close() }
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

    // Text of a page (button "T"), formulas as LaTeX once recognised.
    readonly property alias pageTextShown: textPopup.visible
    readonly property alias pageText: textPopup.text
    readonly property alias pageTextBusy: textPopup.busy
    readonly property alias pageTextError: textPopup.error
    function showPageText(page) { doc.extractText(page) }
    function hidePageText() { textPopup.close() }

    Popup {
        id: textPopup
        property int page: -1
        property string title
        property string text
        property string status
        property string error
        property bool busy: false
        onClosed: busy = false
        anchors.centerIn: Overlay.overlay
        width: Math.min(700, root.Window.width - 80)
        height: Math.min(520, root.Window.height - 80)
        modal: true
        background: Rectangle { color: Theme.editor; border.color: Theme.border; radius: 6 }
        ColumnLayout {
            anchors.fill: parent
            RowLayout {
                Layout.fillWidth: true
                Label { text: textPopup.title; color: Theme.textStrong; font.bold: true; Layout.fillWidth: true }
                Button {
                    text: qsTr("Kopieren")
                    flat: true
                    onClicked: { root.app.copyToClipboard(textPopup.text); root.message(qsTr("Text kopiert")) }
                }
            }
            RowLayout {
                Layout.fillWidth: true
                visible: textPopup.status !== "" || textPopup.error !== ""
                BusyIndicator { running: textPopup.busy; visible: running; Layout.preferredWidth: 18; Layout.preferredHeight: 18 }
                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    font.pixelSize: Theme.smallFontSize
                    color: textPopup.error ? Theme.warn : Theme.textMuted
                    text: textPopup.error
                        ? qsTr("Formeln nicht als LaTeX: %1").arg(textPopup.error)
                          + (/Download|github|ONNX Runtime/i.test(textPopup.error)
                             ? "\n" + qsTr("Beim ersten Mal lädt Atlas das Formelmodell aus dem Netz (etwa 170 MB) – mehr in docs/latex-ocr.md") : "")
                        : textPopup.status
                }
            }
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
