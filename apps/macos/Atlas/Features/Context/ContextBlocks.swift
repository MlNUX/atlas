// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import PDFKit
import SwiftUI

/// The content of one block (Linux: `ContextBlock.qml`, `ContextModules.qml`).
/// `empty` tells the frame whether there is anything to show.
struct BlockContent: View {
    let model: ContextModel
    let id: String
    @Binding var empty: Bool

    private var project: ProjectModel { model.project }
    /// The file of the current tab (none for virtual tabs).
    private var path: String? {
        guard let tab = project.editor.currentTab, !tab.isVirtual else { return nil }
        return tab.path
    }

    var body: some View {
        Group {
            switch id {
            case "annotations": AnnotationsBlock(project: project, path: path, empty: $empty)
            case "outline": OutlineBlock(project: project, path: path, empty: $empty)
            case "links": LinksBlock(project: project, path: path, empty: $empty)
            case "tags": TagsBlock(project: project, path: path, empty: $empty)
            case "fileinfo": FileInfoBlock(project: project, path: path, empty: $empty)
            case "graph": ContextGraphBlock(project: project, path: path, empty: $empty)
            case "actions":
                VStack(alignment: .leading, spacing: 8) {
                    BuiltinActionsBlock(project: project, path: path, empty: $empty)
                    UserActionsBlock(project: project, path: path, empty: .constant(false))
                }
            case "builtin-actions": BuiltinActionsBlock(project: project, path: path, empty: $empty)
            case "user-actions": UserActionsBlock(project: project, path: path, empty: $empty)
            case "today": AgendaBlock(model: model, days: 1, empty: $empty)
            case "week": AgendaBlock(model: model, days: 7, empty: $empty)
            case "tasks": TasksBlock(model: model, empty: $empty)
            case "pomodoro": PomodoroBlock(pomodoro: model.pomodoro)
            default:
                if let card = model.card(id) {
                    CardBlock(project: project, card: card, path: path)
                } else {
                    Text("Unbekannter Baustein „\(id)“").font(.caption).foregroundStyle(.secondary)
                }
            }
        }
        .font(.callout)
    }
}

/// Grey hint for an empty block.
private struct Hint: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View { Text(text).font(.caption).foregroundStyle(.secondary) }
}

/// A row that opens something.
private struct Row: View {
    let symbol: String
    let title: String
    var detail = ""
    var tint: Color = .secondary
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Image(systemName: symbol).foregroundStyle(tint).frame(width: 14)
                VStack(alignment: .leading, spacing: 1) {
                    Text(title).lineLimit(2)
                    if !detail.isEmpty { Text(detail).font(.caption).foregroundStyle(.secondary).lineLimit(2) }
                }
                Spacer(minLength: 0)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

// MARK: - this file

private struct AnnotationsBlock: View {
    let project: ProjectModel
    let path: String?
    @Binding var empty: Bool
    @State private var list: [Annotation] = []

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if list.isEmpty {
                Hint(path?.lowercased().hasSuffix(".pdf") == true ? "Noch keine Anmerkungen." : "Nur für PDFs.")
            }
            ForEach(list.prefix(50), id: \.id) { a in
                Row(symbol: symbol(a.kind), title: label(a), detail: a.comment.isEmpty || a.kind == .note ? "S. \(a.page)" : "S. \(a.page) · \(a.comment)",
                    tint: Color(nsColor: NSColor(atlasHex: a.color))) {
                    if let path { project.editor.open(path: path, line: Int(a.page), mode: .tab) }
                }
            }
        }
        .task(id: "\(path ?? "")#\(project.annotationsRevision)") {
            list = path.flatMap { p in p.lowercased().hasSuffix(".pdf") ? (try? project.core.pdf(path: p))?.annotations() : nil }?
                .filter { $0.kind != .bookmark }.sorted { $0.page < $1.page } ?? []
            empty = list.isEmpty
        }
    }

    private func symbol(_ kind: AnnotationKind) -> String {
        switch kind {
        case .highlight: "highlighter"
        case .underline: "underline"
        case .strikeout: "strikethrough"
        case .note: "text.bubble"
        case .ink: "pencil.tip"
        case .bookmark: "bookmark"
        }
    }

    private func label(_ a: Annotation) -> String {
        if !a.text.isEmpty { return a.text }
        if !a.comment.isEmpty { return a.comment }
        return a.kind == .ink ? "Zeichnung" : "Anmerkung"
    }
}

private struct OutlineBlock: View {
    let project: ProjectModel
    let path: String?
    @Binding var empty: Bool
    @State private var entries: [(title: String, line: Int, level: Int)] = []

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            if entries.isEmpty { Hint("Keine Gliederung.") }
            ForEach(Array(entries.prefix(200).enumerated()), id: \.offset) { _, e in
                Button {
                    if let path { project.editor.open(path: path, line: e.line, mode: .tab) }
                } label: {
                    Text(e.title).lineLimit(1)
                        .padding(.leading, CGFloat(max(0, e.level - 1)) * 10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .task(id: "\(path ?? "")#\(project.filesRevision)#\(project.notesRevision)") { load() }
    }

    private func load() {
        defer { empty = entries.isEmpty }
        guard let path else { entries = []; return }
        if path.lowercased().hasSuffix(".pdf") {
            // PDF: its bookmarks (outline), the line is the page.
            var out: [(String, Int, Int)] = []
            if let doc = PDFDocument(url: URL(filePath: path)), let root = doc.outlineRoot {
                func walk(_ node: PDFOutline, _ level: Int) {
                    for i in 0..<node.numberOfChildren {
                        guard let child = node.child(at: i) else { continue }
                        let page = child.destination?.page.map { doc.index(for: $0) + 1 } ?? 1
                        out.append((child.label ?? "", page, level))
                        if out.count < 300 { walk(child, level + 1) }
                    }
                }
                walk(root, 1)
            }
            entries = out.map { (title: $0.0, line: $0.1, level: $0.2) }
        } else if let text = try? String(contentsOfFile: path, encoding: .utf8) {
            // Notes and code: headings / symbols from the core.
            entries = project.core.editor().symbols(path: path, text: text).map {
                (title: $0.name, line: Int($0.line), level: Int($0.level))
            }
        } else {
            entries = []
        }
    }
}

private struct LinksBlock: View {
    let project: ProjectModel
    let path: String?
    @Binding var empty: Bool
    @State private var outgoing: [GraphNode] = []
    @State private var incoming: [Backlink] = []

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if outgoing.isEmpty && incoming.isEmpty { Hint("Keine Verknüpfungen.") }
            if !outgoing.isEmpty {
                Text("Verweist auf").font(.caption.bold()).foregroundStyle(.secondary)
                ForEach(outgoing, id: \.path) { n in
                    Row(symbol: n.missing ? "questionmark.square.dashed" : "arrow.up.right", title: n.title,
                        tint: n.missing ? .orange : .accentColor) {
                        if !n.missing { project.editor.open(path: n.path, mode: .tab) }
                    }
                }
            }
            if !incoming.isEmpty {
                Text("Verlinkt von").font(.caption.bold()).foregroundStyle(.secondary)
                ForEach(Array(incoming.enumerated()), id: \.offset) { _, b in
                    Row(symbol: b.kind == .quote ? "quote.opening" : "arrow.down.left",
                        title: project.core.relativePath(path: b.source) ?? b.source, detail: b.context) {
                        project.editor.open(path: b.source, line: Int(b.line), mode: .tab)
                    }
                }
            }
        }
        .task(id: "\(path ?? "")#\(project.notesRevision)") {
            guard let path else { outgoing = []; incoming = []; empty = true; return }
            let notes = project.core.notes()
            incoming = notes.backlinks(path: path)
            let graph = notes.graph(path: path, depth: 1)
            let me = graph.nodes.firstIndex { $0.depth == 0 }.map(UInt32.init)
            let targets = Set(graph.edges.filter { $0.from == me }.map { Int($0.to) })
            outgoing = targets.sorted().compactMap { graph.nodes.indices.contains($0) ? graph.nodes[$0] : nil }
            empty = outgoing.isEmpty && incoming.isEmpty
        }
    }
}

private struct TagsBlock: View {
    let project: ProjectModel
    let path: String?
    @Binding var empty: Bool
    @State private var tags: [String] = []

    var body: some View {
        Group {
            if tags.isEmpty {
                Hint("Keine Tags.")
            } else {
                FlowLayout(spacing: 4) {
                    ForEach(tags, id: \.self) { tag in
                        Button("#\(tag)") {
                            project.searchPrefill = "#\(tag)"
                            project.searchVisible = true
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .help("Im Projekt suchen")
                    }
                }
            }
        }
        .task(id: "\(path ?? "")#\(project.notesRevision)") {
            tags = path.map { project.core.notes().tagsOf(path: $0) } ?? []
            empty = tags.isEmpty
        }
    }
}

private struct FileInfoBlock: View {
    let project: ProjectModel
    let path: String?
    @Binding var empty: Bool
    @State private var props: FileProperties?
    @State private var pages: Int?

    var body: some View {
        Group {
            if let p = props {
                Grid(alignment: .leading, horizontalSpacing: 8, verticalSpacing: 3) {
                    GridRow { Text("Pfad").foregroundStyle(.secondary); Text(project.core.relativePath(path: p.path) ?? p.path).textSelection(.enabled) }
                    if let size = p.size {
                        GridRow { Text("Größe").foregroundStyle(.secondary); Text(ByteCountFormatter.string(fromByteCount: Int64(size), countStyle: .file)) }
                    }
                    if let pages { GridRow { Text("Seiten").foregroundStyle(.secondary); Text("\(pages)") } }
                    GridRow {
                        Text("Geändert").foregroundStyle(.secondary)
                        Text(Date(timeIntervalSince1970: TimeInterval(p.modified)).formatted(date: .abbreviated, time: .shortened))
                    }
                }
                .font(.caption)
            } else {
                Hint("Keine Datei.")
            }
        }
        .task(id: "\(path ?? "")#\(project.filesRevision)") {
            props = path.flatMap { try? project.core.files().properties(path: $0) }
            pages = path.flatMap { $0.lowercased().hasSuffix(".pdf") ? PDFDocument(url: URL(filePath: $0))?.pageCount : nil }
            empty = props == nil
        }
    }
}

private struct BuiltinActionsBlock: View {
    let project: ProjectModel
    let path: String?
    @Binding var empty: Bool

    var body: some View {
        let _ = project.actions.revision
        VStack(alignment: .leading, spacing: 3) {
            if let path {
                Row(symbol: "folder", title: "Im Finder zeigen") {
                    NSWorkspace.shared.activateFileViewerSelecting([URL(filePath: path)])
                }
                Row(symbol: "doc.on.doc", title: "Pfad kopieren") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(path, forType: .string)
                    project.show(notice: "Pfad kopiert")
                }
                if project.core.actions().isRunnable(path: path) {
                    Row(symbol: "play", title: "Skript ausführen") { project.actions.runScript(path) }
                }
                ForEach(project.actions.processors(for: path), id: \.id) { p in
                    Row(symbol: "eye", title: "Anzeigen mit \(p.name)") { project.actions.show(path, with: p.id) }
                }
                ForEach(project.actions.exporters(for: path), id: \.id) { x in
                    Row(symbol: "square.and.arrow.up", title: "\(x.name) → .\(x.target)") {
                        Task { await project.actions.export(path, with: x) }
                    }
                }
            } else {
                Hint("Keine Datei.")
            }
        }
        .onAppear { empty = path == nil }
        .onChange(of: path) { empty = path == nil }
    }
}

private struct UserActionsBlock: View {
    let project: ProjectModel
    let path: String?
    @Binding var empty: Bool

    var body: some View {
        let _ = project.actions.revision
        let list = project.actions.actions(for: path.map { [$0] } ?? []).filter(\.inMenu)
        VStack(alignment: .leading, spacing: 3) {
            if list.isEmpty { Hint("Keine eigenen Aktionen – „Aktionen → Aktionen des Projekts bearbeiten“.") }
            ForEach(list, id: \.id) { a in
                Row(symbol: "bolt", title: a.name, detail: a.description) {
                    project.actions.run(id: a.id, paths: path.map { [$0] } ?? [], line: project.editor.currentTab?.line ?? 0)
                }
            }
        }
        .onAppear { empty = list.isEmpty }
        .onChange(of: list.count) { empty = list.isEmpty }
    }
}

private struct CardBlock: View {
    let project: ProjectModel
    let card: ExtensionCard
    let path: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            if !card.description.isEmpty { Hint(card.description) }
            ForEach(card.entries, id: \.id) { e in
                Row(symbol: e.kind == .view ? "rectangle.on.rectangle" : "bolt", title: e.name,
                    detail: e.keybinding) {
                    switch e.kind {
                    case .view: project.actions.openView(e.id)
                    case .command: project.actions.run(id: e.id, paths: path.map { [$0] } ?? [])
                    }
                }
            }
        }
    }
}

// MARK: - planning

/// „Heute“ (one day) and „Diese Woche“ (seven days: deadlines and blocks).
private struct AgendaBlock: View {
    let model: ContextModel
    let days: Int
    @Binding var empty: Bool
    @State private var list: [Occurrence] = []
    private static let limit = 6

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            if list.isEmpty {
                Hint(days == 1 ? "Heute keine Termine." : "In den nächsten sieben Tagen nichts geplant.")
            }
            ForEach(Array(list.prefix(Self.limit * days).enumerated()), id: \.offset) { _, o in
                Row(symbol: symbol(o.kind), title: o.title, detail: when(o),
                    tint: o.color.isEmpty ? .accentColor : Color(nsColor: NSColor(atlasHex: o.color))) {
                    model.project.editor.open(path: "calendar:", mode: .tab)
                }
                .opacity(o.status == .done ? 0.5 : 1)
            }
            if list.count > Self.limit * days {
                Hint("und \(list.count - Self.limit * days) weitere")
            }
        }
        .task(id: "\(days)#\(model.calendarRevision)") { load() }
    }

    private func load() {
        let calendar = Calendar.current
        let start = calendar.startOfDay(for: .now)
        let end = calendar.date(byAdding: .day, value: days, to: start) ?? start
        let all = (try? model.project.core.calendar().occurrences(from: Self.iso(start), to: Self.iso(end))) ?? []
        list = days == 1 ? all : all.filter { $0.kind == .deadline || $0.kind == .block }
        empty = list.isEmpty
    }

    static func iso(_ date: Date) -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd'T'HH:mm"
        return f.string(from: date)
    }

    private func when(_ o: Occurrence) -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = o.allDay ? "yyyy-MM-dd" : "yyyy-MM-dd'T'HH:mm"
        guard let date = f.date(from: o.start) ?? f.date(from: String(o.start.prefix(16))) else { return o.start }
        if o.allDay { return days == 1 ? "ganztägig" : date.formatted(.dateTime.weekday(.abbreviated).day().month()) }
        return days == 1 ? date.formatted(date: .omitted, time: .shortened)
            : date.formatted(.dateTime.weekday(.abbreviated).day().hour().minute())
    }

    private func symbol(_ kind: EntryKind) -> String {
        switch kind {
        case .event: "calendar"
        case .block: "rectangle.fill"
        case .deadline: "flag"
        case .task: "checkmark.square"
        }
    }
}

private struct TasksBlock: View {
    let model: ContextModel
    @Binding var empty: Bool
    @State private var tasks: [Entry] = []
    @State private var newTitle = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if tasks.isEmpty { Hint("Keine offenen Aufgaben.") }
            ForEach(tasks.prefix(6), id: \.uid) { t in
                HStack(spacing: 6) {
                    Button {
                        try? model.project.core.calendar().setStatus(uid: t.uid, status: .done)
                        load()
                    } label: {
                        Image(systemName: "square")
                    }
                    .buttonStyle(.plain)
                    .help("Erledigt")
                    Text(t.title).lineLimit(2)
                }
            }
            if tasks.count > 6 { Hint("und \(tasks.count - 6) weitere im Kalender") }
            TextField("Neue Aufgabe (Enter)", text: $newTitle)
                .textFieldStyle(.roundedBorder)
                .controlSize(.small)
                .onSubmit { add() }
        }
        .task(id: model.calendarRevision) { load() }
    }

    private func load() {
        tasks = model.project.core.calendar().tasks().filter { $0.status != .done }
        // A new task field is always there: the block is never empty.
        empty = false
    }

    private func add() {
        let title = newTitle.trimmingCharacters(in: .whitespaces)
        guard !title.isEmpty else { return }
        let entry = Entry(
            uid: "", kind: .task, title: title, description: "", start: nil, end: nil, status: .planned,
            tags: [], checklist: [], files: [], session: "", estimateMinutes: nil, reminders: [], recurrence: nil,
            calendar: "", readOnly: false, pending: false)
        do {
            _ = try model.project.core.calendar().save(entry: entry)
            newTitle = ""
            load()
        } catch {
            model.error = atlasMessage(error)
        }
    }
}

private struct PomodoroBlock: View {
    let pomodoro: Pomodoro

    var body: some View {
        HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                Text(pomodoro.text).font(.system(.title2, design: .monospaced).weight(.semibold))
                Text("\(pomodoro.phase.rawValue) · Runde \(pomodoro.rounds % 4 + 1) von 4")
                    .font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
            Button { pomodoro.toggle() } label: {
                Image(systemName: pomodoro.running ? "pause.fill" : "play.fill")
            }
            .help(pomodoro.running ? "Pause" : "Start")
            Button { pomodoro.skip() } label: { Image(systemName: "forward.end.fill") }
                .help("Nächster Abschnitt")
            Button { pomodoro.reset() } label: { Image(systemName: "arrow.counterclockwise") }
                .help("Zurücksetzen")
        }
        .buttonStyle(.borderless)
    }
}

/// Lays children out in rows, wrapping (tags).
struct FlowLayout: Layout {
    var spacing: CGFloat = 4

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        var x: CGFloat = 0, y: CGFloat = 0, row: CGFloat = 0, widest: CGFloat = 0
        for s in subviews {
            let size = s.sizeThatFits(.unspecified)
            if x > 0, x + size.width > width {
                y += row + spacing
                x = 0
                row = 0
            }
            x += size.width + spacing
            row = max(row, size.height)
            widest = max(widest, x - spacing)
        }
        return CGSize(width: min(widest, width), height: y + row)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX, y = bounds.minY, row: CGFloat = 0
        for s in subviews {
            let size = s.sizeThatFits(.unspecified)
            if x > bounds.minX, x + size.width > bounds.maxX {
                y += row + spacing
                x = bounds.minX
                row = 0
            }
            s.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            row = max(row, size.height)
        }
    }
}
