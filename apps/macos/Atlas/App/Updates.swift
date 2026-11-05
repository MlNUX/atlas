// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import Foundation
import Sparkle
import SwiftUI

/// Updates with Sparkle (P4.4, E15): the appcast of the GitHub releases
/// (`SUFeedURL`), signed with the project's EdDSA key (`SUPublicEDKey`).
/// Builds without a key (local, tests) have no updater.
@MainActor
final class Updates {
    static let shared = Updates()
    let controller: SPUStandardUpdaterController?

    private init() {
        let key = Bundle.main.object(forInfoDictionaryKey: "SUPublicEDKey") as? String ?? ""
        let testing = ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil
        guard !key.isEmpty, !testing else {
            controller = nil
            return
        }
        controller = SPUStandardUpdaterController(startingUpdater: true, updaterDelegate: nil, userDriverDelegate: nil)
        // CI (packaging/macos/ci-update.sh): look at once, without asking.
        if UserDefaults.standard.bool(forKey: "AtlasCheckUpdates") {
            controller?.updater.checkForUpdatesInBackground()
        }
    }

    func check() {
        controller?.checkForUpdates(nil)
    }
}

/// „Nach Updates suchen …“ in the app menu.
struct UpdateCommands: Commands {
    var body: some Commands {
        CommandGroup(after: .appInfo) {
            Button("Nach Updates suchen …") { Updates.shared.check() }
                .disabled(Updates.shared.controller == nil)
        }
    }
}
