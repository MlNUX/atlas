// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import SwiftUI
import WebKit

/// The Markdown preview (P3.3): the page shared with Linux
/// (`crates/atlas-app/web/preview.html`, from the bundle as
/// `atlas://app/web/preview.html`). The core renders the HTML
/// (`Editor.renderMarkdown`); the page adds KaTeX, Mermaid and highlighting
/// and reports clicks and scrolling as console lines `atlas:<kind>:<json>`,
/// as on Linux (`MarkdownView.qml`).
@MainActor
final class MarkdownPreview: NSObject, @preconcurrency WKNavigationDelegate, @preconcurrency WKScriptMessageHandler {
    /// What the page reports.
    enum Message {
        case link(path: String, line: Int, page: Int, external: String, broken: Bool, newTab: Bool)
        case task(line: Int, checked: Bool)
        case copy(String)
        case run(String)
        case image(String)
        /// `exact`: source line (with fraction) at height `at`; `own`: the
        /// page moved for the editor or changed height (no echo).
        case scroll(line: Int, exact: Double, at: Double, own: Bool)
        case rendered
    }

    let webView: WKWebView
    var onMessage: ((Message) -> Void)?
    private(set) var ready = false
    private var pending: (html: String, light: Bool)?
    private var pendingLine = 0
    private var pendingFind: (term: String, caseSensitive: Bool, line: Int)?

    /// `root`: the project (images in it are served); others are refused.
    init(root: String) {
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()
        config.setURLSchemeHandler(PreviewSchemeHandler(root: root), forURLScheme: "atlas")
        config.userContentController.addUserScript(
            WKUserScript(source: Self.consoleScript, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        config.preferences.javaScriptCanOpenWindowsAutomatically = false
        webView = WKWebView(frame: .zero, configuration: config)
        super.init()
        config.userContentController.add(PreviewMessageHandler(self), name: "atlas")
        webView.navigationDelegate = self
        webView.underPageBackgroundColor = .textBackgroundColor
        webView.setValue(false, forKey: "drawsBackground")
        webView.setAccessibilityLabel("Markdown-Vorschau")
        if let url = URL(string: "atlas://app/web/preview.html") {
            webView.load(URLRequest(url: url))
        }
    }

    func close() {
        webView.configuration.userContentController.removeScriptMessageHandler(forName: "atlas")
        webView.stopLoading()
    }

    /// Shows rendered HTML (kept until the page is loaded).
    func show(html: String, light: Bool) {
        guard ready else {
            pending = (html, light)
            return
        }
        run("atlas.setLight(\(light)); atlas.set(\(Self.jsString(html)))")
    }

    /// Puts source line `line` (with fraction) at height `at` of the view.
    func follow(line: Double, at: Double) {
        guard ready else { return }
        run("atlas.follow(\(line), \(at))")
    }

    /// Marks every occurrence of `term` and shows the one at or after `line`.
    func markHits(_ term: String, caseSensitive: Bool, line: Int) {
        guard ready else {
            pendingFind = (term, caseSensitive, line)
            return
        }
        run("atlas.markHits(\(Self.jsString(term)), \(caseSensitive), \(line))")
    }

    func scroll(toLine line: Int) {
        guard ready else {
            pendingLine = line
            return
        }
        run("atlas.scrollToLine(\(line))")
    }

    private func run(_ script: String) {
        webView.evaluateJavaScript(script) { _, error in
            if let error { NSLog("preview: %@", error.localizedDescription) }
        }
    }

    static func jsString(_ text: String) -> String {
        (try? JSONEncoder().encode(text)).flatMap { String(data: $0, encoding: .utf8) } ?? "\"\""
    }

    // MARK: - WKNavigationDelegate

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        ready = true
        if let p = pending {
            pending = nil
            show(html: p.html, light: p.light)
        }
        if let f = pendingFind {
            pendingFind = nil
            markHits(f.term, caseSensitive: f.caseSensitive, line: f.line)
        } else if pendingLine > 1 {
            scroll(toLine: pendingLine)
        }
        pendingLine = 0
    }

    /// Links inside the page never navigate it (the app opens them).
    func webView(
        _ webView: WKWebView, decidePolicyFor action: WKNavigationAction,
        decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void
    ) {
        let url = action.request.url?.absoluteString ?? ""
        decisionHandler(url == "atlas://app/web/preview.html" || url == "about:blank" ? .allow : .cancel)
    }

    // MARK: - messages of the page

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let line = message.body as? String, line.hasPrefix("atlas:") else { return }
        let rest = line.dropFirst(6)
        guard let colon = rest.firstIndex(of: ":") else { return }
        let kind = String(rest[..<colon])
        let json = rest[rest.index(after: colon)...]
        let data = (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [String: Any] ?? [:]
        let int = { (k: String) in (data[k] as? NSNumber)?.intValue ?? 0 }
        let double = { (k: String) in (data[k] as? NSNumber)?.doubleValue ?? 0 }
        let string = { (k: String) in data[k] as? String ?? "" }
        let bool = { (k: String) in (data[k] as? NSNumber)?.boolValue ?? false }
        let parsed: Message? = switch kind {
        case "link":
            .link(path: string("path"), line: int("line"), page: int("page"), external: string("external"),
                  broken: bool("broken"), newTab: bool("newTab"))
        case "task": .task(line: int("line"), checked: bool("checked"))
        case "copy": .copy(string("text"))
        case "run": .run(string("code"))
        case "image": .image(string("src"))
        case "scroll": .scroll(line: int("line"), exact: double("exact"), at: double("at"), own: bool("own"))
        case "rendered": .rendered
        default: nil
        }
        if let parsed { onMessage?(parsed) }
    }

    /// The page reports through `console.log("atlas:…")` (as for Qt WebEngine).
    static let consoleScript = """
        (() => {
            const log = console.log.bind(console);
            console.log = (...a) => {
                if (a.length === 1 && typeof a[0] === "string" && a[0].startsWith("atlas:"))
                    window.webkit.messageHandlers.atlas.postMessage(a[0]);
                else log(...a);
            };
        })();
        """
}

/// Keeps the preview from being retained by its own web view.
private final class PreviewMessageHandler: NSObject, WKScriptMessageHandler {
    weak var target: (any WKScriptMessageHandler)?
    init(_ target: any WKScriptMessageHandler) { self.target = target }
    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        target?.userContentController(controller, didReceive: message)
    }
}

/// `atlas://app/web/…` from the bundle (as `AtlasSchemeHandler`) and
/// `atlas://file/<absolute path>` for pictures in notes (Linux serves the
/// same URLs). Only picture files are served.
final class PreviewSchemeHandler: NSObject, WKURLSchemeHandler {
    private let root: String

    init(root: String) {
        self.root = root
    }

    static let pictures: Set<String> = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif", "heic", "tif", "tiff"]

    static func picture(for url: URL) -> URL? {
        guard url.scheme == "atlas", url.host == "file" else { return nil }
        let path = url.path(percentEncoded: false)
        let file = URL(filePath: path).standardizedFileURL
        guard pictures.contains(file.pathExtension.lowercased()) else { return nil }
        return file
    }

    static func mimeType(_ file: URL) -> String {
        switch file.pathExtension.lowercased() {
        case "jpg", "jpeg": "image/jpeg"
        case "gif": "image/gif"
        case "webp": "image/webp"
        case "svg": "image/svg+xml"
        case "bmp": "image/bmp"
        case "avif": "image/avif"
        case "heic": "image/heic"
        case "tif", "tiff": "image/tiff"
        default: AtlasSchemeHandler.mimeType(file)
        }
    }

    func webView(_ webView: WKWebView, start task: any WKURLSchemeTask) {
        guard let url = task.request.url,
              let file = AtlasSchemeHandler.file(for: url) ?? Self.picture(for: url),
              let data = try? Data(contentsOf: file)
        else {
            task.didFailWithError(URLError(.fileDoesNotExist))
            return
        }
        let mime = Self.mimeType(file)
        task.didReceive(URLResponse(url: url, mimeType: mime, expectedContentLength: data.count,
                                    textEncodingName: mime.hasPrefix("text/") ? "utf-8" : nil))
        task.didReceive(data)
        task.didFinish()
    }

    func webView(_ webView: WKWebView, stop task: any WKURLSchemeTask) {}
}

/// The preview in SwiftUI.
struct MarkdownPreviewView: NSViewRepresentable {
    let preview: MarkdownPreview

    func makeNSView(context: NSViewRepresentableContext<Self>) -> WKWebView { preview.webView }
    func updateNSView(_ view: WKWebView, context: NSViewRepresentableContext<Self>) {}
}
