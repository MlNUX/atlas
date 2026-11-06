// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import SwiftUI

/// A picture in a tab (P3.12): fitted or zoomed, reloaded when the file
/// changes; „In eigenem Fenster“ opens it large (Linux: `ImageWindow.qml`).
struct ImageDocumentView: View {
    let project: ProjectModel
    let path: String
    @State private var image: NSImage?
    @State private var revision = 0

    var body: some View {
        VStack(spacing: 0) {
            if let image {
                ZoomableImage(image: image) {
                    Button("In eigenem Fenster", systemImage: "macwindow") { ImageWindow.show(path: path) }
                }
            } else {
                ContentUnavailableView("Das Bild lässt sich nicht laden.", systemImage: "photo.badge.exclamationmark")
            }
        }
        .task(id: "\(path)#\(revision)") { image = NSImage(contentsOfFile: path) }
        .onChange(of: project.filesRevision) {
            if project.changedDirs.contains((path as NSString).deletingLastPathComponent) { revision += 1 }
        }
    }
}

/// Fit / 1:1 / zoom with buttons and keys (+, −, 0, 1), scrollable;
/// double click switches between fitted and 1:1.
struct ZoomableImage<Extra: View>: View {
    let image: NSImage
    /// Plain keys (+, −, 0, 1): only in the window of its own – in a tab
    /// they would catch typing in an editor of another split.
    var keys = false
    @ViewBuilder var extra: () -> Extra
    /// nil: fitted.
    @State private var zoom: CGFloat?

    private var pixelSize: CGSize {
        guard let rep = image.representations.first, rep.pixelsWide > 0 else { return image.size }
        return CGSize(width: rep.pixelsWide, height: rep.pixelsHigh)
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                Text("\(Int(pixelSize.width)) × \(Int(pixelSize.height)) px").font(.caption).foregroundStyle(.secondary)
                Spacer()
                Button("Verkleinern", systemImage: "minus.magnifyingglass") { step(1 / 1.25) }
                    .keyboardShortcut(key("-"))
                Text(zoom.map { "\(Int(($0 * 100).rounded())) %" } ?? "Eingepasst")
                    .font(.caption.monospacedDigit()).frame(width: 70)
                Button("Vergrößern", systemImage: "plus.magnifyingglass") { step(1.25) }
                    .keyboardShortcut(key("+"))
                Button("Originalgröße") { zoom = 1 }.keyboardShortcut(key("1"))
                Button("Einpassen") { zoom = nil }.keyboardShortcut(key("0"))
                extra()
            }
            .labelStyle(.iconOnly)
            .buttonStyle(.borderless)
            .padding(.horizontal, 10).padding(.vertical, 6)
            Divider()
            GeometryReader { geo in
                let fit = min(geo.size.width / max(pixelSize.width, 1), geo.size.height / max(pixelSize.height, 1), 1)
                let scale = zoom ?? fit
                ScrollView([.horizontal, .vertical]) {
                    Image(nsImage: image)
                        .resizable()
                        .interpolation(scale > 2 ? .none : .high)
                        .frame(width: pixelSize.width * scale, height: pixelSize.height * scale)
                        .frame(minWidth: geo.size.width, minHeight: geo.size.height)
                        .onTapGesture(count: 2) { zoom = zoom == nil ? 1 : nil }
                }
                .onAppear { currentFit = fit }
                .onChange(of: fit) { currentFit = fit }
            }
            .background(Color(nsColor: .underPageBackgroundColor))
        }
    }

    @State private var currentFit: CGFloat = 1

    private func key(_ c: Character) -> KeyboardShortcut? {
        keys ? KeyboardShortcut(KeyEquivalent(c), modifiers: []) : nil
    }

    private func step(_ factor: CGFloat) {
        zoom = min(16, max(0.05, (zoom ?? currentFit) * factor))
    }
}

/// The picture in a window of its own (Esc closes it).
@MainActor
enum ImageWindow {
    private static var windows: [String: NSWindow] = [:]

    static func show(path: String) {
        if let open = windows[path] {
            open.makeKeyAndOrderFront(nil)
            return
        }
        guard let image = NSImage(contentsOfFile: path) else { return }
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 900, height: 700),
                              styleMask: [.titled, .closable, .resizable, .miniaturizable], backing: .buffered, defer: false)
        window.title = (path as NSString).lastPathComponent
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: ZoomableImage(image: image, keys: true) {
            Button("Schließen (Esc)") { windows[path]?.close() }.keyboardShortcut(.cancelAction)
        })
        window.center()
        window.makeKeyAndOrderFront(nil)
        windows[path] = window
        NotificationCenter.default.addObserver(forName: NSWindow.willCloseNotification, object: window, queue: .main) { _ in
            MainActor.assumeIsolated { _ = windows.removeValue(forKey: path) }
        }
    }
}
