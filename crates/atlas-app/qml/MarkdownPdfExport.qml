// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtWebEngine
import Atlas

// Markdown → PDF with the preview itself: the note is rendered like in the
// preview (formulas, diagrams, code) in a hidden page and printed by
// Chromium with print styles (light paper, A4). Needs no pandoc or LaTeX.
Item {
    id: exporter
    required property AppController app

    /// Note being exported and the PDF written ("" when idle).
    property string source: ""
    property string output: ""
    readonly property bool busy: source !== ""

    signal finished(string source, string output, bool ok)

    /// Exports `source` (a Markdown file) to `output`, by default next to
    /// it with the extension .pdf. Returns false if an export is running.
    function exportPdf(source, output) {
        if (busy) return false
        exporter.output = output || source.replace(/\.(md|markdown|mdown)$/i, "") + ".pdf"
        exporter.source = source
        if (web.loaded) start()
        else web.url = "atlas://app/web/preview.html"
        return true
    }
    function start() {
        const result = JSON.parse(app.renderMarkdown(app.readTextFile(source), source))
        web.runJavaScript("atlas.set(" + JSON.stringify(result.html) + ")")
        waitTimer.tries = 0
        waitTimer.start()
    }
    function done(ok) {
        const s = source, o = output
        source = ""
        output = ""
        finished(s, o, ok)
    }

    // Diagrams and fonts render asynchronously; print once they are done.
    Timer {
        id: waitTimer
        property int tries: 0
        interval: 150
        repeat: true
        onTriggered: web.runJavaScript("atlas.busy()", (busy) => {
            if (busy && ++tries < 80) return
            waitTimer.stop()
            web.runJavaScript("atlas.preparePrint()", () => web.printToPdf(exporter.output, WebEngineView.A4, WebEngineView.Portrait))
        })
    }

    WebEngineView {
        id: web
        // Laid out at a page-like size, but transparent and out of the way
        // (Chromium does not lay out hidden pages).
        x: -width - 100
        width: 800
        height: 1100
        opacity: 0
        enabled: false
        property bool loaded: false
        settings.localContentCanAccessFileUrls: true
        settings.localContentCanAccessRemoteUrls: false
        settings.javascriptCanOpenWindows: false
        onLoadingChanged: (request) => {
            if (request.status === WebEngineLoadingInfo.LoadSucceededStatus) {
                loaded = true
                if (exporter.busy) exporter.start()
            } else if (request.status === WebEngineLoadingInfo.LoadFailedStatus && exporter.busy) {
                console.warn("markdown pdf: preview not loaded:", request.errorString)
                exporter.done(false)
            }
        }
        onNavigationRequested: (request) => {
            if (request.url.toString() !== "atlas://app/web/preview.html") request.reject()
        }
        onPdfPrintingFinished: (filePath, success) => {
            if (!success) console.warn("markdown pdf: printing failed:", filePath)
            exporter.done(success)
        }
    }
}
