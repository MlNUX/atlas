// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import Foundation

/// Bridges Rust callbacks (arriving on arbitrary Rust threads) to the main actor.
///
/// Uses the main dispatch queue rather than `Task { @MainActor in }`: the queue
/// is FIFO, so events reach SwiftUI in the order Rust sent them. Unstructured
/// tasks give no such ordering guarantee.
public final class MainActorEventSink: EventSink {
    private let handler: @MainActor @Sendable (SpikeEvent) -> Void

    public init(_ handler: @escaping @MainActor @Sendable (SpikeEvent) -> Void) {
        self.handler = handler
    }

    public func onEvent(event: SpikeEvent) {
        let handler = self.handler
        DispatchQueue.main.async {
            MainActor.assumeIsolated {
                handler(event)
            }
        }
    }
}
