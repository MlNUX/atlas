// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtWebEngine
import Atlas

// A view of an extension (`views:` in its manifest): its HTML page in a
// tab, opened on a file or a folder (`source`). The page reaches Atlas
// only through the bridge (web/extension.js); every file access is checked
// against the extension's permissions. The web stays out unless the
// extension declared network access.
Rectangle {
    id: root
    required property AppController app
    property var actions: null
    required property string source
    /// `view:<extension>/<id>`.
    required property string viewId
    property var info: root.readInfo()
    function readInfo() { try { return JSON.parse(root.app.extensionView(root.viewId)) } catch (e) { return { error: String(e) } } }
    // Switched off, uninstalled or changed: the banner says so.
    Connections {
        target: root.app
        function onUntrustedExtensionsChanged() {
            const next = root.readInfo()
            if (JSON.stringify(next) !== JSON.stringify(root.info)) root.info = next
        }
    }
    readonly property string title: info.name ? info.name + (root.app.fileKind(root.source) === "dir" ? "" : " – " + root.app.fileName(root.source)) : root.app.fileName(root.source)
    /// Error messages of the page since the last load.
    property var pageErrors: []
    /// The page answered the start (for the smoke test).
    property bool started: false
    readonly property alias webItem: web

    color: Theme.editor

    function viewState() { return { processor: root.viewId } }
    function focusContent() { web.forceActiveFocus() }

    readonly property var theme: ({
        window: String(Theme.window), editor: String(Theme.editor), header: String(Theme.header), raised: String(Theme.raised),
        border: String(Theme.border), text: String(Theme.text), strong: String(Theme.textStrong), muted: String(Theme.textMuted),
        faint: String(Theme.textFaint), accent: String(Theme.accent), "accent-soft": String(Theme.accentSoft),
        ok: String(Theme.ok), warn: String(Theme.warn), error: String(Theme.error),
    })

    function reply(id, ok, value) {
        if (id) web.runJavaScript("window.atlas && atlas._reply(" + id + "," + ok + "," + JSON.stringify(value === undefined ? null : value) + ")")
    }
    function handle(request) {
        const p = request.params || {}
        switch (request.method) {
        case "open":
            if (root.actions) root.actions.openRequested(p.path, false)
            return root.reply(request.id, true, true)
        case "reveal":
            if (root.actions) root.actions.revealRequested(p.path)
            return root.reply(request.id, true, true)
        case "message":
            if (root.actions) root.actions.showMessage(String(p.text || ""))
            return root.reply(request.id, true, true)
        }
        const r = JSON.parse(root.app.extensionCall(root.info.extension || "", request.method, JSON.stringify(p)))
        root.reply(request.id, r.ok, r.ok ? r.value : r.error)
    }
    function start() {
        const r = JSON.parse(root.app.extensionCall(root.info.extension || "", "context", "{}"))
        const context = Object.assign(r.ok ? r.value : {}, { file: root.source, view: root.viewId, theme: root.theme })
        web.runJavaScript("window.atlas ? (atlas._start(" + JSON.stringify(context) + "), true) : false",
                          (ok) => { root.started = ok === true })
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Banner {
            Layout.fillWidth: true
            visible: !!root.info.error
            tone: Theme.error
            message: root.info.error || ""
        }
        Banner {
            Layout.fillWidth: true
            visible: root.pageErrors.length > 0
            tone: Theme.warn
            message: qsTr("Die Erweiterung meldet: „%1“").arg(root.pageErrors[0] || "")
            actions: [{ text: qsTr("Neu laden"), run: () => web.reload() }]
        }

        WebEngineView {
            id: web
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: !root.info.error
            url: root.info.url || "about:blank"
            backgroundColor: Theme.editor
            settings.javascriptEnabled: true
            settings.localContentCanAccessFileUrls: false
            settings.localContentCanAccessRemoteUrls: !!root.info.network
            settings.javascriptCanOpenWindows: false
            settings.focusOnNavigationEnabled: false
            // The page stays the page; links go to the system browser.
            onNavigationRequested: (request) => {
                const url = request.url.toString()
                if (url === root.info.url || url.startsWith(root.info.url + "#") || url === "about:blank") return
                request.reject()
                if (request.navigationType === WebEngineNavigationRequest.LinkClickedNavigation && /^https?:/.test(url)) Qt.openUrlExternally(request.url)
            }
            onNewWindowRequested: (request) => { if (/^https?:/.test(request.requestedUrl.toString())) Qt.openUrlExternally(request.requestedUrl) }
            onJavaScriptConsoleMessage: (level, message, line, sourceId) => {
                if (message.startsWith("atlas-ext:")) {
                    let request = null
                    try { request = JSON.parse(message.slice(10)) } catch (e) { return }
                    root.handle(request)
                } else if (level === WebEngineView.ErrorMessageLevel && root.pageErrors.length < 20) {
                    root.pageErrors = root.pageErrors.concat([message.replace(/^Uncaught /, "")])
                }
            }
            onLoadingChanged: (request) => {
                if (request.status === WebEngineLoadingInfo.LoadStartedStatus) { root.pageErrors = []; root.started = false }
                else if (request.status === WebEngineLoadingInfo.LoadSucceededStatus) root.start()
            }
            Keys.onPressed: (event) => {
                if (event.key === Qt.Key_F5) { web.reload(); event.accepted = true }
            }
            WebRepaint {}
        }
    }
}
