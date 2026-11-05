// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import Foundation
import Observation
import WebKit

/// ILIAS in the Mac app (P3.11). The core does the work (reading the
/// favourites, downloads, the remembered login); the app owns the login
/// web view and hands its session cookies to the core.
@MainActor
@Observable
final class IliasModel {
    enum State: Equatable {
        case idle
        case scanning
        /// The session is gone: the login window is shown.
        case login
        case ready
    }

    let ilias: Ilias
    let project: ProjectModel
    private(set) var config: IliasConfig
    private(set) var index: IliasIndex?
    private(set) var downloads: [String: IliasDownload] = [:]
    private(set) var state: State = .idle
    var status = ""
    var error: String?
    /// Downloads running now (by ILIAS address).
    private(set) var loading: Set<String> = []
    let login = IliasLoginWindow()

    init(project: ProjectModel, ilias: Ilias) {
        self.project = project
        self.ilias = ilias
        config = ilias.config()
        index = ilias.lastIndex()
        if index != nil { state = .ready }
        refreshDownloads()
        login.model = self
    }

    private func refreshDownloads() {
        downloads = Dictionary(ilias.downloads().map { ($0.url, $0) }, uniquingKeysWith: { $1 })
    }

    /// The web view's cookies (all of its data store) to the core.
    func handOverSession() async {
        let cookies = await WKWebsiteDataStore.default().httpCookieStore.allCookies()
        ilias.setSession(
            cookies: cookies.map { Cookie(name: $0.name, value: $0.value, domain: $0.domain) },
            userAgent: login.userAgent)
    }

    /// Reads the favourites; asks for a login when the session is gone.
    func scan() async {
        state = .scanning
        status = "Lese die Favoriten …"
        error = nil
        await handOverSession()
        do {
            index = try await ilias.scan()
            state = .ready
            status = "\(index?.courses.count ?? 0) Einträge eingelesen"
        } catch AtlasError.Auth(_) {
            state = .login
            status = "Anmeldung bei ILIAS nötig"
            login.start(base: config.baseUrl)
        } catch {
            state = index == nil ? .idle : .ready
            self.error = "\(error)"
        }
    }

    /// Called by the login window when ILIAS shows a page without login form.
    func loginFinished() {
        guard state == .login else { return }
        Task { await scan() }
    }

    /// Downloads a file into the ILIAS folder and opens it.
    func download(_ item: IliasItem) async {
        guard item.kind == "file" else { return }
        let dir = config.downloadDir.isEmpty ? project.root : config.downloadDir
        loading.insert(item.url)
        defer { loading.remove(item.url) }
        await handOverSession()
        do {
            let path = try await ilias.download(
                url: item.url, download: item.download, title: item.title, info: item.info, dir: dir)
            refreshDownloads()
            project.editor.open(path: path, mode: .tab)
        } catch AtlasError.Auth(_) {
            state = .login
            login.start(base: config.baseUrl)
        } catch {
            self.error = "\(error)"
        }
    }

    /// Whether `item` was downloaded in its current version.
    func isCurrent(_ item: IliasItem) -> Bool {
        guard let d = downloads[item.url] else { return false }
        return d.info == item.info && FileManager.default.fileExists(atPath: d.path)
    }

    func setDownloadDir(_ dir: String) {
        var c = config
        c.downloadDir = dir
        do {
            try ilias.saveConfig(config: c)
            config = ilias.config()
        } catch {
            self.error = "\(error)"
        }
    }

    /// The user allows filling the remembered login in on `host`.
    func allowLogin(on host: String) {
        var c = config
        c.loginHost = host
        try? ilias.saveConfig(config: c)
        config = ilias.config()
    }
}

/// The login window's logic (Linux: `IliasService.qml`): the remembered
/// login is filled in only on the one host the user allowed, once per
/// attempt; ILIAS' own login page leads on to the university's (SSO) once.
@MainActor
@Observable
final class IliasLoginWindow: NSObject, WKNavigationDelegate {
    weak var model: IliasModel?
    let webView: WKWebView
    var visible = false
    /// A login page on this host could get the remembered login (ask first).
    var offerHost = ""
    var problem = ""
    private var filledOn = ""
    private var followedSso = false
    private var credentials: Credentials?

    struct Credentials {
        let user: String
        let password: String
    }

    var userAgent: String { webView.customUserAgent ?? "" }

    override init() {
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .default()
        webView = WKWebView(frame: .zero, configuration: config)
        super.init()
        webView.navigationDelegate = self
    }

    func start(base: String) {
        filledOn = ""
        followedSso = false
        problem = ""
        offerHost = ""
        if let login = try? model?.ilias.rememberedLogin() {
            credentials = Credentials(user: login.user, password: login.password)
        }
        if let url = URL(string: base) { webView.load(URLRequest(url: url)) }
        visible = true
    }

    func close() {
        visible = false
    }

    func allowHere() {
        guard !offerHost.isEmpty else { return }
        let host = offerHost
        model?.allowLogin(on: host)
        offerHost = ""
        fill(on: host)
    }

    nonisolated func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        MainActor.assumeIsolated { inspect() }
    }

    private var iliasHost: String { URL(string: model?.config.baseUrl ?? "")?.host ?? "" }

    private func inspect() {
        let host = webView.url?.host ?? ""
        webView.evaluateJavaScript(Self.pageScript) { [weak self] result, _ in
            MainActor.assumeIsolated {
                guard let self, self.visible, let r = result as? [String: Any] else { return }
                self.react(host: host, password: r["password"] as? Bool ?? false, sso: r["sso"] as? String ?? "")
            }
        }
    }

    private func react(host: String, password: Bool, sso: String) {
        guard let model else { return }
        let saved = model.config.loginSaved
        let allowed = model.config.loginHost
        model.ilias.trace(line: "Anmeldefenster \(host): Passwortfeld \(password), SSO \(sso.isEmpty ? "nein" : "ja")")
        if !sso.isEmpty, saved, host == iliasHost, !allowed.isEmpty, allowed != iliasHost, !followedSso {
            followedSso = true
            if let url = URL(string: sso) { webView.load(URLRequest(url: url)) }
        } else if password {
            guard saved else { return }
            if filledOn == host {
                problem = "Die Anmeldung mit den gespeicherten Daten hat nicht geklappt – bitte selbst anmelden oder das Passwort neu speichern."
            } else if host == allowed {
                fill(on: host)
            } else {
                offerHost = host
            }
        } else if !sso.isEmpty, saved, host == iliasHost, !followedSso {
            followedSso = true
            if let url = URL(string: sso) { webView.load(URLRequest(url: url)) }
        } else if host == iliasHost {
            // An ILIAS page without login form: try reading.
            model.loginFinished()
        }
    }

    private func fill(on host: String) {
        guard let c = credentials, host == model?.config.loginHost, filledOn != host else { return }
        filledOn = host
        let user = (try? JSONEncoder().encode(c.user)).flatMap { String(data: $0, encoding: .utf8) } ?? "\"\""
        let pass = (try? JSONEncoder().encode(c.password)).flatMap { String(data: $0, encoding: .utf8) } ?? "\"\""
        webView.evaluateJavaScript("(\(Self.fillScript))(\(user), \(pass))") { [weak self] result, _ in
            MainActor.assumeIsolated { self?.model?.ilias.trace(line: "  Einsetzen: \(result ?? "")") }
        }
    }

    /// Same scripts as the Linux login window (`IliasService.qml`).
    static let pageScript = """
        (() => {
            const pw = document.querySelector('input[type=password i]');
            const sso = [...document.querySelectorAll('a[href]')].find(a => /shib_login|saml|sso/i.test(a.getAttribute('href') || ''));
            return { password: !!pw && pw.offsetParent !== null, sso: sso ? sso.href : '' };
        })()
        """
    static let fillScript = """
        (user, password) => {
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
        }
        """
}
