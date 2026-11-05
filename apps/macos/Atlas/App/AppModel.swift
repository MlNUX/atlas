// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Observation

/// Process-wide state: the core (`Atlas`), open projects, app events.
@MainActor
@Observable
final class AppModel {
    let core: Atlas
    private(set) var projects: [String: ProjectModel] = [:]
    /// Last error to show (opening a project, …).
    var error: String?
    /// Bumped on `Event.settingsChanged` / `.placesChanged`.
    private(set) var settingsRevision = 0

    init() {
        let box = WeakBox<AppModel>()
        core = Atlas(sink: MainActorEventSink { event in box.value?.handle(event) })
        box.value = self
        NotificationCenter.default.addObserver(
            forName: NSApplication.willTerminateNotification, object: nil, queue: .main
        ) { [core] _ in
            // macOS has no parent-death signal (P1.2): end background jobs here.
            core.stopAllBackgroundJobs()
        }
    }

    /// The model of the project at `root`, opened on first use (nil on errors).
    func project(at root: String) -> ProjectModel? {
        if let open = projects[root] { return open }
        do {
            let model = try ProjectModel(app: self, root: root)
            projects[root] = model
            return model
        } catch {
            self.error = "\(error)"
            return nil
        }
    }

    func knownProjects() -> [KnownProject] { core.knownProjects() }

    private func handle(_ event: Event) {
        switch event {
        case .settingsChanged, .placesChanged:
            settingsRevision += 1
        default:
            break
        }
    }
}

/// Lets an event sink refer back to its owner without a retain cycle.
@MainActor
final class WeakBox<T: AnyObject> {
    weak var value: T?
}
