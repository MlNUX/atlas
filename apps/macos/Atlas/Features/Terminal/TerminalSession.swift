// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Observation
@preconcurrency import SwiftTerm

/// One terminal (P3.5; Linux: `TerminalPane.qml`, `terminal_item.rs`):
/// SwiftTerm's `LocalProcessTerminalView` with its own PTY (decision of
/// P0.5). The view lives as long as the session, so a terminal keeps running
/// while its tab is dragged, hidden or shown elsewhere.
@MainActor
@Observable
final class TerminalSession: Identifiable {
    let id = UUID()
    /// Folder the terminal starts in.
    let startDir: String
    /// A program started by Atlas (an action, a script); `nil`: the shell.
    let prepared: Prepared?
    let projectRoot: String
    /// Title set by the program (OSC 0/2), else the program's name.
    private(set) var title: String
    private(set) var exited = false
    @ObservationIgnored let view: LocalProcessTerminalView
    @ObservationIgnored private var delegate: Delegate?
    @ObservationIgnored private(set) var started = false
    /// Folder reported by the shell (OSC 7), if it does.
    @ObservationIgnored private var reportedDir: String?
    @ObservationIgnored private var dark: Bool?
    /// Called when the program ends (the panel removes the terminal, a tab closes).
    @ObservationIgnored var onExit: (() -> Void)?
    /// Called when title or folder may have changed (the session is saved).
    @ObservationIgnored var onChange: (() -> Void)?
    /// Called when the terminal is clicked (it becomes the active one).
    @ObservationIgnored var onFocus: (() -> Void)?
    /// Builds the context menu (it depends on where the terminal is).
    @ObservationIgnored var menu: (() -> NSMenu)?

    init(startDir: String, prepared: Prepared? = nil, projectRoot: String) {
        self.startDir = startDir
        self.prepared = prepared
        self.projectRoot = projectRoot
        title = prepared?.name ?? "Terminal"
        view = AtlasTerminalView(frame: CGRect(x: 0, y: 0, width: 640, height: 240))
        view.font = .monospacedSystemFont(ofSize: 12.5, weight: .regular)
        view.optionAsMetaKey = false
        let delegate = Delegate(self)
        self.delegate = delegate
        view.processDelegate = delegate
        (view as? AtlasTerminalView)?.session = self
    }

    /// Starts the program (once; the view calls it when it is first shown,
    /// tests call it directly).
    func start() {
        guard !started else { return }
        started = true
        let argv: [String]
        var env = ProcessInfo.processInfo.environment
        if let prepared {
            // Output and status stay until Enter (as on Linux).
            argv = ["/bin/sh", "-c",
                    "\"$@\"; s=$?; printf '\\n\\033[2m[Beendet mit Status %s · Enter schließt]\\033[0m' \"$s\"; read _ ",
                    "sh"] + prepared.argv
            for v in prepared.env { env[v.name] = v.value }
        } else {
            let shell = AtlasKit.Terminal().shell(cwd: startDir, projectRoot: projectRoot)
            argv = shell.argv
            title = shell.name
            for v in shell.env { env[v.name] = v.value }
        }
        env["TERM"] = env["TERM"] ?? "xterm-256color"
        env["COLORTERM"] = "truecolor"
        env["TERM_PROGRAM"] = "atlas"
        env["ATLAS_PROJECT"] = projectRoot
        let dir = prepared.map { $0.cwd.isEmpty ? startDir : $0.cwd } ?? startDir
        // `sh` changes into the folder and replaces itself with the program,
        // so the PTY's process is the shell (its folder: `process_cwd`).
        let script = "cd -- \"$0\" 2>/dev/null || cd; exec \"$@\""
        view.startProcess(executable: "/bin/sh", args: ["-c", script, dir] + argv,
                          environment: env.map { "\($0.key)=\($0.value)" })
    }

    /// Ends the program (closing the terminal).
    func terminate() {
        guard started, !exited else { return }
        exited = true
        view.terminate()
    }

    /// The shell's current folder: from the process (`proc_pidinfo`), else
    /// what the shell reported, else the start folder.
    func cwd() -> String {
        if started, !exited, let pid = view.process?.shellPid, pid > 0,
           let dir = AtlasKit.Terminal().processCwd(pid: UInt32(pid)) {
            return dir
        }
        return reportedDir ?? startDir
    }

    /// Atlas' dark or light palette (`Terminal::palette`).
    func applyPalette(dark: Bool) {
        guard self.dark != dark else { return }
        self.dark = dark
        let p = AtlasKit.Terminal().palette(dark: dark)
        view.installColors(p.ansi.map(Self.swiftTermColor))
        view.nativeForegroundColor = Self.nsColor(p.foreground)
        view.nativeBackgroundColor = Self.nsColor(p.background)
        view.caretColor = Self.nsColor(p.cursor)
        view.selectedTextBackgroundColor = Self.nsColor(p.selection)
        view.needsDisplay = true
    }

    /// The selection, else the visible screen (Linux: `outputText`).
    func outputText() -> String {
        if let selection = view.getSelection(), !selection.isEmpty { return selection }
        return screenText()
    }

    /// The visible lines, without trailing blank lines.
    func screenText() -> String {
        var lines = view.visibleRowsText(0..<view.terminalDimensions.rows)
        while lines.last?.trimmingCharacters(in: .whitespaces).isEmpty == true { lines.removeLast() }
        return lines.joined(separator: "\n")
    }

    func copy() { view.copy(view) }
    func paste() { view.paste(view) }

    fileprivate func titleChanged(_ title: String) {
        if !title.isEmpty { self.title = title }
        onChange?()
    }

    fileprivate func directoryChanged(_ dir: String?) {
        guard let dir, let url = URL(string: dir), url.isFileURL else { return }
        reportedDir = url.path(percentEncoded: false)
        onChange?()
    }

    fileprivate func processEnded() {
        exited = true
        onExit?()
    }

    static func nsColor(_ hex: UInt32) -> NSColor {
        NSColor(srgbRed: CGFloat((hex >> 16) & 0xff) / 255, green: CGFloat((hex >> 8) & 0xff) / 255,
                blue: CGFloat(hex & 0xff) / 255, alpha: 1)
    }

    static func swiftTermColor(_ hex: UInt32) -> SwiftTerm.Color {
        SwiftTerm.Color(red: UInt16((hex >> 16) & 0xff) * 257, green: UInt16((hex >> 8) & 0xff) * 257,
                        blue: UInt16(hex & 0xff) * 257)
    }

    /// SwiftTerm's callbacks (on the main actor).
    @MainActor
    private final class Delegate: LocalProcessTerminalViewDelegate {
        weak var session: TerminalSession?

        init(_ session: TerminalSession) { self.session = session }

        func sizeChanged(source: LocalProcessTerminalView, newCols: Int, newRows: Int) {}

        func setTerminalTitle(source: LocalProcessTerminalView, title: String) {
            session?.titleChanged(title)
        }

        func hostCurrentDirectoryUpdate(source: TerminalView, directory: String?) {
            session?.directoryChanged(directory)
        }

        func processTerminated(source: TerminalView, exitCode: Int32?) {
            session?.processEnded()
        }
    }
}

/// The terminal view with Atlas' context menu.
final class AtlasTerminalView: LocalProcessTerminalView {
    weak var session: TerminalSession?

    override func menu(for event: NSEvent) -> NSMenu? {
        session?.menu?() ?? super.menu(for: event)
    }
}

/// A menu item running a closure.
@MainActor
final class ClosureMenuItem: NSMenuItem {
    private let run: () -> Void

    init(_ title: String, enabled: Bool = true, run: @escaping () -> Void) {
        self.run = run
        super.init(title: title, action: #selector(fire), keyEquivalent: "")
        target = self
        isEnabled = enabled
    }

    @available(*, unavailable)
    required init(coder: NSCoder) { fatalError("init(coder:) is not used") }

    @objc private func fire() { run() }
}
