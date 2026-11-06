// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Observation
import SwiftUI
import WebKit

/// An HTML file shown as a web page (P3.12; Linux: `HtmlView.qml`, tab
/// state `processor: "atlas:html"`): reloads when the file is saved,
/// content from the internet only on request, page errors as a banner.
struct HtmlDocumentView: View {
    let project: ProjectModel
    let path: String
    @State private var page: HtmlPage?

    var body: some View {
        VStack(spacing: 0) {
            if let page {
                HtmlToolbar(project: project, page: page)
                Divider()
                if page.usesRemote, !page.allowRemote {
                    Banner(text: "Inhalte aus dem Internet blockiert – die Seite ist evtl. unvollständig.", color: .yellow) {
                        Button("Trotzdem laden") { page.allowRemote = true }
                    }
                }
                if let first = page.errors.first {
                    Banner(text: page.errors.count == 1 ? "Die Seite meldet einen Fehler: „\(first)“"
                        : "Die Seite meldet \(page.errors.count) Fehler, zuerst: „\(first)“", color: .orange) {
                        Button("Neu laden") { page.reload() }
                    }
                }
                HtmlWebView(webView: page.webView)
            } else {
                ProgressView()
            }
        }
        .task(id: path) {
            let p = HtmlPage(path: path)
            page = p
            await p.load()
        }
        .onChange(of: project.filesRevision) {
            if project.changedDirs.contains((path as NSString).deletingLastPathComponent) { page?.reloadIfChanged() }
        }
    }
}

private struct HtmlToolbar: View {
    let project: ProjectModel
    @Bindable var page: HtmlPage

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "globe").foregroundStyle(.tint)
            Text("Webseite").font(.headline)
            Spacer()
            Button("Verkleinern", systemImage: "minus.magnifyingglass") { page.zoom = max(0.3, page.zoom / 1.15) }
            Text("\(Int((page.zoom * 100).rounded())) %").font(.caption.monospacedDigit()).frame(width: 44)
            Button("Vergrößern", systemImage: "plus.magnifyingglass") { page.zoom = min(4, page.zoom * 1.15) }
            Button("Neu laden", systemImage: "arrow.clockwise") { page.reload() }
            Button("Quelltext anzeigen", systemImage: "chevron.left.forwardslash.chevron.right") {
                project.editor.open(path: page.path, mode: .tab)
            }
            Button("Im Browser öffnen", systemImage: "safari") { NSWorkspace.shared.open(URL(filePath: page.path)) }
        }
        .labelStyle(.iconOnly)
        .buttonStyle(.borderless)
        .padding(.horizontal, 10).padding(.vertical, 6)
    }
}

private struct Banner<Actions: View>: View {
    let text: String
    let color: Color
    @ViewBuilder var actions: () -> Actions

    var body: some View {
        HStack {
            Text(text).font(.callout)
            Spacer()
            actions()
        }
        .padding(8)
        .background(color.opacity(0.18))
    }
}

private struct HtmlWebView: NSViewRepresentable {
    let webView: WKWebView
    func makeNSView(context: NSViewRepresentableContext<Self>) -> WKWebView { webView }
    func updateNSView(_ view: WKWebView, context: NSViewRepresentableContext<Self>) {}
    /// Any size: the page width must not become the window's minimum.
    func sizeThatFits(_ proposal: ProposedViewSize, nsView: WKWebView, context: NSViewRepresentableContext<Self>) -> CGSize? {
        proposal.replacingUnspecifiedDimensions(by: CGSize(width: 300, height: 300))
    }
}

/// The web view of one HTML file.
@MainActor
@Observable
final class HtmlPage: NSObject, @preconcurrency WKNavigationDelegate, @preconcurrency WKScriptMessageHandler {
    let path: String
    @ObservationIgnored let webView: WKWebView
    var zoom: CGFloat = 1 { didSet { webView.pageZoom = zoom } }
    /// Remote content allowed (else blocked by a content rule list).
    var allowRemote = false { didSet { Task { await applyRules(); webView.reload() } } }
    /// The file refers to content on the internet.
    private(set) var usesRemote = false
    private(set) var errors: [String] = []
    @ObservationIgnored private var rules: WKContentRuleList?
    @ObservationIgnored private var modified: Date?

    init(path: String) {
        self.path = path
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()
        config.userContentController.addUserScript(
            WKUserScript(source: Self.errorScript, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        webView = WKWebView(frame: .zero, configuration: config)
        super.init()
        config.userContentController.add(WeakScriptHandler(self), name: "atlasErrors")
        webView.navigationDelegate = self
    }

    func load() async {
        let text = (try? String(contentsOfFile: path, encoding: .utf8)) ?? ""
        usesRemote = Self.refersToRemote(text)
        modified = Self.modificationDate(path)
        await applyRules()
        let url = URL(filePath: path)
        webView.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
    }

    func reload() {
        errors = []
        webView.reload()
    }

    /// The file was saved: reload (also after changes by another program).
    func reloadIfChanged() {
        let now = Self.modificationDate(path)
        guard now != modified else { return }
        modified = now
        usesRemote = Self.refersToRemote((try? String(contentsOfFile: path, encoding: .utf8)) ?? "")
        reload()
    }

    private func applyRules() async {
        let controller = webView.configuration.userContentController
        if allowRemote {
            if let rules { controller.remove(rules) }
            return
        }
        if rules == nil {
            let json = #"[{"trigger":{"url-filter":"^(https?|wss?)://"},"action":{"type":"block"}}]"#
            rules = try? await WKContentRuleListStore.default().compileContentRuleList(
                forIdentifier: "atlas-html-no-network", encodedContentRuleList: json)
        }
        if let rules { controller.add(rules) }
    }

    static func modificationDate(_ path: String) -> Date? {
        (try? FileManager.default.attributesOfItem(atPath: path))?[.modificationDate] as? Date
    }

    /// Scripts, styles, pictures … from the internet (as `HtmlView.qml` checks).
    static func refersToRemote(_ text: String) -> Bool {
        let patterns = [
            #"<(script|link|img|iframe|source|video|audio)\b[^>]*\b(src|href)\s*=\s*["']?(https?:)?//"#,
            #"@import\s+(url\()?["']?https?:"#,
            #"\.src\s*=\s*["'`]https?:|\bfetch\(\s*["'`]https?:|\bimport\s*\(?\s*["'`]https?:"#,
        ]
        return patterns.contains { text.range(of: $0, options: [.regularExpression, .caseInsensitive]) != nil }
    }

    // MARK: navigation: the file stays the page; links go to the browser or open in Atlas.

    func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {
        errors = []
    }

    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction,
                 decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void) {
        guard let url = action.request.url else { return decisionHandler(.cancel) }
        if action.navigationType != .linkActivated || action.targetFrame?.isMainFrame == false
            || url.isFileURL && url.path == path {
            return decisionHandler(.allow)
        }
        decisionHandler(.cancel)
        NSWorkspace.shared.open(url)
    }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        if let text = message.body as? String, errors.count < 20 { errors.append(text.replacingOccurrences(of: "Uncaught ", with: "")) }
    }

    static let errorScript = """
        (() => {
            const post = (m) => window.webkit.messageHandlers.atlasErrors.postMessage(String(m));
            window.addEventListener("error", (e) => post(e.message || e));
            window.addEventListener("unhandledrejection", (e) => post(e.reason));
        })();
        """
}

/// Keeps the page from being retained by its own web view.
private final class WeakScriptHandler: NSObject, WKScriptMessageHandler {
    weak var target: (any WKScriptMessageHandler)?
    init(_ target: any WKScriptMessageHandler) { self.target = target }
    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        target?.userContentController(controller, didReceive: message)
    }
}
