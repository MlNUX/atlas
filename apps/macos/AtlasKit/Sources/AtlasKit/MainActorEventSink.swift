// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import Foundation

/// Delivers the core's events on the main actor, in the order the core sent
/// them (main queue is FIFO; unstructured tasks are not), see
/// docs/porting/spike-uniffi.md.
public final class MainActorEventSink: EventSink {
    private let handler: @MainActor @Sendable (Event) -> Void

    public init(_ handler: @escaping @MainActor @Sendable (Event) -> Void) {
        self.handler = handler
    }

    public func onEvent(event: Event) {
        let handler = self.handler
        DispatchQueue.main.async {
            MainActor.assumeIsolated {
                handler(event)
            }
        }
    }
}
