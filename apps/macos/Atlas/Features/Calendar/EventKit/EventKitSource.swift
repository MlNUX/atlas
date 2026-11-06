// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import EventKit
import Foundation
import Observation

/// The calendars of macOS (EventKit, P3.13, E14) next to the Atlas entries:
/// appointments and reminders with a date appear in week, month and agenda
/// (ids with the prefix `ek:`); new entries can go to a macOS calendar;
/// moving and editing write back if the calendar allows it. Off until the
/// user switches it on (then macOS asks for access once).
@MainActor
@Observable
final class EventKitSource {
    static let shared = EventKitSource()
    nonisolated static let prefix = "ek:"

    @ObservationIgnored let store = EKEventStore()
    /// Switched on by the user (per Mac).
    var enabled: Bool {
        didSet {
            UserDefaults.standard.set(enabled, forKey: "AtlasEventKit")
            if enabled { Task { await requestAccess() } }
            revision += 1
        }
    }
    private(set) var eventsAllowed: Bool
    private(set) var remindersAllowed: Bool
    /// Calendars hidden in Atlas (identifiers).
    private(set) var hidden: Set<String>
    /// Bumped when macOS reports changes (`EKEventStoreChanged`) or the choice changes.
    private(set) var revision = 0
    var error: String?

    private init() {
        enabled = UserDefaults.standard.bool(forKey: "AtlasEventKit")
        eventsAllowed = EKEventStore.authorizationStatus(for: .event) == .fullAccess
        remindersAllowed = EKEventStore.authorizationStatus(for: .reminder) == .fullAccess
        hidden = Set(UserDefaults.standard.stringArray(forKey: "AtlasEventKitHidden") ?? [])
        NotificationCenter.default.addObserver(forName: .EKEventStoreChanged, object: store, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.revision += 1 }
        }
    }

    var active: Bool { enabled && (eventsAllowed || remindersAllowed) }

    func requestAccess() async {
        do {
            eventsAllowed = try await store.requestFullAccessToEvents()
            remindersAllowed = try await store.requestFullAccessToReminders()
            error = eventsAllowed ? nil : "Kein Zugriff auf die Kalender von macOS – in den Systemeinstellungen unter Datenschutz & Sicherheit → Kalender erlauben."
        } catch {
            self.error = error.localizedDescription
        }
        revision += 1
    }

    // MARK: calendars

    var calendars: [EKCalendar] {
        guard active else { return [] }
        var out: [EKCalendar] = []
        if eventsAllowed { out += store.calendars(for: .event) }
        if remindersAllowed { out += store.calendars(for: .reminder) }
        return out.sorted { ($0.source.title, $0.title) < ($1.source.title, $1.title) }
    }

    /// Calendars new appointments can go to.
    var writableEventCalendars: [EKCalendar] {
        calendars.filter { $0.allowedEntityTypes.contains(.event) && $0.allowsContentModifications }
    }

    func isVisible(_ c: EKCalendar) -> Bool { !hidden.contains(c.calendarIdentifier) }

    func setVisible(_ c: EKCalendar, _ visible: Bool) {
        if visible { hidden.remove(c.calendarIdentifier) } else { hidden.insert(c.calendarIdentifier) }
        UserDefaults.standard.set(Array(hidden), forKey: "AtlasEventKitHidden")
        revision += 1
    }

    // MARK: reading

    /// Appointments and dated reminders in `[from, to)` as occurrences.
    func occurrences(from: Date, to: Date) -> [Occurrence] {
        guard active else { return [] }
        let shown = calendars.filter(isVisible)
        var out: [Occurrence] = []
        let eventCalendars = shown.filter { $0.allowedEntityTypes.contains(.event) }
        if eventsAllowed, !eventCalendars.isEmpty {
            let predicate = store.predicateForEvents(withStart: from, end: to, calendars: eventCalendars)
            for e in store.events(matching: predicate) {
                out.append(Self.occurrence(
                    id: e.eventIdentifier ?? e.calendarItemIdentifier, calendar: e.calendar, title: e.title ?? "",
                    start: e.startDate, end: e.endDate, allDay: e.isAllDay, kind: .event, done: false,
                    recurring: e.hasRecurrenceRules))
            }
        }
        // Reminders with a due date in the range (fetching is asynchronous: cached per range).
        out += reminders.filter { o in
            guard let s = CalendarModel.date(o.start) else { return false }
            return s >= from && s < to && shown.contains { Self.prefix + $0.calendarIdentifier == o.calendar }
        }
        return out
    }

    @ObservationIgnored private var reminders: [Occurrence] = []

    /// Loads the reminders of `[from, to)` (then `revision` changes).
    func loadReminders(from: Date, to: Date) {
        guard active, remindersAllowed else { return }
        let calendars = self.calendars.filter { $0.allowedEntityTypes.contains(.reminder) && isVisible($0) }
        guard !calendars.isEmpty else { return }
        let predicate = store.predicateForIncompleteReminders(withDueDateStarting: from, ending: to, calendars: calendars)
        store.fetchReminders(matching: predicate) { [weak self] found in
            let list: [Occurrence] = (found ?? []).compactMap { r in
                guard let due = r.dueDateComponents, let date = Foundation.Calendar.current.date(from: due) else { return nil }
                let allDay = due.hour == nil
                return Self.occurrence(id: r.calendarItemIdentifier, calendar: r.calendar, title: r.title ?? "",
                                       start: date, end: date, allDay: allDay, kind: .deadline, done: r.isCompleted,
                                       recurring: r.hasRecurrenceRules)
            }
            Task { @MainActor in
                guard let self, list != self.reminders else { return }
                self.reminders = list
                self.revision += 1
            }
        }
    }

    /// An EventKit item as an occurrence of the Atlas views.
    nonisolated static func occurrence(id: String, calendarId: String, color: String, writable: Bool, title: String,
                                       start: Date, end: Date, allDay: Bool, kind: EntryKind, done: Bool,
                                       recurring: Bool) -> Occurrence {
        let time: (Date) -> String = { allDay ? CalendarModel.day($0) : CalendarModel.iso($0) }
        // All day in EventKit ends at 23:59:59 of the last day; Atlas ends at the start of the next.
        let endDay = allDay ? CalendarModel.cal.date(byAdding: .day, value: 1, to: CalendarModel.cal.startOfDay(for: end))! : end
        return Occurrence(
            uid: prefix + id, instance: time(start), start: time(start), end: time(endDay), allDay: allDay, kind: kind,
            title: title, status: done ? .done : .planned, calendar: prefix + calendarId, color: color, recurring: recurring,
            readOnly: !writable, files: [], tags: [], session: "", checklistDone: 0, checklistTotal: 0)
    }

    private static func occurrence(id: String, calendar: EKCalendar, title: String, start: Date, end: Date, allDay: Bool,
                                   kind: EntryKind, done: Bool, recurring: Bool) -> Occurrence {
        occurrence(id: id, calendarId: calendar.calendarIdentifier, color: hex(calendar.cgColor),
                   writable: calendar.allowsContentModifications, title: title, start: start, end: end, allDay: allDay,
                   kind: kind, done: done, recurring: recurring)
    }

    nonisolated static func hex(_ color: CGColor?) -> String {
        guard let color, let c = NSColor(cgColor: color)?.usingColorSpace(.sRGB) else { return "" }
        return String(format: "#%02x%02x%02x", Int(round(c.redComponent * 255)), Int(round(c.greenComponent * 255)),
                      Int(round(c.blueComponent * 255)))
    }

    nonisolated static func isEventKit(_ uid: String) -> Bool { uid.hasPrefix(prefix) }

    private func item(_ uid: String) -> EKCalendarItem? {
        let id = String(uid.dropFirst(Self.prefix.count))
        return store.event(withIdentifier: id) ?? store.calendarItem(withIdentifier: id)
    }

    // MARK: writing

    /// The item as a draft of the entry dialog.
    func draft(uid: String) -> EntryDraft? {
        guard let item = item(uid) else { return nil }
        let calendar = Self.prefix + item.calendar.calendarIdentifier
        let readOnly = !item.calendar.allowsContentModifications
        if let e = item as? EKEvent {
            let end = e.isAllDay ? CalendarModel.cal.date(byAdding: .day, value: 1, to: CalendarModel.cal.startOfDay(for: e.endDate))! : e.endDate!
            return EntryDraft(uid: uid, kind: .event, title: e.title ?? "", description: e.notes ?? "", allDay: e.isAllDay,
                              start: e.startDate, end: end, status: .planned, tags: "", checklist: [], files: [], session: "",
                              estimate: nil, reminders: (e.alarms ?? []).map { Int64(-$0.relativeOffset / 60) },
                              recurrence: nil, calendar: calendar, readOnly: readOnly, original: nil)
        }
        if let r = item as? EKReminder {
            let due = r.dueDateComponents.flatMap { Foundation.Calendar.current.date(from: $0) } ?? Date()
            return EntryDraft(uid: uid, kind: .deadline, title: r.title ?? "", description: r.notes ?? "",
                              allDay: r.dueDateComponents?.hour == nil, start: due, end: due,
                              status: r.isCompleted ? .done : .planned, tags: "", checklist: [], files: [], session: "",
                              estimate: nil, reminders: [], recurrence: nil, calendar: calendar, readOnly: readOnly, original: nil)
        }
        return nil
    }

    /// New or changed appointment / reminder in a macOS calendar.
    func save(_ draft: EntryDraft) throws {
        let calendarId = String(draft.calendar.dropFirst(Self.prefix.count))
        guard let calendar = store.calendar(withIdentifier: calendarId) else {
            throw AtlasError.NotFound(message: "Der macOS-Kalender gibt es nicht mehr.")
        }
        if draft.kind == .deadline || draft.kind == .task, calendar.allowedEntityTypes.contains(.reminder) {
            let r = (item(draft.uid) as? EKReminder) ?? EKReminder(eventStore: store)
            r.calendar = calendar
            r.title = draft.title
            r.notes = draft.description.isEmpty ? nil : draft.description
            if draft.kind == .deadline {
                let parts: Set<Foundation.Calendar.Component> = draft.allDay ? [.year, .month, .day] : [.year, .month, .day, .hour, .minute]
                r.dueDateComponents = Foundation.Calendar.current.dateComponents(parts, from: draft.start)
            }
            r.isCompleted = draft.status == .done
            try store.save(r, commit: true)
            return
        }
        let e = (item(draft.uid) as? EKEvent) ?? EKEvent(eventStore: store)
        e.calendar = calendar
        e.title = draft.title
        e.notes = draft.description.isEmpty ? nil : draft.description
        e.isAllDay = draft.allDay
        e.startDate = draft.start
        // Atlas' all-day end is the next day's start.
        e.endDate = draft.allDay ? max(draft.start, draft.end.addingTimeInterval(-1)) : max(draft.end, draft.start)
        e.alarms = draft.reminders.map { EKAlarm(relativeOffset: TimeInterval(-$0 * 60)) }
        try store.save(e, span: .thisEvent, commit: true)
    }

    func move(uid: String, start: Date, end: Date, allDay: Bool) throws {
        guard let e = item(uid) as? EKEvent else { return }
        e.isAllDay = allDay
        e.startDate = start
        e.endDate = allDay ? max(start, end.addingTimeInterval(-1)) : end
        try store.save(e, span: .thisEvent, commit: true)
    }

    func remove(uid: String) throws {
        if let e = item(uid) as? EKEvent { try store.remove(e, span: .thisEvent, commit: true) }
        else if let r = item(uid) as? EKReminder { try store.remove(r, commit: true) }
    }

    func setDone(uid: String, _ done: Bool) throws {
        guard let r = item(uid) as? EKReminder else { return }
        r.isCompleted = done
        try store.save(r, commit: true)
    }
}
