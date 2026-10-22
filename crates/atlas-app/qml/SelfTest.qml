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

    property int failures: 0
    property int step: 0
    property var steps: []
    property string annotId
    property string pdfBefore
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
        pts.forEach((e, i) => app.postMouse(window, e[0], e[1], e[2], 40 * (i + 1)))
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
                app.sendWheel(window, p.x, p.y, 120, 0)
            },
            () => {
                const panel = editor.activeTab.viewerItem.contextPanelItem
                check("Graph: Mausrad ohne Strg zoomt nicht", panel.zoom === test.zoomBefore, panel.zoom)
                // Ctrl+wheel, as Wayland reports a mouse wheel ("touchpad").
                const area = panel.graphView
                const p = area.mapToItem(null, area.width / 2, area.height / 2)
                app.sendWheel(window, p.x, p.y, 120, 17)
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
                app.sendWheel(window, p.x, p.y, 120, 17)
                app.sendWheel(window, p.x, p.y, 120, 17)
                check("Wayland: Strg+Mausrad vom „Touchpad“-Gerät zoomt", v.zoomScale > before * 1.15, before + " → " + v.zoomScale)
                // Wheel notches in scroll phases without a ScrollBegin (as from
                // some Wayland compositors) zoom too.
                const start = v.zoomScale
                app.sendWheel(window, p.x, p.y, 120, 9)
                app.sendWheel(window, p.x, p.y, 120, 9)
                check("Strg+Mausrad ohne Gestenbeginn zoomt", v.zoomScale > start * 1.15, start + " → " + v.zoomScale)
                test.zoomBefore = v.zoomScale
                app.sendWheel(window, p.x, p.y, 120, true)
                check("Strg+Mausrad vergrößert die PDF", v.zoomScale > test.zoomBefore * 1.05, test.zoomBefore + " → " + v.zoomScale)
                const zoomed = v.zoomScale
                app.sendWheel(window, p.x, p.y, -240, true)
                check("Strg+Mausrad zurück verkleinert", v.zoomScale < zoomed, zoomed + " → " + v.zoomScale)
                const plain = v.zoomScale
                const y0 = v.pagesView.contentY
                app.sendWheel(window, p.x, p.y, -120, false)
                check("Mausrad allein zoomt nicht", v.zoomScale === plain, plain + " → " + v.zoomScale)
                check("Mausrad scrollt eine große Stufe", v.pagesView.contentY - y0 >= v.wheelStep - 1, y0 + " → " + v.pagesView.contentY)
                app.sendWheel(window, p.x, p.y, 120, false)
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
                app.sendWheel(window, side.x, side.y, 120, true)
                check("Strg+Mausrad: schmale Seite bleibt mittig", v.zoomScale > test.zoomBefore && v.pagesView.contentX === v.pagesView.originX,
                      v.pagesView.contentX + " / " + v.pagesView.originX)
                v.zoom = plain
                // Touchpads (and fine scrolling on Wayland): a gesture with
                // pixel-only deltas zooms as well and never scrolls.
                app.sendWheel(window, p.x, p.y, 0, 7)
                app.sendWheel(window, p.x, p.y, 40, 3)
                app.sendWheel(window, p.x, p.y, 40, 3)
                check("Strg+Touchpad zoomt die PDF", v.zoomScale > plain * 1.1, plain + " → " + v.zoomScale)
                // Wayland: wheel notches come in scroll phases; a gesture begun
                // without Ctrl (scrolling) continues with Ctrl held.
                const touch = v.zoomScale
                app.sendWheel(window, p.x, p.y, 0, 4)
                app.sendWheel(window, p.x, p.y, -120, 8)
                app.sendWheel(window, p.x, p.y, 120, 9)
                app.sendWheel(window, p.x, p.y, 120, 9)
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
                md.editorItem.save()
                check("Abhaken gespeichert", app.readTextFile(path("notes/todo.md")).indexOf("- [x] Literatur sichten") >= 0)
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
                app.postMouse(window, 0, p.x, p.y, 20)
                app.postMouse(window, 2, p.x, p.y, 60)
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
                app.postMouse(window, 10, p.x, p.y, 20)
                app.postMouse(window, 12, p.x, p.y, 60)
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
                app.postMouse(window, 0, p.x, p.y, 20)
                app.postMouse(window, 2, p.x, p.y, 60)
                test.zoomBefore = f.iconSize
                app.sendWheel(window, p.x, p.y, 120, 17)
            },
            () => {
                const f = editor.activeTab.viewerItem
                check("Ordner: Klick wählt aus", f.selectedPaths().length === 1 && f.selectedPaths()[0] === f.folderModel.pathAt(1), f.selectedPaths().join(","))
                check("Ordner: Strg+Mausrad zoomt", f.iconSize > test.zoomBefore, test.zoomBefore + " → " + f.iconSize)
                f.viewMode = "details"
            },
            () => {
                const f = editor.activeTab.viewerItem
                check("Ordner: Detailansicht behält Auswahl", f.activeView.count === f.folderModel.count && f.activeView.currentIndex === 1
                      && f.selectedPaths().length === 1, f.activeView.currentIndex)
                const state = f.viewState()
                check("Ordner: Ansicht im Sitzungszustand", state.viewMode === "details" && state.iconSize === f.iconSize)
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
                app.postMouse(window, 0, p.x, p.y, 20)
                app.postMouse(window, 2, p.x, p.y, 60)
            },
            () => {
                check("Terminal-Knopf: ein Klick blendet aus", !terminalPanel.visible && !window.terminalButton.checked,
                      terminalPanel.visible + " / " + window.terminalButton.checked)
                const b = window.terminalButton
                const p = b.mapToItem(null, b.width / 2, b.height / 2)
                app.postMouse(window, 0, p.x, p.y, 20)
                app.postMouse(window, 2, p.x, p.y, 60)
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
                app.postMouse(window, 0, p.x, p.y, 20)
                app.postMouse(window, 2, p.x, p.y, 60)
            },
            () => {
                check("Kalender-Knopf öffnet", editor.activeTab && editor.activeTab.kind === "calendar")
                const b = window.calendarButton
                const p = b.mapToItem(null, b.width / 2, b.height / 2)
                app.postMouse(window, 0, p.x, p.y, 20)
                app.postMouse(window, 2, p.x, p.y, 60)
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
                check("Kontextmenü Notiz: externer Editor", md.indexOf("Im Editor öffnen") >= 0 && md.indexOf("Checkliste als Aufgaben übernehmen …") >= 0, md.join(" | "))
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
                // The project folder cannot be collapsed: a click on it keeps the tree.
                const rows = treeModel.rowOf(path("docs")) >= 0
                const header = projectTree.rootRow
                const p = header.mapToItem(null, header.width / 2, header.height / 2)
                app.postMouse(window, 0, p.x, p.y, 20)
                app.postMouse(window, 2, p.x, p.y, 60)
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
                const bar = inspector.tabBar
                check("Kontextleiste schmal: Reiter scrollbar", bar.contentWidth > bar.width, bar.contentWidth + " / " + bar.width)
                inspector.tab = 3
            },
            () => {
                const bar = inspector.tabBar
                check("Aktiver Reiter ins Bild gerückt", bar.contentX > 0 && bar.contentX + bar.width >= bar.contentWidth - 1, bar.contentX)
                inspector.tab = 0
                window.inspectorWidth = 400
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
