// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
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
        button: "#2d2d2d"
        buttonText: Theme.text
        highlight: Theme.accent
        highlightedText: "white"
        placeholderText: Theme.textFaint
        toolTipBase: "#252526"
        toolTipText: Theme.text
        mid: Theme.border
        dark: "#111111"
        light: "#3a3a3a"
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
    }
    Connections {
        target: fileActions
        function onQuickLookRequested(path, navigate) { quickLook.show(path, navigate) }
    }
    Connections {
        target: fileActions.ops
        function onMoved(from, to) {
            editor.pathMoved(from, to)
            // Links in notes follow moves and renames made in Atlas.
            app.filesMoved(from, to)
        }
        function onTrashed(paths) { editor.pathsRemoved(paths) }
    }

    ProjectTreeModel {
        id: treeModel
        root: app.projectRoot
    }

    SearchModel { id: searchModel }

    FolderDialog {
        id: folderDialog
        title: qsTr("Projektordner öffnen")
        onAccepted: app.openProject(app.urlToPath(selectedFolder.toString()))
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
        for (const file of app.openFiles) editor.open(app.absolutePath(file), 0, editor.documents().length > 0 ? "split" : "tab")
        restoring = false
    }

    function readLayout() {
        if (!app.restoreSession) return {}
        try { return JSON.parse(app.loadLayout() || "{}") } catch (e) { return {} }
    }

    /// Applies tabs, splits, tree, sidebar and search of a session layout.
    function applyLayout(layout) {
        treeModel.collapseAll()
        if (app.restoreSession) treeModel.restoreExpanded(app.loadExpanded())
        restoreEditor(layout.editor || {})
        if (layout.sidebarWidth) sidebar.SplitView.preferredWidth = layout.sidebarWidth
        sidebar.visible = layout.sidebarVisible !== undefined ? layout.sidebarVisible : true
        sidebarTabs.currentIndex = layout.sidebarTab || 0
        searchPanel.applyState(layout.search || {})
    }

    function currentLayout() {
        return {
            version: 2,
            editor: editor.serialize(),
            sidebarVisible: sidebar.visible,
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

    /// Saves the active session, then activates `id` and applies its layout.
    function switchSession(id) {
        if (id === app.activeSession) return
        app.requestSaveAll()
        saveSession()
        if (!app.switchSession(id)) return
        restoring = true
        applyLayout(readLayout())
        restoring = false
    }

    function createSession(name, duplicate) {
        saveSession()
        const id = app.createSession(name, duplicate ? JSON.stringify(currentLayout()) : "",
                                     duplicate ? treeModel.expandedDirs() : [])
        if (id) switchSession(id)
    }

    /// The active session is deleted only after another one is active.
    function deleteSession(id) {
        if (id === app.activeSession) {
            const other = app.sessionIds.find(s => s !== id)
            if (other) switchSession(other)
            else createSession(qsTr("Neue Session"), false)
        }
        if (id !== app.activeSession) app.deleteSession(id)
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
        else if (app.openFiles.length > 0) for (const f of app.openFiles) editor.open(f, 0, editor.documents().length > 0 ? "split" : "tab")
        else editor.addGroup()
        if (app.initialSearch) {
            sidebarTabs.currentIndex = 1
            searchPanel.setQuery(app.initialSearch)
        }
        if (app.screenshotPath) screenshotTimer.start()
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

    // -------------------------------------------------------------- shortcuts

    Shortcut { sequence: "Ctrl+O"; onActivated: folderDialog.open() }
    Shortcut { sequence: "Ctrl+`"; context: Qt.ApplicationShortcut; onActivated: window.toggleTerminal() }
    Shortcut { sequence: "Ctrl+Shift+`"; context: Qt.ApplicationShortcut; onActivated: { terminalPanel.visible = true; terminalPanel.newTerminal() } }
    Shortcut { sequence: "Ctrl+Shift+F"; context: Qt.ApplicationShortcut; onActivated: { sidebarTabs.currentIndex = 1; searchPanel.focusQuery() } }
    Shortcut { sequence: "Ctrl+Shift+E"; onActivated: sidebarTabs.currentIndex = 0 }
    Shortcut { sequence: "Ctrl+\\"; onActivated: if (editor.activeGroup) editor.splitGroup(editor.activeGroup) }
    Shortcut { sequence: "Ctrl+W"; onActivated: if (editor.activeGroup) editor.activeGroup.closeCurrent() }
    Shortcut { sequence: "Ctrl+Tab"; onActivated: if (editor.activeGroup) editor.activeGroup.nextTab(1) }
    Shortcut { sequence: "Ctrl+Shift+Tab"; onActivated: if (editor.activeGroup) editor.activeGroup.nextTab(-1) }
    Shortcut { sequence: "Alt+Left"; onActivated: if (editor.activeTab) editor.activeTab.goBack() }
    Shortcut { sequence: "Alt+Right"; onActivated: if (editor.activeTab) editor.activeTab.goForward() }
    Shortcut { sequence: "Ctrl+B"; onActivated: { sidebar.visible = !sidebar.visible; saveTimer.restart() } }

    function toggleTerminal() {
        if (terminalPanel.visible && terminalPanel.currentGroup && terminalPanel.currentGroup.activeTerminal
                && !terminalPanel.currentGroup.activeTerminal.view.activeFocus) {
            terminalPanel.focusTerminal()
            return
        }
        terminalPanel.visible = !terminalPanel.visible
        if (terminalPanel.visible) terminalPanel.focusTerminal()
        saveTimer.restart()
    }

    // ----------------------------------------------------------------- layout

    header: Rectangle {
        height: 36
        color: Theme.window
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 10
            anchors.rightMargin: 10
            spacing: 8
            Label {
                text: "Atlas"
                color: Theme.textStrong
                font.bold: true
                font.pixelSize: 14
            }
            Button {
                text: qsTr("Projekt öffnen…")
                flat: true
                onClicked: folderDialog.open()
            }
            SessionMenu {
                app: app
                onSwitchRequested: (id) => window.switchSession(id)
                onCreateRequested: (name, duplicate) => window.createSession(name, duplicate)
                onSaveRequested: { window.saveSession(); app.flushSession(); fileActions.showMessage(qsTr("Session gespeichert")) }
                onDeleteRequested: (id) => window.deleteSession(id)
            }
            Label {
                Layout.fillWidth: true
                text: app.projectRoot
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
                elide: Text.ElideMiddle
            }
            IconButton {
                glyph: editor.orientation === Qt.Horizontal ? "⇆" : "⇅"
                tip: qsTr("Split-Ausrichtung wechseln")
                onClicked: { editor.orientation = editor.orientation === Qt.Horizontal ? Qt.Vertical : Qt.Horizontal; saveTimer.restart() }
            }
            IconButton {
                glyph: "◫"
                tip: qsTr("Editor teilen (Strg+\\)")
                onClicked: if (editor.activeGroup) editor.splitGroup(editor.activeGroup)
            }
            IconButton {
                glyph: "›_"
                tip: qsTr("Terminal ein-/ausblenden (Strg+`)")
                checkable: true
                checked: terminalPanel.visible
                onClicked: window.toggleTerminal()
            }
        }
        Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
    }

    SplitView {
        anchors.fill: parent
        orientation: Qt.Horizontal
        handle: Rectangle {
            implicitWidth: 1
            color: SplitHandle.pressed || SplitHandle.hovered ? Theme.accent : Theme.border
        }

        Rectangle {
            id: sidebar
            SplitView.preferredWidth: 300
            SplitView.minimumWidth: 180
            color: Theme.sidebar
            onWidthChanged: saveTimer.restart()

            ColumnLayout {
                anchors.fill: parent
                spacing: 0
                TabBar {
                    id: sidebarTabs
                    Layout.fillWidth: true
                    background: Rectangle { color: Theme.sidebar }
                    onCurrentIndexChanged: saveTimer.restart()
                    TabButton { text: qsTr("Explorer"); font.pixelSize: Theme.smallFontSize }
                    TabButton { text: qsTr("Suche"); font.pixelSize: Theme.smallFontSize }
                }
                StackLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    currentIndex: sidebarTabs.currentIndex
                    ColumnLayout {
                        spacing: 0
                        PlacesList {
                            Layout.fillWidth: true
                            names: app.placeNames
                            paths: app.placePaths
                            icons: app.placeIcons
                            currentPath: editor.activeTab ? editor.activeTab.path : ""
                            onOpenRequested: (path, newPane) => editor.open(path, 0, newPane ? "split" : "preview")
                        }
                        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }
                        ProjectTree {
                            actions: fileActions
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            model: treeModel
                            projectName: app.projectName
                            currentPath: editor.activeTab ? editor.activeTab.path : ""
                            onOpenRequested: (path, newPane) => editor.open(path, 0, newPane ? "split" : "preview")
                            onOpenProjectRequested: folderDialog.open()
                            onPinRequested: (path) => editor.open(path, 0, "tab")
                        }
                    }
                    SearchPanel {
                        id: searchPanel
                        model: searchModel
                        projectRoot: app.projectRoot
                        onOpenRequested: (path, line, newPane) => editor.open(path, line, newPane ? "split" : "temporary")
                    }
                }
            }
        }

        SplitView {
            SplitView.fillWidth: true
            orientation: Qt.Vertical
            handle: Rectangle {
                implicitHeight: 1
                color: SplitHandle.pressed || SplitHandle.hovered ? Theme.accent : Theme.border
            }

            EditorArea {
                actions: fileActions
                id: editor
                app: app
                SplitView.fillHeight: true
                SplitView.minimumHeight: 120
            }

            TerminalPanel {
                id: terminalPanel
                SplitView.preferredHeight: 280
                SplitView.minimumHeight: 90
                workingDirectory: app.projectRoot
                onHideRequested: { visible = false; saveTimer.restart() }
                onHeightChanged: saveTimer.restart()
                onTerminalCountChanged: saveTimer.restart()
            }
        }
    }

    footer: Rectangle {
        height: 24
        color: Theme.window
        Rectangle { anchors.top: parent.top; width: parent.width; height: 1; color: Theme.border }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 10
            anchors.rightMargin: 10
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
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
            }
            Label {
                visible: terminalPanel.terminalCount > 0
                text: qsTr("%n Terminal(s)", "", terminalPanel.terminalCount)
                color: Theme.textMuted
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
