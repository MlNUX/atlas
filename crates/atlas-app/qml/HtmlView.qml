// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtWebEngine
import Atlas

// An HTML file rendered (action "Als Webseite anzeigen"). Reloads when the
// file is saved. Pages may run scripts and load files next to them, but
// reach the network only when allowed for this file (a page referring to
// web resources says so); links to other sites open in the system browser.
Rectangle {
    id: root
    required property AppController app
    property var actions: null
    required property string source
    property real zoom: 1
    /// Network access for this file (asked, kept in the session).
    property bool allowRemote: false
    /// The page refers to scripts, styles or images on the web.
    property bool usesRemote: false
    /// Error messages of the page (and its frames) since the last load.
    property var pageErrors: []

    /// Back to the source in this tab.
    signal showSourceRequested()

    color: Theme.editor

    readonly property url fileUrl: "file://" + source.split("/").map(encodeURIComponent).join("/")
    readonly property alias webItem: web
    property real modified: 0

    function viewState() { return { processor: "atlas:html", zoom: root.zoom, remote: root.allowRemote } }
    function applyState(state) {
        if (state.zoom) root.zoom = state.zoom
        if (state.remote !== undefined) root.allowRemote = !!state.remote
    }
    function checkRemote() {
        const text = root.app.readTextFile(root.source) || ""
        usesRemote = /<(script|link|img|iframe|source|video|audio)\b[^>]*\b(src|href)\s*=\s*["']?(https?:)?\/\//i.test(text)
            || /@import\s+(url\()?["']?https?:/i.test(text)
            // Loaded from scripts: `x.src = "https://…"`, fetch(), import().
            || /\.src\s*=\s*["'`]https?:|\bfetch\(\s*["'`]https?:|\bimport\s*\(?\s*["'`]https?:/i.test(text)
    }
    onAllowRemoteChanged: web.reload()
    function reload() { web.reload() }

    function modifiedTime() {
        try { return JSON.parse(root.actions.ops.properties(root.source)).modified || 0 } catch (e) { return 0 }
    }
    Component.onCompleted: { modified = modifiedTime(); checkRemote() }

    // Saved in an editor (Atlas or another): show the new version.
    Timer {
        interval: 1000
        repeat: true
        running: root.visible && !!root.actions
        onTriggered: {
            const m = root.modifiedTime()
            if (m && m !== root.modified) {
                root.modified = m
                root.checkRemote()
                web.reload()
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 30
            color: Theme.header
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 8
                spacing: 4
                Label {
                    Layout.fillWidth: true
                    text: web.title && web.title !== root.app.fileName(root.source)
                        ? qsTr("Webseite: %1").arg(web.title) : qsTr("Webseite")
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                }
                IconButton { iconName: "minus"; tip: qsTr("Verkleinern"); onClicked: root.zoom = Math.max(0.3, root.zoom / 1.1) }
                Label {
                    text: Math.round(root.zoom * 100) + " %"
                    color: Theme.text
                    font.pixelSize: Theme.smallFontSize
                    MouseArea { anchors.fill: parent; onClicked: root.zoom = 1 }
                }
                IconButton { iconName: "plus"; tip: qsTr("Vergrößern"); onClicked: root.zoom = Math.min(5, root.zoom * 1.1) }
                IconButton { iconName: "refresh"; tip: qsTr("Neu laden (F5)"); onClicked: web.reload() }
                IconButton { iconName: "file-code"; tip: qsTr("Quelltext anzeigen"); onClicked: root.showSourceRequested() }
                IconButton { iconName: "external"; tip: qsTr("Im Browser öffnen"); onClicked: Qt.openUrlExternally(root.fileUrl) }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }

        // Web resources are blocked unless allowed for this file.
        Banner {
            Layout.fillWidth: true
            visible: root.usesRemote && !root.allowRemote
            tone: Theme.warn
            message: qsTr("Inhalte aus dem Internet blockiert – die Seite ist evtl. unvollständig.")
                + (root.pageErrors.length ? qsTr(" Fehler: „%1“").arg(root.pageErrors[0]) : "")
            actions: [
                { text: qsTr("Trotzdem laden"), run: () => root.allowRemote = true },
                { text: qsTr("Im Browser öffnen"), run: () => Qt.openUrlExternally(root.fileUrl) },
            ]
        }

        Banner {
            Layout.fillWidth: true
            visible: root.pageErrors.length > 0 && !(root.usesRemote && !root.allowRemote)
            tone: Theme.warn
            message: root.pageErrors.length === 1 ? qsTr("Die Seite meldet einen Fehler: „%1“").arg(root.pageErrors[0])
                : qsTr("Die Seite meldet %1 Fehler, zuerst: „%2“").arg(root.pageErrors.length).arg(root.pageErrors[0])
            actions: [
                { text: qsTr("Neu laden"), run: () => web.reload() },
                { text: qsTr("Im Browser öffnen"), run: () => Qt.openUrlExternally(root.fileUrl) },
            ]
        }

        WebEngineView {
            id: web
            Layout.fillWidth: true
            Layout.fillHeight: true
            url: root.fileUrl
            zoomFactor: root.zoom
            backgroundColor: "white"
            settings.javascriptEnabled: true
            settings.localContentCanAccessFileUrls: true
            settings.localContentCanAccessRemoteUrls: root.allowRemote
            settings.javascriptCanOpenWindows: false
            settings.focusOnNavigationEnabled: false
            // Local pages (also linked ones) stay here; the web goes to the
            // system browser.
            onNavigationRequested: (request) => {
                const url = request.url.toString()
                if (url.startsWith("file:") || url.startsWith("data:") || url.startsWith("about:")) return
                request.reject()
                if (request.navigationType === WebEngineNavigationRequest.LinkClickedNavigation) Qt.openUrlExternally(request.url)
            }
            onNewWindowRequested: (request) => Qt.openUrlExternally(request.requestedUrl)
            // Page errors show in the view (not on the terminal).
            onJavaScriptConsoleMessage: (level, message, line, sourceId) => {
                if (level !== WebEngineView.ErrorMessageLevel || root.pageErrors.length >= 20) return
                root.pageErrors = root.pageErrors.concat([message.replace(/^Uncaught /, "")])
            }
            onLoadingChanged: (request) => { if (request.status === WebEngineLoadingInfo.LoadStartedStatus) root.pageErrors = [] }
            Keys.onPressed: (event) => {
                if (event.key === Qt.Key_F5) { web.reload(); event.accepted = true }
            }
            // Never stays black after a workspace switch (#2).
            WebRepaint { id: webRepaint }
        }
    }
}
