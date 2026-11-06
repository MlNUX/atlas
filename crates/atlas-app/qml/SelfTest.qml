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
    property var sendToNote
    property var sessionMenu
    property var folderDialog
    property var relateDialog
    property var settingsDialog
    property var projectTree
    property var quickLook
    property var treeModel
    property var inspector
    property var placesList

    property int failures: 0
    property int step: 0
    property var steps: []
    property string annotId
    property string pdfBefore
    property string memPdf
    property int memBase
    property string calUid
    property var term: null
    property var moved: null
    property string before
    property string firstRoot
    property string firstSession

    property string layoutBefore
    property real zoomBefore
    property int tabsBefore
    property string htmlProbe
    property string mdPdf
    property string modEvent
    property string modTask
    property string modTomorrow
    property bool inspectorWasVisible
    property int repaints
    property real flingY
    property real flingDirect
    property int flings
    property var erase: []
    property int syncMid
    property var iliasOrga: null
    property string iliasRoot: ""
    property string goneFile: ""
    property bool goneOpened: false
    property var fullscreenTab: null
    property var folderTab: null
    property var imageView: null
    property var bnkiView: null
    property var bnkiPage: null
    property var jupyterPage: null
    property var extensionWindow
    property string docsCache: ""
    property int docsTabs: 0
    property bool docsReady: false
    property var docsSearch: null
    property var docsExamples: null
    property var docsDark: null
    property var docsDarkAfter: null
    property var previewBg: null
    property var sheetHeader: null
    property var zoomPoint: null
    property var zoomAnchor: null
    /// Widths of the editor area and its splits, for messages.
    function layoutText() { return "Bereich " + Math.round(editor.width) + ": " + editor.groups.map(g => Math.round(g.width)).join(" + ") }
    /// The splits (with the gaps between them) cover the editor area.
    function filled() {
        const sum = editor.groups.reduce((n, g) => n + g.width, 0) + (editor.groups.length - 1) * 8
        return editor.width > 100 && Math.abs(sum - editor.width) <= 2
    }

    /// "a.md,b.md | c.md" – tabs per split, for messages.
    function names() { return editor.groups.map(g => g.tabs.map(t => t.title).join(",")).join(" | ") }

    // Real drag-and-drop: mouse events through the window system, one
    // every 40 ms.
    function dragPoints(from, to) {
        const pts = [[0, from.x, from.y]]
        for (let i = 1; i <= 10; i++) pts.push([1, from.x + (to.x - from.x) * i / 10, from.y + (to.y - from.y) * i / 10])
        pts.push([1, to.x, to.y], [2, to.x, to.y])
        pts.forEach((e, i) => TestDriver.postMouse(window, e[0], e[1], e[2], 40 * (i + 1)))
    }
    /// Drags tab `index` of `group` to (`x`, `y`) in `target`.
    function dragTabTo(group, index, target, x, y) {
        const item = group.tabItemAt(index)
        if (!item) { check("Tab " + index + " sichtbar", false, names()); return }
        dragPoints(item.mapToItem(null, item.width / 2, item.height / 2), target.mapToItem(null, x, y))
    }
    function dragTab(group, index, target, x, y) { dragTabTo(group, index, target, x, y) }

    /// Background work (autosave, storing a password, watched files): a
    /// step starting with `if (waitFor(cond)) return` runs again on the next
    /// tick until `cond()` holds, at most 8 times; then it checks anyway.
    property bool retry: false
    property int attempts: 0
    function waitFor(cond) {
        if (cond() || attempts >= 8) return false
        retry = true
        return true
    }

    function check(name, ok, detail) {
        if (!ok) failures++
        console.log("selftest:", ok ? "ok  " : "FAIL", name, ok ? "" : "– " + (detail === undefined ? "" : detail))
    }
    function path(rel) { return app.projectRoot + "/" + rel }
    /// In the presentation only page `index` of viewer `v` is to be seen
    /// (completely, its neighbours not at all).
    function slideAlone(v, index) {
        const list = v.pagesView
        const top = list.contentY, bottom = list.contentY + list.height
        const inside = (i) => {
            const it = list.itemAtIndex(i)
            return it && it.y < bottom - 1 && it.y + it.height > top + 1
        }
        const it = list.itemAtIndex(index)
        return !!it && it.y >= top - 1 && it.y + it.height <= bottom + 1 && !inside(index - 1) && !inside(index + 1)
    }
    /// The keyboard focus is in `item` (or one of its children).
    function hasFocusIn(item) {
        for (let p = window.activeFocusItem; p; p = p.parent) if (p === item) return true
        return false
    }

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
                check("Markdown-Hervorhebung mit Codeblöcken (Atlas Markdown)", !!md && md.editorItem.syntax === "Atlas Markdown",
                      md && md.editorItem.syntax)
                md.mode = "split"
                md.editorItem.insertText("\nAutosave-Test [Plan](plan.md#ziele)\n", true)
                check("Bearbeitung erkannt", md.editorItem.modified)
            },
            () => {
                if (waitFor(() => app.readTextFile(note).indexOf("Autosave-Test") >= 0)) return
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
                check("Suche im eigenen Fenster vor Atlas", searchPanel.visible && searchPanel.opened, searchPanel.visible)
                const e = JSON.parse(searchModel.entry(0))
                check("Suchtreffer markiert", e.ranges && e.ranges.length === 1 && e.text.slice(e.ranges[0][0], e.ranges[0][1]) === "EINZIGARTIG", JSON.stringify(e))
                searchPanel.applyState({ query: "einzigartig" })
            },
            () => {
                check("Suche ignoriert Groß-/Kleinschreibung", searchModel.count === 1, searchModel.count)
                searchPanel.applyState({ query: "einzigartig", caseSensitive: true })
            },
            () => {
                check("Suche beachtet Groß-/Kleinschreibung auf Wunsch", searchModel.count === 0 && !searchModel.running, searchModel.count)
                searchPanel.applyState({ query: "EINZIGART", wholeWord: true })
            },
            () => {
                check("Suche nach ganzen Wörtern", searchModel.count === 0 && !searchModel.running, searchModel.count)
                searchPanel.applyState({ query: "EINZIGARTIG", kinds: ["markdown"] })
            },
            () => {
                check("Suche: Typfilter Markdown", searchModel.count === 0 && !searchModel.running, searchModel.count)
                searchPanel.applyState({ query: "zweite zeile", kinds: ["pdf"] })
            },
            () => {
                const e = JSON.parse(searchModel.entry(0))
                check("Suche findet Text in PDFs", searchModel.count >= 1 && e.pdf === true && e.relative === "docs/quelle.pdf" && e.line === 1,
                      searchModel.count + " " + JSON.stringify(e))
                const page = JSON.parse(searchModel.context(e.path || "", e.line || 1, 6, true))
                check("Suche: Vorschau der PDF-Seite", page.some(l => l.text.indexOf("Zweite Zeile") >= 0), JSON.stringify(page))
                searchPanel.openRow(0, false)
            },
            () => {
                const v = editor.activeTab.viewerItem
                check("Suchtreffer öffnet als fester Tab im aktiven Split", !editor.activeTab.preview && !editor.activeGroup.temporary,
                      editor.activeTab.preview + " " + editor.activeGroup.temporary)
                check("PDF-Treffer öffnet die Seite und markiert das Wort", editor.activeTab.path === path("docs/quelle.pdf")
                      && v.searchQuery === "Zweite Zeile" && v.hits.length >= 1 && !searchPanel.visible,
                      editor.activeTab.path + " " + (v && v.searchQuery) + " " + (v && v.hits.length))
                searchPanel.applyState({ query: "ziele", kinds: ["markdown"] })
            },
            () => {
                let row = -1
                for (let i = 0; i < searchModel.count; ++i) if (JSON.parse(searchModel.entry(i)).relative === "notes/plan.md") row = i
                check("Suche: Treffer in plan.md", row >= 0, searchModel.count)
                searchPanel.openRow(Math.max(0, row), false)
            },
            () => {
                const md = editor.activeTab.viewerItem
                check("Markdown-Treffer im Editor ausgewählt", editor.activeTab.path === path("notes/plan.md") && md.editorItem.selectedText === "Ziele",
                      editor.activeTab.path + " „" + (md && md.editorItem.selectedText) + "“")
                md.previewItem.runJavaScript("document.querySelectorAll('mark.search-hit.current').length + ':' + (document.querySelector('mark.search-hit') || {}).textContent",
                    (r) => check("Markdown-Treffer in der Vorschau markiert", r === "1:Ziele", r))
                // Spreadsheets: the cells are searched (here in a hidden second sheet).
                searchPanel.applyState({ query: "zweiten blatt", kinds: ["sheet"] })
            },
            () => {
                if (waitFor(() => !searchModel.running && searchModel.count > 0)) return
                const e = JSON.parse(searchModel.entry(0))
                check("Suche findet Text in Tabellen (mit Blatt und Zelle)", searchModel.count === 1 && e.relative === "tabellen/Mappe.xlsx"
                      && !!e.sheet && e.sheet.name === "Zweites" && e.sheet.address === "C3" && e.line === 3, searchModel.count + " " + JSON.stringify(e))
                const ctx = JSON.parse(searchModel.sheetContext(e.path || "", 1, 2))
                check("Suche: Vorschau der Tabellenzeile", ctx.length === 1 && ctx[0].text === "C3  Gefunden im zweiten Blatt", JSON.stringify(ctx))
                searchPanel.openRow(0, false)
            },
            () => {
                const v = editor.activeTab.viewerItem
                if (waitFor(() => !!v && v.model && v.model.revision > 0 && !v.model.loading && v.model.sheet === 1)) return
                check("Tabellen-Treffer öffnet das Blatt an der Zelle", editor.activeTab.kind === "sheet" && v.model.sheet === 1
                      && v.currentRow === 2 && v.currentColumn === 2 && v.model.textAt(2, 2) === "Gefunden im zweiten Blatt",
                      editor.activeTab.kind + " " + (v && [v.model.sheet, v.currentRow, v.currentColumn]))
                check("Tabelle: Blätter als Reiter, ausgeblendetes markiert", v.sheetList.length === 2 && v.sheetList[0].name === "Daten"
                      && !v.sheetList[0].hidden && v.sheetList[1].hidden, JSON.stringify(v.sheetList))
                v.model.sheet = 0
            },
            () => {
                const v = editor.activeTab.viewerItem
                if (waitFor(() => v.model.sheet === 0 && !v.model.loading && v.model.textAt(1, 1) !== "")) return
                const m = v.model
                check("Tabelle: Zahlen, Prozent, Datum, Währung wie in Excel", [1, 2, 3, 7].map(c => m.textAt(1, c)).join("|") === "1.234,50|25%|15.03.2023|-19,90 €",
                      [1, 2, 3, 7].map(c => m.textAt(1, c)).join("|"))
                check("Tabelle: Formel zum Wert", m.formulaAt(1, 4) === "B2*2" && m.textAt(1, 4) === "2469" && m.formulaAt(1, 1) === "",
                      m.formulaAt(1, 4) + " " + m.textAt(1, 4))
                check("Tabelle: Spalten- und Zellnamen", m.columnName(27) === "AB" && m.cellName(1, 4) === "E2")
                check("Tabelle: Auswahl als Text mit Tabulatoren", m.copyText(0, 0, 1, 2) === "Name\tWert\t\nAlpha\t1.234,50\t25%", JSON.stringify(m.copyText(0, 0, 1, 2)))
                const hit = JSON.parse(m.find("wahr", 0, 0, false))
                check("Tabelle: Suchen im Blatt", hit.row === 1 && hit.column === 5 && hit.count === 1, JSON.stringify(hit))
                // The header: bold, red on yellow.
                test.sheetHeader = [m.data(m.index(0, 0), 0x0102), m.data(m.index(0, 0), 0x0106), m.data(m.index(0, 0), 0x0108)]
                check("Tabelle: Formatierung (fett, Schrift- und Füllfarbe)", test.sheetHeader[0] === true && test.sheetHeader[1] === "#c00000"
                      && test.sheetHeader[2] === "#ffff00", JSON.stringify(test.sheetHeader))
                searchPanel.applyState({})
                // A CSV file: text, shown as a table on request.
                check("CSV-Aktion im Kontextmenü", (() => {
                    fileActions.showMenu([path("tabellen/noten.csv")], path("tabellen"))
                    const e = fileActions.contextMenu.shownEntries()
                    fileActions.contextMenu.close()
                    return e.indexOf("Als Tabelle anzeigen") >= 0
                })())
                fileActions.showAsTable(path("tabellen/noten.csv"))
            },
            () => {
                const v = editor.activeTab.viewerItem
                if (waitFor(() => !!v && !!v.model && v.model.revision > 0 && !v.model.loading)) return
                const m = v.model
                check("CSV als Tabelle (Trennzeichen erkannt)", editor.activeTab.kind === "text" && m.info === "Trennzeichen „;“ · UTF-8"
                      && m.textAt(1, 0) === "Müller" && m.textAt(2, 2) === "mit ; Semikolon" && m.textAt(0, 1) === "Note",
                      m.info + " " + m.textAt(1, 0) + " " + m.textAt(2, 2))
                check("CSV: Zahlen rechtsbündig, wie geschrieben", m.textAt(1, 1) === "1,7" && m.data(m.index(1, 1), 0x0109) === 2
                      && m.data(m.index(1, 0), 0x0109) === 0, m.textAt(1, 1))
                check("CSV: Tabellenansicht in der Sitzung", editor.activeTab.viewState().processor === "atlas:table", JSON.stringify(editor.activeTab.viewState()))
                test.tabsBefore = editor.groups.reduce((n, g) => n + g.tabs.length, 0)
                check("CSV geändert", app.appendToNote(path("tabellen/noten.csv"), "Neu;3,0;spät") === "")
            },
            () => {
                const m = editor.activeTab.viewerItem.model
                const found = () => { for (let r = 0; r < 8; ++r) if (m.textAt(r, 0) === "Neu") return r; return -1 }
                if (waitFor(() => found() > 0)) return
                check("CSV-Tabelle lädt beim Speichern neu", found() > 0 && m.textAt(found(), 1) === "3,0", found())
                editor.activeTab.viewerItem.showSourceRequested()
            },
            () => {
                const tab = editor.activeTab
                const tabs = editor.groups.reduce((n, g) => n + g.tabs.length, 0)
                check("CSV: zurück zum Text im selben Tab", tab.path === path("tabellen/noten.csv") && tabs === test.tabsBefore
                      && !tab.viewerItem.model && tab.viewerItem.text !== undefined && tab.viewerItem.text.indexOf("Müller;1,7") >= 0,
                      tab.kind + " " + tabs + "/" + test.tabsBefore)
                // Zip archives: "Hier entpacken" in the context menu.
                const entries = (paths) => {
                    fileActions.showMenu(paths, path("tabellen"))
                    const e = fileActions.contextMenu.shownEntries()
                    fileActions.contextMenu.close()
                    return e
                }
                check("Zip: „Hier entpacken“ im Kontextmenü", entries([path("tabellen/archiv.zip")]).indexOf("Hier entpacken") >= 0
                      && entries([path("tabellen/noten.csv")]).indexOf("Hier entpacken") < 0)
                fileActions.extractHere([path("tabellen/archiv.zip")])
            },
            () => {
                if (waitFor(() => !fileActions.busy && app.exists(path("tabellen/archiv/xl/workbook.xml")))) return
                check("Zip entpackt in einen Ordner mit seinem Namen", app.exists(path("tabellen/archiv/xl/worksheets/sheet2.xml"))
                      && app.exists(path("tabellen/archiv/[Content_Types].xml")) && app.exists(path("tabellen/archiv.zip")),
                      fileActions.message)
                check("Zip: Meldung nennt Ziel und Anzahl", fileActions.message.indexOf("entpackt nach „archiv“ (8 Dateien)") >= 0, fileActions.message)
                // Again: nothing is overwritten.
                fileActions.extractHere([path("tabellen/archiv.zip")])
            },
            () => {
                if (waitFor(() => !fileActions.busy && app.exists(path("tabellen/archiv (Kopie)/xl/workbook.xml")))) return
                check("Zip: zweites Entpacken überschreibt nichts", app.exists(path("tabellen/archiv (Kopie)/xl/workbook.xml")), fileActions.message)
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
                // A click into the split (not on a page) gives the PDF the keys.
                projectTree.forceActiveFocus()
                editor.activeGroup.focusAfterTap()
                let inPdf = false
                for (let p = window.activeFocusItem; p; p = p.parent) if (p === v) inPdf = true
                check("Klick in den PDF-Split: Pfeiltasten ohne Klick auf die Seite", inPdf, window.activeFocusItem)
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
                // ---- Phase 2: wiki links, backlink filters, context graph
                const wiki = path("notes/wiki.md")
                const html = JSON.parse(app.renderMarkdown(app.readTextFile(wiki), wiki)).html
                check("Wiki-Link mit Alias", html.indexOf("class=\"wikilink\"") > 0 && html.indexOf(">Plan</a>") > 0, html)
                check("Wiki-Link auf Überschrift", html.indexOf("data-atlas-anchor=\"einleitung\"") > 0, html)
                const back = JSON.parse(app.backlinks(path("notes/plan.md")))
                const fromWiki = back.find(b => b.relative === "notes/wiki.md")
                check("Rückverweis kennt Art, Kontext und Tags", !!fromWiki && fromWiki.kind === "wiki"
                      && fromWiki.context.indexOf("Siehe") === 0 && fromWiki.tags[0] === "wiki", JSON.stringify(back))
                const graph = JSON.parse(app.contextGraph(path("docs/umbenannt.pdf"), 1))
                const zitate = graph.nodes.findIndex(n => n.relative === "notes/zitate.md")
                check("Graph der PDF zeigt zitierende Notiz", zitate > 0 && graph.edges.some(e => e.from === zitate && e.to === 0 && e.quotes === 1), JSON.stringify(graph))
                // The graph shows the whole project, this note first.
                const all = JSON.parse(app.contextGraph(wiki, 0))
                const depthOf = (rel) => { const n = all.nodes.find(n => n.relative === rel); return n ? n.depth : -1 }
                check("Graph zeigt das ganze Projekt", all.nodes[0].relative === "notes/wiki.md" && depthOf("notes/plan.md") === 1
                      && depthOf("notes/zitate.md") >= 1 && depthOf("docs/umbenannt.pdf") >= 1 && depthOf("notes/aufgaben.md") >= 1
                      && depthOf("src/hallo.sh") === 255 && depthOf("docs/seite.css") === 255 && !all.nodes.some(n => /(^|\/)\./.test(n.relative)),
                      JSON.stringify(all.nodes.map(n => n.relative + ":" + n.depth)))
                editor.open(wiki, 0, "tab", { backlinks: true, contextTab: 1 })
                fileActions.ops.rename(path("notes/plan.md"), "plan-neu.md")
            },
            () => {
                const text = app.readTextFile(path("notes/wiki.md"))
                check("Wiki-Link folgt Umbenennung", text.indexOf("[[/notes/plan-neu|Plan]]") > 0, text)
                const md = editor.activeTab.viewerItem
                check("Kontextpanel mit Graph", !!md && md.showBacklinks && md.viewState().contextTab === 1, JSON.stringify(md ? md.viewState() : {}))
                // Plain wheel: no zoom.
                const area = md.contextPanelItem.graphView
                const p = area.mapToItem(null, area.width / 2, area.height / 2)
                test.zoomBefore = md.contextPanelItem.zoom
                TestDriver.sendWheel(window, p.x, p.y, 120, 0)
            },
            () => {
                const panel = editor.activeTab.viewerItem.contextPanelItem
                check("Graph: Mausrad ohne Strg zoomt nicht", panel.zoom === test.zoomBefore, panel.zoom)
                // Ctrl+wheel, as Wayland reports a mouse wheel ("touchpad").
                const area = panel.graphView
                const p = area.mapToItem(null, area.width / 2, area.height / 2)
                TestDriver.sendWheel(window, p.x, p.y, 120, 17)
            },
            () => {
                const md = editor.activeTab.viewerItem
                check("Graph: Strg+Mausrad zoomt", md.contextPanelItem.zoom > test.zoomBefore * 1.1, test.zoomBefore + " → " + md.contextPanelItem.zoom)
                md.mode = "split"
            },
            () => {
                // `[[` suggests project files.
                const ed = editor.activeTab.viewerItem.editorItem
                ed.insertText("\nNeu: [[plan", true)
                ed.applyState({ cursor: ed.text.length })
                ed.focusText()
                ed.updateCompletion()
                check("Wiki-Link-Vorschläge", ed.completing)
                ed.acceptCompletion()
                check("Wiki-Link vervollständigt", ed.text.indexOf("Neu: [[/notes/plan-neu.md]]") > 0, ed.text.slice(-60))
                // Markdown: line numbers also for wrapped text, Tab indents a list item.
                check("Markdown mit Zeilennummern", ed.showLineNumbers && ed.wrappedNumbers.length > 0 && ed.wrappedNumbers[0].n >= 1,
                      JSON.stringify(ed.wrappedNumbers.slice(0, 3)))
                ed.insertText("\n- Punkt", true)
                ed.applyState({ cursor: ed.text.length })
                const indented = ed.indentListItems(false) && ed.text.endsWith("\n  - Punkt")
                const outdented = ed.indentListItems(true) && ed.text.endsWith("\n- Punkt")
                ed.insertText("\nText", true)
                ed.applyState({ cursor: ed.text.length })
                check("Tab rückt Listenpunkte ein und aus, nicht normalen Text", indented && outdented && !ed.indentListItems(false), ed.text.slice(-30))
                // Enter continues lists (bullets, checkboxes, numbers, quotes)
                // and ends them on an empty item.
                const enter = () => TestDriver.sendKey(window, Qt.Key_Return, 0)
                ed.focusText()
                ed.insertText("\n- eins", true)
                ed.applyState({ cursor: ed.text.length })
                enter()
                ed.insertText("zwei", true)
                ed.applyState({ cursor: ed.text.length })
                enter()
                enter()
                check("Enter: neuer Spiegelstrich, leerer Punkt beendet die Liste", ed.text.endsWith("\n- eins\n- zwei\n"), JSON.stringify(ed.text.slice(-30)))
                ed.insertText("- [x] erledigt", true)
                ed.applyState({ cursor: ed.text.length })
                enter()
                check("Enter nach Checkbox: neue offene Checkbox", ed.text.endsWith("- [x] erledigt\n- [ ] "), JSON.stringify(ed.text.slice(-30)))
                enter()
                ed.insertText("9. neun", true)
                ed.applyState({ cursor: ed.text.length })
                enter()
                check("Enter in nummerierter Liste zählt weiter", ed.text.endsWith("9. neun\n10. "), JSON.stringify(ed.text.slice(-20)))
                enter()
                ed.insertText("Text", true)
                ed.applyState({ cursor: ed.text.length })
                enter()
                check("Enter nach normalem Text: nur Zeilenumbruch", ed.text.endsWith("Text\n"), JSON.stringify(ed.text.slice(-10)))
                // Inserting: table, diagram, folder tree, files; `#` suggests tags.
                const mdv = editor.activeTab.viewerItem
                ed.applyState({ cursor: ed.text.length })
                mdv.insertTable(2, 2)
                check("Tabelle einfügen (ausgerichtet)", ed.text.indexOf("| Spalte 1 | Spalte 2 |\n| -------- | -------- |\n|          |          |\n|          |          |") > 0, JSON.stringify(ed.text.slice(-120)))
                // Tables: Tab aligns and goes from cell to cell, Enter adds a row,
                // Enter in an empty last row ends the table.
                const tab = (shift) => TestDriver.sendKey(window, shift ? Qt.Key_Backtab : Qt.Key_Tab, shift ? Qt.ShiftModifier : 0)
                ed.insertText("\n\n| a | b |\n|:-:|--:|\n| langer Text | 1 |", true)
                const tableAt = ed.text.lastIndexOf("| a | b |")
                ed.applyState({ cursor: tableAt + 2 })
                tab(false)
                const aligned = "|      a      |   b |\n| :---------: | --: |\n| langer Text |   1 |"
                check("Tabelle: Tab richtet die Spalten aus (mit Ausrichtung)", ed.text.endsWith(aligned), JSON.stringify(ed.text.slice(-70)))
                check("Tabelle: Tab markiert die nächste Zelle", ed.selectedText === "b", JSON.stringify(ed.selectedText))
                tab(false)
                check("Tabelle: Tab springt über die Trennzeile", ed.selectedText === "langer Text", JSON.stringify(ed.selectedText))
                tab(true)
                check("Tabelle: Umschalt+Tab geht zurück", ed.selectedText === "b", JSON.stringify(ed.selectedText))
                tab(false); tab(false); tab(false)
                check("Tabelle: Tab in der letzten Zelle fügt eine Zeile an", ed.text.endsWith(aligned + "\n|             |     |"), JSON.stringify(ed.text.slice(-50)))
                ed.insertText("x", false)
                ed.applyState({ cursor: ed.text.length })
                enter()
                check("Tabelle: Enter am Zeilenende fügt eine Zeile an",
                      ed.text.endsWith("|      x      |     |\n|             |     |"), JSON.stringify(ed.text.slice(-50)))
                ed.applyState({ cursor: ed.text.length })
                enter()
                check("Tabelle: Enter in leerer letzter Zeile beendet die Tabelle",
                      ed.text.endsWith("|      x      |     |\n") && !ed.text.endsWith("|     |\n|             |     |\n"), JSON.stringify(ed.text.slice(-50)))
                ed.insertText("\n| c | d |\n|---|---|\n| e | ffff |\n\n```\n| x | y |\n|---|---|\n```\n", true)
                check("Tabellen ausrichten: ganze Notiz", ed.alignTables() === 1
                      && ed.text.indexOf("| c   | d    |\n| --- | ---- |\n| e   | ffff |") > 0
                      && ed.text.indexOf("```\n| x | y |\n|---|---|\n```") > 0, JSON.stringify(ed.text.slice(-120)))
                ed.applyState({ cursor: ed.text.length })
                mdv.insertDiagram(0)
                check("Diagramm-Vorlage einfügen", ed.text.indexOf("```mermaid\nflowchart TD") > 0)
                ed.applyState({ cursor: ed.text.length })
                mdv.insertFolderTree(path("notes"))
                check("Ordnerbaum einfügen", ed.text.indexOf("```tree\n" + path("notes") + "\n├── ") > 0, JSON.stringify(ed.text.slice(-120)))
                const linked = app.importIntoNote(ed.source, JSON.stringify(["file://" + path("src/main.rs")]))
                check("Datei aus dem Projekt einfügen: Link relativ zur Notiz", /^\[main\.rs\]\((\.\.\/)*src\/main\.rs\)$/.test(linked), linked)
                app.copyToClipboard("nur Text")
                check("Strg+V mit Text: normales Einfügen", app.pasteIntoNote(ed.source) === "")
                ed.applyState({ cursor: ed.text.length })
                ed.insertText("\n```bash\necho hallo\n```\n\nLernen für #wi", true)
                ed.applyState({ cursor: ed.text.length })
                ed.focusText()
                ed.updateCompletion()
                check("# schlägt Tags des Projekts vor", ed.completing && ed.completionKind === "tag"
                      && ed.completionResults.some(r => r.tag === "wiki"), JSON.stringify(ed.completionResults))
                ed.acceptCompletion()
                check("Tag vervollständigt", ed.text.endsWith("Lernen für #wiki"), JSON.stringify(ed.text.slice(-20)))
                // Spellcheck (German and English): typo found, suggestion, own dictionary.
                ed.insertText("\n\nDas ist ein Tippfeler, the lecture is online.", true)
                ed.updateSpelling()
                const typo = ed.text.lastIndexOf("Tippfeler")
                const flagged = () => ed.spellRanges.some(r => r[0] === typo && r[1] === 9)
                check("Rechtschreibprüfung: Tippfehler unterstrichen, richtige Wörter nicht", Spelling.languages() === "de_DE, en_US" && flagged()
                      && !ed.spellRanges.some(r => r[0] > typo + 9), Spelling.languages() + " " + JSON.stringify(ed.spellRanges.slice(-3)))
                check("Rechtschreibprüfung: Vorschlag", Spelling.suggest("Tippfeler").indexOf("Tippfehler") >= 0, Spelling.suggest("Tippfeler"))
                Spelling.add("Tippfeler")
                ed.updateSpelling()
                check("Rechtschreibprüfung: eigenes Wörterbuch", !flagged(), JSON.stringify(ed.spellRanges.slice(-3)))
                // Ranges of an older, longer text are not drawn (no positions
                // past the end: "QTextCursor::setPosition: … out of range").
                ed.spellRanges = [[ed.text.length + 10, 5]]
                ed.updateSpellMarks()
                check("Rechtschreibprüfung: alte Stellen hinter dem Textende ignoriert", ed.spellMarks.length === 0, JSON.stringify(ed.spellMarks))
                ed.updateSpelling()
                // A picture (cut out of a PDF) for the preview's picture window.
                ed.insertText("\n\n" + app.pdfAreaToNote(path("docs/umbenannt.pdf"), 1, JSON.stringify([0, 0, 300, 200]), ed.source), true)
                ed.insertText("\n\n## Abschnitt\n\n> Zitat", true)
                // ---- Phase 2: user defined actions
                check("Aktionsdatei fehlerfrei", app.actionErrors() === "[]", app.actionErrors())
                const names = (paths) => fileActions.actionsFor(paths).map(a => a.name).join(",")
                check("Aktion passt zu Markdown", names([note]) === "Zeilen zählen", names([note]))
                check("Aktion passt zu Ordnern", names([path("notes")]) === "Ordner zeigen", names([path("notes")]))
                check("Projektaktion", names([]).split(",").indexOf("Liste schreiben") >= 0, names([]))
                const wc = JSON.parse(app.prepareAction("zeilen-zählen", [note], 0))
                check("Programm ohne Shell, Arbeitsordner Projekt", wc.argv.slice(-3).join(" ") === "wc -l " + note
                      && wc.cwd === app.projectRoot && !wc.confirm, JSON.stringify(wc))
                const list = JSON.parse(app.prepareAction("liste-schreiben", [], 0))
                check("Umleitung verlangt Bestätigung", list.confirm && list.destructive, JSON.stringify(list))
                fileActions.runInTerminal(list.argv, list.cwd)
            },
            () => {
                // Preview: tags in the text, tree blocks, buttons on code blocks,
                // folding under headings, closing quotation mark.
                const md = editor.activeTab.viewerItem
                md.render()
                md.previewItem.runJavaScript("(() => { const b = Array.from(document.querySelectorAll('.code-tools button')).map(x => x.textContent);"
                    + " const t = document.querySelector('pre.tree'); const q = document.querySelector('blockquote > p:last-child');"
                    + " const h = Array.from(document.querySelectorAll('h2')).pop(); const f = h && h.querySelector('.fold'); if (f) f.click();"
                    + " const hidden = document.querySelectorAll('.hidden-by-fold').length; if (f) f.click();"
                    + " return JSON.stringify({ buttons: b, tree: t ? t.textContent.slice(0, 80) : '', tag: !!document.querySelector('.tag.inline'),"
                    + " close: q ? getComputedStyle(q, '::after').content : '', hidden: hidden, after: document.querySelectorAll('.hidden-by-fold').length,"
                    + " mermaid: (document.querySelector('pre.mermaid') ? getComputedStyle(document.querySelector('pre.mermaid')).backgroundColor : '') }) })()", (r) => {
                    const o = JSON.parse(r)
                    check("Vorschau: Kopieren-Knopf an Codeblöcken, Ausführen bei bash", o.buttons.indexOf("Kopieren") >= 0 && o.buttons.indexOf("▶ Ausführen") >= 0, r)
                    check("Vorschau: Ordnerbaum mit Linien", o.tree.indexOf("├── ") > 0, r)
                    check("Vorschau: #tag im Text", o.tag, r)
                    check("Vorschau: Zitat mit schließendem Anführungszeichen", o.close.indexOf("”") >= 0, r)
                    check("Vorschau: Überschrift klappt ihren Abschnitt ein und aus", o.hidden > 0 && o.after === 0, r)
                    check("Vorschau: Diagramm auf dem Hintergrund der Seite", o.mermaid === "rgba(0, 0, 0, 0)", r)
                })
                md.previewItem.runJavaScript("(() => { const b = document.querySelector('.img-zoom'); if (b) b.click(); return !!b })()",
                    (found) => check("Vorschau: Bild mit Vergrößern-Knopf", found))
                test.imageView = md
            },
            () => {
                const w = test.imageView.imageWindowItem
                if (waitFor(() => w.visible && w.ready)) return
                check("Bild vergrößern öffnet eigenes Fenster mit dem Bild", w.visible && w.ready && w.caption.indexOf("S. 1") > 0,
                      [w.visible, w.ready, w.source, w.caption].join(" "))
                w.close()
            },
            () => {
                check("Aktion im Projektordner ausgeführt", app.readTextFile(path("aktion-ergebnis.txt")).indexOf("idee.md") >= 0)
                const out = terminalPanel.currentGroup.activeTerminal.outputText()
                check("Aktion sichtbar mit Status", out.indexOf("Liste schreiben beendet") >= 0, out)
                // ---- Phase 2: extensions
                const exts = JSON.parse(app.extensions())
                const status = (n) => (exts.find(e => e.name === n) || {}).status
                check("Erweiterungen gefunden", status("demo") === "active" && status("proj") === "untrusted", JSON.stringify(exts.map(e => e.name + ":" + e.status)))
                const core = exts.find(e => e.name === "core")
                check("Alte Beispiel-Erweiterung heißt jetzt Core", !!core && core.title === "Core" && !exts.some(e => e.name === "beispiel"),
                      JSON.stringify(exts.map(e => e.name + ":" + e.title)))
                check("Projekt-Erweiterung wartet auf Freigabe", app.untrustedExtensions.length === 1 && app.untrustedExtensions[0] === "proj")
                const projectActions = () => fileActions.actionsFor([]).map(a => a.name)
                check("Nicht freigegeben: keine Aktionen", projectActions().indexOf("Hallo") >= 0 && projectActions().indexOf("Projekt-Hallo") < 0, projectActions())
                check("Freigabe", app.trustExtension("proj") && app.untrustedExtensions.length === 0)
                check("Freigegeben: Aktionen da", projectActions().indexOf("Projekt-Hallo") >= 0, projectActions())
                app.setExtensionEnabled("demo", false)
                check("Ausgeschaltet: keine Aktionen", projectActions().indexOf("Hallo") < 0, projectActions())
                app.setExtensionEnabled("demo", true)
                // API 2: permissions, settings, key sequences, menus.
                const nb = JSON.parse(app.extensions()).find(e => e.name === "nb") || {}
                check("API 2: Rechte gelesen", nb.api === 2 && !!nb.permissions && nb.permissions.run.join() === "echo" && nb.permissions.files === "project",
                      JSON.stringify(nb.permissions))
                check("API 2: nicht angemeldetes Programm abgelehnt", (nb.errors || []).some(e => e.indexOf("curl") >= 0), JSON.stringify(nb.errors))
                const greet = fileActions.actionsFor([]).find(a => a.id === "nb/grüßen")
                check("API 2: Befehl nur in der Arbeitsleiste, mit Kürzel", !!greet && greet.palette && !greet.menu && greet.keybinding === "Ctrl+Alt+G", JSON.stringify(greet))
                check("API 2: Kürzel angemeldet", JSON.parse(app.keybindings()).some(k => k.sequence === "Ctrl+Alt+G" && k.id === "nb/grüßen"), app.keybindings())
                const greetArgv = () => JSON.parse(app.prepareAction("nb/grüßen", [], 0)).argv || []
                check("Einstellung: Standardwert als Platzhalter und Variable",
                      greetArgv().slice(-2).join(" ") === "echo hallo" && greetArgv().indexOf("ATLAS_SETTING_GRUSS=hallo") >= 0, greetArgv().join(" "))
                check("Einstellung ändern", app.setExtensionSetting("nb", "gruss", JSON.stringify("servus")) === "" && greetArgv().slice(-1)[0] === "servus", greetArgv().join(" "))
                check("Einstellung: falscher Typ abgelehnt", app.setExtensionSetting("nb", "gruss", "5") !== "")
                app.setExtensionSetting("nb", "gruss", "null")
                check("Einstellung zurückgesetzt", greetArgv().slice(-1)[0] === "hallo", greetArgv().join(" "))
                editor.open(path("daten.dat"), 0, "tab")
                const exporter = fileActions.exportersFor([note])[0]
                check("Exporter für Markdown", !!exporter && exporter.output === path("notes/idee.txt"), JSON.stringify(exporter))
                if (exporter) app.exportFile(exporter.id, note, exporter.output)
            },
            () => {
                const tab = editor.groups.reduce((all, g) => all.concat(g.tabs), []).find(t => t.path === path("daten.dat"))
                check("Unbekannter Typ über Erweiterung angezeigt", tab.processed && tab.viewerItem.kind === "markdown"
                      && tab.viewerItem.content.indexOf("# Daten") === 0 && tab.viewerItem.content.indexOf("HALLO") > 0,
                      tab ? tab.kind + " " + tab.processed + " " + tab.viewerItem : "kein Tab")
                check("Export geöffnet", !!editor.viewerFor(path("notes/idee.txt")))
                check("Export geschrieben", app.readTextFile(path("notes/idee.txt")).indexOf("Autosave-Test") >= 0)
                // ---- Extension views: the bnki plugin in a tab
                check("bnki-Karten installiert", app.installBundled("bnki") === "")
                check("bnki: Ansicht mit Strg+Alt+B", JSON.parse(app.keybindings()).some(k => k.sequence === "Ctrl+Alt+B" && k.id === "view:bnki/editor"), app.keybindings())
                settingsDialog.reloadExtensions()
                check("Einstellungsdialog zeigt die Einstellungen von bnki-Karten", settingsDialog.configurable.some(e => e.name === "bnki"),
                      JSON.stringify(settingsDialog.configurable.map(e => e.name)))
                window.openExtensionView("view:bnki/editor", "")
            },
            () => {
                const v = editor.activeTab ? editor.activeTab.viewerItem : null
                if (waitFor(() => !!v && v.started === true)) return
                check("bnki: Tab mit der Seite der Erweiterung", !!v && v.viewId === "view:bnki/editor" && editor.activeTab.title === "bnki-Karten" && v.started === true,
                      editor.activeTab ? editor.activeTab.title + " " + (v ? v.started + " " + JSON.stringify(v.pageErrors) : "") : "kein Tab")
                check("bnki: Ordner ohne Projekt-Ordner bleibt unangetastet", !app.exists(path("bnki")))
                test.bnkiView = v
                if (v) v.webItem.runJavaScript("createDeck('Biologie').then(() => { $('front').value = 'Was ist $E=mc^2$?'; $('back').value = 'Energie'; $('tags').value = 'physik';"
                    + " $('cardForm').requestSubmit(); return true })")
            },
            () => {
                const csv = app.readTextFile(path("bnki/Biologie.csv"))
                if (waitFor(() => csv.indexOf("Energie") > 0)) return
                check("bnki: Stapel als CSV-Datei im Projekt gespeichert", csv === "front;back;hint;tags\nWas ist $E=mc^2$?;Energie;;physik\n", JSON.stringify(csv))
                const outside = JSON.parse(app.extensionCall("bnki", "readFile", JSON.stringify({ path: "/etc/hostname" })))
                check("bnki: Dateien außerhalb des Projekts gesperrt", outside.ok === false && outside.error.indexOf("außerhalb") >= 0, JSON.stringify(outside))
                test.bnkiView.webItem.runJavaScript("[typeof katex, document.querySelector('#cardList .katex') !== null, getComputedStyle(document.body).backgroundColor]",
                    (r) => { test.bnkiPage = r })
                editor.open(path("bnki/Biologie.csv"), 0, "tab", { processor: "view:bnki/editor" })
            },
            () => {
                if (waitFor(() => !!test.bnkiPage)) return
                check("bnki: KaTeX von Atlas, Formel in der Liste gesetzt", !!test.bnkiPage && test.bnkiPage[0] === "object" && test.bnkiPage[1] === true, JSON.stringify(test.bnkiPage))
                check("bnki: Farben von Atlas", !!test.bnkiPage && test.bnkiPage[2] === "rgb(15, 23, 38)", JSON.stringify(test.bnkiPage))
                const tab = editor.activeTab
                check("bnki: CSV-Datei mit „Anzeigen mit“ als eigener Tab", !!tab && tab.title === "bnki-Karten – Biologie.csv" && tab.path === path("bnki/Biologie.csv"), tab ? tab.title : "kein Tab")
                // Uninstalling: folder in the trash, settings gone, the decks stay.
                const bnki = JSON.parse(app.extensions()).find(e => e.name === "bnki") || {}
                app.setExtensionSetting("bnki", "ordner", JSON.stringify("karten"))
                check("bnki deinstalliert", app.uninstallExtension(bnki.dir || "") === "" && !app.exists(bnki.dir || "/fehlt"))
                check("Deinstalliert: nicht mehr in der Liste, Kürzel frei",
                      !JSON.parse(app.extensions()).some(e => e.name === "bnki") && !JSON.parse(app.keybindings()).some(k => k.sequence === "Ctrl+Alt+B"))
                check("Deinstalliert: wieder installierbar", JSON.parse(app.bundledExtensions()).some(b => b.name === "bnki" && !b.installed))
                check("Deinstalliert: offener Tab sagt es", !!tab && !!tab.viewerItem.info.error, tab ? JSON.stringify(tab.viewerItem.info) : "")
                check("Deinstalliert: Stapel bleiben", app.readTextFile(path("bnki/Biologie.csv")).indexOf("Energie") > 0)
                app.installBundled("bnki")
                const fresh = JSON.parse(app.extensions()).find(e => e.name === "bnki") || { settings: [] }
                check("Neu installiert: Einstellungen wie neu", (fresh.settings.find(x => x.key === "ordner") || {}).value === "bnki", JSON.stringify(fresh.settings))
                // ---- Jupyter: notebooks open by themselves in the view of the extension
                check("Jupyter installiert", app.installBundled("jupyter") === "")
                const nb = { cells: [{ cell_type: "markdown", id: "m", metadata: {}, source: ["# Titel $x^2$"] },
                                     { cell_type: "code", execution_count: 1, id: "c", metadata: {}, outputs: [{ output_type: "stream", name: "stdout", text: ["42\n"] }], source: ["print(42)"] }],
                             metadata: {}, nbformat: 4, nbformat_minor: 5 }
                const written = JSON.parse(app.extensionCall("jupyter", "writeFile", JSON.stringify({ path: "analyse.ipynb", text: JSON.stringify(nb) })))
                check("Jupyter: Notebook über die Brücke geschrieben", written.ok, JSON.stringify(written))
                editor.open(path("analyse.ipynb"), 0, "tab", {})
            },
            () => {
                const tab = editor.activeTab
                const v = tab ? tab.viewerItem : null
                if (waitFor(() => !!v && v.started === true)) return
                check("Jupyter: .ipynb öffnet in der Notebook-Ansicht", !!v && v.viewId === "view:jupyter/notebook" && tab.title === "Jupyter-Notebook – analyse.ipynb",
                      tab ? tab.title + " " + (v ? v.viewId + " " + JSON.stringify(v.pageErrors) : "") : "kein Tab")
                check("Jupyter: Ansicht nicht als Zustand gespeichert", tab && tab.viewState().processor === undefined, tab ? JSON.stringify(tab.viewState()) : "")
                if (v) v.webItem.runJavaScript("[state.cells.length, document.querySelectorAll('.markdown-out .katex').length, document.querySelector('.output pre') ? document.querySelector('.output pre').textContent : '', $('notice').hidden]",
                    (r) => { test.jupyterPage = r })
            },
            () => {
                if (waitFor(() => !!test.jupyterPage)) return
                const r = test.jupyterPage
                check("Jupyter: Zellen, Formel und Ausgabe gezeigt", !!r && r[0] === 2 && r[1] === 1 && r[2] === "42\n", JSON.stringify(r))
                editor.open(path("analyse.ipynb"), 0, "tab", { processor: "atlas:source" })
            },
            () => {
                const tab = editor.activeTab
                check("Jupyter: „Datei selbst“ zeigt das JSON", !!tab && tab.shownProcessor === "" && tab.kind === "text", tab ? tab.kind + " " + tab.shownProcessor : "kein Tab")
                // ---- Docs search: a view in its own window, index from Python's json module
                check("Doku-Suche installiert", app.installBundled("docs") === "")
                check("Doku-Suche: Fenster mit F1", JSON.parse(app.keybindings()).some(k => k.sequence === "F1" && k.id === "view:docs/search"), app.keybindings())
                app.setExtensionSetting("docs", "pakete", JSON.stringify("json"))
                const ctx = JSON.parse(app.extensionCall("docs", "context", "{}"))
                test.docsCache = ctx.ok ? ctx.value.cache : ""
                check("Doku-Suche: eigener Cache-Ordner im Kontext", test.docsCache.indexOf("/extensions/docs") > 0, JSON.stringify(ctx))
                const own = JSON.parse(app.extensionCall("docs", "writeFile", JSON.stringify({ path: test.docsCache + "/probe.txt", text: "x" })))
                const foreign = JSON.parse(app.extensionCall("docs", "readFile", JSON.stringify({ path: path("notes/idee.md") })))
                check("Doku-Suche: Cache-Ordner offen, Projekt zu (files: none)", own.ok && !foreign.ok, JSON.stringify([own, foreign]))
                test.docsTabs = editor.groups.reduce((n, g) => n + g.tabs.length, 0)
                window.openExtensionView("view:docs/search", "")
            },
            () => {
                const w = extensionWindow
                if (waitFor(() => w.visible)) return
                check("Doku-Suche: eigenes Fenster statt Tab", w.visible && w.viewId === "view:docs/search"
                      && editor.groups.reduce((n, g) => n + g.tabs.length, 0) === test.docsTabs, w.viewId + " " + w.visible)
            },
            () => {
                const page = extensionWindow.page
                if (waitFor(() => !!page && page.started === true && !!test.docsReady)) {
                    if (page && page.started) page.webItem.runJavaScript("state.entries.length", (n) => { test.docsReady = n > 0 })
                    return
                }
                check("Doku-Suche: Index aus dem installierten Paket gebaut", !!test.docsReady && app.exists(test.docsCache + "/json.json"),
                      page ? JSON.stringify(page.pageErrors) : "keine Seite")
                if (page) page.webItem.runJavaScript(
                    "const ids = (q) => { els.query.value = q; render(); return state.results.map(e => e.id) };"
                    + "[ids('loads')[0], ids('p:fp k:funktion'), ids('in:JSONEncoder').indexOf('json.JSONEncoder.encode') >= 0, ids('pkg:json k:klasse').indexOf('json.JSONDecoder') >= 0,"
                    + " (ids('loads'), document.querySelector('#detail h1').textContent.trim().split(' ')[0])]",
                    (r) => { test.docsSearch = r })
            },
            () => {
                const r = test.docsSearch
                if (waitFor(() => !!r)) return
                check("Doku-Suche: Name zuerst", !!r && r[0] === "json.loads", JSON.stringify(r))
                check("Doku-Suche: Filter Parameter und Art", !!r && r[1].indexOf("json.dump") >= 0 && r[1].indexOf("json.load") >= 0
                      && r[1].indexOf("json.loads") < 0 && r[1].indexOf("json.JSONDecoder") < 0, JSON.stringify(r))
                check("Doku-Suche: Filter In und Art Klasse", !!r && r[2] === true && r[3] === true, JSON.stringify(r))
                check("Doku-Suche: Text des Treffers gezeigt", !!r && r[4] === "loads", JSON.stringify(r))
                // Online examples, already in the cache (no network here): a
                // seaborn entry whose package keeps its examples outside.
                const dir = test.docsCache + "/examples/seaborn/0.13.2/seaborn.heatmap"
                const svg = dir + "/01-heatmap.svg"
                app.extensionCall("docs", "writeFile", JSON.stringify({ path: svg,
                    text: '<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="red"/></svg>' }))
                app.extensionCall("docs", "writeFile", JSON.stringify({ path: dir + "/examples.json", text: JSON.stringify({ format: 1,
                    url: "https://seaborn.pydata.org/generated/seaborn.heatmap.html",
                    blocks: [{ text: "Pass a <code>DataFrame</code>:" }, { code: "sns.heatmap(glue)" }, { image: svg }] }) }))
                extensionWindow.page.webItem.runJavaScript(
                    "(() => { const e = { id: 'seaborn.heatmap', k: 'function', sum: 'Plot rectangular data.',"
                    + " doc: 'Plot rectangular data.\\n\\n§examples\\n.. include:: ../docstrings/heatmap.rst\\n', p: [] };"
                    + " prepare(e, 'seaborn', '0.13.2'); show(e); return true })()")
            },
            () => {
                const page = extensionWindow.page
                page.webItem.runJavaScript("(() => { const img = document.querySelector('#ex-online img.ex-img');"
                    + " return [!!img && img.complete && img.naturalWidth > 0, document.querySelector('#ex-online pre.code')?.textContent,"
                    + " document.querySelector('#ex-online .ex-text')?.innerHTML, !!document.querySelector('.toc [data-jump=\"s-examples\"]') || !document.querySelector('.toc'),"
                    + " document.documentElement.classList.contains('dark')] })()", (r) => { test.docsExamples = r })
                const r = test.docsExamples
                if (waitFor(() => !!r && r[0] === true)) return
                check("Doku-Suche: Beispiele mit Bild aus dem Cache", !!r && r[0] === true && r[1] === "sns.heatmap(glue)"
                      && r[2] === "Pass a <code>DataFrame</code>:" && r[3] === true, JSON.stringify(r))
                test.docsDark = !!r && r[4]
                app.theme = test.docsDark ? "light" : "dark"
            },
            () => {
                extensionWindow.page.webItem.runJavaScript("document.documentElement.classList.contains('dark')", (d) => { test.docsDarkAfter = d })
                if (waitFor(() => test.docsDarkAfter === !test.docsDark)) return
                check("Doku-Suche: Farbschema wechselt sofort", test.docsDarkAfter === !test.docsDark, test.docsDark + " → " + test.docsDarkAfter)
                app.theme = test.docsDark ? "dark" : "light"
                // Esc with an empty search closes the window (atlas.close()).
                extensionWindow.page.webItem.runJavaScript("clearAll(); document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))")
            },
            () => {
                if (waitFor(() => !extensionWindow.visible)) return
                check("Doku-Suche: Esc schließt das Fenster", !extensionWindow.visible)
                // ---- Phase 2: project calendar
                window.openCalendar({ mode: "week", date: "2026-10-19" })
            },
            () => {
                const v = editor.activeTab.viewerItem
                const cal = fileActions.calendar
                check("Kalender als Tab", editor.activeTab.kind === "calendar" && editor.activeTab.title === "Kalender" && !!v && v.mode === "week", editor.activeTab.kind)
                check("Ohne Nextcloud: Projektkalender", !cal.connected && cal.defaultCalendar() === "local")
                test.calUid = cal.save(JSON.stringify({ kind: "event", title: "Kapitel 3 besprechen", start: "2026-10-20T10:00", end: "2026-10-20T11:30",
                                                          tags: ["schreiben"], session: "Standard", reminders: [15] }))
                const occ = JSON.parse(cal.occurrences("2026-10-19T00:00", "2026-10-26T00:00"))
                check("Termin angelegt", test.calUid !== "" && occ.some(o => o.uid === test.calUid && o.start === "2026-10-20T10:00" && o.end === "2026-10-20T11:30"), JSON.stringify(occ))
                const ics = app.readTextFile(app.atlasDir + "/calendar/events.ics")
                check("events.ics im iCalendar-Format", ics.indexOf("BEGIN:VEVENT") >= 0 && ics.indexOf("SUMMARY:Kapitel 3 besprechen") >= 0 && ics.indexOf("X-ATLAS-SESSION:Standard") >= 0, ics.slice(0, 200))
                check("Datei verknüpft", cal.linkFile(test.calUid, path("notes/aufgaben.md")) && JSON.parse(cal.entry(test.calUid)).files[0] === "/notes/aufgaben.md")
                check("links.json über die UID", app.readTextFile(app.atlasDir + "/calendar/links.json").indexOf(test.calUid) >= 0)
                check("Woche zeigt den Termin", v.bodyItem && v.bodyItem.segments.some(sg => sg.o.uid === test.calUid), v.bodyItem ? v.bodyItem.segments.length : "keine Ansicht")
                check("Notiz-Frist aus dem Frontmatter", v.occs.some(o => o.kind === "note" && o.title.length > 0 && o.start.indexOf("2026-10-23") === 0), JSON.stringify(v.occs.map(o => o.kind + ":" + o.start)))
                // Move by drag: one day later, same time.
                check("Termin verschoben", cal.moveTo(test.calUid, "", "2026-10-21T10:00", "2026-10-21T11:30", false)
                      && JSON.parse(cal.entry(test.calUid)).start === "2026-10-21T10:00")
                // Markdown checklist → tasks, only by an explicit action.
                const items = JSON.parse(cal.checklistOf(path("notes/aufgaben.md")))
                check("Checkliste erkannt", items.length === 3 && items[1].done, JSON.stringify(items))
                check("Aufgaben übernommen", cal.addTasks(path("notes/aufgaben.md"), JSON.stringify([items[0], items[2]])) === 2)
                check("Markdown unverändert", app.readTextFile(path("notes/aufgaben.md")).indexOf("- [ ] Literatur sichten") >= 0)
                const tasks = JSON.parse(cal.tasks())
                check("Noch nicht eingeplant", tasks.length === 2 && tasks.every(t => t.files[0] === "/notes/aufgaben.md"), JSON.stringify(tasks))
                const t = tasks.find(x => x.title === "Literatur sichten")
                check("Aufgabe eingeplant", cal.schedule(t.uid, "2026-10-22T14:00", "") && JSON.parse(cal.entry(t.uid)).kind === "block")
                fileActions.planAsTask(path("src/main.rs"))
                check("Als Aufgabe einplanen", JSON.parse(cal.tasks()).some(x => x.title === "main" && x.files[0] === "/src/main.rs"))
                const found = JSON.parse(cal.search("kapitel bespr", 5))
                check("Arbeitsleiste findet Termine", found.length >= 1 && found[0].uid === test.calUid, JSON.stringify(found))
                fileActions.ops.rename(path("notes/aufgaben.md"), "todo.md")
            },
            () => {
                const cal = fileActions.calendar
                check("Dateilink folgt Umbenennung", JSON.parse(cal.entry(test.calUid)).files[0] === "/notes/todo.md", cal.entry(test.calUid))
                window.saveSession()
                app.flushSession()
                const layout = JSON.stringify(JSON.parse(app.loadLayout() || "{}").editor)
                check("Kalender-Tab in der Session", layout.indexOf("calendar:") >= 0 && layout.indexOf("\"mode\":\"week\"") >= 0, layout.slice(0, 300))
                check("Löschen", cal.remove(test.calUid) && cal.entry(test.calUid) === "")
                const v = editor.activeTab.viewerItem
                v.mode = "month"
            },
            () => {
                const v = editor.activeTab.viewerItem
                check("Monatsansicht", v.mode === "month" && !!v.bodyItem && v.bodyItem.perDay.length === 42)
                v.mode = "agenda"
            },
            () => {
                const v = editor.activeTab.viewerItem
                check("Agenda", v.mode === "agenda" && !!v.bodyItem)
                // ---- moving tabs between splits by real drag-and-drop
                editor.restore({})
                editor.open(path("src/main.rs"), 0, "tab")
                editor.open(path("notes/idee.md"), 0, "tab")
                window.openTerminal(app.projectRoot, "tab")
                editor.open(path("notes/wiki.md"), 0, "split")
            },
            () => {
                const [a, b] = editor.groups
                check("Ausgangslage: zwei Splits", editor.groups.length === 2 && a.tabs.length === 3 && b.tabs.length === 1, names())
                test.term = a.tabs.find(t => t.kind === "terminal")
                dragTab(a, a.tabs.indexOf(test.term), b, b.width / 2, b.height / 2)
            },
            () => {
                const [a, b] = editor.groups
                check("Tab in anderen Split: im alten verschwunden", a.tabs.indexOf(test.term) < 0 && a.tabs.length === 2, names())
                check("Tab im neuen Split, aktiv", b.tabs.length === 2 && b.currentTab === test.term && editor.activeGroup === b, names())
                check("Terminal läuft nach dem Ziehen weiter", test.term.viewerItem && test.term.viewerItem.terminalView.running)
                check("Ziehzustand zurückgesetzt", editor.draggedTab === null)
                check("Sichtbar: verschobener Tab im Ziel, nächster im alten Split", test.term.visible && !!a.currentTab && a.currentTab.visible
                      && a.tabs.filter(t => t.visible).length === 1 && b.tabs.filter(t => t.visible).length === 1)
                // To the right edge: a new split.
                const idee = a.tabs.find(t => t.path === path("notes/idee.md"))
                dragTab(a, a.tabs.indexOf(idee), b, b.width * 0.92, b.height / 2)
            },
            () => {
                const [a, b, c] = editor.groups
                check("An den Rand: neuer Split rechts", editor.groups.length === 3 && !!c && c.tabs.length === 1 && c.tabs[0].path === path("notes/idee.md"), names())
                check("… und im alten Split verschwunden", a.tabs.length === 1 && a.tabs[0].path === path("src/main.rs"), names())
                // The last tab of a split: the empty split closes.
                dragTab(a, 0, b, b.width / 2, b.height / 2)
            },
            () => {
                const [b, c] = editor.groups
                check("Letzter Tab verschoben: leerer Split geschlossen", editor.groups.length === 2 && b.tabs.length === 3
                      && b.tabs.some(t => t.path === path("src/main.rs")), names())
                // Back onto its own split: nothing changes.
                test.before = names()
                dragTab(b, 0, b, b.width / 2, b.height / 2)
            },
            () => {
                check("Auf den eigenen Split: unverändert", names() === test.before, names())
                // Reorder: the last tab to the front of the strip (without
                // the long terminal title all tabs fit side by side).
                const b = editor.groups[0]
                b.closeTab(b.tabs.indexOf(test.term))
                test.moved = b.tabs[b.tabs.length - 1]
                const first = b.tabItemAt(0)
                dragTabTo(b, b.tabs.length - 1, first, 4, first.height / 2)
            },
            () => {
                const b = editor.groups[0]
                check("Im Tab-Streifen umsortiert", b.tabs[0] === test.moved && b.tabs.length === 2 && editor.groups.length === 2, names())
                // The only tab of a split onto its own edge: no empty split.
                const c = editor.groups[1]
                dragTab(c, 0, c, c.width * 0.92, c.height / 2)
            },
            () => {
                check("Einziger Tab an den eigenen Rand: kein leerer Split", editor.groups.length === 2 && editor.groups[1].tabs.length === 1, names())
                // Released outside of any split: nothing changes.
                test.before = names()
                const b = editor.groups[0]
                dragTabTo(b, 1, window.contentItem, 40, 5)
            },
            () => {
                check("Abgebrochenes Ziehen: unverändert", names() === test.before && editor.draggedTab === null, names())
                // The same file already open in the target: one tab remains.
                const [b, c] = editor.groups
                b.openTab(path("notes/idee.md"), 0, "tab")
                dragTab(c, 0, b, b.width / 2, b.height / 2)
            },
            () => {
                const b = editor.groups[0]
                check("Gleiche Datei im Ziel: nur ein Tab", editor.groups.length === 1
                      && b.tabs.filter(t => t.path === path("notes/idee.md")).length === 1, names())
                // Four splits are the maximum: an edge drop moves into the split.
                editor.open(path("notes/plan-neu.md"), 0, "split")
                editor.open(path("notes/zitate.md"), 0, "split")
                editor.open(path("notes/todo.md"), 0, "split")
            },
            () => {
                check("Vier Splits", editor.groups.length === 4, names())
                const [b, , , d] = editor.groups
                test.moved = b.tabs[0]
                dragTab(b, 0, d, d.width * 0.92, d.height / 2)
            },
            () => {
                const d = editor.groups[3]
                check("Bei vier Splits: in den Split statt neuem", editor.groups.length === 4 && d.tabs.indexOf(test.moved) >= 0
                      && editor.groups[0].tabs.indexOf(test.moved) < 0, names())
                // A split showing the calendar takes tabs too.
                editor.groups[1].openTab("calendar:", 0, "tab", {})
            },
            () => {
                const [b, cal] = editor.groups
                check("Kalender im Split", cal.currentTab && cal.currentTab.kind === "calendar")
                test.moved = b.tabs[0]
                dragTab(b, 0, cal, cal.width / 2, cal.height / 2)
            },
            () => {
                const cal = editor.groups[1]
                check("Tab auf Split mit Kalender verschoben", cal.tabs.indexOf(test.moved) >= 0 && editor.groups[0].tabs.indexOf(test.moved) < 0, names())
                window.saveSession()
                app.flushSession()
                const groups = JSON.parse(app.loadLayout()).editor.groups
                const paths = groups.reduce((all, g) => all.concat(g.tabs.map(t => t.path)), [])
                check("Session: jeder Tab genau einmal, in seinem Split", paths.length === new Set(paths).size
                      && groups.length === editor.groups.length && groups[1].tabs.some(t => t.path === "calendar:"), JSON.stringify(paths))
            },
            () => {
                // To the left edge: a new split on the left.
                while (editor.groups.length > 1) editor.closeGroup(editor.groups[editor.groups.length - 1])
                const g = editor.groups[0]
                editor.open(path("src/main.rs"), 0, "tab")
            },
            () => {
                // (Own step: the split has its final width now.)
                const g = editor.groups[0]
                test.moved = g.currentTab
                dragTab(g, g.tabs.indexOf(test.moved), g, g.width * 0.08, g.height / 2)
            },
            () => {
                const [left, right] = editor.groups
                check("An den linken Rand: neuer Split links", editor.groups.length === 2 && left.tabs.length === 1 && left.tabs[0] === test.moved
                      && right.tabs.indexOf(test.moved) < 0 && editor.activeGroup === left, names())
                // ---- layout: splits fill the editor area
                editor.restore({})
                editor.open(path("notes/idee.md"), 0, "tab")
                editor.open(path("notes/wiki.md"), 0, "split")
            },
            () => {
                test.layoutBefore = layoutText()
                window.setSidebarVisible(false)
            },
            () => {
                check("Seitenleiste zu: Splits füllen den Platz", filled(), layoutText() + " vorher " + test.layoutBefore)
                window.setSidebarVisible(true)
                window.setSidebarWidth(520)
            },
            () => {
                check("Breitere Seitenleiste: Splits passen sich an", filled() && editor.groups.every(g => g.width > 80), layoutText())
                window.setSidebarWidth(200)
            },
            () => {
                check("Schmalere Seitenleiste: Splits wachsen mit", filled(), layoutText())
                window.setSidebarWidth(280)
                editor.open(path("docs/umbenannt.pdf"), 0, "tab")
            },
            () => {
                // Ctrl+wheel over the pages zooms, the wheel alone scrolls.
                const v = editor.activeTab.viewerItem
                const p = v.mapToItem(null, v.width / 2, v.height * 0.6)
                // Qt on Wayland reports mouse wheels as a "touchpad" device with
                // finger pointer type: Ctrl+wheel zooms all the same.
                const before = v.zoomScale
                TestDriver.sendWheel(window, p.x, p.y, 120, 17)
                TestDriver.sendWheel(window, p.x, p.y, 120, 17)
                check("Wayland: Strg+Mausrad vom „Touchpad“-Gerät zoomt", v.zoomScale > before * 1.15, before + " → " + v.zoomScale)
                // Wheel notches in scroll phases without a ScrollBegin (as from
                // some Wayland compositors) zoom too.
                const start = v.zoomScale
                TestDriver.sendWheel(window, p.x, p.y, 120, 9)
                TestDriver.sendWheel(window, p.x, p.y, 120, 9)
                check("Strg+Mausrad ohne Gestenbeginn zoomt", v.zoomScale > start * 1.15, start + " → " + v.zoomScale)
                test.zoomBefore = v.zoomScale
                TestDriver.sendWheel(window, p.x, p.y, 120, true)
                check("Strg+Mausrad vergrößert die PDF", v.zoomScale > test.zoomBefore * 1.05, test.zoomBefore + " → " + v.zoomScale)
                const zoomed = v.zoomScale
                TestDriver.sendWheel(window, p.x, p.y, -240, true)
                check("Strg+Mausrad zurück verkleinert", v.zoomScale < zoomed, zoomed + " → " + v.zoomScale)
                const plain = v.zoomScale
                const y0 = v.pagesView.contentY
                TestDriver.sendWheel(window, p.x, p.y, -120, false)
                check("Mausrad allein zoomt nicht", v.zoomScale === plain, plain + " → " + v.zoomScale)
                // (Eased in over a few frames, like a browser: settled here.)
                check("Mausrad: Stufe wird weich gescrollt", Math.abs(v.fling.wheelY + v.wheelStep) < 1, v.fling.wheelY)
                v.fling.settle()
                check("Mausrad scrollt eine große Stufe", v.pagesView.contentY - y0 >= v.wheelStep - 1, y0 + " → " + v.pagesView.contentY)
                // Touchpad as Qt on Wayland reports it: 10 px with an angle delta
                // of 120 – a touchpad step, not a wheel notch (it made the view
                // jump by a whole notch).
                const y1 = v.pagesView.contentY
                TestDriver.sendWheel(window, p.x, p.y, -10, 64 | 16 | 4)
                TestDriver.sendWheel(window, p.x, p.y, -10, 64 | 16)
                TestDriver.sendWheel(window, p.x, p.y, -10, 64 | 16)
                v.fling.flush()
                check("Touchpad unter Wayland: Ansicht folgt den Fingern genau", Math.abs(v.pagesView.contentY - y1 - 30) < 1 && v.fling.wheelY === 0,
                      y1 + " → " + v.pagesView.contentY)
                TestDriver.sendWheel(window, p.x, p.y, 0, 2 | 16 | 32)
                v.fling.stop()
                TestDriver.sendWheel(window, p.x, p.y, 120, false)
                v.fling.settle()
                // Dark mode: one switch for all PDFs, stored in config.toml.
                app.pdfDark = true
                check("PDF-Dunkelmodus: Seiten invertiert, Papier dunkel", v.dark && Qt.colorEqual(v.paperColor, "#262626"), v.paperColor)
                app.pdfDark = false
                check("PDF-Dunkelmodus aus", !v.dark && Qt.colorEqual(v.paperColor, "white"))
                // A page narrower than the view stays centred while zooming
                // (no jump to the side).
                v.zoom = 0.4
                test.zoomBefore = v.zoomScale
                const side = v.mapToItem(null, v.width * 0.8, v.height * 0.6)
                TestDriver.sendWheel(window, side.x, side.y, 120, true)
                check("Strg+Mausrad: schmale Seite bleibt mittig", v.zoomScale > test.zoomBefore && v.pagesView.contentX === v.pagesView.originX,
                      v.pagesView.contentX + " / " + v.pagesView.originX)
                v.zoom = plain
                // Touchpads (and fine scrolling on Wayland): a gesture with
                // pixel-only deltas zooms as well and never scrolls.
                TestDriver.sendWheel(window, p.x, p.y, 0, 7)
                TestDriver.sendWheel(window, p.x, p.y, 40, 3)
                TestDriver.sendWheel(window, p.x, p.y, 40, 3)
                check("Strg+Touchpad zoomt die PDF", v.zoomScale > plain * 1.1, plain + " → " + v.zoomScale)
                // Wayland: wheel notches come in scroll phases; a gesture begun
                // without Ctrl (scrolling) continues with Ctrl held.
                const touch = v.zoomScale
                TestDriver.sendWheel(window, p.x, p.y, 0, 4)
                TestDriver.sendWheel(window, p.x, p.y, -120, 8)
                TestDriver.sendWheel(window, p.x, p.y, 120, 9)
                TestDriver.sendWheel(window, p.x, p.y, 120, 9)
                check("Wayland: Strg+Mausrad mitten in einer Scroll-Geste zoomt", v.zoomScale > touch * 1.15, touch + " → " + v.zoomScale)
                // Dialogs of the new design.
                sendToNote.ask("ok", app.projectRoot)
                check("„In Notiz übernehmen“ öffnet", sendToNote.visible && sendToNote.filtered.length > 0, sendToNote.filtered.length)
                sendToNote.close()
                const sessions = JSON.parse(app.sessionSummaries())
                check("Sessionliste mit Umfang", sessions.length >= 1 && sessions.every(s => s.tabs >= 0 && s.splits >= 0 && !!s.kind), JSON.stringify(sessions))
                sessionMenu.clicked()
                check("Sessionfenster öffnet", sessionMenu.sessions.length === sessions.length)
            },
            () => {
                // Zooming on page 2: the spot under the mouse stays (it used
                // to drift with the gaps between pages – the pages scrolled).
                const v = editor.activeTab.viewerItem
                v.zoom = 1
                v.goToPage(1)
            },
            () => {
                const v = editor.activeTab.viewerItem
                test.zoomPoint = v.mapToItem(null, v.width / 2, v.height * 0.5)
                const q = v.mapFromItem(null, test.zoomPoint.x, test.zoomPoint.y)
                const vp = v.pagesView.mapFromItem(v, q.x, q.y)
                test.zoomAnchor = v.pointAt(vp.x, vp.y)
                for (let i = 0; i < 6; i++) TestDriver.sendWheel(window, test.zoomPoint.x, test.zoomPoint.y, 120, true)
            },
            () => {
                const v = editor.activeTab.viewerItem
                const q = v.mapFromItem(null, test.zoomPoint.x, test.zoomPoint.y)
                const vp = v.pagesView.mapFromItem(v, q.x, q.y)
                const now = v.pointAt(vp.x, vp.y)
                const a = test.zoomAnchor
                check("Strg+Mausrad auf Seite 2: die Stelle unter der Maus bleibt (kein Wegscrollen)",
                      !!a && !!now && now.page === a.page && Math.abs(now.y - a.y) < 1 && Math.abs(now.x - a.x) < 1 && v.zoomScale > 1.6,
                      JSON.stringify(a) + " → " + JSON.stringify(now) + " · Zoom " + v.zoomScale)
                v.zoom = 1
            },
            () => {
                // Task list checkboxes in the preview toggle the source line.
                editor.open(path("notes/todo.md"), 0, "tab")
                const md = editor.activeTab.viewerItem
                md.mode = "preview"
            },
            () => {
                const md = editor.activeTab.viewerItem
                md.previewItem.runJavaScript("document.querySelector('input.task[data-line=\"7\"]').click()")
            },
            () => {
                const md = editor.activeTab.viewerItem
                check("Checkbox in der Vorschau hakt ab", md.editorItem.text.indexOf("- [x] Literatur sichten") >= 0, md.editorItem.text)
                md.previewItem.runJavaScript("document.querySelector('input.task[data-line=\"8\"]').click()")
            },
            () => {
                const md = editor.activeTab.viewerItem
                check("… und wieder ab", md.editorItem.text.indexOf("- [ ] Gliederung") >= 0, md.editorItem.text)
                // Shown again (here: back from the editor), the preview asks
                // for a fresh frame instead of staying black (#2).
                test.repaints = md.previewRepaint.count
                md.mode = "edit"
                md.mode = "preview"
                // Headings without "#" markers (#3).
                md.previewItem.runJavaScript("(() => { const h = document.querySelector('h1, h2'); return h ? getComputedStyle(h, '::before').content + '|' + h.textContent : 'keine' })()",
                    (r) => check("Überschriften in der Vorschau ohne #", typeof r === "string" && r.startsWith("none|") && r.indexOf("#") < 0, r))
                md.editorItem.save()
                check("Abhaken gespeichert", app.readTextFile(path("notes/todo.md")).indexOf("- [x] Literatur sichten") >= 0)
            },
            () => {
                const md = editor.activeTab.viewerItem
                check("Vorschau zeichnet nach dem Wiederzeigen neu", md.previewRepaint.count > test.repaints, test.repaints + " → " + md.previewRepaint.count)
                // Leaving the workspace: a snapshot of the preview …
                md.previewRepaint.snapshot()
            },
            () => {
                // … covers it at once when coming back, until Chromium painted.
                const r = editor.activeTab.viewerItem.previewRepaint
                check("Vorschau: Momentaufnahme beim Verlassen", r.hasSnapshot)
                test.repaints = r.painted
                r.showCover()
                check("Zurück: Momentaufnahme sofort sichtbar statt Schwarz", r.covering)
            },
            () => {
                const r = editor.activeTab.viewerItem.previewRepaint
                // (Gone because the page reported its frame: the fallback takes 2 s.)
                check("… und danach wieder die echte Vorschau, sobald sie gezeichnet ist", !r.covering && r.painted > test.repaints,
                      r.covering + " " + test.repaints + " → " + r.painted)
                // The spare preview loaded ahead has no colours of its own yet: it
                // shows the view's (white in the light design), never navy.
                const spare = PreviewPool.spare
                if (spare && spare.ready) spare.runJavaScript("(() => { const bg = () => getComputedStyle(document.body).backgroundColor;"
                    + " const before = bg(); atlas.setLight(true); const after = bg();"
                    + " document.documentElement.classList.remove('themed', 'light'); atlas.setLight(false); document.documentElement.classList.remove('themed');"
                    + " return [before, after] })()", (v) => { test.previewBg = v })
                else test.previewBg = ["kein Ersatz"]
            },
            () => {
                const v = test.previewBg
                if (waitFor(() => !!v)) return
                check("Vorschau lädt im hellen Design weiß statt dunkel (bis zum Inhalt durchsichtig)",
                      !!v && v[0] === "rgba(0, 0, 0, 0)" && v[1] === "rgb(255, 255, 255)", JSON.stringify(v))
            },
            () => {
                // A single click opens a file for good: no preview tab that the
                // next click would replace.
                const g = editor.activeGroup
                const before = g.tabs.length
                const a = editor.open(path("notes/idee.md"), 0, "preview")
                const b = editor.open(path("notes/wiki.md"), 0, "preview")
                check("Einzelklick öffnet bleibende Tabs (keine Vorschau-Tabs)",
                      g.tabs.indexOf(a) >= 0 && g.tabs.indexOf(b) >= 0 && !a.preview && !b.preview && g.tabs.length >= before + 1,
                      names())
            },
            () => {
                // An empty window has no close button and never stays as a split.
                editor.restore({})
                check("Leeres Fenster ohne Schließen-Knopf", editor.groups.length === 1 && !editor.groups[0].closeButton.visible)
                editor.open(path("notes/idee.md"), 0, "split")
                check("Öffnen im Split nutzt das leere Fenster", editor.groups.length === 1 && editor.groups[0].tabs.length === 1, names())
                check("Fenster mit Dokument hat Schließen-Knopf", editor.groups[0].closeButton.visible)
                editor.restore({})
                editor.openInSplit(editor.groups[0], path("notes/wiki.md"), 0, "right", {})
                check("Split neben leerem Fenster: kein leerer Split bleibt", editor.groups.length === 1 && editor.groups[0].tabs.length === 1, names())
                editor.restore({})
                editor.open(path("notes/wiki.md"), 0, "temporary")
                check("Temporärer Split ersetzt das leere Fenster", editor.groups.length === 1, names())
            },
            () => {
                // "Neue Session" asks for the project folder (own dialog).
                sessionMenu.askNew()
                check("Neue Session fragt nach dem Projektordner", folderDialog.visible && folderDialog.mode === "session"
                      && app.projectRoot.indexOf(folderDialog.folderPath) === 0, folderDialog.folderPath)
                folderDialog.navigate(path("notes"))
            },
            () => {
                check("Ordnerauswahl: nur Ordner", folderDialog.folders.count === 0 && !folderDialog.folders.error, folderDialog.folders.count)
                const crumbs = folderDialog.segments(path("notes"))
                // (notes is also a file manager place, "Notizen", in the smoke test.)
                check("Pfadleiste endet im Ordner", crumbs[crumbs.length - 1].path === path("notes"), JSON.stringify(crumbs))
                // A click next to the segments switches to typing the path.
                const bar = folderDialog.pathBar
                const p = bar.mapToItem(null, bar.width - 12, bar.height / 2)
                TestDriver.postMouse(window, 0, p.x, p.y, 20)
                TestDriver.postMouse(window, 2, p.x, p.y, 60)
            },
            () => {
                check("Pfadleiste: Klick daneben zum Eintippen", folderDialog.pathBar.editing)
                folderDialog.pathBar.editing = false
                folderDialog.navigate(app.projectRoot)
            },
            () => {
                check("Ordnerauswahl listet Unterordner", folderDialog.folders.count >= 3, folderDialog.folders.count)
                folderDialog.sessionName = "Selbsttest-Session"
                folderDialog.accept()
            },
            () => {
                // The folder already has its session: it is named, not doubled.
                check("Eine Session je Projektordner", !folderDialog.visible && app.activeSessionName === "Selbsttest-Session"
                      && JSON.parse(app.allSessions()).length === 1, app.activeSessionName + " " + app.allSessions())
                folderDialog.askProject()
                check("Projektordner öffnen: eigener Dialog", folderDialog.visible && folderDialog.mode === "open")
                folderDialog.close()
            },
            () => {
                // Ctrl+click on a PDF in the tree: quick look, no new tab.
                if (treeModel.rowOf(path("docs/umbenannt.pdf")) < 0) treeModel.toggle(treeModel.rowOf(path("docs")))
                test.tabsBefore = editor.groups.reduce((n, g) => n + g.tabs.length, 0)
            },
            () => {
                const item = projectTree.itemFor(path("docs/umbenannt.pdf"))
                check("PDF im Baum sichtbar", !!item)
                const p = item.mapToItem(null, item.width / 2, item.height / 2)
                TestDriver.postMouse(window, 10, p.x, p.y, 20)
                TestDriver.postMouse(window, 12, p.x, p.y, 60)
            },
            () => {
                const tabs = editor.groups.reduce((n, g) => n + g.tabs.length, 0)
                check("Strg+Klick: Schnellansicht offen", quickLook.visible && quickLook.path === path("docs/umbenannt.pdf"), quickLook.path)
                check("… ohne neuen Tab", tabs === test.tabsBefore, tabs + " statt " + test.tabsBefore)
            },
            () => {
                check("Schnellansicht: PDF ganzseitig, ohne eigene Leiste", !!quickLook.pdf && quickLook.pdf.zoom < 0 && !quickLook.pdf.chrome)
                check("Schnellansicht: Seiten und Infos", quickLook.pdf && quickLook.pdf.pageCount > 0 && quickLook.info.sizeText.length > 0, JSON.stringify(quickLook.info))
                quickLook.openAsSplit()
            },
            () => {
                check("Schnellansicht → im Split öffnen", !quickLook.visible && editor.activeTab && editor.activeTab.path === path("docs/umbenannt.pdf")
                      && editor.groups.every(g => g.tabs.length > 0), names())
            },
            () => {
                // PDF previews come from Atlas' own PDF engine (Qt's image
                // plugin fails on many PDFs and floods the log).
                editor.open(path("docs"), 0, "tab")
            },
            () => {
                const f = editor.activeTab.viewerItem
                const row = f.folderModel.rowOf(path("docs/umbenannt.pdf"))
                const cell = row >= 0 ? f.activeView.itemAtIndex(row) : null
                if (waitFor(() => !!cell && cell.pdfPreview)) return
                check("Ordner: PDF-Vorschau mit der ersten Seite", !!cell && cell.pdfPreview, "Zeile " + row)
            },
            () => {
                // Folder tab like Dolphin: icons, details, zoom, status bar.
                editor.open(path("notes"), 0, "tab")
            },
            () => {
                const f = editor.activeTab.viewerItem
                check("Ordner: Symbolansicht", f.viewMode === "icons" && f.activeView.count === f.folderModel.count && f.folderModel.count > 3, f.viewMode + " " + f.folderModel.count)
                const sum = JSON.parse(f.folderModel.summary)
                check("Ordner: Statusleiste zählt", sum.files === f.folderModel.count && sum.dirs === 0, f.folderModel.summary)
                check("Ordner: nichts vorausgewählt", f.selectedPaths().length === 0, f.selectedPaths().join(","))
                const item = f.activeView.itemAtIndex(1)
                const p = item.mapToItem(null, item.width / 2, item.height / 2)
                TestDriver.postMouse(window, 0, p.x, p.y, 20)
                TestDriver.postMouse(window, 2, p.x, p.y, 60)
                test.zoomBefore = f.iconSize
                TestDriver.sendWheel(window, p.x, p.y, 120, 17)
            },
            () => {
                const f = editor.activeTab.viewerItem
                check("Ordner: Klick wählt aus", f.selectedPaths().length === 1 && f.selectedPaths()[0] === f.folderModel.pathAt(1), f.selectedPaths().join(","))
                const pressed = f.activeView.itemAtIndex(1)
                check("Ordner: Bild am Mauszeiger fürs Ziehen bereit", !!pressed && String(pressed.Drag.imageSource).length > 0,
                      pressed ? String(pressed.Drag.imageSource) : "kein Eintrag")
                check("Ordner: Strg+Mausrad zoomt", f.iconSize > test.zoomBefore, test.zoomBefore + " → " + f.iconSize)
                f.viewMode = "details"
            },
            () => {
                const f = editor.activeTab.viewerItem
                check("Ordner: Detailansicht behält Auswahl", f.activeView.count === f.folderModel.count && f.activeView.currentIndex === 1
                      && f.selectedPaths().length === 1, f.activeView.currentIndex)
                const state = f.viewState()
                check("Ordner: Ansicht im Sitzungszustand", state.viewMode === "details" && state.iconSize === f.iconSize)
                // Selection rectangle: from the empty area below the rows up
                // into the second row.
                const last = f.activeView.itemAtIndex(f.folderModel.count - 1)
                const from = last.mapToItem(null, last.width - 40, last.height + 12)
                const second = f.activeView.itemAtIndex(1)
                const to = second.mapToItem(null, second.width / 2, second.height / 2)
                TestDriver.postMouse(window, 0, from.x, from.y, 20)
                TestDriver.postMouse(window, 1, from.x - 10, from.y - 10, 60)
                TestDriver.postMouse(window, 1, to.x, to.y, 100)
                TestDriver.postMouse(window, 2, to.x, to.y, 140)
            },
            () => {
                const f = editor.activeTab.viewerItem
                const n = f.folderModel.count
                if (waitFor(() => f.selectedPaths().length === n - 1)) return
                check("Ordner: Markierungsrechteck wählt die Zeilen darin", f.selectedPaths().length === n - 1
                      && f.selectedPaths()[0] === f.folderModel.pathAt(1) && !f.rubberBanding, f.selectedPaths().length + " von " + n)
            },
            () => {
                // Nextcloud app password from a command (never stored).
                const cal = window.calendar
                cal.saveAccount("http://127.0.0.1:9", "selbsttest", "", "", "[]", "sh -c 'echo kaputt >&2; exit 2'")
            },
            () => {
                const cal = window.calendar
                check("Passwort-Befehl: Fehler sichtbar", cal.connected && !cal.unlocked && cal.passwordError.indexOf("kaputt") >= 0
                      && cal.passwordError.indexOf("Code 2") >= 0, cal.passwordError)
                cal.saveAccount("http://127.0.0.1:9", "selbsttest", "", "", "[]", "sh -c 'echo pw-$((6*7))'")
            },
            () => {
                const cal = window.calendar
                check("Passwort-Befehl liefert das App-Passwort", cal.unlocked && cal.passwordError === "" && !cal.fetchingPassword, cal.syncStatus)
                check("Passwort-Befehl steht im Konto, das Passwort nicht", JSON.parse(cal.account()).passwordCommand.indexOf("6*7") >= 0
                      && cal.account().indexOf("pw-42") < 0, cal.account())
                // Entered once and remembered on this machine.
                cal.rememberPassword(true)
                cal.saveAccount("http://127.0.0.1:9", "selbsttest", "einmal-42", "", "[]", "")
            },
            () => {
                const cal = window.calendar
                // (Storing runs in the background; systemd-creds may be slow.)
                if (waitFor(() => cal.remembered !== "")) return
                check("Passwort gemerkt", cal.remembered === "encrypted" || cal.remembered === "file", cal.remembered + " " + cal.error)
                check("Gemerktes Passwort nicht in den Einstellungen", cal.account().indexOf("einmal-42") < 0 && JSON.parse(cal.account()).rememberPassword)
                // As after a restart: nothing in memory, the remembered one loads.
                cal.setPassword("")
                check("Ohne Passwort im Speicher gesperrt", !cal.unlocked)
                cal.retryPasswordCommand()
            },
            () => {
                const cal = window.calendar
                if (waitFor(() => cal.unlocked)) return
                check("Nach Neustart ohne Eingabe entsperrt", cal.unlocked && cal.passwordError === "", cal.syncStatus + " " + cal.passwordError)
                cal.rememberPassword(false)
                check("Nicht mehr merken: gelöscht", cal.remembered === "", cal.remembered)
                cal.rememberPassword(true)
            },
            () => {
                const cal = window.calendar
                if (waitFor(() => cal.remembered !== "")) return
                check("Wieder merken mit bekanntem Passwort", cal.remembered !== "", cal.remembered)
                cal.disconnectAccount()
                check("Konto getrennt: Passwort vergessen", !cal.connected && !cal.unlocked && cal.remembered === "")
            },
            () => {
                // The terminal button hides a shown terminal with one click,
                // also while the focus is in the editor.
                terminalPanel.visible = true
                editor.open(path("notes/idee.md"), 0, "tab")
            },
            () => {
                editor.activeTab.forceActiveFocus()
                const b = window.terminalButton
                const p = b.mapToItem(null, b.width / 2, b.height / 2)
                TestDriver.postMouse(window, 0, p.x, p.y, 20)
                TestDriver.postMouse(window, 2, p.x, p.y, 60)
            },
            () => {
                check("Terminal-Knopf: ein Klick blendet aus", !terminalPanel.visible && !window.terminalButton.checked,
                      terminalPanel.visible + " / " + window.terminalButton.checked)
                const b = window.terminalButton
                const p = b.mapToItem(null, b.width / 2, b.height / 2)
                TestDriver.postMouse(window, 0, p.x, p.y, 20)
                TestDriver.postMouse(window, 2, p.x, p.y, 60)
            },
            () => {
                check("Terminal-Knopf: ein Klick blendet ein", terminalPanel.visible && window.terminalButton.checked,
                      terminalPanel.visible + " / " + window.terminalButton.checked)
                // Split beside, then below: every terminal gets room.
                test.tabsBefore = terminalPanel.currentGroup.terminals.length
                terminalPanel.splitTerminal(false)
            },
            () => {
                const g = terminalPanel.currentGroup
                const sizes = () => g.terminals.map(t => Math.round(t.width) + "x" + Math.round(t.height)).join(", ")
                // (One terminal before: two halves.)
                check("Terminal nebeneinander geteilt, gleich breit", g.terminals.length === test.tabsBefore + 1 && g.orientation === Qt.Horizontal
                      && g.terminals.every(t => t.visible && t.height > 40)
                      && (test.tabsBefore !== 1 || Math.abs(g.terminals[0].width - g.terminals[1].width) < 8), sizes())
                terminalPanel.splitTerminal(true)
            },
            () => {
                const g = terminalPanel.currentGroup
                const sizes = () => g.terminals.map(t => Math.round(t.width) + "x" + Math.round(t.height)).join(", ")
                // Only the active (right) terminal is divided; the left one keeps
                // its full height.
                const [left, right, below] = g.terminals.slice(-3)
                check("Terminal untereinander: nur das aktive geteilt", g.terminals.length === test.tabsBefore + 2 && g.orientation === Qt.Horizontal
                      && Math.abs(left.height - g.height) < 4 && Math.abs(right.height - below.height) < 8
                      && Math.abs(right.width - below.width) < 4 && right.height < g.height * 0.7, sizes())
                test.layoutBefore = JSON.stringify(g.serialize())
                while (g.terminals.length > test.tabsBefore) g.removeTerminal(g.terminals[g.terminals.length - 1])
            },
            () => {
                const g = terminalPanel.currentGroup
                const tree = JSON.parse(test.layoutBefore)
                // (Terminals running a program are not saved.)
                check("Terminal-Splits in der Session als Baum", tree.orientation === "horizontal"
                      && tree.items.some(i => i.orientation === "vertical" && i.items.length === 2), test.layoutBefore)
                check("Geteilte Terminals wieder geschlossen", g.terminals.length === test.tabsBefore && g.terminals.every(t => Math.abs(t.height - g.height) < 4),
                      g.terminals.map(t => Math.round(t.width) + "x" + Math.round(t.height)).join(", "))
            },
            () => {
                // With Dolphin's places file: exactly its visible local places,
                // instead of the fixed ones (GTK bookmarks are not added).
                check("Orte wie in Dolphin", app.fileManagerPlaces && app.fixedPlaceCount === 0
                      && app.placeNames.join(",") === "Belege" && app.placePaths[0] === path("docs"), app.placeNames.join(","))
                inspector.graphWindow.show(path("notes/wiki.md"))
                check("Graph im eigenen Fenster", inspector.graphWindow.visible && inspector.graphWindow.width > 600)
                inspector.graphWindow.close()
                settingsDialog.open()
                check("Einstellungen öffnen", settingsDialog.visible)
                // The design: light for all windows, stored in config.toml.
                app.theme = "light"
                check("Helles Farbschema", app.theme === "light" && Theme.light && Qt.colorEqual(Theme.editor, "#ffffff")
                      && Qt.colorEqual(window.color, Theme.window), Theme.editor)
                app.theme = "unbekannt"
                check("Unbekanntes Farbschema wird dunkel", app.theme === "dark" && !Theme.light, app.theme)
                // The ILIAS address: settable, checked, applied.
                const field = settingsDialog.iliasAddressField
                const before = JSON.parse(IliasBackend.config).base_url
                field.text = "nicht da"
                const invalid = !field.valid
                field.text = "ilias.uni-beispiel.de"
                settingsDialog.applyIliasAddress()
                const after = JSON.parse(IliasBackend.config).base_url
                check("Einstellungen: ILIAS-Adresse einstellbar (ohne https:// ergänzt, Ungültiges abgelehnt)",
                      invalid && after === "https://ilias.uni-beispiel.de/" && fileActions.ilias.host === "ilias.uni-beispiel.de"
                      && fileActions.ilias.index.courses.length === 0 && field.text === after, after + " · " + field.text)
                IliasBackend.setConfig("base_url", before)
                const cals = settingsDialog.defaultCalendarChoices
                check("Einstellungen: Kalender für neue Termine wählbar", cals.length >= 1 && cals.some(c => c.isDefault),
                      JSON.stringify(cals.map(c => c.name + (c.isDefault ? "★" : ""))))
                app.fileManagerPlaces = false
                check("Ausgeschaltet: feste Orte", app.fixedPlaceCount > 0 && app.placeNames.length === app.fixedPlaceCount
                      && app.placeNames.indexOf("Belege") < 0, app.placeNames.join(","))
                app.fileManagerPlaces = true
                check("Wieder an: Dolphins Orte", app.placeNames.join(",") === "Belege" && app.fixedPlaceCount === 0)
                settingsDialog.close()
            },
            () => {
                // All quotes of the project: the marks of all PDFs and where
                // notes quote them.
                const quotes = JSON.parse(app.projectQuotes())
                const q = quotes.find(x => x.annot === test.annotId)
                check("Zitate im Projekt: Markierung mit Notiz", !!q && q.target === path("docs/umbenannt.pdf") && q.page === 1
                      && q.quote.indexOf("Zitat aus der Quelle") === 0 && q.comment === "Wichtig"
                      && q.cited.length === 1 && q.cited[0].source === path("notes/zitate.md"), JSON.stringify(quotes))
                const panel = inspector.linksPanel
                window.inspectorVisible = true
                inspector.tab = 1
                panel.scope = "project"
                const header = panel.rows.find(r => r.header && r.source === path("docs/umbenannt.pdf"))
                check("Verknüpfungen: Zitate nach PDF gruppiert", !!header && header.pdf
                      && panel.rows.some(r => !r.header && r.annot === test.annotId), JSON.stringify(panel.rows.slice(0, 3)))
                // As reported: mark text in a PDF, close it – the mark shows up.
                editor.open(path("docs/umbenannt.pdf"), 0, "tab")
            },
            () => {
                const v = editor.activeTab.viewerItem
                v.annotationStore.add(JSON.stringify({ kind: "highlight", page: 2, text: "Nur markiert, nicht zitiert",
                    rects: [[40, 40, 200, 12]], color: "#7bd88f" }))
                editor.activeGroup.closeTab(editor.activeGroup.tabs.indexOf(editor.activeTab))
            },
            () => {
                const panel = inspector.linksPanel
                // (The store is written and watched in the background.)
                if (waitFor(() => panel.rows.some(r => r.quote === "Nur markiert, nicht zitiert"))) return
                const row = panel.rows.find(r => !r.header && r.quote === "Nur markiert, nicht zitiert")
                check("Neue Markierung erscheint nach dem Schließen", !!row && row.page === 2 && row.cited.length === 0,
                      JSON.stringify(panel.rows.map(r => r.quote || r.relative)))
                // … and is really visible: the list keeps its height without
                // tag/kind filters (they took the space before).
                check("Zitatliste hat Platz", panel.backlinkView.height > panel.height / 2
                      && panel.backlinkView.contentHeight > 0, panel.backlinkView.height + " von " + panel.height)
                // Opening a quote at its place in the PDF.
                const mine = panel.rows.find(r => !r.header && r.annot === test.annotId)
                panel.openRequested(mine.target, mine.page, false, { annot: mine.annot })
            },
            () => {
                const v = editor.activeTab.viewerItem
                check("Zitat öffnet die PDF-Stelle", editor.activeTab.path === path("docs/umbenannt.pdf") && v.focusedAnnot === test.annotId,
                      editor.activeTab.path + " " + (v ? v.focusedAnnot : ""))
                // Deleting marks takes them out of the list – also one a note
                // still quotes.
                const panel = inspector.linksPanel
                const loose = panel.rows.find(r => !r.header && r.quote === "Nur markiert, nicht zitiert")
                v.annotationStore.remove(loose.annot)
                v.annotationStore.remove(test.annotId)
            },
            () => {
                const panel = inspector.linksPanel
                const gone = () => !panel.rows.some(r => !r.header && (r.annot === test.annotId || r.quote === "Nur markiert, nicht zitiert"))
                if (waitFor(gone)) return
                check("Gelöschte Markierungen verschwinden", gone() && !JSON.parse(app.projectQuotes()).some(q => q.annot === test.annotId),
                      JSON.stringify(panel.rows.map(r => r.quote || r.relative)))
                inspector.linksPanel.scope = "file"
                // The calendar button opens the calendar and closes it again.
                const b = window.calendarButton
                const p = b.mapToItem(null, b.width / 2, b.height / 2)
                TestDriver.postMouse(window, 0, p.x, p.y, 20)
                TestDriver.postMouse(window, 2, p.x, p.y, 60)
            },
            () => {
                check("Kalender-Knopf öffnet", editor.activeTab && editor.activeTab.kind === "calendar")
                const b = window.calendarButton
                const p = b.mapToItem(null, b.width / 2, b.height / 2)
                TestDriver.postMouse(window, 0, p.x, p.y, 20)
                TestDriver.postMouse(window, 2, p.x, p.y, 60)
            },
            () => {
                const open = editor.groups.some(g => g.tabs.some(t => t.kind === "calendar"))
                check("Kalender-Knopf schließt", !open && (!editor.activeTab || editor.activeTab.kind !== "calendar"), names())
            },
            () => {
                // Context menu: only what applies, no settings, no duplicates.
                const m = fileActions.contextMenu
                fileActions.showMenu([path("docs/umbenannt.pdf")], path("docs"))
                const pdf = m.shownEntries()
                m.close()
                check("Kontextmenü PDF: nur Passendes", pdf.indexOf("Öffnen") >= 0 && pdf.indexOf("Umbenennen …") >= 0
                      && pdf.indexOf("In den Papierkorb") >= 0 && pdf.indexOf("Im Editor öffnen") < 0
                      && pdf.indexOf("Terminal als Tab öffnen") < 0 && pdf.indexOf("Externen Editor festlegen …") < 0, pdf.join(" | "))
                fileActions.showMenu([path("notes/idee.md")], path("notes"))
                const md = m.shownEntries()
                m.close()
                check("Kontextmenü Notiz: externer Editor", md.indexOf("Im Editor öffnen") >= 0, md.join(" | "))
                fileActions.showMenu([path("notes/todo.md")], path("notes"))
                const withList = m.shownEntries()
                m.close()
                check("Kontextmenü: „Checkliste als Aufgaben“ nur bei einer Checkliste",
                      withList.indexOf("Checkliste als Aufgaben übernehmen …") >= 0 && md.indexOf("Checkliste als Aufgaben übernehmen …") < 0,
                      withList.join(" | ") + " // " + md.join(" | "))
                fileActions.showMenu([], path("notes"))
                const empty = m.shownEntries()
                m.close()
                check("Kontextmenü leere Fläche: keine Datei-Befehle", empty.indexOf("Neue Datei …") >= 0 && empty.indexOf("Terminal hier öffnen") >= 0
                      && ["Öffnen", "Umbenennen …", "Kopieren", "Ausschneiden", "In den Papierkorb", "Pfad kopieren"].every(t => empty.indexOf(t) < 0),
                      empty.join(" | "))
                // No system program or file manager entries; new files and
                // folders only on folders.
                const gone = ["Mit Standardprogramm öffnen", "Im Dateimanager zeigen"]
                check("Kontextmenü Datei: ohne Startprogramm, Dateimanager, Neu", gone.concat(["Neue Datei …", "Neuer Ordner …"]).every(t => pdf.indexOf(t) < 0 && md.indexOf(t) < 0),
                      pdf.join(" | "))
                fileActions.showMenu([path("docs")], app.projectRoot)
                const folder = m.shownEntries()
                m.close()
                check("Kontextmenü Ordner: Neu, ohne Startprogramm und Dateimanager", folder.indexOf("Neue Datei …") >= 0
                      && folder.indexOf("Neuer Ordner …") >= 0 && gone.every(t => folder.indexOf(t) < 0), folder.join(" | "))
            },
            () => {
                // HTML rendered as a web page (built-in action).
                check("HTML-Aktion im Kontextmenü", (() => {
                    fileActions.showMenu([path("docs/seite.html")], path("docs"))
                    const e = fileActions.contextMenu.shownEntries()
                    fileActions.contextMenu.close()
                    return e.indexOf("Als Webseite anzeigen") >= 0
                })())
                fileActions.renderHtml(path("docs/seite.html"))
            },
            () => {
                const v = editor.activeTab.viewerItem
                if (waitFor(() => v && v.webItem && !v.webItem.loading && v.webItem.title === "Testseite")) return
                check("HTML gerendert", !!v.webItem && v.webItem.title === "Testseite", v.webItem ? v.webItem.title : "kein Web")
                v.webItem.runJavaScript("document.getElementById('js').textContent + '|' + getComputedStyle(document.getElementById('kopf')).color",
                                        (r) => test.htmlProbe = r)
                test.tabsBefore = editor.groups.reduce((n, g) => n + g.tabs.length, 0)
            },
            () => {
                if (waitFor(() => test.htmlProbe !== "")) return
                check("HTML: Skript und CSS wirken", test.htmlProbe === "mit Skript|rgb(1, 2, 3)", test.htmlProbe)
                editor.activeTab.viewerItem.showSourceRequested()
            },
            () => {
                const tab = editor.activeTab
                check("HTML: Quelltext im selben Tab", tab.path === path("docs/seite.html") && !!tab.viewerItem.editorItem === false
                      && tab.viewerItem.text !== undefined && tab.viewerItem.text.indexOf("<h1 id=\"kopf\">") >= 0, tab.kind)
                fileActions.renderHtml(path("docs/seite.html"))
            },
            () => {
                const tabs = editor.groups.reduce((n, g) => n + g.tabs.length, 0)
                check("HTML: wieder gerendert, kein zweiter Tab", !!editor.activeTab.viewerItem.webItem && tabs === test.tabsBefore, tabs + " / " + test.tabsBefore)
                check("HTML: Ansicht in der Sitzung", editor.activeTab.viewState().processor === "atlas:html", JSON.stringify(editor.activeTab.viewState()))
                // Saved elsewhere: the page reloads by itself.
                check("HTML geändert", app.appendToNote(path("docs/seite.html"), "<script>document.title = 'Neu geladen'</script>") === "")
            },
            () => {
                const v = editor.activeTab.viewerItem
                if (waitFor(() => v.webItem && v.webItem.title === "Neu geladen")) return
                check("HTML lädt beim Speichern neu", v.webItem.title === "Neu geladen", v.webItem.title)
                check("Eigene Seite ohne Netz-Hinweis", !v.usesRemote)
                // A page loading from the web: blocked, with the choice.
                fileActions.renderHtml(path("docs/web.html"))
            },
            () => {
                const v = editor.activeTab.viewerItem
                check("Seite mit Web-Inhalten: Hinweis", v.usesRemote && !v.allowRemote, v.usesRemote + " " + v.allowRemote)
                v.allowRemote = true
                check("„Trotzdem laden“ gilt für die Datei", editor.activeTab.viewState().remote === true, JSON.stringify(editor.activeTab.viewState()))
                // Errors of the page show in the view.
                fileActions.renderHtml(path("docs/dyn.html"))
            },
            () => {
                const v = editor.activeTab.viewerItem
                if (waitFor(() => v.pageErrors && v.pageErrors.some(e => e.indexOf("initSqlJs is not defined") >= 0))) return
                check("Seitenfehler in der Ansicht", v.pageErrors.some(e => e.indexOf("initSqlJs is not defined") >= 0)
                      && v.usesRemote, JSON.stringify(v.pageErrors))
                // Thumbnails only for formats Qt reads here.
                check("Bildformate von Qt", app.imageFormats.indexOf("png") >= 0 && app.imageFormats.indexOf("jpg") >= 0, app.imageFormats.join(","))
            },
            () => {
                // A PDF changed since its annotations: notice only if there are any.
                editor.open(path("docs/zweite.pdf"), 0, "tab")
            },
            () => {
                const v = editor.activeTab.viewerItem
                if (waitFor(() => v.annotationStore.status === "new")) return
                test.annotId = v.annotationStore.add(JSON.stringify({ kind: "bookmark", page: 1, title: "Merk" }))
                editor.activeGroup.closeTab(editor.activeGroup.tabs.indexOf(editor.activeTab))
                check("PDF verändert", app.appendToNote(path("docs/zweite.pdf"), "% geändert") === "")
            },
            () => {
                editor.open(path("docs/zweite.pdf"), 0, "tab")
            },
            () => {
                const v = editor.activeTab.viewerItem
                if (waitFor(() => v.annotationStore.status === "changed")) return
                check("Geänderte PDF mit Anmerkung: Hinweis", v.changeNotice && v.annotationStore.count === 1, v.annotationStore.status)
                v.annotationStore.remove(test.annotId)
                editor.activeGroup.closeTab(editor.activeGroup.tabs.indexOf(editor.activeTab))
                app.appendToNote(path("docs/zweite.pdf"), "% nochmal")
            },
            () => {
                editor.open(path("docs/zweite.pdf"), 0, "tab")
            },
            () => {
                const v = editor.activeTab.viewerItem
                // (Without annotations the store forgets the PDF: "new".)
                if (waitFor(() => ["exact", "new"].indexOf(v.annotationStore.status) >= 0)) return
                check("Geänderte PDF ohne Anmerkungen: kein Hinweis", !v.changeNotice && v.annotationStore.count === 0, v.annotationStore.status)
            },
            () => {
                // Links of the PDF: one to page 2, one to a web address.
                const v = editor.activeTab.viewerItem
                if (waitFor(() => (v.linksByPage[0] || []).length === 2)) return
                const jump = v.linkAt(0, 100, 842 - 415)
                const web = v.linkAt(0, 100, 842 - 315)
                check("PDF-Links gelesen", !!jump && jump.target === 1 && Math.abs(jump.top - 342) < 1 && !!web && web.uri === "https://example.org/",
                      JSON.stringify(v.linksByPage))
                check("Kein Link neben den Links", v.linkAt(0, 400, 842 - 415) === null)
                v.goToPage(0)
                v.followLink(jump)
                check("Link springt zur Zielseite", v.currentPage === 1, v.currentPage)
            },
            () => {
                // Eraser: a drag over two strokes removes both, a third
                // stays; one undo step brings both back.
                const v = editor.activeTab.viewerItem
                v.goToPage(0)
                const line = (y) => v.addAnnot(JSON.stringify({ kind: "ink", page: 1, ink: [[[20, y], [200, y]]], color: "#ff6b6b", width: 2 }))
                test.erase = [line(100), line(140), line(400)]
                v.tool = "erase"
            },
            () => {
                const v = editor.activeTab.viewerItem
                const paper = v.pagesView.itemAtIndex(0).paperItem
                const at = (x, y) => paper.mapToItem(null, x * v.zoomScale, y * v.zoomScale)
                const pts = [[0, 60, 80], [1, 60, 110], [1, 60, 150], [1, 60, 165], [2, 60, 170]]
                pts.forEach((e, i) => { const q = at(e[1], e[2]); TestDriver.postMouse(window, e[0], q.x, q.y, 40 * (i + 1)) })
            },
            () => {
                const v = editor.activeTab.viewerItem
                const store = v.annotationStore
                const there = test.erase.map(id => !!store.annotation(id))
                check("Radierer löscht die berührten Striche ganz", !there[0] && !there[1] && there[2], JSON.stringify(there))
                const last = v.undoStack[v.undoStack.length - 1]
                check("Radier-Zug ist ein Rückgängig-Schritt", !!last && last.type === "remove" && last.parts.length === 2)
                v.undo()
                check("Strg+Z holt beide Striche zurück", test.erase.every(id => !!store.annotation(id)))
                v.tool = "select"
                for (const id of test.erase) store.remove(id)
            },
            () => {
                // Formula as LaTeX: a box dragged with the ∑ tool goes to the
                // built-in model (here the test models, which spell
                // \frac{a}{b}) – with real ONNX Runtime.
                const v = editor.activeTab.viewerItem
                v.goToPage(0)
                v.tool = "latex"
                const paper = v.pagesView.itemAtIndex(0).paperItem
                const at = (x, y) => paper.mapToItem(null, x * v.zoomScale, y * v.zoomScale)
                const a = at(72, 130), b = at(97, 145), c = at(122, 160)
                TestDriver.postMouse(window, 0, a.x, a.y, 30)
                TestDriver.postMouse(window, 1, b.x, b.y, 60)
                TestDriver.postMouse(window, 2, c.x, c.y, 90)
            },
            () => {
                const v = editor.activeTab.viewerItem
                if (waitFor(() => v.latexShown && !v.latexBusy)) return
                check("Formel als LaTeX: Rahmen aufziehen erkennt die Formel (eingebautes Modell)",
                      v.latexShown && v.latexText === "\\frac{a}{b}" && !v.latexError, v.latexText + " · " + v.latexError)
                check("Formel als LaTeX: Werkzeug bleibt aktiv, kein Rahmen übrig", v.tool === "latex" && v.latexBox === null)
                v.tool = "select"
            },
            () => {
                // Text of a page ("T"): the formula "x = y" in CMMI/CMR on
                // page 2 comes as LaTeX from the (test) model.
                const v = editor.activeTab.viewerItem
                v.showPageText(1)
            },
            () => {
                const v = editor.activeTab.viewerItem
                if (waitFor(() => v.pageTextShown && !v.pageTextBusy && v.pageText.indexOf("$") >= 0)) return
                check("Text der Seite: Formel als LaTeX eingesetzt, übriger Text bleibt",
                      v.pageText.indexOf("Formel $\\frac{a}{b}$") >= 0 && v.pageText.indexOf("Seite zwei Text") >= 0 && !v.pageTextError,
                      JSON.stringify(v.pageText) + " · " + v.pageTextError)
                v.hidePageText()
            },
            () => {
                // Presentation (like PowerPoint): the same viewer full screen,
                // one page at a time; arrows, click and wheel turn pages.
                const v = editor.activeTab.viewerItem
                v.zoom = 1.5
                test.fullscreenTab = editor.activeTab
            },
            () => {
                const v = test.fullscreenTab.viewerItem
                v.goToPage(1)
                v.forceActiveFocus()
                if (waitFor(() => v.currentPage === 1)) return
                // F5: from the start (wherever the focus is).
                TestDriver.sendKey(window, Qt.Key_F5, 0)
            },
            () => {
                const v = test.fullscreenTab.viewerItem
                const w = v.bodyItem.Window.window
                if (waitFor(() => v.presenting && v.bodyItem.width === w.width && w.visible && slideAlone(v, 0))) return
                check("Präsentation (F5): eigenes Vollbildfenster, von Seite 1, nur diese Seite zu sehen",
                      v.presenting && w === v.presentationWindowItem && v.bodyItem.width === w.width && v.bodyItem.height === w.height
                      && v.slide === 0 && slideAlone(v, 0), [v.presenting, w === window, v.slide, slideAlone(v, 0)].join(" "))
                check("Präsentation: keine Leiste, keine Statuszeile", !v.toolbarShown && !v.statusShown)
                // The tab resizing its viewer (e.g. when the main window changes
                // as the presentation appears) leaves the presentation alone.
                const before = v.width
                v.width = 120
                check("Präsentation: Größenänderung im Tab erreicht sie nicht", v.bodyItem.width === w.width && v.bodyItem.height === w.height,
                      v.bodyItem.width + "×" + v.bodyItem.height + " in " + w.width + "×" + w.height)
                v.width = before
                TestDriver.sendKey(w, Qt.Key_Right, 0)
            },
            () => {
                const v = test.fullscreenTab.viewerItem
                const w = v.presentationWindowItem
                if (waitFor(() => v.slide === 1 && slideAlone(v, 1))) return
                check("Präsentation: Pfeil rechts zeigt die nächste Seite allein", v.slide === 1 && slideAlone(v, 1), v.slide)
                TestDriver.sendKey(w, Qt.Key_Right, 0)
                check("Präsentation: nach der letzten Seite bleibt sie stehen", v.slide === 1, v.slide)
                TestDriver.sendKey(w, Qt.Key_Left, 0)
                check("Präsentation: Pfeil links zurück", v.slide === 0, v.slide)
                TestDriver.postMouse(w, 0, w.width / 2, w.height / 2, 10)
                TestDriver.postMouse(w, 2, w.width / 2, w.height / 2, 30)
            },
            () => {
                const v = test.fullscreenTab.viewerItem
                const w = v.presentationWindowItem
                if (waitFor(() => v.slide === 1)) return
                check("Präsentation: Klick zeigt die nächste Seite", v.slide === 1 && slideAlone(v, 1), v.slide)
                TestDriver.sendWheel(w, w.width / 2, w.height / 2, 120, 0)
                check("Präsentation: Mausrad hoch zeigt die vorige Seite", v.slide === 0, v.slide)
                TestDriver.sendKey(w, Qt.Key_2, 0)
                TestDriver.sendKey(w, Qt.Key_Return, 0)
                check("Präsentation: Zahl + Enter springt zur Seite", v.slide === 1, v.slide)
                TestDriver.sendKey(w, Qt.Key_B, 0)
                check("Präsentation: B macht den Bildschirm schwarz", v.blank === "black")
                TestDriver.sendKey(w, Qt.Key_Space, 0)
                check("Präsentation: nächste Taste hebt Schwarz auf, ohne zu blättern", v.blank === "" && v.slide === 1, v.blank + " " + v.slide)
                TestDriver.sendKey(w, Qt.Key_Escape, 0)
            },
            () => {
                const v = test.fullscreenTab.viewerItem
                if (waitFor(() => !v.presenting && v.currentPage === 1)) return
                check("Esc beendet die Präsentation: zurück im Tab, vorheriger Zoom, zuletzt gezeigte Seite",
                      !v.presenting && v.bodyItem.Window.window === window && v.bodyItem.width === v.width && v.zoom === 1.5 && v.currentPage === 1 && editor.activeTab === test.fullscreenTab && v.toolbarShown,
                      [v.presenting, v.Window.window === window, v.zoom, v.currentPage].join(" "))
                v.zoom = 0
            },
            () => {
                // Issue #9: switching tabs moves the keyboard focus to the tab
                // switched to (here: PDF → folder → PDF).
                const g = editor.activeGroup
                test.fullscreenTab.viewerItem.forceActiveFocus()
                test.folderTab = g.openTab(path("src"), 0, "tab")
            },
            () => {
                const g = editor.activeGroup
                const view = test.folderTab.viewerItem
                if (waitFor(() => !!view && hasFocusIn(view))) return
                check("Tab-Wechsel: der neue Tab (Ordner) bekommt den Fokus", hasFocusIn(view) && view.activeView.activeFocus,
                      String(window.activeFocusItem))
                g.current = g.tabs.indexOf(test.fullscreenTab)
            },
            () => {
                const g = editor.activeGroup
                const v = test.fullscreenTab.viewerItem
                if (waitFor(() => hasFocusIn(v))) return
                check("Tab-Wechsel zurück: die PDF hat den Fokus, nicht mehr der Ordner",
                      hasFocusIn(v) && !hasFocusIn(test.folderTab.viewerItem), String(window.activeFocusItem))
                g.closeTab(g.tabs.indexOf(test.folderTab))
            },
            () => {
                // Quoting text with a formula: it goes in as LaTeX (test model:
                // \frac{a}{b}); a figure of the page goes in as picture.
                const v = test.fullscreenTab.viewerItem
                v.selection = [{ page: 1, rects: [[60, 215, 200, 40]], text: "Formel x = y" }]
                v.quoteSelection()
            },
            () => {
                const v = test.fullscreenTab.viewerItem
                const quoted = () => v.annots.find(a => a.page === 2 && a.kind === "highlight")
                if (waitFor(() => !!quoted() && sendToNote.visible)) return
                const a = quoted()
                check("Zitat aus PDF: Formel als LaTeX im Zitat", !!a && a.text.indexOf("$\\frac{a}{b}$") >= 0, a ? a.text : "kein Zitat")
                sendToNote.close()
                if (a) v.removeAnnot(a.id)
                // (quelle.pdf was renamed further up.)
                const md = app.pdfAreaToNote(path("docs/umbenannt.pdf"), 2, JSON.stringify([60, 200, 300, 300]), path("notes/zitate.md"))
                const m = /^!\[umbenannt\.pdf, S\. 2\]\(\.\.\/(assets\/umbenannt-s2-[0-9-]+\.png)\)\n\*\[umbenannt\.pdf, S\. 2\]\(\/docs\/umbenannt\.pdf#page=2\)\*$/.exec(md)
                check("Bild aus PDF als Zitat: PNG in assets/, Bildunterschrift mit Link auf die Seite", !!m && app.exists(path(m[1])), md)
            },
            () => {
                // Arrow keys: down scrolls, right/left turn pages.
                const v = editor.activeTab.viewerItem
                v.goToPage(0)
                v.forceActiveFocus()
                const y0 = v.pagesView.contentY
                TestDriver.sendKey(window, Qt.Key_Down, 0)
                check("Pfeil runter scrollt die PDF", v.pagesView.contentY > y0, y0 + " → " + v.pagesView.contentY)
                TestDriver.sendKey(window, Qt.Key_Right, 0)
                check("Pfeil rechts blättert weiter", v.currentPage === 1, v.currentPage)
                TestDriver.sendKey(window, Qt.Key_Left, 0)
                check("Pfeil links blättert zurück", v.currentPage === 0, v.currentPage)
                // Undo and redo of annotations (#6).
                const store = v.annotationStore
                const n0 = store.count
                const id = v.addAnnot(JSON.stringify({ kind: "ink", page: 1, ink: [[[10, 10], [40, 40]]], color: "#ff6b6b", width: 2 }))
                check("Strich gezeichnet", store.count === n0 + 1)
                TestDriver.sendKey(window, Qt.Key_Z, Qt.ControlModifier)
                check("Strg+Z nimmt den Strich zurück", store.count === n0 && !store.annotation(id), store.count)
                TestDriver.sendKey(window, Qt.Key_Z, Qt.ControlModifier | Qt.ShiftModifier)
                check("Strg+Umschalt+Z stellt ihn wieder her (gleiche ID)", store.count === n0 + 1 && !!store.annotation(id), store.count)
                v.updateAnnot(id, JSON.stringify({ color: "#5ac8fa" }))
                v.undo()
                check("Rückgängig: Farbe wieder wie vorher", JSON.parse(store.annotation(id)).color === "#ff6b6b", store.annotation(id))
                v.removeAnnot(id)
                v.undo()
                check("Rückgängig: Löschen zurückgenommen", !!store.annotation(id) && v.canRedo)
                v.redo()
                check("Wiederholen: wieder gelöscht", !store.annotation(id) && store.count === n0)
                // Touchpad momentum (#4): a quick swipe glides on.
                v.goToPage(0)
                const tp = v.mapToItem(null, v.width / 2, v.height * 0.6)
                test.flingY = v.pagesView.contentY
                test.flings = v.fling.count
                TestDriver.sendWheel(window, tp.x, tp.y, -40, 2 | 4 | 16)
                for (let i = 0; i < 5; ++i) TestDriver.sendWheel(window, tp.x, tp.y, -40, 2 | 16)
                // Fingers lifted (Wayland's axis stop): the momentum starts at
                // once, not after a pause.
                TestDriver.sendWheel(window, tp.x, tp.y, 0, 2 | 16 | 32)
                check("Touchpad: Schwung setzt beim Loslassen sofort ein", v.fling.count === test.flings + 1 && v.fling.gliding,
                      v.fling.count - test.flings)
                // libinput reports a "hold" while the fingers leave the touchpad:
                // that must not stop the momentum it just started.
                app.touchpadHeld()
                check("Touchpad: Finger abheben stoppt den Schwung nicht", v.fling.gliding)
                test.flingDirect = v.pagesView.contentY - test.flingY
                // Markdown → PDF with the preview (no pandoc).
                test.mdPdf = app.projectRoot.replace(/\/[^/]+$/, "") + "/export-idee.pdf"
                check("Markdown-PDF-Export startet", fileActions.markdownPdf.exportPdf(path("notes/idee.md"), test.mdPdf))
            },
            () => {
                // The context bar is shown for the module checks below.
                test.inspectorWasVisible = window.inspectorVisible
                window.inspectorVisible = true
                if (waitFor(() => !fileActions.markdownPdf.busy)) return
            },
            () => { if (waitFor(() => !fileActions.markdownPdf.busy)) return },
            () => {
                if (waitFor(() => !fileActions.markdownPdf.busy)) return
                {
                    const v = editor.activeTab.viewerItem
                    check("Touchpad-Wischen läuft mit Schwung weiter", v.fling && v.fling.count === test.flings + 1
                          && v.pagesView.contentY - test.flingY > test.flingDirect + 100,
                          "direkt " + test.flingDirect + ", gesamt " + (v.pagesView.contentY - test.flingY))
                }
                check("Markdown als PDF exportiert", app.exists(test.mdPdf) && app.readTextFile(test.mdPdf).startsWith("%PDF"),
                      fileActions.markdownPdf.busy ? "läuft noch" : app.readTextFile(test.mdPdf).slice(0, 8))
                // Context bar modules: pomodoro, upcoming entries, tasks.
                const m = inspector.modules
                check("Module der Kontextleiste eingeschaltet", m.showCalendar && m.showTasks && inspector.pomodoro.visible)
                const cal = fileActions.calendar
                const now = new Date()
                const day = m.isoDate(now)
                const tomorrow = m.isoDate(new Date(now.getTime() + 86400000))
                test.modEvent = cal.save(JSON.stringify({ kind: "event", title: "Modul-Termin", start: day + "T00:00", end: day + "T23:59" }))
                test.modTomorrow = cal.save(JSON.stringify({ kind: "event", title: "Modul-Morgen", start: tomorrow + "T10:00", end: tomorrow + "T11:00" }))
                test.modTask = cal.save(JSON.stringify({ kind: "task", title: "Modul-Aufgabe" }))
                check("Heute zeigt den Termin von heute, nicht den von morgen",
                      m.today.some(o => o.title === "Modul-Termin") && !m.today.some(o => o.title === "Modul-Morgen"), JSON.stringify(m.today.map(o => o.title)))
                check("Aufgaben zeigen die neue Aufgabe", m.openTasks.some(o => o.title === "Modul-Aufgabe"))
                cal.setStatus(test.modTask, "done")
                check("Erledigte Aufgabe verschwindet", !m.openTasks.some(o => o.title === "Modul-Aufgabe"))
                app.setContextModule("calendar", 0)
                check("Kalendermodul abschaltbar", !m.showCalendar && JSON.parse(app.contextModules).calendar === false)
                app.setContextModule("calendar", 1)
                // Pomodoro at the bottom edge, on every tab.
                const p = inspector.pomodoro
                inspector.tab = 2
                const bottom = p.mapToItem(inspector, 0, p.height).y
                check("Pomodoro unten fixiert, auch auf anderen Reitern", p.visible && Math.abs(inspector.height - bottom - 10) < 3,
                      bottom + " / " + inspector.height)
                inspector.tab = 0
                app.setContextModule("pomodoro", 0)
                check("Pomodoro abschaltbar", !p.visible)
                app.setContextModule("pomodoro", 1)
                p.startPause()
                check("Pomodoro läuft", p.running && p.phase === "work")
                p.endsAt = Date.now()
            },
            () => {
                const p = inspector.pomodoro
                check("Pomodoro: nach dem Lernen kommt die Pause", !p.running && p.phase === "break" && p.rounds === 1 && p.remaining === 5 * 60,
                      p.phase + " " + p.rounds + " " + p.remaining)
                fileActions.calendar.remove(test.modTomorrow)
                window.inspectorVisible = test.inspectorWasVisible
                // Natural sorting (#5).
                editor.open(app.projectRoot.replace(/\/[^/]+$/, "") + "/nummern", 0, "tab")
            },
            () => {
                const f = editor.activeTab.viewerItem
                if (waitFor(() => f.folderModel && f.folderModel.count === 3)) return
                const names = () => [0, 1, 2].map(i => f.folderModel.pathAt(i).replace(/.*\//, "")).join(" ")
                check("Natürliche Sortierung: 2 vor 10", names() === "1.data.pdf 2.data.pdf 10.data.pdf", names())
                app.naturalSort = false
            },
            () => {
                const f = editor.activeTab.viewerItem
                const names = () => [0, 1, 2].map(i => f.folderModel.pathAt(i).replace(/.*\//, "")).join(" ")
                if (waitFor(() => names() === "1.data.pdf 10.data.pdf 2.data.pdf")) return
                check("Ohne natürliche Sortierung: nach Zeichen", names() === "1.data.pdf 10.data.pdf 2.data.pdf", names())
                app.naturalSort = true
                f.sortBy("size")
                check("Gewählte Sortierung gemerkt", JSON.parse(app.folderSort).key === "size", app.folderSort)
                f.sortBy("name")
                // Scroll sync of the Markdown split view, both ways (#7).
                editor.open(app.projectRoot.replace(/\/[^/]+$/, "") + "/lang.md", 0, "tab")
                editor.activeTab.viewerItem.mode = "split"
            },
            () => {
                const md = editor.activeTab.viewerItem
                if (waitFor(() => md.pageReady)) return
                test.syncMid = Math.round(Math.min(md.editorItem.viewHeight, md.previewItem.height) / 2)
            },
            () => {
                // Opened in the split view: lined up at once, without scrolling.
                const md = editor.activeTab.viewerItem
                md.previewItem.runJavaScript("atlas.exactLineAt(" + test.syncMid + ")", (line) => {
                    const e = md.editorItem.exactLineAt(test.syncMid)
                    check("Split: nach dem Öffnen gleich ausgerichtet", Math.abs(line - e) < 1, "Vorschau " + line + ", Editor " + e)
                    md.editorItem.scrollToExactLine(81, test.syncMid)
                })
            },
            () => {
                // Lined up in the middle of the view: the same line on both sides.
                const md = editor.activeTab.viewerItem
                md.previewItem.runJavaScript("atlas.exactLineAt(" + test.syncMid + ")", (line) => {
                    check("Split: Vorschau folgt dem Editor, Mitte auf gleicher Höhe", Math.abs(line - 81) < 1, line)
                    // The user scrolls the preview: the editor follows.
                    md.previewItem.runJavaScript("window.scrollBy(0, -400)")
                })
            },
            () => {
                const md = editor.activeTab.viewerItem
                md.previewItem.runJavaScript("atlas.exactLineAt(" + test.syncMid + ")", (line) => {
                    const e = md.editorItem.exactLineAt(test.syncMid)
                    check("Split: Editor folgt der Vorschau, Mitte auf gleicher Höhe", line < 78 && Math.abs(e - line) < 1, "Vorschau " + line + ", Editor " + e)
                })
            },
            () => {
                // Re-rendering the preview (after typing) leaves the editor where it is.
                const md = editor.activeTab.viewerItem
                test.flingY = md.editorItem.scrollY
                md.render()
            },
            () => {
                const md = editor.activeTab.viewerItem
                check("Split: Neurendern der Vorschau bewegt den Editor nicht", Math.abs(md.editorItem.scrollY - test.flingY) < 1,
                      test.flingY + " → " + md.editorItem.scrollY)
            },
            () => {
                // Over the editor: the mouse wheel scrolls as before, a
                // touchpad swipe glides on (#4).
                const ed = editor.activeTab.viewerItem.editorItem
                ed.scrollToExactLine(1)
                const p = ed.mapToItem(null, ed.width / 2, ed.height / 2)
                test.flingY = ed.scrollY
                test.flings = ed.fling.count
                TestDriver.sendWheel(window, p.x, p.y, -120, 0)
            },
            () => {
                // (Qt animates wheel scrolling: checked a step later.)
                const ed = editor.activeTab.viewerItem.editorItem
                check("Editor: Mausrad scrollt weiter, ohne Schwung", ed.scrollY > test.flingY && ed.fling.count === test.flings,
                      test.flingY + " → " + ed.scrollY)
                const p = ed.mapToItem(null, ed.width / 2, ed.height / 2)
                TestDriver.sendWheel(window, p.x, p.y, -30, 2 | 4 | 16)
                for (let i = 0; i < 5; ++i) TestDriver.sendWheel(window, p.x, p.y, -30, 2 | 16)
            },
            () => {
                const ed = editor.activeTab.viewerItem.editorItem
                check("Editor: Touchpad-Schwung", ed.fling.count === test.flings + 1, ed.fling.count)
                // Fingers put down on the touchpad (hold gesture) stop it.
                ed.scrollToExactLine(60)
                ed.fling.glide(0, -4)
                ed.fling.flingStart = 0
                const was = ed.fling.gliding
                app.touchpadHeld()
                check("Finger auflegen stoppt den Schwung", was && !ed.fling.gliding, was + " " + ed.fling.gliding)
                fileActions.calendar.remove(test.modEvent)
            },
            () => {
                // A file deleted while it is open: its tab closes.
                test.goneFile = fileActions.ops.newFile(path("docs"), "weg.txt")
                editor.open(test.goneFile, 0, "tab")
            },
            () => {
                const open = (p) => editor.groups.some(g => g.tabs.some(t => t.path === p))
                if (waitFor(() => open(test.goneFile))) return
                check("Datei zum Löschen geöffnet", open(test.goneFile), test.goneFile)
                fileActions.ops.moveToTrash([test.goneFile])
            },
            () => {
                const open = (p) => editor.groups.some(g => g.tabs.some(t => t.path === p))
                if (waitFor(() => !open(test.goneFile))) return
                check("In den Papierkorb: ihr Tab schließt sich", !open(test.goneFile))
                // Deleted elsewhere (the tab shows a file that is no longer there).
                test.goneFile = path("docs/schon-weg.txt")
                editor.open(test.goneFile, 0, "tab")
                test.goneOpened = open(test.goneFile)
            },
            () => {
                const open = (p) => editor.groups.some(g => g.tabs.some(t => t.path === p))
                if (waitFor(() => !open(test.goneFile))) return
                check("Außerhalb gelöscht: Tab schließt sich (ohne ungespeicherte Änderungen)", test.goneOpened && !open(test.goneFile),
                      "geöffnet: " + test.goneOpened)
            },
            () => {
                // ---- ILIAS (against scripts/ilias-mock.py): read the courses.
                check("ILIAS ist der letzte Ort", placesList.paths[placesList.paths.length - 1] === "ilias:", placesList.names.join(" | "))
                editor.open("ilias:", 0, "tab")
                const il = fileActions.ilias
                check("ILIAS-Tab offen", editor.activeTab.kind === "ilias" && !!editor.activeTab.viewerItem)
                // The university login, remembered (fake systemd-creds).
                settingsDialog.rememberIliasLogin("uxtest", "geheim")
            },
            () => {
                const saved = () => JSON.parse(IliasBackend.config).login_saved === true
                if (waitFor(saved)) return
                const cred = app.readTextFile(app.stateDir() + "/secrets/ilias.cred")
                check("ILIAS: Anmeldung nur verschlüsselt gespeichert", saved() && cred.length > 0 && cred.indexOf("geheim") < 0
                      && !app.exists(app.stateDir() + "/secrets/ilias.secret") && app.readTextFile(app.configDir() + "/config.toml").indexOf("geheim") < 0,
                      settingsDialog.iliasLoginMessage)
                fileActions.ilias.refresh()
            },
            () => {
                const il = fileActions.ilias
                if (waitFor(() => il.state === "login" && il.loginVisible && il.offerHost !== "")) return
                check("ILIAS: ohne Sitzung wird nach der Anmeldung gefragt", il.state === "login" && il.loginVisible, il.state + " " + il.status)
                check("ILIAS: fragt einmal, ob die Anmeldung auf dieser Seite eingesetzt werden darf", il.offerHost === il.host, il.offerHost)
                il.allowLoginHere()
            },
            () => { const il = fileActions.ilias; if (waitFor(() => !il.loginVisible && il.state === "idle" && il.index.courses.length > 0)) return },
            () => {
                const il = fileActions.ilias
                if (waitFor(() => !il.loginVisible && il.state === "idle" && il.index.courses.length > 0)) return
                const names = (list) => (list || []).map(n => n.name).sort().join(" | ")
                const course = il.index.courses.find(c => c.name === "Höhere Mathematik I")
                check("ILIAS: nach der Anmeldung nur die Favoriten eingelesen (nicht „Meine Kurse“)", !il.loginVisible && names(il.index.courses) === "Höhere Mathematik I | Tutorium 3",
                      il.state + " " + il.status + " " + names(il.index.courses))
                check("ILIAS: Ordner, Datei und Sitzung im Kurs (ohne Navigation)", !!course && names(course.children) === "Organisatorisches | Sitzung 1: Einführung | Vorlesung",
                      course ? names(course.children) : "kein Kurs")
                const sess = course && course.children.find(n => n.kind === "sess")
                check("ILIAS: Sitzung am Symbol erkannt, mit Datei", !!sess && names(sess.children) === "Skript", JSON.stringify(sess))
                const ilog = app.readTextFile(IliasBackend.debugLog())
                check("ILIAS: Protokoll mit Anmeldung, Seiten und Kursen, ohne Cookie-Werte",
                      ilog.indexOf("Anmeldefenster: angemeldet") >= 0 && ilog.indexOf("Favoriten: Höhere Mathematik I (crs100)") >= 0 && ilog.indexOf("crs300") < 0
                      && ilog.indexOf("Cookies: sid") >= 0 && ilog.indexOf("sid=1") < 0, ilog.slice(-600))
                const dbLine = ilog.split("\n").filter(l => l.indexOf("Cookie-Datenbank") >= 0).pop() || "keine Zeile"
                check("ILIAS: Cookie-Datenbank des Web-Profils lesbar (für die Sitzung nach einem Neustart)",
                      dbLine.indexOf("Cookie-Datenbank des Profils:") >= 0, dbLine)
                const view = editor.activeTab.viewerItem
                view.enter(course)
                check("ILIAS: Ordner zuerst, dann Dateien", view.items.map(n => n.name).join(" | ") === "Sitzung 1: Einführung | Vorlesung | Organisatorisches",
                      view.items.map(n => n.name).join(" | "))
                view.enter(view.items.find(n => n.name === "Vorlesung"))
                view.enter(view.items.find(n => n.name === "Übungen"))
                check("ILIAS: Dateien natürlich sortiert", view.items.map(n => n.name).join(" | ") === "Blatt 2 | Blatt 10", view.items.map(n => n.name).join(" | "))
                test.iliasRoot = fileActions.ilias.downloadDir
                view.activate(view.items[0])
            },
            () => {
                // Every download asks for its folder; the last one is offered.
                const chooser = editor.activeTab.viewerItem.downloadChooser
                if (waitFor(() => chooser.visible)) return
                check("ILIAS: Download fragt nach dem Zielordner (zuletzt gewählter vorgeschlagen)",
                      chooser.visible && chooser.folderPath === test.iliasRoot && chooser.pickTitle.indexOf("Blatt 2") >= 0,
                      chooser.folderPath + " · " + chooser.pickTitle)
                chooser.chosen(test.iliasRoot, "")
                chooser.close()
            },
            () => { if (waitFor(() => !fileActions.ilias.current && fileActions.ilias.fileState(editor.activeTab.viewerItem.items ? editor.activeTab.viewerItem.items[0] : {}) === "loaded")) return },
            () => {
                const il = fileActions.ilias
                const target = test.iliasRoot + "/blatt02.pdf"
                if (waitFor(() => editor.activeTab.path === target)) return
                check("ILIAS: Datei landet direkt im gewählten Ordner und öffnet sich", app.exists(target) && editor.activeTab.path === target
                      && !app.exists(test.iliasRoot + "/Höhere Mathematik I"),
                      editor.activeTab.path + " / " + il.status)
                // Opening it again: no question, no download.
                editor.open("ilias:", 0, "tab")
                const view = editor.activeTab.viewerItem
                view.activate(view.items.find(n => n.name === "Blatt 2"))
                check("ILIAS: geladene Datei öffnet ohne Nachfrage", !view.downloadChooser.visible && editor.activeTab.path === target, editor.activeTab.path)
                // A second file, into another folder.
                editor.open("ilias:", 0, "tab")
                const v2 = editor.activeTab.viewerItem
                v2.up(10)
                v2.enter(il.index.courses.find(c => c.name === "Höhere Mathematik I"))
                test.iliasOrga = v2.items.find(n => n.name === "Organisatorisches")
                v2.activate(test.iliasOrga)
            },
            () => {
                const chooser = editor.activeTab.viewerItem.downloadChooser
                if (waitFor(() => chooser.visible)) return
                chooser.chosen(test.iliasRoot + "/Mathe", "")
                chooser.close()
            },
            () => {
                const il = fileActions.ilias
                if (waitFor(() => !il.current && il.fileState(test.iliasOrga) === "loaded")) return
                check("ILIAS: zweite Datei im anderen Ordner", il.fileState(test.iliasOrga) === "loaded" && app.exists(test.iliasRoot + "/Mathe/orga.pdf"), il.status)
                // ILIAS shows version 2 now.
                const xhr = new XMLHttpRequest()
                xhr.open("GET", il.baseUrl + "bump")
                xhr.onreadystatechange = () => { if (xhr.readyState === XMLHttpRequest.DONE) il.refresh() }
                xhr.send()
            },
            () => { const il = fileActions.ilias; if (waitFor(() => il.state === "idle" && il.status.indexOf("eingelesen") >= 0)) return },
            () => {
                const il = fileActions.ilias
                if (waitFor(() => il.state === "idle")) return
                const course = il.index.courses.find(c => c.name === "Höhere Mathematik I")
                const orga = course.children.find(n => n.name === "Organisatorisches")
                check("ILIAS: neue Version erkannt", il.fileState(orga) === "changed", il.fileState(orga) + " " + orga.info)
                test.iliasOrga = orga
                editor.open("ilias:", 0, "tab")
                il.open(orga, [course.name])
            },
            () => {
                // The new version: its earlier folder is offered.
                const chooser = editor.activeTab.viewerItem.downloadChooser
                if (waitFor(() => chooser.visible)) return
                check("ILIAS: neue Version bietet den Ordner der alten an", chooser.folderPath === test.iliasRoot + "/Mathe", chooser.folderPath)
                chooser.chosen(chooser.folderPath, "")
                chooser.close()
            },
            () => {
                const il = fileActions.ilias
                if (waitFor(() => !il.current && il.fileState(test.iliasOrga) === "loaded")) return
                const file = test.iliasRoot + "/Mathe/orga.pdf"
                check("ILIAS: neue Version geladen und ersetzt die alte", il.fileState(test.iliasOrga) === "loaded"
                      && app.readTextFile(file).indexOf("Version 2") >= 0 && !app.exists(test.iliasRoot + "/Mathe/orga (2).pdf"), il.status)
            },
            () => {
                // Signed out: the next login uses the remembered data by itself.
                const il = fileActions.ilias
                il.logout()
                il.refresh()
            },
            () => {
                const il = fileActions.ilias
                if (waitFor(() => !il.loginVisible && il.state === "idle" && il.status.indexOf("eingelesen") >= 0)) return
                check("ILIAS: nach dem Abmelden ohne Zutun wieder angemeldet (gespeicherte Anmeldung)",
                      !il.loginVisible && il.state === "idle" && il.filledOn === il.host && JSON.parse(IliasBackend.config).login_host === il.host,
                      il.state + " " + il.status + " · " + il.filledOn)
            },
            () => {
                // The project folder cannot be collapsed: a click on it keeps the tree.
                const rows = treeModel.rowOf(path("docs")) >= 0
                const header = projectTree.rootRow
                const p = header.mapToItem(null, header.width / 2, header.height / 2)
                TestDriver.postMouse(window, 0, p.x, p.y, 20)
                TestDriver.postMouse(window, 2, p.x, p.y, 60)
                check("Projektordner-Zeile ohne Pfeil", !header.hasChevron && rows)
            },
            () => {
                check("Projektordner bleibt offen", projectTree.treeList.visible && projectTree.treeList.count > 0
                      && treeModel.rowOf(path("docs")) >= 0, projectTree.treeList.count)
            },
            () => {
                // Context bar tabs: written out, scrollable when narrow.
                window.inspectorVisible = true
                window.inspectorWidth = 270
            },
            () => {
                // Narrow: the tabs not shown become icons instead of scrolling.
                const bar = inspector.tabBar
                check("Kontextleiste schmal: Reiter als Symbole, kein Seitwärtsscrollen", bar.contentWidth <= bar.width,
                      bar.contentWidth + " / " + bar.width)
                window.inspectorWidth = 400
            },
            () => {
                const bar = inspector.tabBar
                check("Kontextleiste breit: Reiter ausgeschrieben", bar.contentWidth > 300, bar.contentWidth)
                // ---- Blocks and profiles (docs/concept/kontextleiste.html).
                const L = () => inspector.layout
                check("Kontextleiste: Standard wie bisher (ohne Profile)",
                      L().name === "default" && L().overview.slice(0, 5).join() === "today,tasks,annotations,graph,actions"
                      && L().tabs.join() === "links,graph,actions" && L().pinned.join() === "pomodoro", JSON.stringify(L()))
                inspector.toggleCollapsed("graph")
                const cfg = () => JSON.parse(app.contextModules)
                check("Baustein einklappen: gespeichert in „Allgemein“", cfg().profiles.default.collapsed.indexOf("graph") >= 0
                      && inspector.isCollapsed("graph"), JSON.stringify(cfg().profiles))
                inspector.toggleCollapsed("graph")
                check("Baustein ausklappen", !inspector.isCollapsed("graph"))
                inspector.moveBlock("tasks", "overview", -1)
                check("Baustein nach oben (Alt+↑)", L().overview.slice(0, 2).join() === "tasks,today", L().overview.join())
                inspector.moveBlock("tasks", "overview", 1)
                inspector.hideBlock("annotations")
                check("Baustein ausblenden, mit Rückgängig", inspector.overviewIds.indexOf("annotations") < 0 && !!inspector.undoHide,
                      inspector.overviewIds.join())
                inspector.undoHiding()
                check("Ausblenden rückgängig", inspector.overviewIds.indexOf("annotations") >= 0, inspector.overviewIds.join())
                // Graph is a tab and in the overview: hiding it there keeps the tab.
                inspector.hideBlock("graph")
                check("Graph in der Übersicht ausblenden: Reiter bleibt", inspector.overviewIds.indexOf("graph") < 0
                      && inspector.tabIds.indexOf("graph") >= 0, inspector.tabIds.join() + " / " + inspector.overviewIds.join())
                inspector.undoHiding()
                inspector.hideTab("graph")
                check("Reiter ausblenden: Baustein bleibt in der Übersicht", inspector.tabIds.indexOf("graph") < 0
                      && inspector.overviewIds.indexOf("graph") >= 0, inspector.tabIds.join() + " / " + inspector.overviewIds.join())
                inspector.undoHiding()
                check("Reiter ausblenden rückgängig", inspector.tabIds.indexOf("graph") >= 0, inspector.tabIds.join())
                // Tabs are pages of blocks: „Aktionen“ = built-in + own actions.
                check("Reiter Aktionen aus Bausteinen", inspector.layout.pages.actions.blocks.join() === "builtin-actions,user-actions",
                      JSON.stringify(inspector.layout.pages.actions))
                inspector.moveBlockTo("tasks", "overview", "actions")
                check("Baustein in einen Reiter verschieben", inspector.layout.pages.actions.blocks.indexOf("tasks") >= 0
                      && inspector.overviewIds.indexOf("tasks") < 0, JSON.stringify(inspector.layout.pages.actions))
                inspector.moveBlockTo("tasks", "actions", "overview")
                check("… und zurück in die Übersicht", inspector.layout.pages.actions.blocks.indexOf("tasks") < 0
                      && inspector.overviewIds.indexOf("tasks") >= 0, inspector.overviewIds.join())
                inspector.addPage("Lesen", "", "")
                const own = inspector.tabIds.find(t => t.startsWith("page:"))
                check("Eigener Reiter angelegt", !!own && inspector.tabTitle(own) === "Lesen", inspector.tabIds.join())
                inspector.renamePage(own, "Lektüre")
                inspector.moveBlockTo("outline", "overview", own)
                check("Eigener Reiter: umbenannt, mit Baustein", inspector.tabTitle(own) === "Lektüre"
                      && inspector.layout.pages[own].blocks.join() === "outline", JSON.stringify(inspector.layout.pages[own]))
                inspector.hideTab(own)
                check("Eigener Reiter entfernt (samt Seite)", inspector.tabIds.indexOf(own) < 0
                      && !(JSON.parse(app.contextModules).profiles.default.pages || {})[own], inspector.tabIds.join())
                inspector.blockToTab("annotations")
                check("Baustein als eigener Reiter", inspector.tabIds.indexOf("annotations") > 0 && inspector.overviewIds.indexOf("annotations") < 0,
                      inspector.tabIds.join())
            },
            () => {
                check("Als Reiter angezeigt", inspector.tabId === "annotations", inspector.tabId)
                inspector.blockTo("annotations", "overview")
                inspector.blockTo("tasks", "pinned")
                check("Baustein unten anheften", inspector.pinnedIds.join() === "pomodoro,tasks" && inspector.overviewIds.indexOf("tasks") < 0,
                      inspector.pinnedIds.join())
                inspector.blockTo("tasks", "overview")
                // Profiles by file type: a note.
                editor.open(path("notes/plan-neu.md"), 0, "tab")
            },
            () => {
                // Profiles: a note uses „Allgemein“ until „Markdown“ is set.
                check("Markdown ohne eigenes Profil: Allgemein", inspector.kind === "markdown" && inspector.profileName === "default")
                inspector.chooseScope(true)
                check("„Gilt für: nur Markdown“ legt das Profil an", inspector.profileName === "markdown" && inspector.editTarget === "markdown",
                      inspector.profileName)
                inspector.hideBlock("today")
                const c = JSON.parse(app.contextModules)
                check("Änderung nur im Markdown-Profil, als Abweichung gespeichert",
                      inspector.overviewIds.indexOf("today") < 0
                      && ContextProfiles.layout(c, "default", []).overview.indexOf("today") >= 0
                      && c.profiles.markdown.hidden.indexOf("today") >= 0 && c.profiles.markdown.tabs === undefined,
                      JSON.stringify(c.profiles.markdown))
                // „Gilt für: alle Dateitypen“: changes every profile; undo restores all.
                inspector.chooseScope(false)
                inspector.hideBlock("tasks")
                const c2 = JSON.parse(app.contextModules)
                check("„Alle Dateitypen“ ändert Allgemein und das Markdown-Profil",
                      inspector.overviewIds.indexOf("tasks") < 0 && c2.profiles.default.hidden.indexOf("tasks") >= 0
                      && c2.profiles.markdown.hidden.indexOf("tasks") >= 0, JSON.stringify(c2.profiles))
                inspector.undoHiding()
                const c3 = JSON.parse(app.contextModules)
                check("Rückgängig stellt alle Profile wieder her", c3.profiles.default.hidden.indexOf("tasks") < 0
                      && c3.profiles.markdown.hidden.indexOf("tasks") < 0 && c3.profiles.markdown.hidden.indexOf("today") >= 0,
                      JSON.stringify(c3.profiles))
                inspector.chooseScope(true)
                // New blocks: outline and tags of the note.
                check("Gliederung der Notiz", inspector.outline.map(o => o.title).join().startsWith("Plan,Ziele"), JSON.stringify(inspector.outline))
                editor.open(path("notes/idee.md"), 0, "tab")
            },
            () => {
                check("Tags der Notiz", inspector.noteTags.length === 2 && inspector.noteTags.indexOf("rust") >= 0, JSON.stringify(inspector.noteTags))
                check("Dateiinfo", !!inspector.fileInfo && inspector.fileInfo.size > 0, JSON.stringify(inspector.fileInfo))
                editor.open(path("src/main.rs"), 0, "tab")
            },
            () => {
                check("Code: kein eigenes Profil, also Allgemein", inspector.kind === "code" && inspector.profileName === "default", inspector.kind)
                // Empty blocks (code has no outline): only the header, or hidden.
                check("Leerer Baustein klappt zu", inspector.isEmpty("outline") && inspector.isCollapsed("outline"))
                app.setContextOption("empty", "hide")
                check("Leere Bausteine ausblenden", inspector.shownIn(["outline", "graph"]).join() === "graph")
                app.setContextOption("empty", "collapse")
                // Editing mode: add a block, take a suggestion over, reset.
                inspector.customize()
                const ed = inspector.editorPanel
                check("Bearbeitungsmodus öffnet mit dem Profil der Datei", ed.active && ed.profile === "default", ed.profile)
                ed.profile = "code"
                check("Profil noch nicht eingestellt: Vorschlag", !ed.profileSet && ed.lists.overview.join() === "actions,fileinfo", ed.lists.overview.join())
                ed.adopt()
                check("Vorschlag übernommen: Code-Profil gilt", inspector.profileName === "code"
                      && inspector.overviewIds.slice(0, 2).join() === "actions,fileinfo", inspector.overviewIds.join())
                ed.addBlock("week")
                check("Baustein hinzufügen (Galerie)", inspector.overviewIds.indexOf("week") >= 0 && inspector.modules.showWeek, inspector.overviewIds.join())
                ed.place("week", "overview", 0)
                check("Baustein ziehen: an den Anfang", inspector.overviewIds[0] === "week", inspector.overviewIds.join())
                ed.place("today", "pinned", 0)
                check("Ziehen nach „Unten angeheftet“", inspector.pinnedIds[0] === "today", inspector.pinnedIds.join())
                ed.setHidden("fileinfo", true)
                check("Auge blendet aus, Platz bleibt", inspector.overviewIds.indexOf("fileinfo") < 0 && ed.overviewRows.indexOf("fileinfo") >= 0)
                // A tab's page in the editing mode.
                ed.page = "actions"
                ed.addBlock("tasks")
                check("Bearbeitungsmodus: Baustein auf den Reiter „Aktionen“", inspector.layout.pages.actions.blocks.indexOf("tasks") >= 0
                      && ed.pageRows.indexOf("tasks") >= 0, JSON.stringify(inspector.layout.pages.actions))
                ed.place("tasks", "actions", 0)
                check("Bearbeitungsmodus: auf dem Reiter ordnen", inspector.layout.pages.actions.blocks[0] === "tasks",
                      JSON.stringify(inspector.layout.pages.actions))
                ed.removeFromPage("tasks", "actions")
                check("Bearbeitungsmodus: vom Reiter entfernen", inspector.layout.pages.actions.blocks.indexOf("tasks") < 0)
                ed.newPage()
                check("Bearbeitungsmodus: neuer Reiter gewählt", ed.page.startsWith("page:") && inspector.tabIds.indexOf(ed.page) >= 0, ed.page)
                ed.renamePage("Notizen")
                check("Bearbeitungsmodus: Reiter benannt", inspector.tabTitle(ed.page) === "Notizen", inspector.tabTitle(ed.page))
                ed.removeTab(ed.page)
                check("Bearbeitungsmodus: Reiter entfernt, Übersicht gewählt", ed.page === "overview")
                ed.removeTab("actions")
                check("Reiter entfernen", inspector.tabIds.indexOf("actions") < 0, inspector.tabIds.join())
                ed.reset()
                check("Zurücksetzen: Profil gelöscht, es gilt Allgemein", inspector.profileName === "default"
                      && JSON.parse(app.contextModules).profiles.code === undefined)
                ed.finish()
                check("Fertig beendet den Bearbeitungsmodus", !ed.active)
                // Profiles off: „Allgemein“ everywhere.
                app.setContextOption("by_kind", "false")
                editor.open(path("notes/plan-neu.md"), 0, "tab")
            },
            () => {
                check("Profile nach Dateityp aus: überall Allgemein", inspector.profileName === "default", inspector.profileName)
                app.setContextOption("by_kind", "true")
                check("Profile nach Dateityp an: Markdown-Profil", inspector.profileName === "markdown", inspector.profileName)
                // Back to the shipped bar for the remaining tests.
                for (const name of Object.keys(JSON.parse(app.contextModules).profiles || {})) app.setContextProfile(name, "")
                check("Alle Profile zurückgesetzt: Leiste wie ausgeliefert", inspector.profileName === "default"
                      && inspector.layout.overview.slice(0, 5).join() === "today,tasks,annotations,graph,actions", inspector.layout.overview.join())
            },
            () => {
                // Built-in action „Ausführen“ for scripts.
                fileActions.showMenu([path("src/hallo.sh")], path("src"))
                const sh = fileActions.contextMenu.shownEntries()
                fileActions.contextMenu.close()
                fileActions.showMenu([path("notes/idee.md")], path("notes"))
                const md = fileActions.contextMenu.shownEntries()
                fileActions.contextMenu.close()
                check("„Ausführen“ nur bei Skripten", sh.indexOf("Ausführen") >= 0 && md.indexOf("Ausführen") < 0, sh.join(" | "))
                fileActions.runScript(path("src/hallo.sh"))
            },
            () => {
                const done = () => app.readTextFile(path("src/gelaufen.txt")).indexOf("lief in src") === 0
                if (waitFor(done)) return
                check("Skript lief im Terminal, im eigenen Ordner", done() && terminalPanel.visible, app.readTextFile(path("src/gelaufen.txt")))
            },
            () => {
                // "Verknüpfen mit …": two PDFs without a note.
                app.requestRelate(path("docs/zweite.pdf"))
            },
            () => {
                const target = relateDialog.results.find(r => r.relative === "docs/umbenannt.pdf")
                check("Verknüpfen: Dialog mit Dateien und Ordnern, ohne versteckte", relateDialog.visible && !!target
                      && relateDialog.results.some(r => r.kind === "dir")
                      && !relateDialog.results.some(r => r.relative.startsWith(".") || r.path === path("docs/zweite.pdf")),
                      relateDialog.results.length)
                relateDialog.pick(target)
            },
            () => {
                const related = JSON.parse(app.relatedFiles(path("docs/umbenannt.pdf")))
                const graph = JSON.parse(app.contextGraph(path("docs/zweite.pdf"), 0))
                const other = graph.nodes.findIndex(n => n.relative === "docs/umbenannt.pdf")
                check("Zwei PDFs verknüpft, im Graph", !relateDialog.visible && related.length === 1 && related[0].relative === "docs/zweite.pdf"
                      && graph.edges.some(e => e.related && ((e.from === 0 && e.to === other) || (e.to === 0 && e.from === other))),
                      JSON.stringify(related) + " " + JSON.stringify(graph.edges))
                fileActions.ops.rename(path("docs/zweite.pdf"), "zweite-neu.pdf")
            },
            () => {
                const related = JSON.parse(app.relatedFiles(path("docs/umbenannt.pdf")))
                check("Verknüpfung folgt Umbenennung", related.length === 1 && related[0].relative === "docs/zweite-neu.pdf", JSON.stringify(related))
                check("Verknüpfung gelöst", app.unrelateFiles(path("docs/umbenannt.pdf"), path("docs/zweite-neu.pdf"))
                      && app.relatedFiles(path("docs/umbenannt.pdf")) === "[]")
                fileActions.ops.rename(path("docs/zweite-neu.pdf"), "zweite.pdf")
            },
            () => {
                // Folder node: all files of docs/ hang on a node "docs".
                fileActions.showMenu([path("docs")], app.projectRoot)
                const entries = fileActions.contextMenu.shownEntries()
                fileActions.contextMenu.close()
                check("Ordner: „Als Ordner-Knoten verknüpfen“ im Menü", entries.indexOf("Als Ordner-Knoten verknüpfen") >= 0, entries.join(" | "))
                check("Ordner-Knoten angelegt", app.setFolderNode(path("docs"), true) && app.isFolderNode(path("docs")))
                // A file added later belongs to it too.
                fileActions.ops.newFile(path("docs"), "neu.txt")
                app.refreshFileList()
            },
            () => {
                const graph = JSON.parse(app.contextGraph(path("docs/zweite.pdf"), 0))
                const node = graph.nodes.findIndex(n => n.relative === "docs" && n.kind === "dir")
                const members = graph.edges.filter(e => e.related && (e.from === node || e.to === node))
                    .map(e => graph.nodes[e.from === node ? e.to : e.from].relative)
                const done = () => members.indexOf("docs/neu.txt") >= 0
                if (waitFor(done)) return
                check("Ordner-Knoten im Graph, mit allen Dateien des Ordners", node > 0 && done() && members.indexOf("docs/zweite.pdf") >= 0
                      && members.indexOf("docs/umbenannt.pdf") >= 0 && !members.some(m => m.indexOf("notes/") === 0), JSON.stringify(members))
                const related = JSON.parse(app.relatedFiles(path("docs/zweite.pdf")))
                check("Datei kennt ihren Ordner-Knoten", related.length > 0 && related[0].relative === "docs", JSON.stringify(related))
                fileActions.showMenu([path("docs")], app.projectRoot)
                const entries = fileActions.contextMenu.shownEntries()
                fileActions.contextMenu.close()
                // Two folder nodes linked with each other.
                app.setFolderNode(path("notes"), true)
                app.relateFiles(path("docs"), path("notes"))
                const g2 = JSON.parse(app.contextGraph(path("docs/zweite.pdf"), 0))
                const di = g2.nodes.findIndex(n => n.relative === "docs"), ni = g2.nodes.findIndex(n => n.relative === "notes")
                check("Ordner-Knoten untereinander verknüpft", di >= 0 && ni >= 0
                      && g2.edges.some(e => e.related && ((e.from === di && e.to === ni) || (e.from === ni && e.to === di))))
                app.unrelateFiles(path("docs"), path("notes"))
                app.setFolderNode(path("notes"), false)
                check("Ordner-Knoten auflösen", entries.indexOf("Ordner-Knoten auflösen") >= 0 && app.setFolderNode(path("docs"), false)
                      && !app.isFolderNode(path("docs")), entries.join(" | "))
            },
            () => {
                // A session in another folder must not touch this one.
                editor.open(path("notes/idee.md"), 0, "tab")
                window.saveSession()
                app.flushSession()
                test.firstRoot = app.projectRoot
                test.firstSession = app.activeSessionName
                test.layoutBefore = app.loadLayout()
                folderDialog.ask("session", app.projectRoot.replace(/\/[^/]*$/, "/zweites"), "")
                folderDialog.accept()
            },
            () => {
                check("Neue Session im anderen Ordner, nach ihm benannt", app.projectRoot.endsWith("/zweites") && app.activeSessionName === "zweites", app.projectRoot + " " + app.activeSessionName)
                editor.open(app.projectRoot + "/texte/start.md", 0, "tab")
                window.saveSession()
                app.flushSession()
                sessionMenu.clicked()
                const all = sessionMenu.sessions
                check("Sessionliste: ein Eintrag je Ordner", all.length === 2 && all[0].root === app.projectRoot && all[0].name === "zweites"
                      && all[1].root === test.firstRoot && all[1].name === test.firstSession, JSON.stringify(all.map(s => s.root + ": " + s.name)))
                sessionMenu.clicked()
                // A click on the first session: back into its project.
                sessionMenu.openRequested(test.firstRoot)
            },
            () => {
                check("Erste Session unverändert", app.projectRoot === test.firstRoot && app.activeSessionName === test.firstSession
                      && app.loadLayout() === test.layoutBefore && names().indexOf("idee.md") >= 0 && names().indexOf("start.md") < 0,
                      app.activeSessionName + ": " + names())
                // A calendar entry names a session: its folder opens.
                test.before = app.projectRoot.replace(/\/[^/]*$/, "/zweites")
                window.switchSessionByName("zweites")
            },
            () => {
                check("Kalender: „In Session wechseln“ öffnet ihren Ordner", app.projectRoot === test.before, app.projectRoot)
                window.openProject(test.firstRoot)
                check("Offene Session lässt sich nicht entfernen", !app.forgetProject(test.firstRoot))
                check("Andere Session aus der Liste entfernt", app.forgetProject(test.before)
                      && JSON.parse(app.allSessions()).length === 1, app.allSessions())
            },
            () => {
                // ---- Memory: a closed PDF tab lets go of its document (other
                // users, e.g. thumbnails of an open folder view, keep theirs).
                test.memPdf = path("docs/umbenannt.pdf")
                test.memBase = app.pdfUsers(test.memPdf)
                editor.open(test.memPdf, 0, "split")
            },
            () => {
                check("PDF-Tab hält sein Dokument offen", app.pdfUsers(test.memPdf) === test.memBase + 1, test.memBase + " → " + app.pdfUsers(test.memPdf))
                editor.activeGroup.closeCurrent()
            },
            () => {
                check("Geschlossener PDF-Tab gibt das Dokument frei", app.pdfUsers(test.memPdf) === test.memBase, test.memBase + " → " + app.pdfUsers(test.memPdf))
                // A note in edit mode starts no preview page.
                const note = fileActions.ops.newFile(path("notes"), "speicher.md")
                editor.open(note, 0, "split", { mode: "edit" })
            },
            () => {
                const md = editor.activeTab.viewerItem
                check("Markdown im Bearbeiten-Modus hat keine Vorschau (kein Browser)", md.mode === "edit" && !md.previewItem && !md.pageReady,
                      md.mode + " " + md.previewItem)
                md.mode = "preview"
            },
            () => {
                const md = editor.activeTab.viewerItem
                if (waitFor(() => md.pageReady)) return
                check("Vorschau entsteht beim ersten Anzeigen", md.pageReady && !!md.previewItem)
                md.mode = "edit"
                check("Verdeckte Vorschau wird verworfen", md.discardPreview() && !md.pageReady && !md.previewItem)
                md.mode = "preview"
            },
            () => {
                const md = editor.activeTab.viewerItem
                if (waitFor(() => md.pageReady)) return
                check("Verworfene Vorschau entsteht beim Zeigen neu", md.pageReady && !!md.previewItem)
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
                test.retry = false
                try {
                    test.steps[test.step]()
                } catch (e) {
                    test.check("Schritt " + (test.step + 1), false, e + (e.lineNumber ? " (Zeile " + e.lineNumber + ")" : "") + (e.stack ? " " + e.stack.split("\n")[0] : ""))
                }
                if (test.retry) {
                    test.attempts++
                } else {
                    test.attempts = 0
                    test.step++
                }
                return
            }
            stop()
            console.log("selftest:", test.failures === 0 ? "BESTANDEN" : test.failures + " FEHLER")
            Qt.exit(test.failures === 0 ? 0 : 1)
        }
    }
}
