// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

ApplicationWindow {
    id: window
    width: 1440
    height: 900
    visible: true
    title: app.projectName ? app.projectName + " — Atlas" : "Atlas"
    color: Theme.window

    palette {
        window: Theme.window
        windowText: Theme.text
        base: Theme.editor
        alternateBase: Theme.sidebar
        text: Theme.text
        button: Theme.raised
        buttonText: Theme.text
        highlight: Theme.accent
        highlightedText: "white"
        placeholderText: Theme.textFaint
        toolTipBase: Theme.raised
        toolTipText: Theme.text
        mid: Theme.border
        dark: "#070b14"
        light: Theme.borderStrong
        midlight: Theme.border
    }

    AppController { id: app }

    FileActions {
        id: fileActions
        app: app
        onOpenRequested: (path, newPane) => editor.open(path, 0, newPane ? "split" : "tab")
    }
    QuickLook {
        id: quickLook
        app: app
        actions: fileActions
        onOpenRequested: (path) => editor.open(path, 0, "tab")
        onSplitRequested: (path) => editor.open(path, 0, "split")
    }
    Connections {
        target: fileActions
        function onQuickLookRequested(path, navigate) { quickLook.show(path, navigate) }
        function onRunInTerminal(command, cwd) {
            terminalPanel.visible = true
            terminalPanel.runCommand(command, cwd)
        }
        function onOpenWithState(path, state) { editor.open(path, 0, "tab", state) }
        function onExtensionsRequested() { extensionsDialog.open() }
        function onRunInTerminalTab(command, cwd) {
            editor.open("terminal:" + cwd + "#" + (++window.terminalTabs), 0, "tab", { command: command })
        }
        function onOpenTerminalRequested(dir, where) { window.openTerminal(dir, where) }
        function onSendToNoteRequested(text, cwd) { sendToNote.ask(text, cwd) }
        function onBlockToNoteRequested(heading, makeBlock, cwd, doneText, newNoteName) { sendToNote.askBlock(heading, makeBlock, cwd, doneText, newNoteName) }
        function onCalendarRequested(state) { window.openCalendar(state) }
        function onSessionRequested(name) { window.switchSessionByName(name) }
    }

    // ---------------------------------------------------------- calendar

    readonly property var calendar: fileActions.calendar
    /// The calendar tab (`state`: {mode, date, selected}).
    function openCalendar(state) { return editor.open("calendar:", 0, "tab", state || {}) }
    /// The calendar button: closes the calendar when it is the active tab,
    /// opens (or shows) it otherwise.
    function toggleCalendar() {
        const tab = editor.activeTab
        if (tab && tab.kind === "calendar" && tab.ownerGroup) {
            tab.ownerGroup.closeTab(tab.ownerGroup.tabs.indexOf(tab))
            return
        }
        openCalendar({})
    }
    readonly property alias calendarButton: calendarButton
    function newCalendarEntry(kind) {
        const tab = openCalendar({})
        Qt.callLater(() => { if (tab && tab.viewerItem && tab.viewerItem.newEntry) tab.viewerItem.newEntry({ kind: kind, allDay: kind === "deadline" }) })
    }
    function dismissReminder(index) {
        const list = reminders.slice()
        list.splice(index, 1)
        reminders = list
    }

    /// "In Session wechseln": saves the current session and opens the
    /// project folder of the session called `name`.
    function switchSessionByName(name) {
        let sessions = []
        try { sessions = JSON.parse(app.allSessions()) } catch (e) {}
        const s = sessions.find(s => s.name === name)
        if (!s) fileActions.showMessage(qsTr("Keine Session „%1“ – zuerst über „Neue Session“ einen Ordner wählen").arg(name))
        else if (s.root === app.projectRoot) fileActions.showMessage(qsTr("Session „%1“ ist bereits aktiv").arg(name))
        else openProject(s.root)
    }

    // Reminders appear while Atlas runs.
    property var reminders: []
    Timer {
        interval: 20000
        running: !!app.projectRoot
        repeat: true
        triggeredOnStart: true
        onTriggered: {
            let due = []
            try { due = JSON.parse(window.calendar.dueReminders()) } catch (e) {}
            if (due.length) window.reminders = window.reminders.concat(due)
        }
    }
    // Background synchronisation; after a failure (offline) a quicker retry
    // brings queued changes over once the connection is back.
    Timer {
        interval: window.calendar.syncFailed || window.calendar.pending > 0 ? 60000 : 5 * 60000
        running: window.calendar.connected && window.calendar.unlocked
        repeat: true
        onTriggered: window.calendar.syncNow()
    }
    property bool calendarPasswordLater: false
    Connections { target: app; function onProjectRootChanged() { window.calendarPasswordLater = false } }
    RelateDialog {
        id: relateDialog
        app: app
    }

    SendToNoteDialog {
        id: sendToNote
        app: app
        editor: editor
        actions: fileActions
    }

    // ------------------------------------------------------- universal bar

    CommandPalette {
        id: commandPalette
        app: app
        calendar: fileActions.calendar
        onCalendarEntryRequested: (uid, start) => window.openCalendar({ selected: uid, date: start ? start.slice(0, 10) : "" })
        actions: window.paletteActions()
        onOpenRequested: (path, line, where, state) => editor.open(path, line, where, state)
        onSearchRequested: (text) => { sidebar.visible = true; sidebarTabs.currentIndex = 1; searchPanel.setQuery(text) }
        onLineRequested: (line) => { if (editor.activeTab) editor.activeTab.reveal(line) }
        onAboutToShow: actions = window.paletteActions()
    }

    /// Actions of the universal bar (Ctrl+P, ">" for actions only).
    function paletteActions() {
        const current = editor.activeTab && editor.activeTab.path && !editor.activeTab.path.startsWith("terminal:")
            && !editor.activeTab.path.startsWith("calendar:") && !editor.activeTab.path.startsWith("ilias:") ? editor.activeTab.path : ""
        const dir = current ? fileActions.ops.parentDir(current) : app.projectRoot
        const list = [
            { name: qsTr("Session speichern"), hint: app.activeSessionName, run: () => { app.requestSaveAll(); saveSession(); app.flushSession(); fileActions.showMessage(qsTr("Session gespeichert")) } },
            { name: qsTr("Neue Session …"), run: () => sessionMenu.askNew() },
            { name: qsTr("Alle Dateien speichern"), hint: "Strg+S", run: () => app.requestSaveAll() },
            { name: qsTr("Projektordner öffnen …"), hint: "Strg+O", run: () => folderDialog.askProject() },
            { name: qsTr("Neue Datei …"), hint: app.relativePath(dir), run: () => fileActions.newFile(dir) },
            { name: qsTr("Neuer Ordner …"), hint: app.relativePath(dir), run: () => fileActions.newFolder(dir) },
            { name: qsTr("Projektweit suchen"), hint: "Strg+Umschalt+F", run: () => { sidebar.visible = true; sidebarTabs.currentIndex = 1; searchPanel.focusQuery() } },
            { name: qsTr("TODO und FIXME finden"), run: () => { sidebar.visible = true; sidebarTabs.currentIndex = 1; searchPanel.searchTodos() } },
            { name: qsTr("Terminal ein-/ausblenden"), hint: "Strg+`", run: () => window.toggleTerminal(true) },
            { name: qsTr("Neues Terminal"), hint: "Strg+Umschalt+`", run: () => window.openTerminal(dir, "panel") },
            { name: qsTr("Terminal als Tab öffnen"), run: () => window.openTerminal(dir, "tab") },
            { name: qsTr("Editor teilen"), hint: "Strg+\\", run: () => { if (editor.activeGroup) editor.splitGroup(editor.activeGroup) } },
            { name: qsTr("Tab schließen"), hint: "Strg+W", run: () => { if (editor.activeGroup) editor.activeGroup.closeCurrent() } },
            { name: qsTr("Seitenleiste ein-/ausblenden"), hint: "Strg+B", run: () => window.toggleSidebar() },
            { name: qsTr("Explorer zeigen"), hint: "Strg+Umschalt+E", run: () => { sidebar.visible = true; sidebarTabs.currentIndex = 0 } },
            { name: qsTr("Versteckte Dateien ein-/ausblenden"), run: () => { treeModel.showHidden = !treeModel.showHidden } },
            { name: qsTr("Papierkorb öffnen"), run: () => { const t = app.placePaths.find(p => app.fileKind(p) === "trash"); if (t) editor.open(t, 0, "tab") } },
            { name: qsTr("Externen Editor festlegen …"), run: () => fileActions.configureEditor() },
            { name: qsTr("Kalender öffnen"), hint: qsTr("Woche, Monat, Agenda"), run: () => window.openCalendar({}) },
            { name: qsTr("Kalender: Woche"), run: () => window.openCalendar({ mode: "week" }) },
            { name: qsTr("Kalender: Monat"), run: () => window.openCalendar({ mode: "month" }) },
            { name: qsTr("Kalender: Agenda"), run: () => window.openCalendar({ mode: "agenda" }) },
            { name: qsTr("Neuer Termin …"), run: () => window.newCalendarEntry("event") },
            { name: qsTr("Neuer Arbeitsblock …"), run: () => window.newCalendarEntry("block") },
            { name: qsTr("Neue Frist …"), run: () => window.newCalendarEntry("deadline") },
            { name: qsTr("Neue Aufgabe …"), run: () => window.newCalendarEntry("task") },
            { name: qsTr("Einstellungen …"), hint: qsTr("Orte, Editor, Kalender"), run: () => settingsDialog.open() },
            { name: qsTr("Kalender-Einstellungen …"), hint: "Nextcloud", run: () => fileActions.calendarSync.openSettings() },
        ]
        if (calendar.connected) {
            list.push({ name: qsTr("Kalender synchronisieren"), hint: calendar.syncStatus,
                        run: () => calendar.unlocked ? calendar.syncNow() : fileActions.calendarSync.askPassword() })
            list.push({ name: calendar.google ? qsTr("Bei Google anmelden …") : qsTr("App-Passwort für Nextcloud eingeben …"), run: () => fileActions.calendarSync.askPassword() })
        }
        if (calendar.conflictCount > 0) list.push({ name: qsTr("Kalenderkonflikte lösen …"), hint: String(calendar.conflictCount), run: () => fileActions.calendarSync.showConflicts() })
        if (current) {
            list.push({ name: qsTr("Im Editor öffnen"), hint: app.fileName(current), run: () => fileActions.openInEditor(current, 0) })
            list.push({ name: qsTr("Pfad kopieren"), hint: app.fileName(current), run: () => fileActions.copyPaths([current]) })
            list.push({ name: qsTr("Im Explorer zeigen"), hint: app.fileName(current), run: () => window.revealInTree(current) })
            if (app.isRunnable(current))
                list.push({ name: qsTr("Ausführen"), hint: app.fileName(current), run: () => fileActions.runScript(current) })
            if (fileActions.isHtml(current))
                list.push({ name: qsTr("Als Webseite anzeigen"), hint: app.fileName(current), run: () => fileActions.renderHtml(current) })
            if (app.atlasDir && app.exists(current))
                list.push({ name: qsTr("Verknüpfen mit …"), hint: app.fileName(current), run: () => app.requestRelate(current) })
            list.push({ name: qsTr("Als Aufgabe einplanen"), hint: app.fileName(current), run: () => fileActions.planAsTask(current) })
            if (app.hasChecklist(current))
                list.push({ name: qsTr("Checkliste als Aufgaben übernehmen …"), hint: app.fileName(current), run: () => fileActions.calendarSync.importChecklist(current) })
        }
        let sessions = []
        try { sessions = JSON.parse(app.allSessions()) } catch (e) {}
        for (const s of sessions) {
            if (s.root !== app.projectRoot) list.push({ name: qsTr("Session wechseln: %1").arg(s.name), hint: s.root, run: () => window.openProject(s.root) })
        }
        // User defined actions: for the active document, then for the project.
        const seen = {}
        const viewer = editor.activeTab ? editor.activeTab.viewerItem : null
        const textEditor = viewer && viewer.editorItem ? viewer.editorItem : viewer && viewer.currentLine ? viewer : null
        const line = textEditor && textEditor.currentLine ? textEditor.currentLine() : 0
        for (const ctx of current ? [[current], []] : [[]]) {
            for (const a of fileActions.actionsFor(ctx)) {
                if (seen[a.id]) continue
                seen[a.id] = true
                const paths = ctx
                list.push({ name: qsTr("Aktion: %1").arg(a.name), hint: ctx.length ? app.fileName(current) : a.source,
                            run: () => fileActions.runAction(a.id, paths, line) })
            }
        }
        list.push({ name: qsTr("Projektaktionen bearbeiten …"), run: () => fileActions.editActions(false) })
        list.push({ name: qsTr("Globale Aktionen bearbeiten …"), run: () => fileActions.editActions(true) })
        // Extensions: exporters and viewers for the active document.
        if (current) {
            for (const x of fileActions.exportersFor([current]))
                list.push({ name: qsTr("Exportieren: %1").arg(x.name), hint: "." + x.target, run: () => fileActions.exportWith(x, current) })
            for (const p of fileActions.processorsFor(current))
                list.push({ name: qsTr("Anzeigen mit: %1").arg(p.name), hint: p.extension, run: () => editor.open(current, 0, "tab", { processor: p.id }) })
        }
        list.push({ name: qsTr("Erweiterungen verwalten …"), run: () => extensionsDialog.open() })
        list.push({ name: qsTr("Neue Erweiterung …"), run: () => { extensionsDialog.open(); extensionsDialog.askNew() } })
        return list
    }

    ExtensionsDialog {
        id: extensionsDialog
        app: app
        actions: fileActions
    }

    /// Shows `path` in the project tree (expanding its folders).
    function revealInTree(path) {
        sidebar.visible = true
        sidebarTabs.currentIndex = 0
        const dirs = []
        let dir = fileActions.ops.parentDir(path)
        while (dir && dir.length >= app.projectRoot.length && dir.startsWith(app.projectRoot)) {
            dirs.push(dir)
            dir = fileActions.ops.parentDir(dir)
        }
        treeModel.restoreExpanded(dirs)
        projectTree.selectedPath = path
    }

    /// Opens a terminal in `dir`: "panel" (bottom panel), "tab" or "split".
    property int terminalTabs: 0
    function openTerminal(dir, where) {
        if (where === "panel") {
            terminalPanel.visible = true
            terminalPanel.newTerminal(dir)
        } else {
            editor.open("terminal:" + (dir || app.projectRoot) + "#" + (++terminalTabs), 0, where === "split" ? "split" : "tab")
        }
    }
    Connections {
        target: fileActions.ops
        function onMoved(from, to) {
            editor.pathMoved(from, to)
            // So do files linked with calendar entries.
            fileActions.calendar.filesMoved(from, to)
            // Links in notes follow moves and renames made in Atlas.
            app.filesMoved(from, to)
            fileListTimer.restart()
        }
        function onTrashed(paths) { editor.pathsRemoved(paths); app.filesTrashed(paths); fileListTimer.restart() }
    }
    Connections {
        target: fileActions
        function onCreated(path) { fileListTimer.restart() }
    }
    // The file list of quick open and wiki link completion follows file
    // operations in Atlas (bundled, it is re-read in the background).
    Timer { id: fileListTimer; interval: 300; onTriggered: app.refreshFileList() }

    // Natural sorting switched in the settings: the tree sorts again.
    Connections {
        target: app
        function onNaturalSortChanged() { treeModel.refresh() }
    }
    ProjectTreeModel {
        id: treeModel
        root: app.projectRoot
    }

    SearchModel { id: searchModel }

    SettingsDialog {
        id: settingsDialog
        app: app
        actions: fileActions
        onExtensionsRequested: extensionsDialog.open()
    }

    FolderChooser {
        id: folderDialog
        app: app
        function askProject() { ask("open", app.projectRoot ? parentOf(app.projectRoot) : "", "") }
        function parentOf(path) { const i = path.lastIndexOf("/"); return i > 0 ? path.slice(0, i) : "/" }
        onChosen: (path, name) => {
            // The session of a folder is opened with it; a new name names it.
            if (!window.openProject(path)) return
            if (mode === "session" && name && name !== app.activeSessionName) app.renameProjectSession(app.projectRoot, name)
            if (mode === "session") fileActions.showMessage(qsTr("Session „%1“ – %2").arg(app.activeSessionName).arg(app.projectRoot))
        }
    }

    // ---------------------------------------------------------------- session

    property bool restoring: false

    /// Restores the active session after a project was opened. Terminals are
    /// only created at startup; switching sessions keeps running terminals.
    function restoreSession() {
        restoring = true
        searchModel.clear()
        const layout = readLayout()
        applyLayout(layout)
        const term = layout.terminal || {}
        if (term.height) terminalPanel.SplitView.preferredHeight = term.height
        terminalPanel.visible = term.visible !== undefined ? term.visible : (layout.terminalVisible !== undefined ? layout.terminalVisible : true)
        if (terminalPanel.visible && terminalPanel.groups.length === 0) terminalPanel.restore(term)
        openCliFiles()
        restoring = false
    }

    /// Files from `--open FILE[:LINE]`.
    /// Only for the project Atlas started with: projects opened later get
    /// their own session, not the files from the command line.
    property bool cliFilesOpened: false
    function openCliFiles() {
        if (cliFilesOpened) return
        cliFilesOpened = true
        for (let i = 0; i < app.openFiles.length; ++i) {
            const file = app.openFiles[i]
            if (file === "calendar:") openCalendar({})
            else if (app.exists(app.absolutePath(file)))
                editor.open(app.absolutePath(file), parseInt(app.openLines[i]) || 0, editor.documents().length > 0 ? "split" : "tab")
        }
    }

    function readLayout() {
        // `--open` skips the session of the start project only.
        if (!app.restoreSession && !(cliFilesOpened && app.persistSession)) return {}
        try { return JSON.parse(app.loadLayout() || "{}") } catch (e) { return {} }
    }

    /// Applies tabs, splits, tree, sidebar and search of a session layout.
    function applyLayout(layout) {
        treeModel.collapseAll()
        if (app.restoreSession) treeModel.restoreExpanded(app.loadExpanded())
        restoreEditor(layout.editor || {})
        if (layout.sidebarWidth) sidebar.SplitView.preferredWidth = layout.sidebarWidth
        sidebar.visible = layout.sidebarVisible !== undefined ? layout.sidebarVisible : true
        inspectorVisible = layout.inspector !== undefined ? layout.inspector.visible : true
        if (layout.inspector && layout.inspector.tab !== undefined) inspector.tab = layout.inspector.tab
        if (layout.inspector && layout.inspector.width) inspector.SplitView.preferredWidth = layout.inspector.width
        sidebarTabs.currentIndex = layout.sidebarTab || 0
        searchPanel.applyState(layout.search || {})
    }

    function currentLayout() {
        return {
            version: 2,
            editor: editor.serialize(),
            sidebarVisible: sidebar.visible,
            inspector: { visible: inspectorVisible, tab: inspector.tab, width: inspector.width },
            sidebarWidth: sidebar.width,
            sidebarTab: sidebarTabs.currentIndex,
            search: searchPanel.viewState(),
            terminal: Object.assign(terminalPanel.serialize(), { visible: terminalPanel.visible, height: terminalPanel.height })
        }
    }

    // Documents that moved since the last session are re-found through the
    // project index, which is rebuilt in the background after opening.
    property var pendingEditorState: null
    function restoreEditor(state) {
        const tabs = (state.groups || []).reduce((all, g) => all.concat(g.tabs || []), state.panes || [])
        const missing = tabs.some(t => t.path && !app.exists(t.path))
        if (missing && !app.indexReady) {
            pendingEditorState = state
            editor.restore({})
            return
        }
        pendingEditorState = null
        editor.restore(state)
    }
    Connections {
        target: app
        function onIndexReadyChanged() {
            if (app.indexReady && window.pendingEditorState) {
                window.restoring = true
                window.restoreEditor(window.pendingEditorState)
                window.restoring = false
            }
        }
        function onSessionError(message) {
            sessionError.text = message
            sessionError.open()
        }
    }

    function saveSession() {
        if (!app.projectRoot || restoring || !app.persistSession) return
        app.saveSession(JSON.stringify(currentLayout()), treeModel.expandedDirs(), editor.documents())
    }

    /// Opens another project folder and so its session (one per folder);
    /// the session of the current one is saved first.
    function openProject(path) {
        if (path === app.projectRoot) return true
        app.requestSaveAll()
        saveSession()
        app.flushSession()
        return app.openProject(path)
    }


    Dialog {
        id: sessionError
        property alias text: sessionErrorLabel.text
        anchors.centerIn: Overlay.overlay
        width: 420
        modal: true
        title: qsTr("Session")
        standardButtons: Dialog.Ok
        Label { id: sessionErrorLabel; width: parent.width; wrapMode: Text.Wrap }
    }

    Timer { id: saveTimer; interval: 1500; onTriggered: window.saveSession() }
    Connections { target: app; function onProjectRootChanged() { window.restoreSession() } }
    Connections { target: editor; function onLayoutChanged() { saveTimer.restart() } }
    Connections { target: treeModel; function onRefreshCountChanged() { saveTimer.restart() } }

    onClosing: { app.requestSaveAll(); saveSession() }
    Connections {
        target: app
        function onNotesMessage(message) { fileActions.showMessage(message) }
        function onRelateRequested(path) { relateDialog.ask(path) }
    }
    Connections {
        target: Qt.application
        function onAboutToQuit() {
            app.requestSaveAll()
            window.saveSession()
            app.flushSession()
        }
    }

    Component.onCompleted: {
        if (app.projectRoot) restoreSession()
        else if (app.openFiles.length > 0) openCliFiles()
        else editor.addGroup()
        if (app.initialSearch) {
            sidebarTabs.currentIndex = 1
            searchPanel.setQuery(app.initialSearch)
        }
        if (app.screenshotPath) screenshotTimer.start()
        if (app.screenshotDialog) dialogTimer.start()
        if (app.quitAfterMs > 0) quitTimer.start()
        if (app.screenshotPath || app.quitAfterMs > 0) stallProbe.start()
    }

    // Measures the longest gap between 10 ms timer ticks, i.e. how long the
    // UI thread was blocked. Only active with the test hooks.
    Timer {
        id: stallProbe
        interval: 10
        repeat: true
        property real last: 0
        property real worst: 0
        onTriggered: {
            const now = Date.now()
            if (last > 0) worst = Math.max(worst, now - last - interval)
            last = now
        }
    }
    function reportProbe() {
        if (stallProbe.running)
            console.info("probe: max UI stall", stallProbe.worst, "ms; tree rows", treeModel.count,
                         "; search hits", searchModel.count, "in", searchModel.elapsedMs, "ms")
    }

    // Test hooks (--screenshot / --quit-after).
    // For "md-sync": line 30 of the editor in the middle, once laid out.
    Timer {
        id: syncShot
        interval: 600
        onTriggered: { const e = editor.activeTab.viewerItem.editorItem; e.scrollToExactLine(30, Math.round(e.viewHeight / 2)) }
    }
    // `--screenshot-dialog NAME`: a dialog for design checks (fixed list).
    Timer {
        id: dialogTimer
        interval: Math.max(200, app.screenshotDelayMs - 1500)
        onTriggered: {
            const first = app.openFiles.length ? app.openFiles[0] : ""
            const file = first.startsWith("/") ? first : app.projectRoot + "/" + first
            const show = {
                "sessions": () => sessionMenu.clicked(),
                "open-folder": () => folderDialog.ask("open", app.projectRoot, ""),
                "settings": () => settingsDialog.open(),
                "graph": () => inspector.graphWindow.show(file),
                "actions": () => { window.inspectorWidth = 300; inspector.tab = 3 },
                "md-edit": () => { if (editor.activeTab && editor.activeTab.viewerItem) editor.activeTab.viewerItem.mode = "split" },
                "md-sync": () => {
                    const v = editor.activeTab && editor.activeTab.viewerItem
                    if (!v) return
                    v.mode = "split"
                    syncShot.start()
                },
                "html": () => fileActions.renderHtml(file),
                "md-pdf": () => fileActions.markdownToPdf(file),
                "pomodoro": () => { inspector.pomodoro.startPause(); inspector.pomodoro.endsAt = Date.now() + (17 * 60 + 42) * 1000; inspector.pomodoro.rounds = 2 },
                "relate": () => app.requestRelate(file),
                "html-remote": () => { fileActions.renderHtml(file); Qt.callLater(() => { editor.activeTab.viewerItem.allowRemote = true }) },
                "quotes": () => { window.inspectorVisible = true; inspector.tab = 1; inspector.linksPanel.scope = "project" },
                "new-session": () => sessionMenu.askNew(),
                "quick-look": () => fileActions.quickLook(file),
                "calendar-settings": () => fileActions.calendarSync.openSettings(),
                "conflicts": () => fileActions.calendarSync.showConflicts(),
                "extensions": () => extensionsDialog.open(),
                "properties": () => fileActions.showProperties(app.projectRoot),
                "editor": () => fileActions.configureEditor(),
                "palette": () => commandPalette.show(""),
                "menu": () => mainMenu.popup(),
                "context-menu": () => fileActions.showMenu([app.openFiles.length ? app.openFiles[0] : app.projectRoot], app.projectRoot),
                "send-to-note": () => sendToNote.ask("$ cargo test\nok", app.projectRoot),
                "rename": () => fileActions.rename(app.openFiles.length ? app.openFiles[0] : app.projectRoot),
                "confirm": () => fileActions.purge(["x"], [qsTr("Beispiel.txt")]),
                "entry": () => window.newCalendarEntry("event"),
            }[app.screenshotDialog]
            if (show) show()
            else console.warn("unknown --screenshot-dialog", app.screenshotDialog)
        }
    }
    Timer {
        id: screenshotTimer
        interval: app.screenshotDelayMs
        onTriggered: {
            window.reportProbe()
            const ok = app.saveWindowImage(window, app.screenshotPath)
            console.info(ok ? "screenshot saved to" : "screenshot failed:", app.screenshotPath)
            if (app.quitAfterMs <= 0) Qt.quit()
        }
    }
    Timer { id: quitTimer; interval: Math.max(1, app.quitAfterMs); onTriggered: { window.reportProbe(); Qt.quit() } }












    // Built-in smoke test (`--self-test`).
    Loader {
        active: app.selfTest
        sourceComponent: Component { SelfTest {} }
        onLoaded: {
            Object.assign(item, {
                app: app, editor: editor, fileActions: fileActions, commandPalette: commandPalette,
                searchPanel: searchPanel, searchModel: searchModel, terminalPanel: terminalPanel, window: window,
                sendToNote: sendToNote, sessionMenu: sessionMenu, folderDialog: folderDialog, relateDialog: relateDialog, settingsDialog: settingsDialog,
                projectTree: projectTree, quickLook: quickLook, treeModel: treeModel, inspector: inspector, placesList: placesList,
            })
            item.start()
        }
    }

    // -------------------------------------------------------------- shortcuts

    Shortcut { sequence: "Ctrl+O"; onActivated: folderDialog.askProject() }
    Shortcut { sequence: "Ctrl+P"; context: Qt.ApplicationShortcut; onActivated: commandPalette.show("") }
    Shortcut { sequence: "Ctrl+Shift+P"; context: Qt.ApplicationShortcut; onActivated: commandPalette.show(">") }
    Shortcut { sequence: "Ctrl+`"; context: Qt.ApplicationShortcut; onActivated: window.toggleTerminal() }
    Shortcut { sequence: "Ctrl+Shift+`"; context: Qt.ApplicationShortcut; onActivated: { terminalPanel.visible = true; terminalPanel.newTerminal() } }
    Shortcut { sequence: "Ctrl+Shift+F"; context: Qt.ApplicationShortcut; onActivated: { sidebarTabs.currentIndex = 1; searchPanel.focusQuery() } }
    Shortcut { sequence: "Ctrl+Shift+E"; onActivated: sidebarTabs.currentIndex = 0 }
    Shortcut { sequence: "Ctrl+\\"; onActivated: if (editor.activeGroup) editor.splitGroup(editor.activeGroup) }
    Shortcut { sequence: "Ctrl+W"; onActivated: if (editor.activeGroup) editor.activeGroup.closeCurrent() }
    // PDF full screen, wherever the focus is (the viewer handles F11 and Esc
    // itself while in full screen).
    Shortcut {
        sequence: "F11"
        onActivated: {
            const v = editor.activeTab && editor.activeTab.kind === "pdf" ? editor.activeTab.viewerItem : null
            if (v && v.enterFullscreen) v.enterFullscreen()
        }
    }
    Shortcut { sequence: "Ctrl+Tab"; onActivated: if (editor.activeGroup) editor.activeGroup.nextTab(1) }
    Shortcut { sequence: "Ctrl+Shift+Tab"; onActivated: if (editor.activeGroup) editor.activeGroup.nextTab(-1) }
    // In the calendar: previous / next week or month.
    Shortcut { sequence: "Alt+Left"; onActivated: if (editor.activeTab) { if (editor.activeTab.kind === "calendar") editor.activeTab.viewerItem.step(-1); else editor.activeTab.goBack() } }
    Shortcut { sequence: "Alt+Right"; onActivated: if (editor.activeTab) { if (editor.activeTab.kind === "calendar") editor.activeTab.viewerItem.step(1); else editor.activeTab.goForward() } }
    Shortcut { sequence: "Ctrl+B"; onActivated: window.toggleSidebar() }

    function toggleSidebar() { setSidebarVisible(!sidebar.visible) }
    function setSidebarVisible(visible) { sidebar.visible = visible; saveTimer.restart() }
    /// For tests: the sidebar's width while it is shown.
    function setSidebarWidth(width) { sidebar.SplitView.preferredWidth = width }

    /// Strg+`: a shown terminal without focus gets the focus first; the
    /// button (`direct`) always shows or hides.
    readonly property alias terminalButton: terminalButton
    function toggleTerminal(direct) {
        if (!direct && terminalPanel.visible && terminalPanel.currentGroup && terminalPanel.currentGroup.activeTerminal
                && !terminalPanel.currentGroup.activeTerminal.terminalView.activeFocus) {
            terminalPanel.focusTerminal()
            return
        }
        terminalPanel.visible = !terminalPanel.visible
        if (terminalPanel.visible) terminalPanel.focusTerminal()
        saveTimer.restart()
    }

    // ----------------------------------------------------------------- layout

    property bool extensionNoticeHidden: false
    Connections { target: app; function onProjectRootChanged() { window.extensionNoticeHidden = false } }
    /// Right context bar (annotations, links, graph, actions).
    property bool inspectorVisible: true
    /// Preferred width of the context bar.
    property real inspectorWidth: 400
    header: Column {
        Rectangle {
            width: parent.width
            height: 56
            color: Theme.window
            // Brand, aligned with the sidebar.
            Row {
                id: brand
                anchors.left: parent.left
                anchors.leftMargin: 18
                anchors.verticalCenter: parent.verticalCenter
                spacing: 12
                Icon { name: "logo"; size: 28; color: Theme.accent; anchors.verticalCenter: parent.verticalCenter }
                Label {
                    text: "Atlas"
                    color: Theme.textStrong
                    font.pixelSize: 19
                    font.weight: Font.Medium
                    anchors.verticalCenter: parent.verticalCenter
                }
            }
            // Universal bar: a click (or Ctrl+P) opens the palette.
            Rectangle {
                id: searchField
                anchors.verticalCenter: parent.verticalCenter
                x: Math.max(brand.width + 40, Math.min((parent.width - width) / 2, rightTools.x - width - 16))
                width: Math.max(220, Math.min(480, parent.width - brand.width - rightTools.width - 90))
                height: 36
                radius: 8
                color: searchHover.hovered ? Theme.hover : Theme.header
                border.color: Theme.border
                HoverHandler { id: searchHover; cursorShape: Qt.IBeamCursor }
                Row {
                    anchors.verticalCenter: parent.verticalCenter
                    x: 14
                    spacing: 10
                    Icon { name: "search"; size: 18; color: Theme.textMuted; anchors.verticalCenter: parent.verticalCenter }
                    Label {
                        text: qsTr("Dateien, Befehle und Aktionen suchen …")
                        color: Theme.textMuted
                        font.pixelSize: Theme.fontSize
                        anchors.verticalCenter: parent.verticalCenter
                    }
                }
                Label {
                    anchors.right: parent.right
                    anchors.rightMargin: 12
                    anchors.verticalCenter: parent.verticalCenter
                    visible: searchField.width > 360
                    text: qsTr("Strg+P")
                    color: Theme.textFaint
                    font.pixelSize: Theme.smallFontSize
                }
                MouseArea { anchors.fill: parent; onClicked: commandPalette.show("") }
            }
            Row {
                id: rightTools
                anchors.right: parent.right
                anchors.rightMargin: 14
                anchors.verticalCenter: parent.verticalCenter
                spacing: 10
                AbstractButton {
                    id: calendarButton
                    readonly property bool active: !!editor.activeTab && editor.activeTab.kind === "calendar"
                    implicitHeight: 34
                    implicitWidth: calendarRow.implicitWidth + 28
                    hoverEnabled: true
                    onClicked: window.toggleCalendar()
                    background: Rectangle {
                        radius: 8
                        color: calendarButton.active ? Theme.accentSoft : calendarButton.hovered ? Theme.hover : Theme.header
                        border.color: calendarButton.active ? Qt.rgba(0.24, 0.55, 0.99, 0.5) : Theme.border
                    }
                    contentItem: Item {
                        Row {
                            id: calendarRow
                            anchors.centerIn: parent
                            spacing: 8
                            Icon { name: "calendar"; size: 17; color: calendarButton.active ? Theme.accent : Theme.text; anchors.verticalCenter: parent.verticalCenter }
                            Label { text: qsTr("Kalender"); color: Theme.textStrong; font.pixelSize: Theme.fontSize; anchors.verticalCenter: parent.verticalCenter }
                        }
                    }
                    ToolTip.visible: hovered
                    ToolTip.delay: 600
                    ToolTip.text: qsTr("Projektkalender: Woche, Monat, Agenda")
                }
                SessionMenu {
                    id: sessionMenu
                    app: app
                    activeTabs: editor.groups.reduce((n, g) => n + g.tabs.length, 0)
                    activeSplits: editor.groups.filter(g => g.tabs.length > 0).length
                    onOpenRequested: (projectRoot) => window.openProject(projectRoot)
                    onNewSessionRequested: (name) => folderDialog.ask("session", app.projectRoot ? folderDialog.parentOf(app.projectRoot) : "", name)
                    onSaveRequested: { window.saveSession(); app.flushSession(); fileActions.showMessage(qsTr("Session gespeichert")) }
                }
                IconButton {
                    id: terminalButton
                    iconName: "terminal"
                    iconSize: 18
                    implicitWidth: 34
                    implicitHeight: 34
                    tip: qsTr("Terminal ein-/ausblenden (Strg+`)")
                    checkable: true
                    checked: terminalPanel.visible
                    onClicked: window.toggleTerminal(true)
                }
                IconButton {
                    iconName: "sidebar-right"
                    iconSize: 18
                    implicitWidth: 34
                    implicitHeight: 34
                    tip: qsTr("Kontextleiste: Anmerkungen, Verknüpfungen, Graph, Aktionen")
                    checkable: true
                    checked: window.inspectorVisible
                    onClicked: { window.inspectorVisible = !window.inspectorVisible; saveTimer.restart() }
                }
                IconButton {
                    iconName: "menu"
                    iconSize: 20
                    implicitWidth: 34
                    implicitHeight: 34
                    tip: qsTr("Menü")
                    onClicked: mainMenu.popup(0, height + 4)
                    Menu {
                        id: mainMenu
                        MenuItem { text: qsTr("Projektordner öffnen …\tStrg+O"); onTriggered: folderDialog.askProject() }
                        MenuItem { text: qsTr("Alle Befehle …\tStrg+Umschalt+P"); onTriggered: commandPalette.show(">") }
                        MenuSeparator {}
                        MenuItem { text: qsTr("Seitenleiste ein-/ausblenden\tStrg+B"); onTriggered: window.toggleSidebar() }
                        MenuItem { text: qsTr("Projektweit suchen\tStrg+Umschalt+F"); onTriggered: { sidebar.visible = true; sidebarTabs.currentIndex = 1; searchPanel.focusQuery() } }
                        MenuSeparator {}
                        // (Calendar, extensions and editor: in the settings.)
                        MenuItem { text: qsTr("Einstellungen …"); onTriggered: settingsDialog.open() }
                    }
                }
            }
        }
        // Project extensions run only after the user checked them.
        Banner {
            height: visible ? 36 : 0
            width: parent.width
            visible: app.untrustedExtensions.length > 0 && !window.extensionNoticeHidden
            message: qsTr("Dieses Projekt bringt Erweiterungen mit (%1). Sie bleiben inaktiv, bis du sie prüfst und freigibst.")
                .arg(app.untrustedExtensions.join(", "))
            actions: [
                { text: qsTr("Prüfen …"), run: () => extensionsDialog.open() },
                { text: qsTr("Ausblenden"), run: () => window.extensionNoticeHidden = true },
            ]
        }
        // Nextcloud: the app password is asked once per Atlas start.
        Banner {
            height: visible ? 36 : 0
            width: parent.width
            // While the password command runs (pinentry, …) nothing is asked.
            visible: window.calendar.connected && !window.calendar.unlocked && !window.calendar.fetchingPassword
                && !window.calendarPasswordLater
            tone: window.calendar.passwordError ? Theme.warn : Theme.accent
            message: (window.calendar.google ? qsTr("Google-Kalender: Bitte neu anmelden, um Termine abzugleichen.")
                    : window.calendar.passwordError
                    ? qsTr("Nextcloud-Kalender: Passwort-Befehl fehlgeschlagen – %1").arg(window.calendar.passwordError)
                    : qsTr("Nextcloud-Kalender: App-Passwort für diese Atlas-Sitzung eingeben, um Termine abzugleichen."))
                + (window.calendar.pending ? qsTr(" %n Änderung(en) warten auf Übertragung.", "", window.calendar.pending) : "")
            actions: (window.calendar.passwordError ? [{ text: qsTr("Erneut versuchen"), run: () => window.calendar.retryPasswordCommand() }] : [])
                .concat([
                    { text: window.calendar.google ? qsTr("Anmelden …") : qsTr("Eingeben …"), run: () => fileActions.calendarSync.askPassword() },
                    { text: qsTr("Später"), run: () => window.calendarPasswordLater = true },
                ])
        }
        Banner {
            height: visible ? 36 : 0
            width: parent.width
            visible: window.calendar.conflictCount > 0
            message: qsTr("%n Kalendereintrag/-einträge wurde(n) in Nextcloud und Atlas geändert. Nichts wurde überschrieben.", "", window.calendar.conflictCount)
            actions: [{ text: qsTr("Version wählen …"), run: () => fileActions.calendarSync.showConflicts() }]
        }
        Repeater {
            model: window.reminders
            Banner {
                required property var modelData
                required property int index
                height: 36
                width: parent.width
                tone: Theme.ok
                message: "⏰ " + modelData.title + " – " + modelData.start.slice(11, 16)
                    + (modelData.minutes > 0 ? qsTr(" (in %n Minute(n))", "", modelData.minutes) : qsTr(" (jetzt)"))
                actions: [
                    { text: qsTr("Zeigen"), run: () => { window.openCalendar({ selected: modelData.uid, date: modelData.start.slice(0, 10) }); window.dismissReminder(index) } },
                    { text: qsTr("OK"), run: () => window.dismissReminder(index) },
                ]
            }
        }
    }

    /// Splitter between cards: a gap that shows the accent while dragged.
    component Gap: Item {
        implicitWidth: Theme.gap
        implicitHeight: Theme.gap
        Rectangle {
            anchors.centerIn: parent
            width: parent.width > parent.height ? 36 : 2
            height: parent.width > parent.height ? 2 : 36
            radius: 1
            color: Theme.accent
            opacity: parent.SplitHandle.pressed ? 1 : parent.SplitHandle.hovered ? 0.6 : 0
        }
    }

    SplitView {
        anchors.fill: parent
        anchors.rightMargin: Theme.gap
        anchors.bottomMargin: 2
        orientation: Qt.Horizontal
        handle: Gap {}

        Rectangle {
            id: sidebar
            SplitView.preferredWidth: 280
            SplitView.minimumWidth: 200
            color: Theme.window
            onWidthChanged: saveTimer.restart()

            // Explorer (0) or project search (1); the search shows a header
            // with a way back.
            TabBar { id: sidebarTabs; visible: false; onCurrentIndexChanged: saveTimer.restart(); TabButton {} TabButton {} }

            ColumnLayout {
                anchors.fill: parent
                spacing: 0
                StackLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    currentIndex: sidebarTabs.currentIndex
                    ColumnLayout {
                        spacing: 6
                        PlacesList {
                            id: placesList
                            // ILIAS as the last place (if switched on).
                            readonly property bool ilias: { try { return JSON.parse(app.iliasConfig).show_in_places !== false } catch (e) { return true } }
                            Layout.fillWidth: true
                            names: ilias ? app.placeNames.concat(["ILIAS"]) : app.placeNames
                            paths: ilias ? app.placePaths.concat(["ilias:"]) : app.placePaths
                            icons: ilias ? app.placeIcons.concat(["school"]) : app.placeIcons
                            fixedCount: app.fixedPlaceCount
                            currentPath: editor.activeTab ? editor.activeTab.path : ""
                            onOpenRequested: (path, newPane) => editor.open(path, 0, newPane ? "split" : "tab")
                        }
                        ProjectTree {
                            id: projectTree
                            actions: fileActions
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            Layout.topMargin: 10
                            color: Theme.window
                            model: treeModel
                            projectName: app.projectName
                            currentPath: editor.activeTab ? editor.activeTab.path : ""
                            onOpenRequested: (path, newPane) => editor.open(path, 0, newPane ? "split" : "tab")
                            onOpenProjectRequested: folderDialog.askProject()
                            onPinRequested: (path) => editor.open(path, 0, "tab")
                        }
                    }
                    ColumnLayout {
                        spacing: 0
                        RowLayout {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 34
                            Layout.leftMargin: 16
                            Layout.rightMargin: 8
                            Label { Layout.fillWidth: true; text: qsTr("Suche"); color: Theme.textStrong; font.bold: true; font.pixelSize: Theme.fontSize }
                            IconButton { iconName: "close"; iconSize: 14; tip: qsTr("Zurück zum Explorer (Strg+Umschalt+E)"); onClicked: sidebarTabs.currentIndex = 0 }
                        }
                        SearchPanel {
                            id: searchPanel
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            model: searchModel
                            projectRoot: app.projectRoot
                            onOpenRequested: (path, line, newPane) => editor.open(path, line, newPane ? "split" : "temporary")
                        }
                    }
                }

                // Free space of the project's file system.
                ColumnLayout {
                    id: disk
                    Layout.fillWidth: true
                    Layout.leftMargin: 18
                    Layout.rightMargin: 18
                    Layout.topMargin: 8
                    Layout.bottomMargin: 14
                    spacing: 6
                    property var usage: ({})
                    function refresh() { try { usage = JSON.parse(app.diskUsage(app.projectRoot || app.placePaths[0] || "/") || "{}") } catch (e) { usage = {} } }
                    Component.onCompleted: refresh()
                    Connections { target: app; function onProjectRootChanged() { disk.refresh() } }
                    Timer { interval: 60000; running: true; repeat: true; onTriggered: disk.refresh() }
                    visible: !!usage.total
                    function gb(bytes) { return (bytes / 1e9).toLocaleString(Qt.locale(), "f", bytes < 1e10 ? 1 : 0) }
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 6
                        radius: 3
                        color: Theme.raised
                        Rectangle {
                            width: disk.usage.total ? parent.width * (1 - disk.usage.free / disk.usage.total) : 0
                            height: parent.height
                            radius: 3
                            color: disk.usage.total && disk.usage.free / disk.usage.total < 0.1 ? Theme.warn : "#4cc3ff"
                        }
                    }
                    Label {
                        text: qsTr("%1 GB frei von %2 GB").arg(disk.gb(disk.usage.free || 0)).arg(disk.gb(disk.usage.total || 0))
                        color: Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                    }
                }
            }
        }

        SplitView {
            SplitView.fillWidth: true
            SplitView.minimumWidth: 300
            orientation: Qt.Vertical
            handle: Gap {}

            EditorArea {
                actions: fileActions
                id: editor
                app: app
                onQuickOpenRequested: commandPalette.show("")
                SplitView.fillHeight: true
                SplitView.minimumHeight: 120
            }

            TerminalPanel {
                id: terminalPanel
                SplitView.preferredHeight: 240
                SplitView.minimumHeight: 90
                workingDirectory: app.projectRoot
                actions: fileActions
                onHideRequested: { visible = false; saveTimer.restart() }
                onHeightChanged: saveTimer.restart()
                onTerminalCountChanged: saveTimer.restart()
            }
        }

        Inspector {
            id: inspector
            visible: window.inspectorVisible
            SplitView.preferredWidth: window.inspectorWidth
            SplitView.minimumWidth: 260
            app: app
            editor: editor
            actions: fileActions
            onOpenRequested: (path, line, where, state) => editor.open(path, line, where, state)
        }
    }

    footer: Rectangle {
        height: 24
        color: Theme.window
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            spacing: 16
            Label {
                // Shows the QPA platform plugin: must be "wayland" on Hyprland.
                text: "● " + Qt.platform.pluginName
                color: Qt.platform.pluginName === "wayland" ? Theme.ok : Theme.warn
                font.pixelSize: Theme.smallFontSize
            }
            Label {
                text: treeModel.loading ? qsTr("Lade Verzeichnisse …")
                    : qsTr("%1 Zeilen im Baum · %2 ms").arg(treeModel.count).arg(treeModel.lastLoadMs)
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
            }
            Label {
                visible: terminalPanel.terminalCount > 0
                text: qsTr("%n Terminal(s)", "", terminalPanel.terminalCount)
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
            }
            Item { Layout.fillWidth: true }
            Label {
                text: fileActions.message || app.indexStatus
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
            }
            Label {
                text: app.storageError ? qsTr("Projektdaten werden nicht gespeichert: %1").arg(app.storageError)
                    : app.atlasDir ? app.atlasDir
                    : app.projectRoot ? qsTr("Sitzung wird nicht gespeichert") : ""
                color: app.storageError ? Theme.warn : Theme.textFaint
                font.pixelSize: Theme.smallFontSize
                elide: Text.ElideMiddle
                Layout.maximumWidth: 420
            }
            Label {
                text: "v" + app.version
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
            }
        }
    }
}
