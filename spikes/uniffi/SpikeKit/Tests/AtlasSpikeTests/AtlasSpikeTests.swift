// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasSpike
import Foundation
import Testing

@Test func plainCalls() {
    #expect(add(a: 2, b: 40) == 42)
    #expect(coreVersion().hasPrefix("atlas-spike"))
}

@Test func recordAndError() throws {
    let dir = FileManager.default.temporaryDirectory
        .appending(path: "atlas-spike-\(UUID().uuidString)")
    try FileManager.default.createDirectory(
        at: dir.appending(path: ".atlas"), withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: dir) }

    let info = try describeProject(path: dir.path)
    #expect(info.hasAtlasDir)
    #expect(info.entries == 1)
    #expect(info.name == dir.lastPathComponent)

    #expect(throws: SpikeError.self) {
        try describeProject(path: "/definitely/missing")
    }
}

@Test func asyncCall() async {
    let sum = await delayedSum(values: [1, 2, 3, 36], delayMs: 50)
    #expect(sum == 42)
}

@Test func manyConcurrentAsyncCalls() async {
    let total = await withTaskGroup(of: Int64.self) { group in
        for i in 1...50 {
            group.addTask { await delayedSum(values: [Int64(i)], delayMs: 10) }
        }
        return await group.reduce(0, +)
    }
    #expect(total == 1275)
}

@MainActor
final class Recorder {
    var events: [SpikeEvent] = []
    var onMainThread = true
}

@MainActor
@Test func callbackArrivesOnMainActorInOrder() async {
    let recorder = Recorder()
    await withCheckedContinuation { (done: CheckedContinuation<Void, Never>) in
        let sink = MainActorEventSink { event in
            MainActor.assertIsolated()
            recorder.onMainThread = recorder.onMainThread && Thread.isMainThread
            recorder.events.append(event)
            if case .done = event { done.resume() }
        }
        Ticker(intervalMs: 5).start(sink: sink, count: 20)
    }
    #expect(recorder.onMainThread)
    #expect(recorder.events.count == 21)
    #expect(recorder.events.first == .tick(n: 1))
    #expect(recorder.events.last == .done(total: 20))
    #expect(Array(recorder.events.dropLast()) == (1...20).map { SpikeEvent.tick(n: $0) })
}
