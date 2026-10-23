// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import QtWebEngine
import Atlas

// ILIAS behind the scenes: the login, reading the course tree and
// downloads. One instance (in FileActions); the view is IliasView.qml.
//
// - The login happens in a web view with a persistent profile (cookies,
//   also Shibboleth's session cookies); Atlas never sees the password.
// - Reading and downloading run in Rust (atlas_core::ilias, through
//   `app.iliasScan` / `app.iliasDownload`) with that profile's session
//   cookies. The tree and the downloads are JSON files in the state
//   directory (`app.iliasLoad`).
// - A file goes into the chosen folder (one folder per course, group,
//   session and folder on the way) and is opened; it is downloaded again
//   when ILIAS shows a new version.
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
    /// "idle", "scanning", "login" (login needed) or "error".
    property string state: "idle"
    property string status: ""
    readonly property bool busy: state === "scanning"
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
        state = "scanning"
        status = qsTr("Lese Kurse von %1 …").arg(host)
        app.iliasScan()
    }
    Connections {
        target: service.app
        function onIliasProgress(text) { if (service.busy) service.status = text }
        function onIliasScanned(status, text) {
            if (status === "login") { service.needLogin(); return }
            if (status === "error") { service.state = "error"; service.status = text; return }
            service.index = service.loadState("index", { courses: [] })
            service.state = "idle"
            service.status = qsTr("%n Kurs(e) eingelesen", "", Number(text))
            service.revision++
        }
        function onIliasDownloadProgress(url, received, total) {
            if (service.current && service.current.item.url === url)
                service.current = Object.assign({}, service.current, { received: received, total: total })
        }
        function onIliasDownloaded(url, path, status, text) {
            if (!service.current || service.current.item.url !== url) return
            const job = service.current
            if (status === "login") {
                // Again after the login.
                service.queue = [job].concat(service.queue)
                service.current = null
                service.needLogin()
                return
            }
            if (status === "ok") {
                service.downloads = service.loadState("downloads", {})
                service.finish(qsTr("„%1“ geladen").arg(job.item.name))
                service.openLocal(path)
            } else {
                const failed = qsTr("Download von „%1“ fehlgeschlagen: %2").arg(job.item.name).arg(text)
                service.finish(failed)
                service.message(failed)
            }
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
    /// Signs out of ILIAS: the session is forgotten, the next reading asks
    /// for the login again.
    function logout() {
        app.iliasLogout()
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
        const item = current.item
        app.iliasDownload(item.url, item.download || "", item.name, item.info || "", JSON.stringify(current.segments || []))
    }
    function finish(text) {
        if (text) status = text
        current = null
        revision++
        next()
    }

    // ------------------------------------------------------------ the web

    WebEngineProfilePrototype {
        id: profilePrototype
        storageName: "atlas-ilias"
        // Shibboleth keeps its session in session cookies: kept as well, so
        // the login survives a restart.
        persistentCookiesPolicy: WebEngineProfile.ForcePersistentCookies
    }
    // (Ready only once the prototype is complete; the login view is
    // created after that – a view given no profile crashes.)
    property var profile: null
    Component.onCompleted: {
        profile = profilePrototype.instance()
        app.iliasUseProfile(profile)
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
                    if (service.queue.length) { service.status = ""; service.next() }
                    else service.refresh()
                })
            }
        }
    }
    readonly property var loginPage: loginLoader.item
    readonly property alias loginVisible: loginWindow.visible
}
