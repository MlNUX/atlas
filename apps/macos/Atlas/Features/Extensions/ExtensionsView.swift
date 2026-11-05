// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import SwiftUI

/// „Erweiterungen“ (P3.10; Linux: `ExtensionsDialog.qml`): list, switch on
/// and off, trust project extensions, settings, bundled ones, new, remove.
struct ExtensionsView: View {
    let project: ProjectModel
    @State private var list: [ExtensionInfo] = []
    @State private var bundled: [BundledExtension] = []
    @State private var selection: String?
    @State private var error: String?
    @State private var creating = false
    @State private var newTitle = ""
    @Environment(\.dismiss) private var dismiss

    private var extensions: Extensions { project.core.extensions() }

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Erweiterungen").font(.headline)
                Spacer()
                Menu("Mitgeliefert") {
                    ForEach(bundled, id: \.name) { b in
                        Button(b.installed ? "\(b.title) (installiert)" : "Installieren: \(b.title)") {
                            perform { try extensions.installBundled(name: b.name) }
                            project.show(notice: "„\(b.title)“ installiert")
                        }
                        .disabled(b.installed)
                    }
                }
                .fixedSize()
                Button("Neue Erweiterung …") { creating = true }
                Button("Neu laden", systemImage: "arrow.clockwise") { extensions.reload(); load() }
                    .labelStyle(.iconOnly)
                Button("Schließen") { dismiss() }.keyboardShortcut(.cancelAction)
            }
            .padding(10)
            Text("Ordner mit einer extension.yaml – global in ~/Library/Application Support/Atlas/extensions/, für ein Projekt in .atlas/extensions/. Sie fügen Aktionen, Anzeigen für weitere Dateitypen, Exporter und Ansichten hinzu.")
                .font(.caption).foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 10).padding(.bottom, 8)
            if let error {
                Text(error).foregroundStyle(.red).font(.callout).padding(.horizontal, 10).padding(.bottom, 6)
            }
            Divider()
            if list.isEmpty {
                ContentUnavailableView("Keine Erweiterungen installiert", systemImage: "puzzlepiece.extension",
                                       description: Text("„Mitgeliefert“ bringt fertige Erweiterungen mit."))
            } else {
                HSplitView {
                    List(list, id: \.name, selection: $selection) { e in
                        HStack {
                            Image(systemName: "puzzlepiece.extension")
                            VStack(alignment: .leading, spacing: 1) {
                                Text(e.title.isEmpty ? e.name : e.title)
                                Text("\(location(e.location)) · \(status(e.status))")
                                    .font(.caption)
                                    .foregroundStyle(e.status == .active ? Color.secondary : e.status == .broken ? Color.red : Color.orange)
                            }
                        }
                        .tag(e.name)
                    }
                    .frame(minWidth: 220, maxWidth: 300)
                    Group {
                        if let e = list.first(where: { $0.name == selection }) {
                            ExtensionDetail(project: project, info: e, perform: perform, reload: load)
                        } else {
                            ContentUnavailableView("Erweiterung wählen", systemImage: "puzzlepiece.extension")
                        }
                    }
                    .frame(minWidth: 380, maxWidth: .infinity, maxHeight: .infinity)
                }
            }
        }
        .frame(width: 820, height: 560)
        .onAppear { load() }
        .onChange(of: project.actions.revision) { load() }
        .alert("Neue Erweiterung", isPresented: $creating) {
            TextField("z. B. PDF-Werkzeuge", text: $newTitle)
            Button("Anlegen") {
                let title = newTitle.trimmingCharacters(in: .whitespaces)
                newTitle = ""
                guard !title.isEmpty else { return }
                perform {
                    let manifest = try extensions.create(title: title)
                    dismiss()
                    project.editor.open(path: manifest, mode: .tab)
                }
            }
            Button("Abbrechen", role: .cancel) { newTitle = "" }
        } message: {
            Text("Wie soll sie heißen? Die Vorlage enthält Beispiele als Kommentare – sie tut nichts, bis du sie ausfüllst.")
        }
    }

    private func load() {
        list = extensions.list()
        bundled = extensions.bundled()
        if selection == nil || !list.contains(where: { $0.name == selection }) { selection = list.first?.name }
    }

    private func perform(_ work: () throws -> Void) {
        do {
            try work()
            error = nil
        } catch {
            self.error = atlasMessage(error)
        }
        load()
    }

    private func location(_ l: ExtensionLocation) -> String {
        switch l {
        case .user: "Global"
        case .project: "Projekt"
        case .bundled: "Mitgeliefert"
        }
    }

    private func status(_ s: ExtensionStatus) -> String {
        switch s {
        case .active: "aktiv"
        case .disabled: "ausgeschaltet"
        case .untrusted: "nicht freigegeben"
        case .broken: "fehlerhaft"
        }
    }
}

private struct ExtensionDetail: View {
    let project: ProjectModel
    let info: ExtensionInfo
    let perform: (() throws -> Void) -> Void
    let reload: () -> Void

    private var extensions: Extensions { project.core.extensions() }

    var body: some View {
        Form {
            Section {
                LabeledContent("Name", value: info.name)
                if !info.version.isEmpty { LabeledContent("Version", value: info.version) }
                if !info.description.isEmpty { Text(info.description).foregroundStyle(.secondary) }
                Text("\(info.actions.count) Aktion(en) · \(info.processors.count) Anzeige(n) · \(info.exporters.count) Exporter · \(info.views.count) Ansicht(en)")
                    .font(.caption).foregroundStyle(.secondary)
                ForEach(info.errors, id: \.self) { Text($0).foregroundStyle(.red).font(.callout) }
            }
            Section {
                switch info.status {
                case .untrusted:
                    Button("Prüfen und freigeben …") { trust() }
                case .broken:
                    EmptyView()
                default:
                    Toggle("Eingeschaltet", isOn: Binding(
                        get: { info.status == .active },
                        set: { on in perform { try extensions.setEnabled(name: info.name, enabled: on) } }))
                }
                Text(permissions).font(.caption).foregroundStyle(.secondary)
            }
            if !info.settings.isEmpty {
                Section("Einstellungen") {
                    ForEach(info.settings, id: \.key) { s in
                        SettingRow(setting: s) { value in
                            perform { try extensions.setSetting(name: info.name, key: s.key, value: value) }
                        }
                    }
                }
            }
            Section {
                HStack {
                    Button("Manifest öffnen") { project.editor.open(path: info.manifest, mode: .tab) }
                    Button("Im Finder zeigen") {
                        NSWorkspace.shared.activateFileViewerSelecting([URL(filePath: info.dir)])
                    }
                    Spacer()
                    Button("Deinstallieren …", role: .destructive) { uninstall() }
                }
            }
        }
        .formStyle(.grouped)
    }

    private var permissions: String {
        if info.api < 2 { return "Format 1: keine Rechte angemeldet – die Befehle dürfen alles, was du darfst." }
        return info.permissions.isEmpty ? "Keine Rechte angemeldet." : info.permissions.joined(separator: "\n")
    }

    private func trust() {
        let alert = NSAlert()
        alert.messageText = "Erweiterung freigeben?"
        alert.informativeText = "„\(info.title.isEmpty ? info.name : info.title)“ gehört zu diesem Projekt (\(info.dir)). Nach der Freigabe kann Atlas ihre Befehle ausführen – Anzeigen auch automatisch beim Öffnen passender Dateien. Gib sie nur frei, wenn du dem Projekt vertraust.\n\n\(permissions)\n\nÄndert sich eine Datei der Erweiterung, wird erneut gefragt."
        alert.addButton(withTitle: "Freigeben")
        alert.addButton(withTitle: "Abbrechen")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        perform { try extensions.trust(name: info.name) }
    }

    private func uninstall() {
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = "Erweiterung deinstallieren?"
        let name = info.title.isEmpty ? info.name : info.title
        alert.informativeText = (info.location == .project
            ? "„\(name)“ gehört zu diesem Projekt. Ihr Ordner \(info.dir) kommt in den Papierkorb – auch für alle, mit denen du das Projekt teilst."
            : "Der Ordner von „\(name)“ (\(info.dir)) kommt in den Papierkorb.")
            + "\n\nIhre Einstellungen und gespeicherten Geheimnisse löscht Atlas. Dateien, die sie in deinen Projekten angelegt hat, bleiben."
        alert.addButton(withTitle: "Deinstallieren")
        alert.addButton(withTitle: "Abbrechen")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        perform { try extensions.uninstall(dir: info.dir) }
        project.show(notice: "„\(name)“ deinstalliert – der Ordner liegt im Papierkorb")
    }
}

/// One setting of an extension; values are JSON as in the core.
private struct SettingRow: View {
    let setting: ExtensionSetting
    /// JSON value, `nil` resets.
    let save: (String?) -> Void
    @State private var text = ""
    @State private var choosing = false

    private static func json(_ s: String) -> String {
        (try? JSONEncoder().encode(s)).flatMap { String(data: $0, encoding: .utf8) } ?? "\"\""
    }

    private var decoded: Any? {
        try? JSONSerialization.jsonObject(with: Data(setting.value.utf8), options: .fragmentsAllowed)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            switch setting.kind {
            case .bool:
                Toggle(setting.title, isOn: Binding(
                    get: { decoded as? Bool ?? false }, set: { save($0 ? "true" : "false") }))
            case .choice:
                Picker(setting.title, selection: Binding(
                    get: { decoded as? String ?? "" }, set: { save(Self.json($0)) })) {
                    ForEach(setting.choices, id: \.self) { Text($0).tag($0) }
                }
            case .secret:
                HStack {
                    SecureField(setting.title, text: $text, prompt: Text((decoded as? Bool ?? false) ? "gespeichert" : "nicht gesetzt"))
                        .onSubmit { commit() }
                    Button("Speichern") { commit() }.disabled(text.isEmpty)
                    if decoded as? Bool ?? false { Button("Löschen") { save(nil) } }
                }
            case .number:
                TextField(setting.title, text: $text)
                    .onSubmit { if Double(text) != nil { save(text) } }
            case .path:
                HStack {
                    TextField(setting.title, text: $text).onSubmit { save(Self.json(text)) }
                    Button("Wählen …") { choosing = true }
                }
                .fileImporter(isPresented: $choosing, allowedContentTypes: [.item, .folder]) { result in
                    if case .success(let url) = result {
                        text = url.path
                        save(Self.json(url.path))
                    }
                }
            case .text:
                TextField(setting.title, text: $text).onSubmit { save(Self.json(text)) }
            }
            if !setting.description.isEmpty {
                Text(setting.description).font(.caption).foregroundStyle(.secondary)
            }
        }
        .onAppear {
            switch setting.kind {
            case .text, .path: text = decoded as? String ?? ""
            case .number: text = (decoded as? NSNumber)?.stringValue ?? ""
            default: text = ""
            }
        }
    }

    private func commit() {
        guard !text.isEmpty else { return }
        save(Self.json(text))
        text = ""
    }
}
