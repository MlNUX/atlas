// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import SwiftUI

/// The `calendar:` tab (P3.7; Linux: `CalendarView.qml`): week, month or
/// agenda, tasks without a time at the side, calendars, the entry dialog.
struct CalendarTabView: View {
    let project: ProjectModel
    @State private var model: CalendarModel?

    var body: some View {
        Group {
            if let model {
                CalendarContent(model: model)
            } else {
                ProgressView()
            }
        }
        .onAppear { if model == nil { model = CalendarModel(project: project) } }
        .onDisappear { model?.stop() }
    }
}

private struct CalendarContent: View {
    @Bindable var model: CalendarModel
    @State private var calendarsShown = false

    var body: some View {
        VStack(spacing: 0) {
            toolbar
            if let error = model.error {
                Text(error).font(.callout).foregroundStyle(.red).padding(6)
            }
            Divider()
            HStack(spacing: 0) {
                Group {
                    switch model.mode {
                    case .week: CalendarWeekView(model: model)
                    case .month: CalendarMonthView(model: model)
                    case .agenda: CalendarAgendaView(model: model)
                    }
                }
                .frame(minWidth: 0, maxWidth: .infinity, maxHeight: .infinity)
                if model.showTasks {
                    Divider()
                    TasksPanel(model: model).frame(width: 230)
                }
            }
        }
        .sheet(item: $model.editing) { draft in
            EntryEditor(model: model, draft: draft)
        }
    }

    private var toolbar: some View {
        HStack(spacing: 8) {
            Button("Zurück", systemImage: "chevron.left") { model.page(-1) }
                .labelStyle(.iconOnly)
            Button("Heute") { model.today() }
                .keyboardShortcut("t", modifiers: [.command, .option])
            Button("Weiter", systemImage: "chevron.right") { model.page(1) }
                .labelStyle(.iconOnly)
            Text(model.title).font(.headline).lineLimit(1)
            Spacer()
            Picker("Ansicht", selection: $model.mode) {
                ForEach(CalendarModel.Mode.allCases, id: \.self) { Text($0.title).tag($0) }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .fixedSize()
            Menu {
                Button("Termin") { model.newEntry(kind: .event) }
                Button("Arbeitsblock") { model.newEntry(kind: .block) }
                Button("Frist") { model.newEntry(kind: .deadline) }
                Button("Aufgabe") { model.newEntry(kind: .task) }
            } label: {
                Label("Neu", systemImage: "plus")
            }
            .fixedSize()
            Button("Kalender", systemImage: "calendar.badge.checkmark") { calendarsShown.toggle() }
                .labelStyle(.iconOnly)
                .help("Kalender ein- und ausblenden")
                .popover(isPresented: $calendarsShown) { CalendarsPopover(model: model) }
            Toggle(isOn: $model.showTasks) { Image(systemName: "checklist") }
                .toggleStyle(.button)
                .help("Aufgaben ohne Zeit")
        }
        .buttonStyle(.borderless)
        .padding(.horizontal, 10).padding(.vertical, 6)
    }
}

/// Calendars: show or hide, the one for new entries.
private struct CalendarsPopover: View {
    let model: CalendarModel

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Kalender").font(.headline)
            ForEach(model.calendars, id: \.id) { c in
                HStack {
                    Toggle(isOn: Binding(get: { c.visible }, set: { model.setVisible(c.id, $0) })) {
                        HStack(spacing: 6) {
                            Circle().fill(Color(nsColor: NSColor(atlasHex: c.color.isEmpty ? "#5ec8f2" : c.color)))
                                .frame(width: 10, height: 10)
                            Text(c.name)
                            if c.missing { Text("(fehlt auf dem Server)").font(.caption).foregroundStyle(.orange) }
                        }
                    }
                    .toggleStyle(.checkbox)
                    Spacer()
                    if c.isDefault {
                        Text("für Neues").font(.caption).foregroundStyle(.secondary)
                    } else if c.writable {
                        Button("Für Neues") { model.setDefault(c.id) }.buttonStyle(.borderless).font(.caption)
                    }
                }
            }
            Text("Nextcloud oder Google verbinden: folgt mit dem zweiten Teil von P3.7.")
                .font(.caption).foregroundStyle(.secondary)
        }
        .padding(12)
        .frame(width: 320)
    }
}

/// Tasks without a time: tick off, plan, new.
private struct TasksPanel: View {
    let model: CalendarModel
    @State private var newTitle = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Aufgaben").font(.headline).padding([.horizontal, .top], 10)
            TextField("Neue Aufgabe (Enter)", text: $newTitle)
                .textFieldStyle(.roundedBorder)
                .padding(.horizontal, 10)
                .onSubmit {
                    model.addTask(newTitle)
                    newTitle = ""
                }
            List(model.tasks, id: \.uid) { t in
                HStack(alignment: .top, spacing: 6) {
                    Button { model.setDone(t.uid, true) } label: { Image(systemName: "square") }
                        .buttonStyle(.plain)
                        .help("Erledigt")
                    VStack(alignment: .leading, spacing: 1) {
                        Text(t.title).lineLimit(2)
                        if !t.checklist.isEmpty {
                            Text("\(t.checklist.filter(\.done).count)/\(t.checklist.count)").font(.caption).foregroundStyle(.secondary)
                        }
                    }
                    Spacer()
                }
                .contentShape(Rectangle())
                .onTapGesture(count: 2) { model.edit(uid: t.uid) }
                .contextMenu {
                    Button("Bearbeiten …") { model.edit(uid: t.uid) }
                    Button("Nächste volle Stunde einplanen") { model.schedule(t.uid, at: CalendarModel.nextFullHour()) }
                    Button("Morgen 9 Uhr einplanen") {
                        let c = CalendarModel.cal
                        let tomorrow = c.date(byAdding: .day, value: 1, to: c.startOfDay(for: Date()))!
                        model.schedule(t.uid, at: c.date(bySettingHour: 9, minute: 0, second: 0, of: tomorrow)!)
                    }
                    Divider()
                    Button("Löschen", role: .destructive) { model.remove(uid: t.uid) }
                }
                .draggable(TaskDrag(uid: t.uid))
            }
            .listStyle(.plain)
            if model.tasks.isEmpty {
                Text("Keine offenen Aufgaben. In die Woche ziehen plant eine Aufgabe ein.")
                    .font(.caption).foregroundStyle(.secondary).padding(10)
            }
        }
    }
}

/// A task dragged onto the week (it becomes a work block there).
struct TaskDrag: Codable, Transferable {
    let uid: String
    static var transferRepresentation: some TransferRepresentation {
        CodableRepresentation(contentType: .json)
    }
}
