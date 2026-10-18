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

    function restoreSession() {
        restoring = true
        searchModel.clear()
        let layout = {}
        if (app.restoreSession) {
            try { layout = JSON.parse(app.loadLayout() || "{}") } catch (e) { layout = {} }
            treeModel.restoreExpanded(app.loadExpanded())
        }
        restoreEditor(layout.editor || {})
        if (layout.sidebarWidth) sidebar.SplitView.preferredWidth = layout.sidebarWidth
        if (layout.sidebarTab !== undefined) sidebarTabs.currentIndex = layout.sidebarTab
        if (layout.terminalHeight) terminalPanel.SplitView.preferredHeight = layout.terminalHeight
        terminalPanel.visible = layout.terminalVisible !== undefined ? layout.terminalVisible : true
        const terminals = Math.max(layout.terminals || 1, 1)
        if (terminalPanel.visible && terminalPanel.groups.length === 0)
            for (let i = 0; i < Math.min(terminals, 4); ++i) terminalPanel.newTerminal()
        for (const file of app.openFiles) editor.openFile(app.absolutePath(file), 0, editor.documents().length > 0)
        restoring = false
    }

    // Documents that moved since the last session are re-found through the
    // project index, which is rebuilt in the background after opening.
    property var pendingEditorState: null
    function restoreEditor(state) {
        const missing = (state.panes || []).some(p => p.path && !app.exists(p.path))
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
    }

    function saveSession() {
        if (!app.projectRoot || restoring || !app.persistSession) return
        const layout = {
            version: 1,
            editor: editor.serialize(),
            sidebarWidth: sidebar.width,
            sidebarTab: sidebarTabs.currentIndex,
            terminalVisible: terminalPanel.visible,
            terminalHeight: terminalPanel.height,
            terminals: terminalPanel.terminalCount
        }
        app.saveSession(JSON.stringify(layout), treeModel.expandedDirs(), editor.documents())
    }

    Timer { id: saveTimer; interval: 1500; onTriggered: window.saveSession() }
    Connections { target: app; function onProjectRootChanged() { window.restoreSession() } }
    Connections { target: editor; function onLayoutChanged() { saveTimer.restart() } }
    Connections { target: treeModel; function onRefreshCountChanged() { saveTimer.restart() } }

    onClosing: saveSession()
    Connections {
        target: Qt.application
        function onAboutToQuit() {
            window.saveSession()
            app.flushSession()
        }
    }

    Component.onCompleted: {
        if (app.projectRoot) restoreSession()
        else if (app.openFiles.length > 0) for (const f of app.openFiles) editor.openFile(f, 0, editor.documents().length > 0)
        else editor.addPane("")
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
    Shortcut { sequence: "Ctrl+\\"; onActivated: if (editor.activePane) editor.splitPane(editor.activePane) }
    Shortcut { sequence: "Ctrl+W"; onActivated: if (editor.activePane) editor.closePane(editor.activePane) }

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
                onClicked: if (editor.activePane) editor.splitPane(editor.activePane)
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
                    ProjectTree {
                        model: treeModel
                        projectName: app.projectName
                        currentPath: editor.activePane ? editor.activePane.path : ""
                        onOpenRequested: (path, newPane) => editor.openFile(path, 0, newPane)
                        onOpenProjectRequested: folderDialog.open()
                    }
                    SearchPanel {
                        id: searchPanel
                        model: searchModel
                        projectRoot: app.projectRoot
                        onOpenRequested: (path, line, newPane) => editor.openFile(path, line, newPane)
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
                text: app.indexStatus
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
