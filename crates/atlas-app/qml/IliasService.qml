// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import QtWebEngine
import Atlas

// ILIAS behind the scenes: the web session, reading the course tree and
// downloads. One instance (in FileActions); the view is IliasView.qml.
//
// - A persistent web profile keeps the login (cookies, also Shibboleth's
//   session cookies); Atlas never sees the password.
// - Reading: a hidden page of the ILIAS site gets web/ilias.js injected,
//   which fetches the dashboard and the containers with the session and
//   reports the tree. The tree and the downloads are stored as JSON in the
//   state directory (`app.iliasSave`).
// - A file is downloaded through the same session into the chosen folder
//   (one folder per course, group, session and folder on the way) and
//   opened; it is downloaded again when ILIAS shows a new version.
Item {
    id: service
    required property AppController app
    /// FileActions: messages and opening files.
    property var actions: null

    readonly property var config: { try { return JSON.parse(app.iliasConfig) } catch (e) { return {} } }
    readonly property string baseUrl: config.base_url || "https://ilias.studium.kit.edu/"
    readonly property string downloadDir: config.download_dir || ""
    readonly property string host: { try { return new URL(baseUrl).host } catch (e) { return "" } }

    /// {base_url, scanned (ms), courses: [{kind, id, name, url, info, children}]}
    property var index: loadState("index", { courses: [] })
    /// url → {path, info, time}
    property var downloads: loadState("downloads", {})
    /// "idle", "connecting", "scanning", "login" (login needed) or "error".
    property string state: "idle"
    property string status: ""
    readonly property bool busy: state === "connecting" || state === "scanning"
    /// The download running now: {item, segments, received, total} or null.
    property var current: null
    property var queue: []
    /// Bumped when downloads change (for bindings in views).
    property int revision: 0

    /// A download folder is needed first (the view asks for one).
    signal downloadDirNeeded()

    function loadState(name, fallback) {
        try { return JSON.parse(app.iliasLoad(name) || "null") || fallback } catch (e) { return fallback }
    }
    function message(text) { if (actions) actions.showMessage(text) }

    // ------------------------------------------------------------ reading

    /// Reads the dashboard's courses again (asks for the login if needed).
    function refresh() {
        if (busy) return
        state = "connecting"
        status = qsTr("Verbinde mit %1 …").arg(host)
        crawler.url = "about:blank"
        crawler.url = baseUrl
        connectTimeout.restart()
    }
    Timer {
        id: connectTimeout
        interval: 45000
        onTriggered: if (service.state === "connecting") { service.state = "error"; service.status = qsTr("ILIAS antwortet nicht") }
    }
    function onCrawlerLoaded(ok) {
        if (state !== "connecting") return
        connectTimeout.stop()
        let url
        try { url = new URL(crawler.url.toString()) } catch (e) { url = null }
        if (!ok) { state = "error"; status = qsTr("ILIAS nicht erreichbar (%1)").arg(host); return }
        // Sent to the university's login (Shibboleth) or to ILIAS' own one.
        if (!url || url.host !== host) { needLogin(); return }
        state = "scanning"
        status = qsTr("Lese Kurse …")
        crawler.runJavaScript("atlasIlias.scan(" + JSON.stringify(baseUrl) + ")")
    }
    function onCrawlerMessage(text) {
        if (!text.startsWith("atlas:ilias:")) return
        let m
        try { m = JSON.parse(text.slice(12)) } catch (e) { return }
        if (m.type === "progress") status = m.pages ? qsTr("Lese … %1 Seiten (%2)").arg(m.pages).arg(m.text) : qsTr("Lese %1 …").arg(m.text)
        else if (m.type === "login") needLogin()
        else if (m.type === "error") { state = "error"; status = m.message }
        else if (m.type === "done") {
            index = { base_url: baseUrl, scanned: Date.now(), courses: m.courses }
            app.iliasSave("index", JSON.stringify(index))
            state = "idle"
            status = qsTr("%n Kurs(e) eingelesen", "", m.courses.length)
            revision++
        }
    }
    function needLogin() {
        state = "login"
        status = qsTr("Anmeldung bei ILIAS nötig")
        login()
    }

    // ------------------------------------------------------------- login

    function login() {
        if (loginPage) loginPage.url = baseUrl
        loginWindow.show()
        loginWindow.raise()
        loginWindow.requestActivate()
    }
    /// Signs out of ILIAS (the next reading asks for the login again).
    function logout() {
        crawler.runJavaScript("(() => { const a = document.querySelector('a[href*=\"logout\"]'); if (a) location.href = a.href; })()")
        state = "login"
        status = qsTr("Abgemeldet")
    }

    // ---------------------------------------------------------- downloads

    function entry(item) { return item && item.url ? downloads[item.url] || null : null }
    /// "loaded", "changed" (ILIAS shows another version) or "" (not yet).
    function fileState(item) {
        const e = entry(item)
        if (!e || !app.exists(e.path)) return ""
        return e.info === (item.info || "") ? "loaded" : "changed"
    }
    /// Opens file `item` (path of containers `segments`): downloads it
    /// first if it is not here or ILIAS shows a new version.
    function open(item, segments, reload) {
        const e = entry(item)
        if (!reload && e && fileState(item) === "loaded") { openLocal(e.path); return }
        if (!downloadDir) { pendingOpen = { item: item, segments: segments }; downloadDirNeeded(); return }
        if (current && current.item.url === item.url || queue.some(q => q.item.url === item.url)) return
        queue = queue.concat([{ item: item, segments: segments }])
        next()
    }
    property var pendingOpen: null
    /// After the download folder was chosen.
    function resume() {
        if (!pendingOpen) return
        const p = pendingOpen
        pendingOpen = null
        open(p.item, p.segments)
    }
    function openLocal(path) { if (actions) actions.openRequested(path, false) }
    function next() {
        if (current || !queue.length) return
        current = Object.assign({ received: 0, total: 0 }, queue[0])
        queue = queue.slice(1)
        status = qsTr("Lade „%1“ …").arg(current.item.name)
        downloadTimeout.restart()
        downloader.url = "about:blank"
        downloader.url = current.item.download || current.item.url
    }
    function finish(ok, text) {
        downloadTimeout.stop()
        if (text) status = text
        current = null
        revision++
        next()
    }
    Timer {
        id: downloadTimeout
        interval: 60000
        onTriggered: if (service.current) service.finish(false, qsTr("Download von „%1“ kam nicht").arg(service.current.item.name))
    }
    function onDownloadRequested(download) {
        if (!current) { download.cancel(); return }
        const job = current
        const target = JSON.parse(app.iliasTarget(JSON.stringify(job.segments), download.suggestedFileName, job.item.name))
        if (target.error) { download.cancel(); finish(false, target.error); message(target.error); return }
        const cut = target.path.lastIndexOf("/")
        download.downloadDirectory = target.path.slice(0, cut)
        download.downloadFileName = target.path.slice(cut + 1)
        downloadTimeout.stop()
        download.receivedBytesChanged.connect(() => {
            if (service.current === job) service.current = Object.assign({}, job, { received: download.receivedBytes, total: download.totalBytes })
        })
        download.isFinishedChanged.connect(() => {
            if (!download.isFinished) return
            if (download.state === WebEngineDownloadRequest.DownloadCompleted) {
                const d = Object.assign({}, service.downloads)
                d[job.item.url] = { path: target.path, info: job.item.info || "", time: Date.now() }
                service.downloads = d
                app.iliasSave("downloads", JSON.stringify(d))
                service.finish(true, qsTr("„%1“ geladen").arg(job.item.name))
                service.openLocal(target.path)
            } else {
                service.finish(false, qsTr("Download von „%1“ abgebrochen").arg(job.item.name))
            }
        })
        download.accept()
    }
    function onDownloaderLoaded(ok) {
        // A page instead of a file: the session expired (login page) or
        // ILIAS shows the file's info page.
        if (!current || !ok) return
        const url = downloader.url.toString()
        if (url === "about:blank") return
        let h = ""
        try { h = new URL(url).host } catch (e) {}
        if (h !== host) { const job = current; queue = [job].concat(queue); current = null; downloadTimeout.stop(); needLogin(); return }
        downloader.runJavaScript("!!document.querySelector('a[href*=\"logout\"]')", (inside) => {
            if (!service.current) return
            if (!inside) { const job = service.current; service.queue = [job].concat(service.queue); service.current = null; downloadTimeout.stop(); service.needLogin() }
        })
    }

    // ------------------------------------------------------------ the web

    WebEngineProfilePrototype {
        id: profilePrototype
        storageName: "atlas-ilias"
        // Shibboleth keeps its session in session cookies: kept as well, so
        // the login survives a restart.
        persistentCookiesPolicy: WebEngineProfile.ForcePersistentCookies
    }
    // (Ready only once the prototype is complete; web views are created
    // after that – a view given no profile crashes.)
    property var profile: null
    Component.onCompleted: profile = profilePrototype.instance()
    readonly property var crawler: crawlerLoader.item
    readonly property var downloader: downloaderLoader.item
    Connections {
        target: service.profile
        function onDownloadRequested(download) { service.onDownloadRequested(download) }
    }

    Loader {
        id: crawlerLoader
        active: !!service.profile
        sourceComponent: WebEngineView {
            visible: false
            width: 800
            height: 600
            profile: service.profile
            settings.javascriptCanOpenWindows: false
            userScripts.collection: [{
                name: "atlasIlias",
                sourceUrl: Qt.resolvedUrl("qrc:/atlas/web/ilias.js"),
                injectionPoint: WebEngineScript.DocumentReady,
                worldId: WebEngineScript.MainWorld
            }]
            onLoadingChanged: (request) => {
                if (request.status === WebEngineLoadingInfo.LoadSucceededStatus) service.onCrawlerLoaded(true)
                else if (request.status === WebEngineLoadingInfo.LoadFailedStatus) service.onCrawlerLoaded(false)
            }
            onJavaScriptConsoleMessage: (level, text) => service.onCrawlerMessage(text)
        }
    }
    Loader {
        id: downloaderLoader
        active: !!service.profile
        sourceComponent: WebEngineView {
            visible: false
            width: 800
            height: 600
            profile: service.profile
            settings.javascriptCanOpenWindows: false
            onLoadingChanged: (request) => {
                if (request.status === WebEngineLoadingInfo.LoadSucceededStatus) service.onDownloaderLoaded(true)
            }
        }
    }

    // The university's login page, in Atlas' own window.
    Window {
        id: loginWindow
        title: qsTr("Bei ILIAS anmelden – Atlas")
        width: 900
        height: 760
        color: Theme.panel
        ColumnLayout {
            anchors.fill: parent
            spacing: 0
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 44
                color: Theme.header
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 14
                    anchors.rightMargin: 8
                    spacing: 10
                    Icon { name: "key"; size: 16; color: Theme.accent }
                    Text {
                        Layout.fillWidth: true
                        text: qsTr("Bei %1 anmelden – Atlas merkt sich nur die Sitzung, nicht das Passwort.").arg(service.host)
                        color: Theme.text
                        font.pixelSize: Theme.smallFontSize
                        elide: Text.ElideRight
                    }
                    Button { text: qsTr("Abbrechen"); flat: true; onClicked: { loginWindow.close(); if (service.state === "login") service.status = qsTr("Nicht angemeldet") } }
                }
            }
            Loader {
                id: loginLoader
                Layout.fillWidth: true
                Layout.fillHeight: true
                active: !!service.profile
                sourceComponent: loginComponent
            }
        }
    }
    Component {
        id: loginComponent
        WebEngineView {
            id: loginView
            profile: service.profile
            // Logged in when ILIAS shows a logout link: then read on.
            onLoadingChanged: (request) => {
                if (request.status !== WebEngineLoadingInfo.LoadSucceededStatus || !loginWindow.visible) return
                let h = ""
                try { h = new URL(loginView.url.toString()).host } catch (e) {}
                if (h !== service.host) return
                loginView.runJavaScript("!!document.querySelector('a[href*=\"logout\"]')", (inside) => {
                    if (!inside) return
                    loginWindow.close()
                    service.message(qsTr("Bei ILIAS angemeldet"))
                    service.state = "idle"
                    if (service.queue.length) service.next()
                    else service.refresh()
                })
            }
        }
    }
    readonly property var loginPage: loginLoader.item
    readonly property alias loginVisible: loginWindow.visible
}
