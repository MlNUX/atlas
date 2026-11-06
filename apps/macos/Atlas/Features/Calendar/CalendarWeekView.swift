// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import SwiftUI

/// The week (Linux: `CalendarWeek.qml`): all-day row, hours, the entries
/// placed by time. Drag an entry to move it (15-minute grid), double click
/// an empty slot for a new appointment, drop a task to plan it.
struct CalendarWeekView: View {
    let model: CalendarModel
    static let hourHeight: CGFloat = 44
    static let gutter: CGFloat = 44

    private var days: [Date] {
        let (start, _) = model.range
        return (0..<7).map { CalendarModel.cal.date(byAdding: .day, value: $0, to: start)! }
    }

    var body: some View {
        GeometryReader { geo in
            let column = max(40, (geo.size.width - Self.gutter) / 7)
            VStack(spacing: 0) {
                header(column: column)
                Divider()
                allDayRow(column: column)
                Divider()
                ScrollViewReader { proxy in
                    ScrollView(.vertical) {
                        ZStack(alignment: .topLeading) {
                            // Invisible hour markers to scroll to.
                            VStack(spacing: 0) {
                                ForEach(0..<24, id: \.self) { h in Color.clear.frame(height: Self.hourHeight).id(h) }
                            }
                            grid(column: column)
                            ForEach(Array(days.enumerated()), id: \.offset) { i, day in
                                ForEach(timed(on: day), id: \.instanceKey) { o in
                                    TimedEntry(model: model, occurrence: o, day: day, column: column)
                                        .offset(x: Self.gutter + CGFloat(i) * column + 2)
                                }
                            }
                            nowLine(column: column)
                        }
                        .frame(height: 24 * Self.hourHeight)
                    }
                    .onAppear {
                        // The working day in view (from 7 o'clock).
                        proxy.scrollTo(7, anchor: .top)
                    }
                }
            }
        }
    }

    private func timed(on day: Date) -> [Occurrence] {
        model.occurrences(on: day).filter { !$0.allDay }
    }

    private func header(column: CGFloat) -> some View {
        HStack(spacing: 0) {
            Color.clear.frame(width: Self.gutter, height: 1)
            ForEach(days, id: \.self) { day in
                let today = CalendarModel.cal.isDateInToday(day)
                Text(day.formatted(.dateTime.weekday(.abbreviated).day()))
                    .font(.callout.weight(today ? .bold : .regular))
                    .foregroundStyle(today ? Color.accentColor : .primary)
                    .frame(width: column)
                    .padding(.vertical, 4)
            }
        }
    }

    private func allDayRow(column: CGFloat) -> some View {
        HStack(alignment: .top, spacing: 0) {
            Text("ganztägig").font(.caption2).foregroundStyle(.secondary).frame(width: Self.gutter)
            ForEach(days, id: \.self) { day in
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(model.occurrences(on: day).filter(\.allDay), id: \.instanceKey) { o in
                        EntryChip(model: model, occurrence: o)
                    }
                }
                .frame(width: column - 4, alignment: .topLeading)
                .frame(minHeight: 22)
                .padding(2)
                .contentShape(Rectangle())
                .onTapGesture(count: 2) { model.newEntry(kind: .event, start: day, allDay: true) }
                .dropDestination(for: TaskDrag.self) { items, _ in
                    for t in items { model.schedule(t.uid, at: CalendarModel.cal.date(bySettingHour: 9, minute: 0, second: 0, of: day)!) }
                    return true
                }
            }
        }
    }

    private func grid(column: CGFloat) -> some View {
        ZStack(alignment: .topLeading) {
            ForEach(0..<24, id: \.self) { h in
                HStack(spacing: 0) {
                    Text(String(format: "%02d:00", h)).font(.caption2.monospacedDigit()).foregroundStyle(.secondary)
                        .frame(width: Self.gutter - 4, alignment: .trailing)
                        .offset(y: -6)
                    Rectangle().fill(Color.secondary.opacity(0.2)).frame(height: 0.5)
                }
                .offset(y: CGFloat(h) * Self.hourHeight)
            }
            HStack(spacing: 0) {
                Color.clear.frame(width: Self.gutter)
                ForEach(days, id: \.self) { day in
                    Rectangle()
                        .fill(CalendarModel.cal.isDateInToday(day) ? Color.accentColor.opacity(0.05) : Color.clear)
                        .overlay(alignment: .leading) { Rectangle().fill(Color.secondary.opacity(0.2)).frame(width: 0.5) }
                        .frame(width: column)
                        .contentShape(Rectangle())
                        .onTapGesture(count: 2, coordinateSpace: .local) { p in
                            model.newEntry(kind: .event, start: Self.time(on: day, y: p.y))
                        }
                        .dropDestination(for: TaskDrag.self) { items, p in
                            for t in items { model.schedule(t.uid, at: Self.time(on: day, y: p.y)) }
                            return true
                        }
                }
            }
            .frame(height: 24 * Self.hourHeight)
        }
    }

    @ViewBuilder
    private func nowLine(column: CGFloat) -> some View {
        if let i = days.firstIndex(where: { CalendarModel.cal.isDateInToday($0) }) {
            let now = Date()
            let minutes = now.timeIntervalSince(CalendarModel.cal.startOfDay(for: now)) / 60
            Rectangle().fill(Color.red).frame(width: column, height: 1.5)
                .offset(x: Self.gutter + CGFloat(i) * column, y: CGFloat(minutes / 60) * Self.hourHeight)
        }
    }

    /// The time at `y` on `day`, on a 15-minute grid.
    static func time(on day: Date, y: CGFloat) -> Date {
        let minutes = max(0, min(24 * 60 - 15, Double(y / hourHeight) * 60))
        return CalendarModel.snap(CalendarModel.cal.startOfDay(for: day).addingTimeInterval(minutes * 60))
    }
}

/// An appointment, block or deadline at its time; drag to move it.
private struct TimedEntry: View {
    let model: CalendarModel
    let occurrence: Occurrence
    let day: Date
    let column: CGFloat
    @State private var drag: CGSize = .zero

    var body: some View {
        let start = CalendarModel.date(occurrence.start) ?? day
        let end = CalendarModel.date(occurrence.end) ?? start
        let dayStart = CalendarModel.cal.startOfDay(for: day)
        let from = max(0, start.timeIntervalSince(dayStart) / 3600)
        let to = min(24, max(from + 0.4, end.timeIntervalSince(dayStart) / 3600))
        let height = max(18, CGFloat(to - from) * CalendarWeekView.hourHeight - 2)
        EntryChip(model: model, occurrence: occurrence, showTime: true)
            .frame(width: column - 4, height: height, alignment: .topLeading)
            .offset(x: drag.width, y: CGFloat(from) * CalendarWeekView.hourHeight + drag.height)
            .gesture(occurrence.readOnly ? nil : DragGesture(minimumDistance: 4)
                .onChanged { drag = $0.translation }
                .onEnded { value in
                    let days = (value.translation.width / column).rounded()
                    let minutes = Double(value.translation.height / CalendarWeekView.hourHeight) * 60
                    let target = CalendarModel.cal.date(byAdding: .day, value: Int(days), to: start)!.addingTimeInterval(minutes * 60)
                    drag = .zero
                    if abs(value.translation.height) > 4 || days != 0 { model.move(occurrence, to: target) }
                })
    }
}

/// One entry as a coloured chip (double click edits).
struct EntryChip: View {
    let model: CalendarModel
    let occurrence: Occurrence
    var showTime = false

    var body: some View {
        let color = Color(nsColor: NSColor(atlasHex: occurrence.color.isEmpty ? "#5ec8f2" : occurrence.color))
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 3) {
                Image(systemName: symbol).font(.caption2)
                Text(occurrence.title.isEmpty ? "Ohne Titel" : occurrence.title).font(.caption.weight(.medium)).lineLimit(2)
                    .strikethrough(occurrence.status == .done)
            }
            if showTime, !occurrence.allDay, let s = CalendarModel.date(occurrence.start) {
                Text(s.formatted(date: .omitted, time: .shortened)).font(.caption2).opacity(0.8)
            }
        }
        .padding(.horizontal, 4).padding(.vertical, 2)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(color.opacity(occurrence.kind == .block ? 0.22 : 0.35), in: .rect(cornerRadius: 4))
        .overlay(alignment: .leading) { Rectangle().fill(color).frame(width: 3) }
        .clipShape(.rect(cornerRadius: 4))
        .opacity(occurrence.status == .done ? 0.55 : 1)
        .contentShape(Rectangle())
        .onTapGesture(count: 2) { model.edit(uid: occurrence.uid) }
        .help(occurrence.title)
        .contextMenu {
            Button("Bearbeiten …") { model.edit(uid: occurrence.uid) }
            if occurrence.kind != .event {
                Button(occurrence.status == .done ? "Nicht erledigt" : "Erledigt") {
                    model.setDone(occurrence.uid, occurrence.status != .done)
                }
            }
            if occurrence.kind == .block {
                Button("Zurück zu den Aufgaben") { model.unschedule(occurrence.uid) }
            }
            ForEach(occurrence.files, id: \.self) { f in
                Button("Öffnen: \((f as NSString).lastPathComponent)") {
                    model.project.editor.open(path: model.project.core.absolutePath(relative: f), mode: .tab)
                }
            }
            Divider()
            Button("Löschen", role: .destructive) { model.remove(uid: occurrence.uid) }
                .disabled(occurrence.readOnly)
        }
    }

    private var symbol: String {
        switch occurrence.kind {
        case .event: occurrence.recurring ? "repeat" : "calendar"
        case .block: "rectangle.fill"
        case .deadline: "flag.fill"
        case .task: "checkmark.square"
        }
    }
}

extension Occurrence {
    /// Unique per occurrence (series repeat their uid).
    var instanceKey: String { uid + "#" + instance }
}
