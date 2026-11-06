// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import Foundation
import Observation
import UserNotifications

/// The Atlas calendar of one window (P3.7; Linux: `CalendarView.qml`): what
/// is shown (week, month, agenda), its occurrences, the tasks without a
/// time, the calendars, reminders. The core stores everything (ICS, sync).
@MainActor
@Observable
final class CalendarModel {
    enum Mode: String, CaseIterable {
        case week, month, agenda
        var title: String {
            switch self {
            case .week: "Woche"
            case .month: "Monat"
            case .agenda: "Agenda"
            }
        }
    }

    let project: ProjectModel
    var calendar: AtlasKit.Calendar { project.core.calendar() }
    var mode: Mode = .week { didSet { remember(); reload() } }
    /// A day in the shown range.
    var anchor = Date() { didSet { reload() } }
    var showTasks = true { didSet { remember() } }
    private(set) var occurrences: [Occurrence] = []
    private(set) var tasks: [Entry] = []
    private(set) var calendars: [CalendarInfo] = []
    var editing: EntryDraft?
    var error: String?
    @ObservationIgnored private var token: UUID?
    @ObservationIgnored private var reminderTask: Task<Void, Never>?
    nonisolated static let cal: Foundation.Calendar = {
        var c = Foundation.Calendar(identifier: .gregorian)
        c.locale = Locale(identifier: "de_DE")
        c.firstWeekday = 2
        return c
    }()

    init(project: ProjectModel) {
        self.project = project
        if let data = project.core.calendar().viewState().data(using: .utf8),
           let state = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            mode = (state["mode"] as? String).flatMap(Mode.init(rawValue:)) ?? .week
            showTasks = state["tasks"] as? Bool ?? true
        }
        token = project.listen { [weak self] event in
            if case .calendarChanged = event { self?.reload() }
        }
        reload()
        startReminders()
    }

    func stop() {
        if let token { project.stopListening(token) }
        reminderTask?.cancel()
    }

    private func remember() {
        let state: [String: Any] = ["mode": mode.rawValue, "tasks": showTasks]
        if let data = try? JSONSerialization.data(withJSONObject: state), let text = String(data: data, encoding: .utf8) {
            calendar.setViewState(state: text)
        }
    }

    // MARK: range

    /// `[from, to)` of the shown range (week from Monday, month as 6 weeks, agenda 30 days).
    var range: (Date, Date) {
        let c = Self.cal
        let day = c.startOfDay(for: anchor)
        switch mode {
        case .week:
            let start = c.dateInterval(of: .weekOfYear, for: day)?.start ?? day
            return (start, c.date(byAdding: .day, value: 7, to: start)!)
        case .month:
            let first = c.dateInterval(of: .month, for: day)?.start ?? day
            let start = c.dateInterval(of: .weekOfYear, for: first)?.start ?? first
            return (start, c.date(byAdding: .day, value: 42, to: start)!)
        case .agenda:
            return (day, c.date(byAdding: .day, value: 30, to: day)!)
        }
    }

    var title: String {
        let (start, end) = range
        let c = Self.cal
        switch mode {
        case .week:
            let last = c.date(byAdding: .day, value: -1, to: end)!
            let week = c.component(.weekOfYear, from: start)
            return "KW \(week) · \(start.formatted(.dateTime.day().month(.abbreviated))) – \(last.formatted(.dateTime.day().month(.abbreviated).year()))"
        case .month:
            return anchor.formatted(.dateTime.month(.wide).year())
        case .agenda:
            return "Ab \(start.formatted(.dateTime.weekday(.wide).day().month(.wide)))"
        }
    }

    func page(_ n: Int) {
        let c = Self.cal
        switch mode {
        case .week: anchor = c.date(byAdding: .day, value: 7 * n, to: anchor)!
        case .month: anchor = c.date(byAdding: .month, value: n, to: anchor)!
        case .agenda: anchor = c.date(byAdding: .day, value: 30 * n, to: anchor)!
        }
    }

    func today() { anchor = Date() }

    func reload() {
        let (from, to) = range
        do {
            occurrences = try calendar.occurrences(from: Self.iso(from), to: Self.iso(to))
            error = nil
        } catch {
            self.error = atlasMessage(error)
        }
        tasks = calendar.tasks().filter { $0.status != .done }
        calendars = calendar.calendars()
    }

    /// Occurrences on `day` (all-day ones first).
    func occurrences(on day: Date) -> [Occurrence] {
        let c = Self.cal
        let start = c.startOfDay(for: day), end = c.date(byAdding: .day, value: 1, to: start)!
        return occurrences.filter { o in
            guard let s = Self.date(o.start), let e = Self.date(o.end) else { return false }
            // Deadlines have no length: they belong to their start.
            return s < end && (e > start || (s >= start && s < end))
        }
        .sorted { ($0.allDay ? 0 : 1, $0.start) < ($1.allDay ? 0 : 1, $1.start) }
    }

    // MARK: editing

    func newEntry(kind: EntryKind, start: Date? = nil, allDay: Bool = false) {
        editing = EntryDraft.new(kind: kind, start: start ?? Self.nextFullHour(), allDay: allDay,
                                 calendar: calendar.defaultCalendar())
    }

    func edit(uid: String) {
        guard let entry = calendar.entry(uid: uid) else { return }
        editing = EntryDraft(entry: entry)
    }

    func save(_ draft: EntryDraft) {
        do {
            _ = try calendar.save(entry: draft.entry())
            editing = nil
            reload()
        } catch {
            self.error = atlasMessage(error)
        }
    }

    func remove(uid: String) {
        perform { try calendar.remove(uid: uid) }
        editing = nil
    }

    /// Moves an occurrence to `start` (keeping its length), on a 15-minute grid.
    func move(_ o: Occurrence, to start: Date, allDay: Bool? = nil) {
        guard let s = Self.date(o.start), let e = Self.date(o.end) else { return }
        let length = max(e.timeIntervalSince(s), o.allDay ? 86400 : 900)
        let all = allDay ?? o.allDay
        let begin = all ? Self.cal.startOfDay(for: start) : Self.snap(start)
        let end = begin.addingTimeInterval(all && !o.allDay ? 86400 : length)
        perform {
            try calendar.moveOccurrence(uid: o.uid, instance: o.instance, start: all ? Self.day(begin) : Self.iso(begin),
                                        end: all ? Self.day(end) : Self.iso(end), allDay: all)
        }
    }

    func setDone(_ uid: String, _ done: Bool) {
        perform { try calendar.setStatus(uid: uid, status: done ? .done : .planned) }
    }

    /// A task as a work block from `start` (an hour, or its estimate).
    func schedule(_ uid: String, at start: Date) {
        perform { try calendar.schedule(uid: uid, start: Self.iso(Self.snap(start)), end: nil) }
    }

    func unschedule(_ uid: String) {
        perform { try calendar.schedule(uid: uid, start: nil, end: nil) }
    }

    func addTask(_ title: String) {
        let t = title.trimmingCharacters(in: .whitespaces)
        guard !t.isEmpty else { return }
        var draft = EntryDraft.new(kind: .task, start: Date(), allDay: false, calendar: calendar.defaultCalendar())
        draft.title = t
        save(draft)
    }

    func setVisible(_ id: String, _ visible: Bool) { perform { try calendar.setCalendarVisible(id: id, visible: visible) } }
    func setDefault(_ id: String) { perform { try calendar.setDefaultCalendar(id: id, project: false) } }

    private func perform(_ work: () throws -> Void) {
        do {
            try work()
            error = nil
        } catch {
            self.error = atlasMessage(error)
        }
        reload()
    }

    // MARK: reminders

    /// Due reminders every 30 s as system notifications (and a note in the window).
    private func startReminders() {
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in }
        reminderTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(30))
                guard let self else { return }
                for r in self.calendar.dueReminders() { self.notify(r) }
            }
        }
    }

    private func notify(_ r: Reminder) {
        let start = Self.date(r.start).map { $0.formatted(date: .omitted, time: .shortened) } ?? r.start
        project.show(notice: "Erinnerung: \(r.title) um \(start)")
        let content = UNMutableNotificationContent()
        content.title = r.title
        content.body = r.minutesBefore > 0 ? "Beginnt in \(r.minutesBefore) Minuten (\(start))" : "Beginnt jetzt (\(start))"
        content.sound = .default
        UNUserNotificationCenter.current().add(UNNotificationRequest(identifier: "\(r.uid)#\(r.start)#\(r.minutesBefore)",
                                                                     content: content, trigger: nil))
    }

    // MARK: dates as the core writes them (local time)

    nonisolated private static func formatter(_ format: String) -> DateFormatter {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.calendar = Foundation.Calendar(identifier: .gregorian)
        f.dateFormat = format
        return f
    }

    nonisolated static func iso(_ d: Date) -> String { formatter("yyyy-MM-dd'T'HH:mm").string(from: d) }
    nonisolated static func day(_ d: Date) -> String { formatter("yyyy-MM-dd").string(from: d) }

    /// `2026-11-06T14:30` or `2026-11-06` (all day).
    nonisolated static func date(_ text: String) -> Date? {
        text.contains("T") ? formatter("yyyy-MM-dd'T'HH:mm").date(from: String(text.prefix(16))) : formatter("yyyy-MM-dd").date(from: text)
    }

    /// On a 15-minute grid.
    nonisolated static func snap(_ d: Date) -> Date {
        let quarter: TimeInterval = 900
        return Date(timeIntervalSinceReferenceDate: (d.timeIntervalSinceReferenceDate / quarter).rounded() * quarter)
    }

    nonisolated static func nextFullHour(after d: Date = Date()) -> Date {
        let c = cal
        let hour = c.dateInterval(of: .hour, for: d)?.start ?? d
        return c.date(byAdding: .hour, value: 1, to: hour)!
    }
}

/// An entry as the dialog edits it (dates as `Date`, all day as a switch).
struct EntryDraft: Identifiable {
    let id = UUID()
    var uid: String
    var kind: EntryKind
    var title: String
    var description: String
    var allDay: Bool
    var start: Date
    var end: Date
    var status: EntryStatus
    var tags: String
    var checklist: [ChecklistItem]
    var files: [String]
    var session: String
    var estimate: Int?
    var reminders: [Int64]
    var recurrence: Recurrence?
    var calendar: String
    var readOnly: Bool
    private var original: Entry?

    static func new(kind: EntryKind, start: Date, allDay: Bool, calendar: String) -> EntryDraft {
        let s = allDay ? CalendarModel.cal.startOfDay(for: start) : start
        return EntryDraft(uid: "", kind: kind, title: "", description: "", allDay: allDay,
                          start: s, end: s.addingTimeInterval(allDay ? 86400 : 3600), status: .planned, tags: "",
                          checklist: [], files: [], session: "", estimate: nil, reminders: kind == .event ? [15] : [],
                          recurrence: nil, calendar: calendar, readOnly: false, original: nil)
    }

    init(uid: String, kind: EntryKind, title: String, description: String, allDay: Bool, start: Date, end: Date,
         status: EntryStatus, tags: String, checklist: [ChecklistItem], files: [String], session: String, estimate: Int?,
         reminders: [Int64], recurrence: Recurrence?, calendar: String, readOnly: Bool, original: Entry?) {
        self.uid = uid; self.kind = kind; self.title = title; self.description = description; self.allDay = allDay
        self.start = start; self.end = end; self.status = status; self.tags = tags; self.checklist = checklist
        self.files = files; self.session = session; self.estimate = estimate; self.reminders = reminders
        self.recurrence = recurrence; self.calendar = calendar; self.readOnly = readOnly; self.original = original
    }

    init(entry e: Entry) {
        let start = e.start.flatMap(CalendarModel.date) ?? CalendarModel.nextFullHour()
        let allDay = e.start.map { !$0.contains("T") } ?? false
        let end = e.end.flatMap(CalendarModel.date) ?? start.addingTimeInterval(allDay ? 86400 : 3600)
        self.init(uid: e.uid, kind: e.kind, title: e.title, description: e.description, allDay: allDay, start: start,
                  end: end, status: e.status, tags: e.tags.joined(separator: ", "), checklist: e.checklist, files: e.files,
                  session: e.session, estimate: e.estimateMinutes.map(Int.init), reminders: e.reminders,
                  recurrence: e.recurrence, calendar: e.calendar, readOnly: e.readOnly, original: e)
    }

    /// The entry for `Calendar.save` (all day: dates; the end of a day is exclusive).
    func entry() -> Entry {
        let s = allDay ? CalendarModel.day(start) : CalendarModel.iso(start)
        var endDate = end
        if allDay, endDate <= start { endDate = start.addingTimeInterval(86400) }
        let e = allDay ? CalendarModel.day(endDate) : CalendarModel.iso(max(endDate, start))
        return Entry(
            uid: uid, kind: kind, title: title, description: description,
            start: kind == .task ? nil : s,
            end: kind == .task || (kind == .deadline && !allDay) ? nil : e,
            status: status,
            tags: tags.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty },
            checklist: checklist.filter { !$0.text.trimmingCharacters(in: .whitespaces).isEmpty },
            files: files, session: session, estimateMinutes: estimate.map { UInt32(max(0, $0)) }, reminders: reminders,
            recurrence: recurrence, calendar: calendar, readOnly: readOnly, pending: original?.pending ?? false)
    }
}
