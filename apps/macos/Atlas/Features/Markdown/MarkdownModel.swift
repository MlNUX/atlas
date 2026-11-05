// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Observation
import SwiftUI

/// One Markdown file in a tab (P3.3; Linux: `MarkdownView.qml`): the
/// editor, the preview, or both side by side with scrolling in step.
/// Saved automatically a second after the last change (Linux: the same).
@MainActor
@Observable
final class MarkdownModel {
    enum Mode: String, CaseIterable {
        case edit, split, preview

        var title: String {
            switch self {
            case .edit: "Bearbeiten"
            case .split: "Geteilt"
            case .preview: "Vorschau"
            }
        }
    }

    @ObservationIgnored let project: ProjectModel
    let path: String
    var mode: Mode {
        didSet {
            if mode != .edit { ensurePreview() }
            if mode == .split { followEditor() }
        }
    }
    private(set) var loaded = false
    private(set) var error: String?
    private(set) var modified = false
    /// Changed on disk while there are unsaved changes here.
    private(set) var conflict = false
    /// Title, headings, tags, broken links of the last render.
    private(set) var info: RenderedMarkdown?
    @ObservationIgnored let editor: TextEditorController
    @ObservationIgnored private(set) var preview: MarkdownPreview?
    @ObservationIgnored private var lastSaved = ""
    @ObservationIgnored private var saveTask: Task<Void, Never>?
    @ObservationIgnored private var renderTask: Task<Void, Never>?
    @ObservationIgnored private var listener: UUID?
    /// The editor moved because the preview scrolled (no echo back).
    @ObservationIgnored private var followingPreview = Date.distantPast
    @ObservationIgnored private var startLine: Int
    /// A hit of the project search to mark (Linux: `find` in the tab state).
    @ObservationIgnored private var find: (text: String, caseSensitive: Bool, line: Int)?
    private(set) var exporting = false

    /// Mermaid templates of „Diagramm einfügen“ (as on Linux).
    static let diagrams: [(name: String, code: String)] = [
        ("Flussdiagramm", "flowchart TD\n    A[Start] --> B{Bedingung?}\n    B -->|ja| C[Schritt]\n    B -->|nein| D[Anderer Schritt]\n    C --> E[Ende]\n    D --> E"),
        ("Sequenzdiagramm", "sequenceDiagram\n    participant A as Client\n    participant B as Server\n    A->>B: Anfrage\n    B-->>A: Antwort"),
        ("Klassendiagramm", "classDiagram\n    class Tier {\n        +String name\n        +laut() void\n    }\n    class Hund\n    Tier <|-- Hund"),
        ("Zustandsdiagramm", "stateDiagram-v2\n    [*] --> Bereit\n    Bereit --> Läuft: start\n    Läuft --> Bereit: stopp\n    Läuft --> [*]"),
        ("ER-Diagramm", "erDiagram\n    STUDENT ||--o{ KURS : belegt\n    KURS }o--|| DOZENT : hält"),
        ("Gantt-Plan", "gantt\n    title Projektplan\n    dateFormat YYYY-MM-DD\n    section Phase 1\n    Recherche :a1, 2026-11-01, 7d\n    Entwurf   :after a1, 5d\n    section Phase 2\n    Umsetzung :2026-11-14, 10d"),
        ("Mindmap", "mindmap\n  root((Thema))\n    Idee A\n      Detail\n    Idee B\n    Idee C"),
        ("Kreisdiagramm", "pie title Verteilung\n    \"A\" : 45\n    \"B\" : 30\n    \"C\" : 25"),
        ("Zeitstrahl", "timeline\n    title Verlauf\n    2024 : Start\n    2025 : Ausbau\n    2026 : Abschluss"),
    ]

    init(project: ProjectModel, path: String, line: Int) {
        self.project = project
        self.path = path
        startLine = line
        // The mode of the tab as the session has it (Linux writes it too).
        let tab = project.editor.groups.flatMap(\.tabs).first { $0.path == path }
        // CI and screenshots: `-AtlasMarkdownMode split` (or `defaults write`).
        let forced = UserDefaults.standard.string(forKey: "AtlasMarkdownMode").flatMap(Mode.init(rawValue:))
        mode = forced ?? tab?.extra["mode"]?.string.flatMap(Mode.init(rawValue:)) ?? (line > 0 ? .edit : .preview)
        if let f = tab?.extra["find"]?.object, let text = f["text"]?.string, !text.isEmpty {
            let line = f["line"]?.int ?? line
            find = (text, f["caseSensitive"]?.bool ?? false, line)
        }
        editor = TextEditorController(editor: project.core.editor(), language: "Markdown", markdown: true)
        editor.onChange = { [weak self] text in self?.changed(text) }
        editor.onScroll = { [weak self] in self?.editorScrolled() }
        editor.onPaste = { [weak self] in self?.pasteSpecial() ?? false }
        listener = project.listen { [weak self] event in self?.handle(event) }
        if mode != .edit { ensurePreview() }
    }

    func close() {
        if modified { save() }
        if let listener { project.stopListening(listener) }
        preview?.close()
        saveTask?.cancel()
        renderTask?.cancel()
    }

    // MARK: - file

    func load() async {
        do {
            let text = try await project.core.files().readText(path: path, maxBytes: 20 << 20)
            lastSaved = text
            editor.text = text
            modified = false
            conflict = false
            loaded = true
            error = nil
            render()
            if let find {
                markHits(find.text, caseSensitive: find.caseSensitive, line: find.line)
                self.find = nil
            } else if startLine > 0 {
                editor.go(toLine: startLine)
                preview?.scroll(toLine: startLine)
            }
            startLine = 0
        } catch {
            self.error = atlasMessage(error)
        }
    }

    private func changed(_ text: String) {
        modified = text != lastSaved
        project.core.notes().noteChanged(path: path, text: text)
        saveTask?.cancel()
        if modified {
            saveTask = Task { [weak self] in
                try? await Task.sleep(for: .seconds(1))
                guard !Task.isCancelled else { return }
                self?.save()
            }
        }
        scheduleRender()
    }

    /// Writes the file now (⌘S, or a second after the last change).
    func save() {
        saveTask?.cancel()
        let text = editor.text
        do {
            try project.core.files().writeText(path: path, text: text)
            lastSaved = text
            modified = false
            conflict = false
        } catch {
            project.show(notice: "Nicht gespeichert: \(atlasMessage(error))")
        }
    }

    /// Takes the version on disk (after a change outside of Atlas).
    func reloadFromDisk() {
        Task { await load() }
    }

    private func handle(_ event: Event) {
        guard case let .fileModified(changed) = event, changed == path else { return }
        if modified {
            conflict = true
        } else {
            Task { [weak self] in
                guard let self else { return }
                let text = (try? await project.core.files().readText(path: path, maxBytes: 20 << 20)) ?? lastSaved
                // Our own save comes back as a change: nothing to do.
                if text != lastSaved { await load() }
            }
        }
    }

    // MARK: - preview

    private func ensurePreview() {
        guard preview == nil else { return }
        let p = MarkdownPreview(root: project.root)
        p.onMessage = { [weak self] message in self?.handle(message) }
        preview = p
        if loaded { render() }
    }

    private func scheduleRender() {
        renderTask?.cancel()
        renderTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(150))
            guard !Task.isCancelled else { return }
            self?.render()
        }
    }

    func render() {
        let result = project.core.editor().renderMarkdown(text: editor.text, path: path)
        info = result
        let light = NSApp.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .aqua
        preview?.show(html: result.html, light: light)
        if mode == .split { followEditor() }
    }

    private func editorScrolled() {
        guard mode == .split, Date().timeIntervalSince(followingPreview) > 0.15 else { return }
        followEditor()
    }

    private func followEditor() {
        guard let preview else { return }
        let mid = editor.scrollView.contentView.bounds.height / 2
        preview.follow(line: editor.exactLine(atY: mid), at: Double(mid))
    }

    private func handle(_ message: MarkdownPreview.Message) {
        switch message {
        case let .link(path, line, page, external, broken, newTab):
            if !external.isEmpty, let url = URL(string: external) {
                NSWorkspace.shared.open(url)
            } else if broken {
                project.show(notice: "Link-Ziel nicht gefunden: \(project.core.relativePath(path: path) ?? path)")
            } else if !path.isEmpty {
                project.editor.open(path: path, line: page > 0 ? page : line, mode: newTab ? .split : .tab)
            }
        case let .task(line, checked):
            if !editor.toggleTask(line: line, done: checked) { render() }
        case let .copy(text):
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
            project.show(notice: "Code kopiert")
        case let .run(code):
            let dir = (path as NSString).deletingLastPathComponent
            let prepared = Prepared(
                name: "Codeblock", argv: ["bash", "-c", code], display: code, cwd: dir, env: [],
                confirm: false, destructive: false, placement: .panel)
            if let run = project.runInTerminal {
                run(prepared, false)
            } else {
                project.show(notice: "Kein Terminal verfügbar")
            }
        case let .image(src):
            let file = src.hasPrefix("atlas://file") ? String(src.dropFirst("atlas://file".count)) : src
            let decoded = file.removingPercentEncoding ?? file
            if decoded.hasPrefix("/") { project.editor.open(path: decoded, mode: .split) }
        case let .scroll(_, exact, at, own):
            guard mode == .split, !own, exact > 0 else { return }
            followingPreview = Date()
            editor.scroll(toExactLine: exact, at: at)
        case .rendered:
            break
        }
    }

    /// Marks a hit of the project search: selected in the editor on its
    /// line, all occurrences marked in the preview (as on Linux).
    func markHits(_ term: String, caseSensitive: Bool, line: Int) {
        editor.select(term: term, caseSensitive: caseSensitive, fromLine: line)
        preview?.markHits(term, caseSensitive: caseSensitive, line: line)
    }

    // MARK: - PDF

    /// „Als PDF exportieren …“: next to the note by default.
    func exportPDF() {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.pdf]
        let url = URL(filePath: path)
        panel.directoryURL = url.deletingLastPathComponent()
        panel.nameFieldStringValue = url.deletingPathExtension().lastPathComponent + ".pdf"
        guard panel.runModal() == .OK, let output = panel.url else { return }
        Task { await exportPDF(to: output) }
    }

    @discardableResult
    func exportPDF(to output: URL) async -> Bool {
        exporting = true
        defer { exporting = false }
        let html = project.core.editor().renderMarkdown(text: editor.text, path: path).html
        let ok = await MarkdownPDFExport(root: project.root).export(html: html, to: output)
        project.show(notice: ok ? "Als PDF exportiert: \(output.lastPathComponent)" : "PDF-Export fehlgeschlagen")
        return ok
    }

    // MARK: - inserting

    private func editorForInsert() -> TextEditorController {
        if mode == .preview { mode = .split }
        return editor
    }

    func insertTable(rows: Int = 2, columns: Int = 3) {
        let cells = { (f: (Int) -> String) in "| " + (0..<columns).map(f).joined(separator: " | ") + " |" }
        var lines = [cells { "Spalte \($0 + 1)" }, cells { _ in "---" }]
        for _ in 0..<rows { lines.append(cells { _ in "" }) }
        let table = project.core.editor().formatTable(markdown: lines.joined(separator: "\n"))
        editorForInsert().insertBlock(table)
    }

    func insertDiagram(_ index: Int) {
        guard Self.diagrams.indices.contains(index) else { return }
        editorForInsert().insertBlock("```mermaid\n" + Self.diagrams[index].code + "\n```")
    }

    func insertFolderTree() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.directoryURL = URL(filePath: project.root, directoryHint: .isDirectory)
        panel.message = "Ordner für den Baum wählen"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        let tree = project.core.editor().folderTree(path: url.path(percentEncoded: false))
        editorForInsert().insertBlock("```tree\n" + tree + (tree.hasSuffix("\n") ? "" : "\n") + "```")
    }

    /// Pictures and files chosen in a panel: copied into `assets/` (files
    /// of the project are linked where they are).
    func insertFiles() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = true
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        guard panel.runModal() == .OK else { return }
        insertFiles(panel.urls.map { $0.path(percentEncoded: false) })
    }

    func insertFiles(_ paths: [String]) {
        let markdown = project.core.notes().importFiles(note: path, paths: paths)
        if !markdown.isEmpty { editorForInsert().insertBlock(markdown) }
    }

    /// ⌘V with files or a picture (and no text) on the pasteboard: into
    /// `assets/`, linked from the note (Linux: `paste_into_note`).
    private func pasteSpecial() -> Bool {
        let board = NSPasteboard.general
        if let urls = board.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL],
           !urls.isEmpty {
            insertFiles(urls.map { $0.path(percentEncoded: false) })
            return true
        }
        guard board.string(forType: .string) == nil,
              let image = board.readObjects(forClasses: [NSImage.self])?.first as? NSImage,
              let tiff = image.tiffRepresentation,
              let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:])
        else { return false }
        do {
            let target = try project.core.notes().newImagePath(note: path, ext: "png")
            try png.write(to: URL(filePath: target))
            editor.insertBlock(project.core.notes().imageMarkdown(note: path, image: target))
        } catch {
            project.show(notice: "Bild nicht eingefügt: \(atlasMessage(error))")
        }
        return true
    }

    /// Aligns all tables of the note (menu).
    func alignTables() {
        let core = project.core.editor()
        let edits = core.alignTables(text: editor.text, cursor: UInt32(editor.textView.selectedRange().location))
        // In the order given: each one counts on the text after the previous.
        for edit in edits { editor.apply(edit) }
    }
}
