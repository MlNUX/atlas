// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import WebKit

/// Markdown → PDF with the preview itself (Linux: `MarkdownPdfExport.qml`):
/// the note is rendered like in the preview (formulas, diagrams, code) on a
/// hidden page with print styles (light paper, A4 with margins from
/// `atlas.preparePrint()`) and printed to a file. Needs no pandoc or LaTeX.
@MainActor
final class MarkdownPDFExport: NSObject {
    private let preview: MarkdownPreview
    private let window: NSWindow
    private var finished: CheckedContinuation<Void, Never>?

    init(root: String) {
        preview = MarkdownPreview(root: root)
        // Laid out at a page-like size in a window out of sight (WebKit only
        // paints pages that are in a window).
        window = NSWindow(contentRect: NSRect(x: -10_000, y: -10_000, width: 800, height: 1100),
                          styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        super.init()
        preview.webView.frame = NSRect(x: 0, y: 0, width: 800, height: 1100)
        window.contentView = preview.webView
    }

    /// Writes `html` (rendered by the core) to `output`; `false` on failure.
    func export(html: String, to output: URL) async -> Bool {
        defer { preview.close() }
        window.orderBack(nil)
        defer { window.orderOut(nil) }
        // The page loads asynchronously.
        for _ in 0..<100 where !preview.ready { try? await Task.sleep(for: .milliseconds(50)) }
        guard preview.ready else { return false }
        _ = try? await preview.webView.evaluateJavaScript(
            "atlas.setPrintMode(true); atlas.set(\(MarkdownPreview.jsString(html))); true")
        // Diagrams and fonts render asynchronously; print once they are done.
        for _ in 0..<80 {
            try? await Task.sleep(for: .milliseconds(150))
            let busy = (try? await preview.webView.evaluateJavaScript("atlas.busy()")) as? Bool ?? false
            if !busy { break }
        }
        _ = try? await preview.webView.evaluateJavaScript("atlas.preparePrint(); true")
        try? FileManager.default.removeItem(at: output)

        let info = NSPrintInfo()
        info.paperSize = NSSize(width: 595.28, height: 841.89)
        info.orientation = .portrait
        info.topMargin = 0
        info.bottomMargin = 0
        info.leftMargin = 0
        info.rightMargin = 0
        info.horizontalPagination = .fit
        info.verticalPagination = .automatic
        info.isHorizontallyCentered = false
        info.isVerticallyCentered = false
        info.jobDisposition = .save
        info.dictionary()[NSPrintInfo.AttributeKey.jobSavingURL] = output
        let operation = preview.webView.printOperation(with: info)
        operation.showsPrintPanel = false
        operation.showsProgressPanel = false
        operation.view?.frame = preview.webView.bounds
        await withCheckedContinuation { (done: CheckedContinuation<Void, Never>) in
            finished = done
            operation.runModal(for: window, delegate: self,
                               didRun: #selector(printOperationDidRun(_:success:contextInfo:)),
                               contextInfo: nil)
            // Never wait forever (a print operation that does not report back).
            Task { [weak self] in
                try? await Task.sleep(for: .seconds(30))
                self?.didRun()
            }
        }
        return FileManager.default.fileExists(atPath: output.path(percentEncoded: false))
    }

    private func didRun() {
        finished?.resume()
        finished = nil
    }
}

extension MarkdownPDFExport {
    /// Called by the print operation when it ended – not necessarily on the
    /// main thread (Swift 6 would trap there), so it hops over.
    @objc nonisolated func printOperationDidRun(
        _ operation: NSPrintOperation, success: Bool, contextInfo: UnsafeMutableRawPointer?
    ) {
        Task { @MainActor in self.didRun() }
    }
}
