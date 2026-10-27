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
// - Each download asks for its folder (offering the file's earlier one, else
//   the last chosen); the file goes straight in (no ILIAS folders; a name
//   taken by another file gets a number) and is opened. A new version in
//   ILIAS is downloaded again, replacing the earlier one in that folder.
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
    property var index: loadIndex()
    /// The tree read last – only if it is of this ILIAS.
    function loadIndex() {
        const i = loadState("index", { courses: [] })
        return i.base_url && i.base_url !== baseUrl ? { courses: [] } : i
    }
    // Another ILIAS: its favourites are read anew (after logging in there).
    onBaseUrlChanged: {
        index = loadIndex()
        if (busy) return
        state = "idle"
        status = qsTr("Neue Adresse: %1").arg(host)
        revision++
    }
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

    /// Where to save `item`? The view asks (offering `dir`) and calls
    /// `resume(folder)`.
    signal downloadDirNeeded(var item, string dir)

    function loadState(name, fallback) {
        try { return JSON.parse(app.iliasLoad(name) || "null") || fallback } catch (e) { return fallback }
    }
    function message(text) { if (actions) actions.showMessage(text) }
    /// A line in the ILIAS log (and on the console).
    function trace(text) {
        console.log("ilias:", text)
        app.iliasTrace(text)
    }
    /// Opens the ILIAS log in a tab.
    function openLog() { openLocal(app.iliasDebugLog()) }

    // ------------------------------------------------------------ reading

    /// Reads the dashboard's courses again (asks for the login if needed).
    function refresh() {
        if (busy) return
        trace("Einlesen gestartet: " + baseUrl)
        state = "scanning"
        status = qsTr("Lese Favoriten von %1 …").arg(host)
        app.iliasScan()
    }
    Connections {
        target: service.app
        function onIliasProgress(text) { if (service.busy) service.status = text }
        function onIliasScanned(status, text) {
            service.trace("Einlesen beendet: " + status + (text ? " – " + text : ""))
            if (status === "login") {
                // Right after a login: ILIAS did not take the session – no
                // closing the window on its own again (no loop).
                if (service.loggedInAt && Date.now() - service.loggedInAt < 60000) {
                    service.loggedInAt = 0
                    service.state = "error"
                    service.status = qsTr("Angemeldet, aber ILIAS lässt Atlas nicht lesen – Details im ILIAS-Protokoll")
                    service.trace("Sitzung aus dem Anmeldefenster nicht angenommen (siehe GET-Zeilen oben)")
                    return
                }
                service.needLogin()
                return
            }
            service.loggedInAt = 0
            if (status === "error") { service.state = "error"; service.status = text; return }
            service.index = service.loadIndex()
            service.state = "idle"
            service.status = qsTr("%n Favorit(en) eingelesen", "", Number(text))
            service.revision++
        }
        function onIliasDownloadProgress(url, received, total) {
            if (service.current && service.current.item.url === url)
                service.current = Object.assign({}, service.current, { received: received, total: total })
        }
        function onIliasDownloaded(url, path, status, text) {
            service.trace("Download beendet: " + status + " " + (path || text))
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

    // ---------------------------------------- the remembered login
    //
    // User name and password (encrypted with systemd-creds, in memory only
    // while Atlas runs) are filled in on one login page only: the host the
    // user allowed once (`[ilias] login_host`), never on another page. One
    // try per login attempt – a wrong password does not loop.
    readonly property bool loginSaved: config.login_saved === true
    readonly property string loginHost: config.login_host || ""
    /// {user, password} once decrypted, else null.
    property var credentials: null
    /// Host the remembered login was filled in on in this attempt.
    property string filledOn: ""
    /// A login page on this host could get the remembered login (asks).
    property string offerHost: ""
    property string loginProblem: ""
    property bool followedSso: false
    onLoginSavedChanged: if (!loginSaved) credentials = null
    Connections {
        target: service.app
        function onIliasLoginReady(user, password, error) {
            service.credentials = password ? { user: user, password: password } : null
            if (error) service.loginProblem = qsTr("Gespeicherte Anmeldung nicht lesbar: %1").arg(error)
            // The page may already be waiting for it.
            if (service.credentials && loginWindow.visible && service.loginPage) service.inspectLoginPage()
        }
    }
    /// Fills the remembered login into the page and sends it (only on the
    /// allowed host, once per attempt).
    function fillLogin(host) {
        if (!credentials || host !== loginHost || filledOn === host) return
        filledOn = host
        trace("Anmeldefenster: setze gespeicherte Anmeldung ein auf " + host)
        loginPage.runJavaScript("(" + fillScript + ")(" + JSON.stringify(credentials.user) + ", " + JSON.stringify(credentials.password) + ")",
                                (r) => trace("  Einsetzen: " + r))
    }
    /// The user allows the login page shown now (offerHost).
    function allowLoginHere() {
        if (!offerHost) return
        const h = offerHost
        app.setIliasConfig("login_host", h)
        offerHost = ""
        fillLogin(h)
    }
    /// Looks at the page in the login window: a password form gets the
    /// remembered login (or the offer), ILIAS' own login page leads on to
    /// the university's (single sign-on) once.
    function inspectLoginPage() {
        let h = ""
        try { h = new URL(loginPage.url.toString()).host } catch (e) {}
        loginPage.runJavaScript(pageScript, (r) => {
            if (!r || !loginWindow.visible) return
            if (r.password) {
                if (!loginSaved) return
                if (filledOn === h) {
                    loginProblem = qsTr("Die Anmeldung mit den gespeicherten Daten hat nicht geklappt – bitte selbst anmelden oder das Passwort in den Einstellungen neu speichern.")
                    trace("Anmeldefenster: Anmeldeseite erneut nach dem Einsetzen – kein zweiter Versuch")
                } else if (h === loginHost) fillLogin(h)
                else offerHost = h
            } else if (r.sso && loginSaved && h === host && !followedSso) {
                followedSso = true
                trace("Anmeldefenster: weiter zur Anmeldung der Hochschule " + r.sso)
                loginPage.url = r.sso
            }
        })
    }
    readonly property string pageScript: `(() => {
        const pw = document.querySelector('input[type=password i]');
        const sso = [...document.querySelectorAll('a[href]')].find(a => /shib_login|saml|sso/i.test(a.getAttribute('href') || ''));
        return { password: !!pw && pw.offsetParent !== null, sso: sso ? sso.href : '' };
    })()`
    readonly property string fillScript: `(user, password) => {
        const pw = document.querySelector('input[type=password i]');
        if (!pw) return 'kein Passwortfeld';
        const form = pw.form || document;
        const name = form.querySelector('input[name=j_username], input[autocomplete=username], input[type=email], input[type=text]:not([hidden])');
        const set = (el, v) => { el.focus(); el.value = v; el.dispatchEvent(new Event('input', { bubbles: true })); el.dispatchEvent(new Event('change', { bubbles: true })); };
        if (name) set(name, user);
        set(pw, password);
        const button = form.querySelector('button[name=_eventId_proceed], button[type=submit], input[type=submit]');
        if (button) button.click(); else if (pw.form) pw.form.submit();
        return name ? 'eingesetzt und gesendet' : 'nur Passwort eingesetzt';
    }`

    /// When the login window last reported success (ms), and whether it
    /// may close by itself on the logged-in ILIAS.
    property real loggedInAt: 0
    property bool autoClose: true
    /// Logged in (the window found the ILIAS session): read on.
    function loggedIn() {
        trace("Anmeldefenster: angemeldet – schließe und lese ein")
        loginWindow.close()
        loggedInAt = Date.now()
        message(qsTr("Bei ILIAS angemeldet"))
        state = "idle"
        if (queue.length) { status = ""; next() }
        else refresh()
    }
    function login() {
        trace("Anmeldefenster geöffnet: " + baseUrl)
        // A new attempt: the remembered login may be filled in once more.
        filledOn = ""
        offerHost = ""
        loginProblem = ""
        followedSso = false
        if (loginSaved && !credentials) app.iliasFetchLogin()
        if (loginPage) loginPage.url = baseUrl
        loginWindow.show()
        loginWindow.raise()
        loginWindow.requestActivate()
    }
    /// Signs out of ILIAS: the session is forgotten, the next reading asks
    /// for the login again.
    function logout() {
        app.iliasLogout()
        autoClose = true
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
    /// Opens file `item` (`segments`: the containers on the way): downloads
    /// it first if it is not here or ILIAS shows a new version (`reload`:
    /// always) – into a folder asked for each time (`downloadDirNeeded`).
    function open(item, segments, reload) {
        const e = entry(item)
        if (!reload && e && fileState(item) === "loaded") { openLocal(e.path); return }
        if (current && current.item.url === item.url || queue.some(q => q.item.url === item.url)) return
        pendingOpen = { item: item, segments: segments }
        downloadDirNeeded(item, suggestedDir(item))
    }
    /// The folder offered for a download: where the file was saved before,
    /// else the folder chosen last.
    function suggestedDir(item) {
        const e = entry(item)
        if (e && e.path && app.exists(e.path)) return e.path.slice(0, e.path.lastIndexOf("/"))
        return downloadDir
    }
    property var pendingOpen: null
    /// The folder for the pending download was chosen: download there.
    function resume(dir) {
        if (!pendingOpen || !dir) return
        const p = pendingOpen
        pendingOpen = null
        // Offered first next time.
        if (dir !== downloadDir) app.setIliasConfig("download_dir", dir)
        queue = queue.concat([{ item: p.item, segments: p.segments, dir: dir }])
        next()
    }
    function openLocal(path) { if (actions) actions.openRequested(path, false) }
    function next() {
        if (current || !queue.length) return
        current = Object.assign({ received: 0, total: 0 }, queue[0])
        queue = queue.slice(1)
        status = qsTr("Lade „%1“ …").arg(current.item.name)
        const item = current.item
        app.iliasDownload(item.url, item.download || "", item.name, item.info || "", current.dir)
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
                        text: service.loginSaved
                            ? qsTr("Bei %1 anmelden – mit der gespeicherten Anmeldung (verschlüsselt mit systemd-creds).").arg(service.host)
                            : qsTr("Bei %1 anmelden – Atlas merkt sich nur die Sitzung, nicht das Passwort.").arg(service.host)
                        color: Theme.text
                        font.pixelSize: Theme.smallFontSize
                        elide: Text.ElideRight
                    }
                    Button { text: qsTr("Abbrechen"); flat: true; onClicked: { loginWindow.close(); if (service.state === "login") service.status = qsTr("Nicht angemeldet") } }
                    // If Atlas does not notice the login by itself.
                    Button { text: qsTr("Angemeldet – Favoriten zeigen"); highlighted: true; onClicked: { service.trace("Knopf „Angemeldet“ gedrückt"); service.autoClose = true; service.loggedIn() } }
                }
            }
            // The remembered login: ask once per login page, or report a
            // failed try.
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: visible ? 40 : 0
                visible: service.offerHost !== "" || service.loginProblem !== ""
                color: service.loginProblem ? Qt.rgba(0.85, 0.3, 0.3, 0.16) : Theme.hover
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 14
                    anchors.rightMargin: 8
                    spacing: 10
                    Text {
                        Layout.fillWidth: true
                        text: service.loginProblem
                            || qsTr("Gespeicherte Anmeldung (%1) auf %2 einsetzen? Atlas setzt sie danach nur auf dieser Seite ein.").arg(service.config.user || "").arg(service.offerHost)
                        color: Theme.text
                        font.pixelSize: Theme.smallFontSize
                        elide: Text.ElideRight
                    }
                    Button {
                        visible: !service.loginProblem
                        text: qsTr("Hier einsetzen")
                        highlighted: true
                        onClicked: service.allowLoginHere()
                    }
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
            // Logged in when the page is on ILIAS and is no login page (it
            // links to the logout – any case: ILIAS 8+ says "doLogout" –, or
            // has neither a password field nor a link to log in): read on.
            onLoadingChanged: (request) => {
                const names = ["gestartet", "gestoppt", "geladen", "fehlgeschlagen"]
                if (loginWindow.visible)
                    service.trace("Anmeldefenster: " + (names[request.status] || request.status) + " " + request.url
                                  + (request.errorString ? " (" + request.errorString + ")" : ""))
                if (request.status !== WebEngineLoadingInfo.LoadSucceededStatus || !loginWindow.visible) return
                let h = ""
                try { h = new URL(loginView.url.toString()).host } catch (e) {}
                if (h !== service.host) {
                    service.trace("  anderer Rechner (" + h + ") als ILIAS (" + service.host + ")")
                    service.inspectLoginPage()
                    return
                }
                loginView.runJavaScript(service.loggedInCheck, (r) => {
                    service.trace("  Prüfung: " + JSON.stringify(r))
                    if (r && r.inside && loginWindow.visible && service.autoClose) service.loggedIn()
                    else if (r && !r.inside) service.inspectLoginPage()
                })
            }
        }
    }
    readonly property string loggedInCheck: `(() => {
        const login = (a) => a.includes('login.php') || a.includes('shib_login') || a.includes('cmd=showlogin');
        const links = [...document.querySelectorAll('a[href], form[action]')]
            .map(e => (e.getAttribute('href') || e.getAttribute('action') || '').toLowerCase());
        const r = { title: document.title, links: links.length };
        const out = links.find(a => a.includes('logout'));
        if (out) return Object.assign(r, { inside: true, why: 'Abmelde-Link ' + out });
        if (login(location.href.toLowerCase())) return Object.assign(r, { inside: false, why: 'Adresse ist eine Anmeldeseite' });
        if (document.querySelector('input[type=password i]')) return Object.assign(r, { inside: false, why: 'Passwortfeld' });
        const inLink = links.find(login);
        if (inLink) return Object.assign(r, { inside: false, why: 'Anmelde-Link ' + inLink });
        return Object.assign(r, { inside: true, why: 'keine Anmeldeseite' });
    })()`
    readonly property var loginPage: loginLoader.item
    readonly property alias loginVisible: loginWindow.visible
}
