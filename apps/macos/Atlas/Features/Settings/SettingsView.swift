// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import SwiftUI

/// „Einstellungen“ (⌘,; P3.9; Linux: `SettingsDialog.qml`). Everything is
/// stored by the core in `~/Library/Application Support/Atlas/config.toml`
/// (E13), in the same format as on Linux. Parts that belong to a project
/// in the API (context bar, extensions, external editor) use the window
/// opened last.
struct SettingsView: View {
    let app: AppModel

    var body: some View {
        TabView {
            GeneralSettings(app: app)
                .tabItem { Label("Allgemein", systemImage: "gearshape") }
            IliasSettings(app: app)
                .tabItem { Label("ILIAS", systemImage: "graduationcap") }
            ProjectSettings(app: app) { project in ContextSettings(project: project) }
                .tabItem { Label("Kontextleiste", systemImage: "sidebar.trailing") }
            ProjectSettings(app: app) { project in ExtensionsView(project: project) }
                .tabItem { Label("Erweiterungen", systemImage: "puzzlepiece.extension") }
            ProjectSettings(app: app) { project in ProgramSettings(app: app, project: project) }
                .tabItem { Label("Programme", systemImage: "terminal") }
        }
        .frame(width: 640, height: 560)
    }
}

/// Settings tied to a project in the API: the window opened last, else a hint.
private struct ProjectSettings<Content: View>: View {
    let app: AppModel
    @ViewBuilder var content: (ProjectModel) -> Content

    var body: some View {
        if let project = app.lastProject {
            content(project)
        } else {
            ContentUnavailableView("Kein Projekt geöffnet", systemImage: "folder",
                                   description: Text("Diese Einstellungen gelten für alle Projekte; zum Bearbeiten ein Projekt öffnen."))
        }
    }
}

// MARK: - Allgemein

private struct GeneralSettings: View {
    let app: AppModel
    @State private var settings: AppSettings?
    @State private var error: String?

    var body: some View {
        Form {
            if let s = settings {
                Section("Darstellung") {
                    Picker("Farbschema", selection: binding(\.theme, s)) {
                        Text("System").tag(Theme.system)
                        Text("Hell").tag(Theme.light)
                        Text("Dunkel").tag(Theme.dark)
                    }
                    .pickerStyle(.segmented)
                    Text("Dunkel oder hell für alle Fenster, Vorschau und Terminal; „System“ folgt der Einstellung von macOS.")
                        .font(.caption).foregroundStyle(.secondary)
                    Toggle("PDF-Seiten dunkel", isOn: binding(\.pdfDark, s))
                    Text("Neue PDF-Tabs öffnen im Dunkelmodus (in jedem Tab umschaltbar).")
                        .font(.caption).foregroundStyle(.secondary)
                }
                Section("Dateien") {
                    Toggle("Natürliche Sortierung", isOn: binding(\.naturalSort, s))
                    Text("Zahlen nach ihrem Wert: „2.pdf“ vor „10.pdf“. Gilt für Ordneransicht und Projektbaum.")
                        .font(.caption).foregroundStyle(.secondary)
                }
                Section("Notizen") {
                    Toggle("Rechtschreibprüfung", isOn: binding(\.spellcheck, s))
                    Text("Mit der Rechtschreibung von macOS (Sprachen in den Systemeinstellungen); gilt für neu geöffnete Notizen.")
                        .font(.caption).foregroundStyle(.secondary)
                }
                Section("Terminal") {
                    TextField("Shell", text: Binding(get: { s.shell }, set: { var n = s; n.shell = $0; settings = n }),
                              prompt: Text("Anmelde-Shell"))
                        .onSubmit { save(settings) }
                    Text("Leer: die Anmelde-Shell. Gilt für neue Terminals.").font(.caption).foregroundStyle(.secondary)
                }
            }
            if let error { Text(error).foregroundStyle(.red) }
            Section {
                LabeledContent("Gespeichert in") {
                    Button((app.core.settings().file() as NSString).abbreviatingWithTildeInPath) {
                        NSWorkspace.shared.activateFileViewerSelecting([URL(filePath: app.core.settings().file())])
                    }
                    .buttonStyle(.link)
                }
                LabeledContent("Version", value: app.core.version())
            }
        }
        .formStyle(.grouped)
        .onAppear { settings = app.core.settings().get() }
        .onChange(of: app.settingsRevision) { settings = app.core.settings().get() }
    }

    private func binding<T>(_ key: WritableKeyPath<AppSettings, T>, _ s: AppSettings) -> Binding<T> {
        Binding(get: { s[keyPath: key] }, set: { value in
            var n = s
            n[keyPath: key] = value
            settings = n
            save(n)
        })
    }

    private func save(_ s: AppSettings?) {
        guard let s else { return }
        do {
            try app.core.settings().save(settings: s)
            error = nil
        } catch {
            self.error = atlasMessage(error)
        }
    }
}

// MARK: - ILIAS

private struct IliasSettings: View {
    let app: AppModel
    @State private var config: IliasConfig?
    @State private var address = ""
    @State private var user = ""
    @State private var password = ""
    @State private var message: String?
    @State private var error: String?
    @State private var choosingDir = false

    private var ilias: Ilias { app.core.ilias() }

    var body: some View {
        Form {
            if let c = config {
                Section {
                    Text("Die Favoriten deines ILIAS-Schreibtischs mit Ordnern, Gruppen, Sitzungen und Dateien; ein Klick lädt eine Datei und öffnet sie (Darstellung → ILIAS).")
                        .font(.caption).foregroundStyle(.secondary)
                    Toggle("Favoriten als Ordner unter „Orte“", isOn: Binding(get: { c.showInPlaces }, set: { v in
                        var n = c; n.showInPlaces = v; save(n)
                    }))
                }
                Section("Adresse") {
                    HStack {
                        TextField("Adresse", text: $address, prompt: Text("https://ilias.uni-beispiel.de/"))
                            .onSubmit { applyAddress(c) }
                        Button("Übernehmen") { applyAddress(c) }.disabled(address == c.baseUrl)
                    }
                    Text("Startseite des ILIAS deiner Hochschule. Nach einer Änderung meldest du dich dort an und liest die Favoriten neu ein.")
                        .font(.caption).foregroundStyle(.secondary)
                }
                Section("Anmeldung") {
                    if c.loginSaved {
                        LabeledContent(c.user.isEmpty ? "Gespeichert" : c.user) {
                            Button("Anmeldung vergessen") {
                                perform { try ilias.forgetLogin() }
                                message = "Anmeldung vergessen"
                            }
                        }
                        Text(c.loginHost.isEmpty
                             ? "Bei der nächsten Anmeldung fragt Atlas einmal, auf welcher Seite es die Daten einsetzen darf."
                             : "Wird nur auf \(c.loginHost) eingesetzt, wenn die Sitzung abgelaufen ist.")
                            .font(.caption).foregroundStyle(.secondary)
                    } else {
                        TextField("Benutzername", text: $user)
                        SecureField("Passwort", text: $password)
                        HStack {
                            Spacer()
                            Button("Merken") {
                                perform { try ilias.rememberLogin(login: IliasLogin(user: user, password: password)) }
                                password = ""
                                message = "Anmeldung im Schlüsselbund gespeichert"
                            }
                            .disabled(user.isEmpty || password.isEmpty)
                        }
                        Text("Optional: Atlas meldet dich dann selbst an. Das Passwort liegt nur im Schlüsselbund von macOS.")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }
                Section("Downloads") {
                    LabeledContent("Ordner") {
                        Text(c.downloadDir.isEmpty ? "Projektordner" : (c.downloadDir as NSString).abbreviatingWithTildeInPath)
                            .foregroundStyle(.secondary)
                        Button("Ändern …") { choosingDir = true }
                        if !c.downloadDir.isEmpty {
                            Button("Projektordner") { var n = c; n.downloadDir = ""; save(n) }
                        }
                    }
                    HStack {
                        Spacer()
                        Button("Abmelden") {
                            ilias.logout()
                            message = "Bei ILIAS abgemeldet"
                        }
                    }
                }
            }
            if let message { Text(message).foregroundStyle(.secondary) }
            if let error { Text(error).foregroundStyle(.red) }
        }
        .formStyle(.grouped)
        .onAppear(perform: load)
        .fileImporter(isPresented: $choosingDir, allowedContentTypes: [.folder]) { result in
            if case .success(let url) = result, let c = config { var n = c; n.downloadDir = url.path; save(n) }
        }
    }

    private func load() {
        let c = ilias.config()
        config = c
        address = c.baseUrl
        user = c.user
    }

    private func applyAddress(_ c: IliasConfig) {
        var n = c
        n.baseUrl = address
        save(n)
    }

    private func save(_ c: IliasConfig) {
        perform { try ilias.saveConfig(config: c) }
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
}

// MARK: - Kontextleiste

private struct ContextSettings: View {
    let project: ProjectModel
    @State private var options: ContextOptions?
    @State private var error: String?

    private var context: Context { project.core.context() }

    var body: some View {
        Form {
            if let o = options {
                Section("Profile") {
                    ForEach(context.profiles(), id: \.self) { name in
                        LabeledContent(ContextModel.profileTitle(name)) {
                            if name == "default" {
                                Text("Standard").foregroundStyle(.secondary)
                            } else if !context.isSet(profile: name) {
                                Text("Nicht eingestellt – wie Allgemein").foregroundStyle(.secondary)
                            } else {
                                Button("Zurücksetzen") { perform { try context.reset(profile: name) } }
                            }
                        }
                    }
                    Text("Eingestellt wird in der Leiste selbst: Darstellung → Kontextleiste bearbeiten, mit einer Datei der Art geöffnet.")
                        .font(.caption).foregroundStyle(.secondary)
                    Toggle("Profile nach Dateityp", isOn: binding(\.byKind, o))
                    Text("Aus: überall gilt „Allgemein“.").font(.caption).foregroundStyle(.secondary)
                }
                Section("Verhalten") {
                    Picker("Leere Bausteine", selection: binding(\.empty, o)) {
                        Text("Zeigen").tag("show")
                        Text("Einklappen").tag("collapse")
                        Text("Ausblenden").tag("hide")
                    }
                    Picker("Reiterbeschriftung", selection: binding(\.tabLabels, o)) {
                        Text("Automatisch").tag("auto")
                        Text("Text").tag("text")
                        Text("Symbole").tag("icons")
                    }
                    Text("Automatisch: Symbole, wenn die Leiste zu schmal ist.").font(.caption).foregroundStyle(.secondary)
                }
                Section("Lernzeit (Minuten)") {
                    Stepper("Lernen: \(o.pomodoroWork)", value: binding(\.pomodoroWork, o), in: 1...180)
                    Stepper("Pause: \(o.pomodoroBreak)", value: binding(\.pomodoroBreak, o), in: 1...180)
                    Stepper("Lange Pause: \(o.pomodoroLongBreak)", value: binding(\.pomodoroLongBreak, o), in: 1...180)
                }
                Section {
                    HStack {
                        Spacer()
                        Button("Alle Profile zurücksetzen", role: .destructive) {
                            perform { for name in context.profiles() { try context.reset(profile: name) } }
                        }
                    }
                }
            }
            if let error { Text(error).foregroundStyle(.red) }
        }
        .formStyle(.grouped)
        .onAppear { options = context.options() }
    }

    private func binding<T>(_ key: WritableKeyPath<ContextOptions, T>, _ o: ContextOptions) -> Binding<T> {
        Binding(get: { o[keyPath: key] }, set: { value in
            var n = o
            n[keyPath: key] = value
            options = n
            perform { try context.setOptions(options: n) }
        })
    }

    private func perform(_ work: () throws -> Void) {
        do {
            try work()
            error = nil
        } catch {
            self.error = atlasMessage(error)
        }
        options = context.options()
    }
}

// MARK: - Programme

private struct ProgramSettings: View {
    let app: AppModel
    let project: ProjectModel
    @State private var editor: ExternalEditor?
    @State private var error: String?

    var body: some View {
        Form {
            if let e = editor {
                Section("Externer Editor") {
                    TextField("Befehl", text: Binding(get: { e.command }, set: { var n = e; n.command = $0; editor = n }),
                              prompt: Text("z. B. code --goto {file}:{line}"))
                        .onSubmit { save() }
                    Toggle("Im Terminal starten", isOn: Binding(get: { e.inTerminal }, set: { var n = e; n.inTerminal = $0; editor = n; save() }))
                    Text("Platzhalter {file}, {line}, {column}, {dir}. Leer: $VISUAL, $EDITOR oder das Programm des Systems.")
                        .font(.caption).foregroundStyle(.secondary)
                    HStack { Spacer(); Button("Sichern") { save() } }
                }
            }
            Section("Aktionen") {
                HStack {
                    Button("Aktionen des Projekts bearbeiten") { project.actions.openActionsFile(global: false) }
                    Button("Eigene Aktionen bearbeiten") { project.actions.openActionsFile(global: true) }
                }
            }
            Section("Kalender") {
                Text("Nextcloud- oder Google-Konto und der Kalender für neue Termine werden im Kalender eingerichtet (Darstellung → Kalender).")
                    .font(.caption).foregroundStyle(.secondary)
                Button("Kalender öffnen") { project.editor.open(path: "calendar:", mode: .tab) }
            }
            if let error { Text(error).foregroundStyle(.red) }
        }
        .formStyle(.grouped)
        .onAppear { editor = project.core.editor().externalEditor() }
    }

    private func save() {
        guard let editor else { return }
        do {
            try project.core.editor().setExternalEditor(editor: editor)
            error = nil
        } catch {
            self.error = atlasMessage(error)
        }
    }
}
