// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import Testing

@testable import AtlasSmoke

@Test func greetingIsSet() {
    #expect(Smoke.greeting.hasPrefix("Atlas"))
}

@MainActor
@Test func contentViewBuilds() {
    _ = ContentView().body
}
