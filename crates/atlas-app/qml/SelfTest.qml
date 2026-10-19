// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import Atlas

// Built-in smoke test (`atlas PROJECT --self-test`), run offscreen by
// scripts/smoke-test.sh against a generated fixture project. Each step runs
// after the previous one had time to settle; the process exits with 0 if all
// checks passed.
Item {
    id: test
    // Set by Main before start().
    property var app
    property var editor
    property var fileActions
    property var commandPalette
    property var searchPanel
    property var searchModel
    property var terminalPanel
    property var window

    property int failures: 0
    property int step: 0
    property var steps: []
    property string annotId
    property string pdfBefore

    function check(name, ok, detail) {
        if (!ok) failures++
        console.log("selftest:", ok ? "ok  " : "FAIL", name, ok ? "" : "– " + (detail === undefined ? "" : detail))
    }
    function path(rel) { return app.projectRoot + "/" + rel }

    function start() {
        const note = path("notes/idee.md")
        const pdf = path("docs/quelle.pdf")
        steps = [
            () => {
                check("Projekt geöffnet", app.projectRoot !== "")
                editor.open(note, 0, "tab")
            },
            () => {
                const md = editor.activeTab.viewerItem
                check("Markdown-Ansicht", !!md && !!md.editorItem)
                md.mode = "split"
                md.editorItem.insertText("\nAutosave-Test [Plan](plan.md#ziele)\n", true)
                check("Bearbeitung erkannt", md.editorItem.modified)
            },
            () => {
                check("Autosave geschrieben", app.readTextFile(note).indexOf("Autosave-Test") >= 0)
                check("keine Recovery nach Speichern", !editor.activeTab.viewerItem.editorItem.document.hasRecovery)
                const back = JSON.parse(app.backlinks(path("notes/plan.md")))
                check("Rückverweis auf plan.md", back.length >= 1, JSON.stringify(back))
                check("Tags hierarchisch", app.notesWithTag("architektur").length === 1)
                const html = JSON.parse(app.renderMarkdown(app.readTextFile(note), note))
                check("defekter Link markiert", html.broken === 1, html.broken)
            },
            () => {
                const found = JSON.parse(app.quickOpen("main", 5))
                check("Arbeitsleiste findet src/main.rs", found.length > 0 && found[0].relative === "src/main.rs", JSON.stringify(found.slice(0, 2)))
                editor.open(path("src/main.rs"), 7, "split")
            },
            () => {
                const code = editor.activeTab.viewerItem
                code.showSymbols = true
                check("Symbole erkannt", code.symbolList.length >= 3, code.symbolList.length)
                searchPanel.setQuery("EINZIGARTIG")
            },
            () => {
                check("Suche ohne Build-Ordner", searchModel.count === 1, searchModel.count)
                window.openTerminal(path("src"), "tab")
            },
            () => {
                const pane = editor.activeTab.viewerItem
                check("Terminal-Tab gestartet", pane && pane.terminalView.running)
                check("Terminal im gewählten Ordner", pane.cwd() === path("src"), pane.cwd())
                pane.terminalView.sendText("echo atlas-$((6*7))\r")
            },
            () => {
                const text = editor.activeTab.viewerItem.outputText()
                check("Terminal-Ausgabe lesbar", text.indexOf("atlas-42") >= 0, text)
                const block = app.codeBlock(text, "console")
                check("Ausgabe in Notiz", app.appendToNote(path("notes/log.md"), block) === "" && app.readTextFile(path("notes/log.md")).indexOf("```console") === 0)
                const ops = fileActions.ops
                check("Rechte setzen", ops.setMode(path("src/main.rs"), 0o640) && JSON.parse(ops.properties(path("src/main.rs"))).modeText === "rw-r-----")
            },
            () => {
                window.saveSession()
                app.requestSaveAll()
                app.flushSession()
                const layout = JSON.parse(app.loadLayout() || "{}")
                check("Session enthält Tabs", !!layout.editor && layout.editor.groups.length >= 1, JSON.stringify(layout).slice(0, 120))
                // ---- Phase 2: PDF annotations and quotes
                test.pdfBefore = app.readTextFile(pdf)
                editor.open(pdf, 0, "tab")
            },
            () => {
                const v = editor.activeTab.viewerItem
                check("PDF-Ansicht", !!v && !!v.annotationStore, editor.activeTab.kind)
                check("Anmerkungen geladen", v.annotationStore.status === "new", v.annotationStore.status)
                // Drag over both lines of page 1.
                v.dragStart = { page: 0, x: 60, y: 120 }
                v.dragEnd = { page: 0, x: 400, y: 176 }
                v.requestSelection()
            },
            () => {
                const v = editor.activeTab.viewerItem
                check("Text ausgewählt", v.selectionText.indexOf("Zitat aus der Quelle") >= 0 && v.selectionText.indexOf("Zweite Zeile") >= 0, v.selectionText)
                test.annotId = v.markSelection("highlight", "#ffd400")
                check("Markierung angelegt", test.annotId !== "" && v.annotationStore.count === 1, v.annotationStore.count)
                v.annotationStore.update(test.annotId, JSON.stringify({ comment: "Wichtig" }))
                v.toggleBookmark()
                v.annotationStore.add(JSON.stringify({ kind: "ink", page: 2, ink: [[[50, 50], [120, 90], [200, 60]]], color: "#ff6b6b", width: 2 }))
                check("Lesezeichen und Freihand", v.annotationStore.count === 3, v.annotationStore.count)
                const quote = v.annotationStore.quoteMarkdown(test.annotId, path("notes/zitate.md"))
                check("Zitat mit Quelle und Seite", quote.indexOf("> Zitat aus der Quelle Zweite Zeile hier") === 0
                      && quote.indexOf("[quelle.pdf, S. 1](/docs/quelle.pdf#page=1&annot=" + test.annotId + ")") > 0 && quote.indexOf("Wichtig") > 0, quote)
                check("Zitat in Notiz", app.appendToNote(path("notes/zitate.md"), quote) === "")
                app.flushSession()
                const found = JSON.parse(app.annotationSearch("quelle zweite", 5))
                check("@-Suche findet Markierung", found.length === 1 && found[0].id === test.annotId, JSON.stringify(found))
                const html = JSON.parse(app.renderMarkdown(app.readTextFile(path("notes/zitate.md")), path("notes/zitate.md"))).html
                check("Zitat-Link mit Annotation", html.indexOf("data-atlas-annot=\"" + test.annotId + "\"") > 0, html.slice(0, 300))
                v.goToPage(1)
            },
            () => {
                // Following the quote link jumps back to the marked place.
                editor.open(pdf, 1, "tab", { annot: test.annotId })
            },
            () => {
                const v = editor.activeTab.viewerItem
                check("Zitat öffnet die Stelle", v.focusedAnnot === test.annotId && v.currentPage === 0, v.focusedAnnot + " / " + v.currentPage)
                v.annotationStore.exportCopy(path("docs/quelle (annotiert).pdf"))
            },
            () => {
                check("Export als Kopie", app.exists(path("docs/quelle (annotiert).pdf")))
                check("Original-PDF unverändert", app.readTextFile(pdf) === test.pdfBefore)
                fileActions.ops.rename(pdf, "umbenannt.pdf")
            },
            () => {
                app.flushSession()
                const text = app.readTextFile(path("notes/zitate.md"))
                check("Zitat-Link folgt Umbenennung", text.indexOf("(/docs/umbenannt.pdf#page=1&annot=" + test.annotId + ")") > 0, text)
                const found = JSON.parse(app.annotationSearch("", 10))
                check("Anmerkungen folgen der PDF", found.length === 3 && found.every(a => a.relative === "docs/umbenannt.pdf"), JSON.stringify(found))
            },
        ]
        timer.start()
    }

    Timer {
        id: timer
        interval: 1500
        repeat: true
        onTriggered: {
            if (test.step < test.steps.length) {
                try {
                    test.steps[test.step]()
                } catch (e) {
                    test.check("Schritt " + (test.step + 1), false, e)
                }
                test.step++
                return
            }
            stop()
            console.log("selftest:", test.failures === 0 ? "BESTANDEN" : test.failures + " FEHLER")
            Qt.exit(test.failures === 0 ? 0 : 1)
        }
    }
}
