// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import SwiftUI

/// Empty SwiftUI app used by the macOS CI to check the toolchain (P0.3).
@main
struct AtlasSmokeApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

struct ContentView: View {
    var body: some View {
        VStack(spacing: 12) {
            Image(systemName: "checkmark.seal")
                .font(.largeTitle)
                .foregroundStyle(.tint)
            Text(Smoke.greeting)
                .font(.title2)
        }
        .padding(40)
    }
}

enum Smoke {
    static let greeting = "Atlas – macOS-CI läuft"
}
