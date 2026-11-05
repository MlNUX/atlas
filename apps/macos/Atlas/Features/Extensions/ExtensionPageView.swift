// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Observation
import SwiftUI
import WebKit

/// A view of an extension (`views:` in its manifest) in a tab (P3.10;
/// Linux: `ExtensionView.qml`): its HTML page, opened on a file or folder.
/// The page reaches Atlas only through the bridge (`web/extension.js`,
/// served as `atlas://app/web/extension.js`); the core checks every file
/// access. The web stays out unless the extension declared network access.
struct ExtensionPageView: View {
    let project: ProjectModel
    let viewId: String
    let source: String
    @State private var page: ExtensionPage?

    var body: some View {
        VStack(spacing: 0) {
            if let page {
                if let error = page.error {
                    Banner(text: error, color: .red)
                }
                if let first = page.pageErrors.first {
                    Banner(text: "Die Erweiterung meldet: „\(first)“", color: .orange) {
                        page.reload()
                    }
                }
                if page.error == nil {
                    ExtensionWebView(webView: page.webView)
                } else {
                    Spacer()
                }
            } else {
                ProgressView()
            }
        }
        .task(id: viewId + "\n" + source) {
            let p = ExtensionPage(project: project, viewId: viewId, source: source)
            page = p
            await p.load()
        }
        .onChange(of: project.actions.revision) { page?.recheck() }
        .onDisappear { page?.close() }
    }
}

private struct Banner: View {
    let text: String
    let color: Color
    var reload: (() -> Void)?

    var body: some View {
        HStack {
            Text(text).font(.callout)
            Spacer()
            if let reload { Button("Neu laden", action: reload) }
        }
        .padding(8)
        .frame(maxWidth: .infinity)
        .background(color.opacity(0.18))
    }
}

private struct ExtensionWebView: NSViewRepresentable {
    let webView: WKWebView
    func makeNSView(context: NSViewRepresentableContext<Self>) -> WKWebView { webView }
    func updateNSView(_ view: WKWebView, context: NSViewRepresentableContext<Self>) {}
}

/// The page, its bridge and its requests.
@MainActor
@Observable
final class ExtensionPage: NSObject, @preconcurrency WKNavigationDelegate, @preconcurrency WKScriptMessageHandler {
    @ObservationIgnored let project: ProjectModel
    let viewId: String
    let source: String
    @ObservationIgnored let webView: WKWebView
    private(set) var info: ViewPage?
    private(set) var error: String?
    /// Error messages of the page since the last load.
    private(set) var pageErrors: [String] = []
    /// The page answered the start (tests, CI).
    private(set) var started = false

    init(project: ProjectModel, viewId: String, source: String) {
        self.project = project
        self.viewId = viewId
        self.source = source
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()
        config.setURLSchemeHandler(AtlasSchemeHandler(), forURLScheme: "atlas")
        config.userContentController.addUserScript(
            WKUserScript(source: Self.bridgeScript, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        config.preferences.javaScriptCanOpenWindowsAutomatically = false
        webView = WKWebView(frame: .zero, configuration: config)
        super.init()
        config.userContentController.add(WeakMessageHandler(self), name: "atlas")
        webView.navigationDelegate = self
        webView.underPageBackgroundColor = .textBackgroundColor
    }

    func close() {
        webView.configuration.userContentController.removeScriptMessageHandler(forName: "atlas")
        webView.stopLoading()
    }

    func load() async {
        do {
            let page = try project.core.extensions().view(id: viewId)
            info = page
            error = nil
            if !page.network {
                // Only the extension's own files and the bridge.
                let rules = #"[{"trigger":{"url-filter":"^(https?|wss?)://"},"action":{"type":"block"}}]"#
                if let list = try? await WKContentRuleListStore.default().compileContentRuleList(
                    forIdentifier: "atlas-no-network", encodedContentRuleList: rules) {
                    webView.configuration.userContentController.add(list)
                }
            }
            guard let url = URL(string: page.url) else { return }
            webView.loadFileURL(url, allowingReadAccessTo: URL(filePath: page.dir, directoryHint: .isDirectory))
        } catch {
            info = nil
            self.error = atlasMessage(error)
        }
    }

    func reload() {
        pageErrors = []
        webView.reload()
    }

    /// Extensions changed: switched off, removed or changed (banner).
    func recheck() {
        do {
            let next = try project.core.extensions().view(id: viewId)
            if next != info {
                info = next
                error = nil
                Task { await load() }
            }
        } catch {
            self.error = atlasMessage(error)
        }
    }

    // MARK: navigation

    func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {
        pageErrors = []
        started = false
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        start()
    }

    /// The page stays the page; links go to the browser.
    func webView(
        _ webView: WKWebView, decidePolicyFor action: WKNavigationAction,
        decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void
    ) {
        guard let url = action.request.url, let page = info.flatMap({ URL(string: $0.url) }) else {
            return decisionHandler(.cancel)
        }
        var bare = URLComponents(url: url, resolvingAgainstBaseURL: false)
        bare?.fragment = nil
        if bare?.url == page || url.absoluteString == "about:blank" || action.targetFrame?.isMainFrame == false {
            return decisionHandler(.allow)
        }
        decisionHandler(.cancel)
        if action.navigationType == .linkActivated, ["http", "https"].contains(url.scheme ?? "") {
            NSWorkspace.shared.open(url)
        }
    }

    // MARK: bridge

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let body = message.body as? [String: Any] else { return }
        if let text = body["error"] as? String {
            if pageErrors.count < 20 { pageErrors.append(text.replacingOccurrences(of: "Uncaught ", with: "")) }
            return
        }
        guard let raw = body["request"] as? String,
              let request = try? JSONSerialization.jsonObject(with: Data(raw.utf8)) as? [String: Any]
        else { return }
        let id = request["id"] as? Int ?? 0
        let method = request["method"] as? String ?? ""
        let params = request["params"] as? [String: Any] ?? [:]
        handle(id: id, method: method, params: params)
    }

    private var extensionName: String { info?.extension ?? "" }

    private func handle(id: Int, method: String, params: [String: Any]) {
        let path = (params["path"] as? String).map { $0.hasPrefix("/") ? $0 : project.core.absolutePath(relative: $0) }
        switch method {
        case "open":
            if let path { project.editor.open(path: path, mode: .tab) }
            reply(id, ok: true, json: "true")
        case "reveal":
            if let path { NSWorkspace.shared.activateFileViewerSelecting([URL(filePath: path)]) }
            reply(id, ok: true, json: "true")
        case "message":
            project.show(notice: params["text"] as? String ?? "")
            reply(id, ok: true, json: "true")
        case "runCommand":
            // Only commands of this extension, as from the palette (with the
            // same question).
            guard !extensionName.isEmpty else {
                return reply(id, ok: false, json: Self.json("Hier können keine Befehle laufen."))
            }
            let command = extensionName + "/" + (params["id"] as? String ?? "")
            if let problem = project.actions.run(id: command, paths: path.map { [$0] } ?? [], quiet: true) {
                reply(id, ok: false, json: Self.json(problem))
            } else {
                reply(id, ok: true, json: "true")
            }
        case "commandStatus":
            let key = extensionName + "/" + (params["id"] as? String ?? "")
            var status: [String: Any] = [:]
            if let job = project.app.core.backgroundJob(key: key) {
                status = ["running": job.running, "code": job.exitCode.map { $0 as Any } ?? NSNull(), "cwd": job.cwd, "log": job.log]
            }
            reply(id, ok: true, json: Self.json(object: status))
        case "stopCommand":
            let stopped = project.app.core.stopBackgroundJob(key: extensionName + "/" + (params["id"] as? String ?? ""))
            reply(id, ok: true, json: stopped ? "true" : "false")
        default:
            let ext = extensionName
            let paramsJSON = Self.json(object: params)
            Task {
                do {
                    let result = try await project.core.extensions().call(extension: ext, method: method, params: paramsJSON)
                    reply(id, ok: true, json: result)
                } catch {
                    reply(id, ok: false, json: Self.json(atlasMessage(error)))
                }
            }
        }
    }

    private func reply(_ id: Int, ok: Bool, json: String) {
        guard id != 0 else { return }
        webView.evaluateJavaScript("window.atlas && atlas._reply(\(id), \(ok), \(json))", completionHandler: nil)
    }

    private func start() {
        let ext = extensionName
        let theme = Self.theme(for: webView.effectiveAppearance)
        let file = source
        let view = viewId
        Task {
            var context: [String: Any] = [:]
            if let r = try? await project.core.extensions().call(extension: ext, method: "context", params: "{}"),
               let object = try? JSONSerialization.jsonObject(with: Data(r.utf8)) as? [String: Any] {
                context = object
            }
            context["file"] = file
            context["view"] = view
            context["theme"] = theme
            let script = "window.atlas ? (atlas._start(\(Self.json(object: context))), true) : false"
            let ok = (try? await webView.evaluateJavaScript(script)) as? Bool
            started = ok == true
        }
    }

    // MARK: helpers

    static func json(_ string: String) -> String {
        (try? JSONEncoder().encode(string)).flatMap { String(data: $0, encoding: .utf8) } ?? "\"\""
    }

    static func json(object: [String: Any]) -> String {
        (try? JSONSerialization.data(withJSONObject: object)).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
    }

    /// The colours of the page (`--atlas-…`), as on Linux.
    static func theme(for appearance: NSAppearance) -> [String: String] {
        var out: [String: String] = [:]
        appearance.performAsCurrentDrawingAppearance {
            let colors: [(String, NSColor)] = [
                ("window", .windowBackgroundColor), ("editor", .textBackgroundColor),
                ("header", .controlBackgroundColor), ("raised", .underPageBackgroundColor),
                ("border", .separatorColor), ("text", .textColor), ("strong", .labelColor),
                ("muted", .secondaryLabelColor), ("faint", .tertiaryLabelColor),
                ("accent", .controlAccentColor), ("accent-soft", NSColor.controlAccentColor.withAlphaComponent(0.2)),
                ("ok", .systemGreen), ("warn", .systemOrange), ("error", .systemRed),
            ]
            for (name, color) in colors { out[name] = hex(color) }
        }
        return out
    }

    static func hex(_ color: NSColor) -> String {
        guard let c = color.usingColorSpace(.sRGB) else { return "#000000" }
        let r = Int(round(c.redComponent * 255)), g = Int(round(c.greenComponent * 255)), b = Int(round(c.blueComponent * 255))
        if c.alphaComponent < 1 {
            return String(format: "#%02x%02x%02x%02x", r, g, b, Int(round(c.alphaComponent * 255)))
        }
        return String(format: "#%02x%02x%02x", r, g, b)
    }

    /// Hands the page's requests (console lines `atlas-ext:{…}`, see
    /// `web/extension.js`) and its errors to the app.
    static let bridgeScript = """
        (() => {
            const post = (m) => window.webkit.messageHandlers.atlas.postMessage(m);
            const log = console.log.bind(console);
            console.log = (...a) => {
                if (a.length === 1 && typeof a[0] === "string" && a[0].startsWith("atlas-ext:")) post({ request: a[0].slice(10) });
                else log(...a);
            };
            const err = console.error.bind(console);
            console.error = (...a) => { post({ error: a.map(String).join(" ") }); err(...a); };
            window.addEventListener("error", (e) => post({ error: String(e.message || e) }));
            window.addEventListener("unhandledrejection", (e) => post({ error: String(e.reason) }));
        })();
        """
}

/// Keeps the page from being retained by its own web view.
private final class WeakMessageHandler: NSObject, WKScriptMessageHandler {
    weak var target: (any WKScriptMessageHandler)?
    init(_ target: any WKScriptMessageHandler) { self.target = target }
    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        target?.userContentController(controller, didReceive: message)
    }
}

/// `atlas://app/web/…`: the shared web files from the app bundle
/// (`Resources/web`: the bridge, KaTeX, Mermaid, highlight.js).
final class AtlasSchemeHandler: NSObject, WKURLSchemeHandler {
    /// The bundled file of `url`, nothing outside `web/`.
    static func file(for url: URL) -> URL? {
        guard url.scheme == "atlas", url.host == "app",
              let root = Bundle.main.resourceURL?.appending(path: "web", directoryHint: .isDirectory).standardizedFileURL
        else { return nil }
        let parts = url.path.split(separator: "/").map(String.init)
        guard parts.first == "web", parts.count > 1, !parts.contains("..") else { return nil }
        let file = parts.dropFirst().reduce(root) { $0.appending(path: $1) }.standardizedFileURL
        return file.path.hasPrefix(root.path) ? file : nil
    }

    static func mimeType(_ file: URL) -> String {
        switch file.pathExtension.lowercased() {
        case "js", "mjs": "text/javascript"
        case "css": "text/css"
        case "html": "text/html"
        case "json": "application/json"
        case "svg": "image/svg+xml"
        case "woff2": "font/woff2"
        case "woff": "font/woff"
        case "ttf": "font/ttf"
        case "png": "image/png"
        default: "application/octet-stream"
        }
    }

    func webView(_ webView: WKWebView, start task: any WKURLSchemeTask) {
        guard let url = task.request.url, let file = Self.file(for: url), let data = try? Data(contentsOf: file) else {
            task.didFailWithError(URLError(.fileDoesNotExist))
            return
        }
        task.didReceive(URLResponse(url: url, mimeType: Self.mimeType(file), expectedContentLength: data.count,
                                    textEncodingName: Self.mimeType(file).hasPrefix("text/") ? "utf-8" : nil))
        task.didReceive(data)
        task.didFinish()
    }

    func webView(_ webView: WKWebView, stop task: any WKURLSchemeTask) {}
}
