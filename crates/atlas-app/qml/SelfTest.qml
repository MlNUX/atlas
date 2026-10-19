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
    property string calUid

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
                const graph2 = JSON.parse(app.contextGraph(wiki, 2))
                check("Graph mit Tiefe 2", graph2.nodes.some(n => n.depth === 2), JSON.stringify(graph2.nodes.map(n => n.relative + ":" + n.depth)))
                editor.open(wiki, 0, "tab", { backlinks: true, contextTab: 1 })
                fileActions.ops.rename(path("notes/plan.md"), "plan-neu.md")
            },
            () => {
                const text = app.readTextFile(path("notes/wiki.md"))
                check("Wiki-Link folgt Umbenennung", text.indexOf("[[/notes/plan-neu|Plan]]") > 0, text)
                const md = editor.activeTab.viewerItem
                check("Kontextpanel mit Graph", !!md && md.showBacklinks && md.viewState().contextTab === 1, JSON.stringify(md ? md.viewState() : {}))
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
