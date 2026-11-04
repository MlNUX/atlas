// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasSpike
import Observation
import SwiftUI

/// Spike P0.2: empty SwiftUI app that shows values coming from Rust.
@main
struct SpikeApp: App {
    @State private var model = SpikeModel()

    var body: some Scene {
        WindowGroup("Atlas UniFFI-Spike") {
            SpikeView(model: model)
                .task { await model.load() }
        }
    }
}

@MainActor
@Observable
final class SpikeModel {
    var version = ""
    var sum: Int64 = 0
    var asyncSum: Int64?
    var project = ""
    var ticks: [String] = []
    private var ticker: Ticker?

    func load() async {
        version = coreVersion()
        sum = add(a: 2, b: 40)
        do {
            let info = try describeProject(path: NSHomeDirectory())
            project = "\(info.name): \(info.entries) Einträge"
        } catch {
            project = "Fehler: \(error)"
        }
        let ticker = Ticker(intervalMs: 300)
        ticker.start(
            sink: MainActorEventSink { [weak self] event in
                switch event {
                case .tick(let n): self?.ticks.append("Tick \(n)")
                case .done(let total): self?.ticks.append("Fertig (\(total))")
                }
            }, count: 5)
        self.ticker = ticker
        asyncSum = await delayedSum(values: [10, 20, 12], delayMs: 200)
    }
}

struct SpikeView: View {
    let model: SpikeModel

    var body: some View {
        Form {
            LabeledContent("Kern", value: model.version)
            LabeledContent("add(2, 40)", value: "\(model.sum)")
            LabeledContent("async delayedSum", value: model.asyncSum.map(String.init) ?? "…")
            LabeledContent("Projekt", value: model.project)
            LabeledContent("Ereignisse", value: model.ticks.joined(separator: ", "))
        }
        .formStyle(.grouped)
        .frame(minWidth: 520, minHeight: 300)
    }
}
