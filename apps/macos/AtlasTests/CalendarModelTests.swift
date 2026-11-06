// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import Foundation
import Testing

@testable import Atlas

@MainActor
struct CalendarModelTests {
    func makeModel() throws -> CalendarModel {
        let dir = FileManager.default.temporaryDirectory.appending(path: "atlas-calendar-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let project = try ProjectModel(app: AppModel(), root: dir.path)
        return CalendarModel(project: project)
    }

    @Test func entriesAppearMoveAndGo() throws {
        let model = try makeModel()
        defer { model.stop() }
        let c = CalendarModel.cal
        let start = c.date(bySettingHour: 10, minute: 0, second: 0, of: Date())!
        var draft = EntryDraft.new(kind: .event, start: start, allDay: false, calendar: model.calendar.defaultCalendar())
        draft.title = "Vorlesung"
        model.save(draft)
        #expect(model.error == nil)
        let o = try #require(model.occurrences(on: start).first { $0.title == "Vorlesung" })
        #expect(o.start == CalendarModel.iso(start))
        #expect(o.end == CalendarModel.iso(start.addingTimeInterval(3600)))

        // A day later and 20 minutes on the 15-minute grid: 11:15.
        model.move(o, to: c.date(byAdding: .day, value: 1, to: start)!.addingTimeInterval(80 * 60))
        let moved = c.date(byAdding: .day, value: 1, to: start)!.addingTimeInterval(75 * 60)
        model.anchor = moved
        #expect(model.occurrences(on: moved).contains { $0.title == "Vorlesung" && $0.start == CalendarModel.iso(moved) })

        model.remove(uid: o.uid)
        #expect(!model.occurrences(on: moved).contains { $0.title == "Vorlesung" })
    }

    @Test func tasksArePlannedAndAllDayKeepsDates() throws {
        let model = try makeModel()
        defer { model.stop() }
        model.addTask("Blatt 3")
        let task = try #require(model.tasks.first { $0.title == "Blatt 3" })
        let at = CalendarModel.nextFullHour()
        model.schedule(task.uid, at: at)
        #expect(!model.tasks.contains { $0.uid == task.uid })
        model.anchor = at
        #expect(model.occurrences(on: at).contains { $0.uid == task.uid && $0.kind == .block })

        // All day: dates only, the end the next day.
        var day = EntryDraft.new(kind: .event, start: Date(), allDay: true, calendar: "")
        day.title = "Ausflug"
        let entry = day.entry()
        #expect(entry.start?.contains("T") == false)
        #expect(entry.end == CalendarModel.day(CalendarModel.cal.date(byAdding: .day, value: 1, to: CalendarModel.cal.startOfDay(for: Date()))!))
        // Timed deadlines have no end.
        var deadline = EntryDraft.new(kind: .deadline, start: Date(), allDay: false, calendar: "")
        deadline.title = "Abgabe"
        #expect(deadline.entry().end == nil)
        #expect(CalendarModel.snap(CalendarModel.cal.date(bySettingHour: 9, minute: 8, second: 0, of: Date())!)
                == CalendarModel.cal.date(bySettingHour: 9, minute: 15, second: 0, of: Date())!)
    }
}

@MainActor
struct CalendarSyncTests {
    @Test func noAccountUntilOneIsSaved() throws {
        let dir = FileManager.default.temporaryDirectory.appending(path: "atlas-sync-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let project = try ProjectModel(app: AppModel(), root: dir.path)
        let sync = CalendarSyncModel(project: project)
        defer { sync.stop() }
        #expect(sync.account.provider == .none)
        #expect(!sync.state.connected)
        #expect(sync.conflicts.isEmpty)
        // Saving „no account“ keeps it local and reports no error.
        sync.save()
        #expect(sync.error == nil)
        #expect(!sync.calendar.syncState().connected)
    }
}
