// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Foundation
import Observation

/// Actions, background jobs, processors and exporters of one window (P3.10;
/// Linux: `FileActions.qml`). The core prepares every command; this asks
/// before running, then hands it to the terminal or starts it in the
/// background.
@MainActor
@Observable
final class ActionsModel {
    @ObservationIgnored private weak var project: ProjectModel?
    @ObservationIgnored private var token: UUID?
    var paletteVisible = false
    var jobsVisible = false
    var extensionsVisible = false
    private(set) var jobs: [BackgroundJob] = []
    /// Bumped when extensions or action files changed (menus re-read).
    private(set) var revision = 0
    /// Asked before running (`confirm` or destructive).
    var pending: PendingRun?
    var error: String?

    struct PendingRun: Identifiable {
        let id = UUID()
        let key: String
        let prepared: Prepared
    }

    func attach(_ project: ProjectModel) {
        self.project = project
        token = project.listen { [weak self] event in
            switch event {
            case .backgroundJobChanged: self?.refreshJobs()
            case .extensionsChanged: self?.revision += 1
            default: break
            }
        }
        refreshJobs()
        // CI: `-AtlasExtension <bundled name>` installs it and opens its view.
        // Opened after the window's own start file, so it is in front.
        if let name = UserDefaults.standard.string(forKey: "AtlasExtension") {
            Task { @MainActor [weak self] in
                guard let self else { return }
                try? project.core.extensions().installBundled(name: name)
                if let view = views().first(where: { $0.extension == name }) { openView(view.id) }
            }
        }
    }

    // MARK: actions

    /// Actions for `paths` (none: the project's).
    func actions(for paths: [String]) -> [ActionInfo] {
        project?.core.actions().actionsFor(paths: paths) ?? []
    }

    /// Paths an action of the current tab works on (none for virtual tabs).
    var currentPaths: [String] {
        guard let tab = project?.editor.currentTab, !tab.isVirtual else { return [] }
        return [tab.path]
    }

    /// Prepares action `id` and runs it (after asking, if it says so).
    /// Returns the problem, if any.
    @discardableResult
    func run(id: String, paths: [String], line: Int = 0, quiet: Bool = false) -> String? {
        guard let project else { return "Kein Projekt" }
        do {
            let p = try project.core.actions().prepare(id: id, paths: paths, line: UInt32(max(0, line)))
            if p.confirm || p.destructive {
                pending = PendingRun(key: id, prepared: p)
            } else {
                execute(key: id, prepared: p, quiet: quiet)
            }
            return nil
        } catch {
            let message = atlasMessage(error)
            self.error = message
            return message
        }
    }

    /// Runs the script `path` (`.sh` or executable with `#!`).
    func runScript(_ path: String) {
        guard let project else { return }
        do {
            let p = try project.core.actions().prepareScript(path: path)
            pending = PendingRun(key: "script:" + path, prepared: p)
        } catch {
            self.error = atlasMessage(error)
        }
    }

    func confirmPending() {
        guard let run = pending else { return }
        pending = nil
        execute(key: run.key, prepared: run.prepared, quiet: false)
    }

    func execute(key: String, prepared p: Prepared, quiet: Bool) {
        guard let project else { return }
        if p.placement != .background, let terminal = project.runInTerminal {
            terminal(p, p.placement == .tab)
            project.show(notice: "Aktion „\(p.name)“ gestartet")
            return
        }
        // No terminal (yet): the core keeps it running, one per key.
        do {
            try project.core.actions().startBackground(key: key, prepared: p)
            if !quiet {
                project.show(notice: p.placement == .background
                    ? "„\(p.name)“ läuft im Hintergrund"
                    : "„\(p.name)“ läuft im Hintergrund (kein Terminal)")
            }
        } catch {
            self.error = atlasMessage(error)
        }
    }

    /// Opens the project's (`global` false) or the user's action file.
    func openActionsFile(global: Bool) {
        guard let project else { return }
        do {
            project.editor.open(path: try project.core.actions().actionsFile(global: global), mode: .tab)
        } catch {
            self.error = atlasMessage(error)
        }
    }

    // MARK: background jobs

    func refreshJobs() {
        jobs = project?.app.core.backgroundJobs() ?? []
    }

    func stop(_ key: String) {
        _ = project?.app.core.stopBackgroundJob(key: key)
        refreshJobs()
    }

    // MARK: processors and exporters

    func processors(for path: String) -> [ProcessorInfo] {
        project?.core.extensions().processorsFor(path: path) ?? []
    }

    func exporters(for path: String) -> [ExporterInfo] {
        project?.core.extensions().exportersFor(path: path) ?? []
    }

    /// Shows `path` through processor or view `id` in a tab of its own.
    func show(_ path: String, with id: String) {
        project?.editor.open(path: path, mode: .tab, processor: id)
    }

    /// Opens view `id` (`view:<ext>/<id>`) on the project folder.
    func openView(_ id: String) {
        guard let project else { return }
        project.editor.open(path: project.root, mode: .tab, processor: id)
    }

    func views() -> [ViewInfo] {
        project?.core.extensions().views() ?? []
    }

    /// Exports `path` with `exporter`; asks before replacing a file.
    func export(_ path: String, with exporter: ExporterInfo) async {
        guard let project else { return }
        if FileManager.default.fileExists(atPath: exporter.output) {
            let alert = NSAlert()
            alert.messageText = "Datei ersetzen?"
            alert.informativeText = "„\((exporter.output as NSString).lastPathComponent)“ existiert bereits und wird durch das Ergebnis von „\(exporter.name)“ ersetzt."
            alert.addButton(withTitle: "Ersetzen")
            alert.addButton(withTitle: "Abbrechen")
            guard alert.runModal() == .alertFirstButtonReturn else { return }
        }
        project.show(notice: "Exportiere mit „\(exporter.name)“ …")
        do {
            let result = try await project.core.extensions().export(id: exporter.id, path: path, output: exporter.output)
            project.show(notice: result.message)
            if result.open { project.editor.open(path: exporter.output, mode: .tab) }
        } catch {
            self.error = atlasMessage(error)
        }
    }
}

/// The message of a core error (without the case name).
func atlasMessage(_ error: Error) -> String {
    guard let e = error as? AtlasError else { return error.localizedDescription }
    switch e {
    case let .NotFound(message), let .Invalid(message), let .Conflict(message),
         let .Unavailable(message), let .Auth(message), let .Failed(message):
        return message
    }
}
