// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Foundation
import Testing

@testable import Atlas

/// Without calendar access (CI): the conversion into Atlas occurrences.
@MainActor
struct EventKitTests {
    @Test func eventsBecomeOccurrences() throws {
        let c = CalendarModel.cal
        let start = c.date(bySettingHour: 14, minute: 0, second: 0, of: Date())!
        let o = EventKitSource.occurrence(id: "ABC", calendarId: "CAL", color: "#ff0000", writable: false, title: "Zahnarzt",
                                          start: start, end: start.addingTimeInterval(1800), allDay: false, kind: .event,
                                          done: false, recurring: false)
        #expect(o.uid == "ek:ABC" && o.calendar == "ek:CAL")
        #expect(EventKitSource.isEventKit(o.uid))
        #expect(o.start == CalendarModel.iso(start) && o.end == CalendarModel.iso(start.addingTimeInterval(1800)))
        #expect(o.readOnly && o.title == "Zahnarzt")

        // All day: EventKit ends at 23:59:59, Atlas at the start of the next day.
        let day = c.startOfDay(for: Date())
        let all = EventKitSource.occurrence(id: "D", calendarId: "CAL", color: "", writable: true, title: "Urlaub",
                                            start: day, end: day.addingTimeInterval(86399), allDay: true, kind: .event,
                                            done: false, recurring: false)
        #expect(all.start == CalendarModel.day(day))
        #expect(all.end == CalendarModel.day(c.date(byAdding: .day, value: 1, to: day)!))
        #expect(EventKitSource.hex(NSColor(srgbRed: 0, green: 0.5, blue: 1, alpha: 1).cgColor) == "#0080ff")
    }

    @Test func offStaysOff() {
        // Nothing is asked or read unless the user switches it on.
        let source = EventKitSource.shared
        if !source.enabled { #expect(source.occurrences(from: .distantPast, to: .distantFuture).isEmpty) }
    }
}
