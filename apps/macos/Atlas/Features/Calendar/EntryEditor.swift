// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import EventKit
import SwiftUI

/// New or changed entry (Linux: `EntryDialog.qml`): kind, title, times,
/// repeat, reminders, checklist, tags, calendar, linked files.
struct EntryEditor: View {
    let model: CalendarModel
    @State var draft: EntryDraft
    @Environment(\.dismiss) private var dismiss
    @State private var newItem = ""

    private static let reminderChoices: [(Int64, String)] = [
        (0, "zu Beginn"), (5, "5 Minuten vorher"), (10, "10 Minuten vorher"), (15, "15 Minuten vorher"),
        (30, "30 Minuten vorher"), (60, "1 Stunde vorher"), (120, "2 Stunden vorher"), (1440, "1 Tag vorher"),
    ]

    var body: some View {
        VStack(spacing: 0) {
            Form {
                Section {
                    Picker("Art", selection: $draft.kind) {
                        Text("Termin").tag(EntryKind.event)
                        Text("Arbeitsblock").tag(EntryKind.block)
                        Text("Frist").tag(EntryKind.deadline)
                        Text("Aufgabe").tag(EntryKind.task)
                    }
                    .pickerStyle(.segmented)
                    TextField("Titel", text: $draft.title)
                }
                if draft.kind != .task {
                    Section("Zeit") {
                        Toggle("Ganztägig", isOn: $draft.allDay)
                        DatePicker(draft.kind == .deadline ? "Fällig" : "Beginn", selection: $draft.start,
                                   displayedComponents: draft.allDay ? [.date] : [.date, .hourAndMinute])
                            .onChange(of: draft.start) { old, new in
                                // The end moves with the start (same length).
                                draft.end = draft.end.addingTimeInterval(new.timeIntervalSince(old))
                            }
                        if draft.kind != .deadline || draft.allDay {
                            DatePicker("Ende", selection: $draft.end, in: draft.start...,
                                       displayedComponents: draft.allDay ? [.date] : [.date, .hourAndMinute])
                        }
                        recurrence
                    }
                } else {
                    Section {
                        TextField("Geschätzte Minuten", value: $draft.estimate, format: .number)
                    }
                }
                Section("Status") {
                    Picker("Status", selection: $draft.status) {
                        Text("Geplant").tag(EntryStatus.planned)
                        Text("Begonnen").tag(EntryStatus.started)
                        Text("Erledigt").tag(EntryStatus.done)
                        Text("Verschoben").tag(EntryStatus.postponed)
                    }
                    .labelsHidden()
                }
                if draft.kind != .task {
                    Section("Erinnerungen") {
                        ForEach(Array(draft.reminders.enumerated()), id: \.offset) { i, m in
                            HStack {
                                Text(Self.reminderChoices.first { $0.0 == m }?.1 ?? "\(m) Minuten vorher")
                                Spacer()
                                Button { draft.reminders.remove(at: i) } label: { Image(systemName: "minus.circle") }
                                    .buttonStyle(.borderless)
                            }
                        }
                        Menu("Erinnerung hinzufügen") {
                            ForEach(Self.reminderChoices.filter { c in !draft.reminders.contains(c.0) }, id: \.0) { c in
                                Button(c.1) { draft.reminders.append(c.0) }
                            }
                        }
                    }
                }
                if !draft.calendar.hasPrefix(EventKitSource.prefix) {
                Section("Checkliste") {
                    ForEach(Array(draft.checklist.enumerated()), id: \.offset) { i, item in
                        HStack {
                            Toggle(isOn: Binding(get: { item.done }, set: { draft.checklist[i].done = $0 })) {
                                TextField("Punkt", text: Binding(get: { item.text }, set: { draft.checklist[i].text = $0 }))
                            }
                            .toggleStyle(.checkbox)
                            Button { draft.checklist.remove(at: i) } label: { Image(systemName: "minus.circle") }
                                .buttonStyle(.borderless)
                        }
                    }
                    TextField("Neuer Punkt (Enter)", text: $newItem)
                        .onSubmit {
                            guard !newItem.trimmingCharacters(in: .whitespaces).isEmpty else { return }
                            draft.checklist.append(ChecklistItem(done: false, text: newItem))
                            newItem = ""
                        }
                }
                }
                Section("Mehr") {
                    if !draft.calendar.hasPrefix(EventKitSource.prefix) {
                        TextField("Tags (mit Komma getrennt)", text: $draft.tags)
                    }
                    TextField("Beschreibung", text: $draft.description, axis: .vertical).lineLimit(3...8)
                    let writable = model.calendars.filter(\.writable)
                    let mac = EventKitSource.shared.active ? EventKitSource.shared.writableEventCalendars : []
                    if writable.count + mac.count > 1 {
                        Picker("Kalender", selection: $draft.calendar) {
                            ForEach(writable, id: \.id) { Text($0.name).tag($0.id) }
                            if !mac.isEmpty {
                                Divider()
                                // Target in the macOS calendars (P3.13).
                                ForEach(mac, id: \.calendarIdentifier) { c in
                                    Text("\(c.title) (macOS)").tag(EventKitSource.prefix + c.calendarIdentifier)
                                }
                            }
                        }
                        .disabled(!draft.uid.isEmpty)
                    }
                    ForEach(draft.files, id: \.self) { f in
                        HStack {
                            Image(systemName: "doc")
                            Text(f).lineLimit(1)
                            Spacer()
                            Button("Öffnen") {
                                model.project.editor.open(path: model.project.core.absolutePath(relative: f), mode: .tab)
                                dismiss()
                            }
                            .buttonStyle(.borderless)
                        }
                    }
                }
            }
            .formStyle(.grouped)
            .disabled(draft.readOnly)
            Divider()
            HStack {
                if !draft.uid.isEmpty, !draft.readOnly {
                    Button("Löschen", role: .destructive) { model.remove(uid: draft.uid) }
                }
                if draft.readOnly { Text("Schreibgeschützter Kalender").font(.caption).foregroundStyle(.secondary) }
                Spacer()
                Button("Abbrechen") { dismiss() }.keyboardShortcut(.cancelAction)
                Button("Sichern") { model.save(draft) }
                    .keyboardShortcut(.defaultAction)
                    .disabled(draft.readOnly || draft.title.trimmingCharacters(in: .whitespaces).isEmpty)
            }
            .padding(12)
        }
        .frame(width: 480, height: 620)
    }

    @ViewBuilder
    private var recurrence: some View {
        let supported = draft.recurrence?.supported ?? true
        Picker("Wiederholen", selection: Binding(
            get: { draft.recurrence?.frequency },
            set: { f in
                if let f {
                    draft.recurrence = Recurrence(frequency: f, interval: draft.recurrence?.interval ?? 1,
                                                  count: draft.recurrence?.count, until: draft.recurrence?.until,
                                                  text: "", supported: true)
                } else {
                    draft.recurrence = nil
                }
            })) {
            Text("Nie").tag(Frequency?.none)
            Text("Täglich").tag(Frequency?.some(.daily))
            Text("Wöchentlich").tag(Frequency?.some(.weekly))
            Text("Monatlich").tag(Frequency?.some(.monthly))
            Text("Jährlich").tag(Frequency?.some(.yearly))
        }
        .disabled(!supported)
        if let r = draft.recurrence {
            Stepper("Alle \(r.interval) \(unit(r.frequency, r.interval))", value: Binding(
                get: { Int(r.interval) }, set: { draft.recurrence?.interval = UInt32(max(1, $0)) }), in: 1...99)
                .disabled(!supported)
            Picker("Endet", selection: Binding(
                get: { r.count != nil ? 1 : r.until != nil ? 2 : 0 },
                set: { choice in
                    draft.recurrence?.count = choice == 1 ? (r.count ?? 10) : nil
                    draft.recurrence?.until = choice == 2 ? (r.until ?? CalendarModel.day(draft.start.addingTimeInterval(86400 * 90))) : nil
                })) {
                Text("Nie").tag(0)
                Text("Nach Anzahl").tag(1)
                Text("Am Datum").tag(2)
            }
            .disabled(!supported)
            if let count = r.count {
                Stepper("\(count) Mal", value: Binding(get: { Int(count) }, set: { draft.recurrence?.count = UInt32(max(1, $0)) }),
                        in: 1...999)
            }
            if let until = r.until, let date = CalendarModel.date(until) {
                DatePicker("Bis", selection: Binding(get: { date }, set: { draft.recurrence?.until = CalendarModel.day($0) }),
                           displayedComponents: [.date])
            }
            if !supported {
                Text("\(r.text) – diese Regel bearbeitet Atlas nicht; sie bleibt, wie sie ist.").font(.caption).foregroundStyle(.secondary)
            }
        }
    }

    private func unit(_ f: Frequency, _ n: UInt32) -> String {
        switch f {
        case .daily: n == 1 ? "Tag" : "Tage"
        case .weekly: n == 1 ? "Woche" : "Wochen"
        case .monthly: n == 1 ? "Monat" : "Monate"
        case .yearly: n == 1 ? "Jahr" : "Jahre"
        }
    }
}
