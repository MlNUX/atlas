// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import SwiftUI

/// The month as six weeks (Linux: `CalendarMonth.qml`): up to three entries
/// a day and „+ n“; a click on a day shows its week, a double click adds an
/// all-day entry, tasks can be dropped on a day.
struct CalendarMonthView: View {
    let model: CalendarModel
    private static let shown = 3

    private var days: [Date] {
        let (start, _) = model.range
        return (0..<42).map { CalendarModel.cal.date(byAdding: .day, value: $0, to: start)! }
    }

    var body: some View {
        let month = CalendarModel.cal.component(.month, from: model.anchor)
        VStack(spacing: 0) {
            HStack(spacing: 0) {
                ForEach(["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"], id: \.self) { d in
                    Text(d).font(.caption.bold()).foregroundStyle(.secondary).frame(maxWidth: .infinity)
                }
            }
            .padding(.vertical, 4)
            Divider()
            Grid(horizontalSpacing: 0, verticalSpacing: 0) {
                ForEach(0..<6, id: \.self) { week in
                    GridRow {
                        ForEach(0..<7, id: \.self) { d in
                            let day = days[week * 7 + d]
                            dayCell(day, inMonth: CalendarModel.cal.component(.month, from: day) == month)
                        }
                    }
                }
            }
        }
    }

    private func dayCell(_ day: Date, inMonth: Bool) -> some View {
        let list = model.occurrences(on: day)
        let today = CalendarModel.cal.isDateInToday(day)
        return VStack(alignment: .leading, spacing: 2) {
            HStack {
                Text("\(CalendarModel.cal.component(.day, from: day))")
                    .font(.callout.weight(today ? .bold : .regular))
                    .foregroundStyle(today ? Color.white : inMonth ? .primary : .secondary)
                    .padding(.horizontal, 5)
                    .background(today ? Color.accentColor : .clear, in: .capsule)
                Spacer()
            }
            ForEach(list.prefix(Self.shown), id: \.instanceKey) { o in
                EntryChip(model: model, occurrence: o, showTime: false).frame(height: 18)
            }
            if list.count > Self.shown {
                Text("+ \(list.count - Self.shown)").font(.caption2).foregroundStyle(.secondary)
            }
            Spacer(minLength: 0)
        }
        .padding(3)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(inMonth ? Color.clear : Color.secondary.opacity(0.06))
        .overlay { Rectangle().stroke(Color.secondary.opacity(0.18), lineWidth: 0.5) }
        .contentShape(Rectangle())
        .onTapGesture(count: 2) { model.newEntry(kind: .event, start: day, allDay: true) }
        .onTapGesture {
            model.anchor = day
            model.mode = .week
        }
        .dropDestination(for: TaskDrag.self) { items, _ in
            for t in items { model.schedule(t.uid, at: CalendarModel.cal.date(bySettingHour: 9, minute: 0, second: 0, of: day)!) }
            return true
        }
    }
}

/// The next 30 days as a list by day (Linux: agenda).
struct CalendarAgendaView: View {
    let model: CalendarModel

    var body: some View {
        let (start, _) = model.range
        let days = (0..<30).map { CalendarModel.cal.date(byAdding: .day, value: $0, to: start)! }
            .filter { !model.occurrences(on: $0).isEmpty }
        List {
            if days.isEmpty {
                Text("In den nächsten 30 Tagen nichts geplant.").foregroundStyle(.secondary)
            }
            ForEach(days, id: \.self) { day in
                Section(day.formatted(.dateTime.weekday(.wide).day().month(.wide))) {
                    ForEach(model.occurrences(on: day), id: \.instanceKey) { o in
                        HStack(spacing: 10) {
                            Text(o.allDay ? "ganztägig" : CalendarModel.date(o.start)?.formatted(date: .omitted, time: .shortened) ?? "")
                                .font(.callout.monospacedDigit()).foregroundStyle(.secondary).frame(width: 70, alignment: .leading)
                            EntryChip(model: model, occurrence: o).frame(height: 22)
                        }
                    }
                }
            }
        }
    }
}
