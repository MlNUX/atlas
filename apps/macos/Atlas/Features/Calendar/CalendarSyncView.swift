// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import Observation
import SwiftUI

/// Calendar account and sync (P3.7, part 2; Linux: `CalendarSync.qml`):
/// Nextcloud/CalDAV or Google, the calendars to show, password, sync state,
/// conflicts. The core syncs; this only shows and asks.
@MainActor
@Observable
final class CalendarSyncModel {
    let project: ProjectModel
    var calendar: AtlasKit.Calendar { project.core.calendar() }
    var account: Account
    var password = ""
    private(set) var state: SyncState
    private(set) var conflicts: [Conflict] = []
    private(set) var found: [CalendarInfo] = []
    var message = ""
    var error: String?
    private(set) var busy = false
    @ObservationIgnored private var token: UUID?

    init(project: ProjectModel) {
        self.project = project
        let calendar = project.core.calendar()
        account = calendar.account()
        state = calendar.syncState()
        found = account.calendars
        conflicts = calendar.conflicts()
        token = project.listen { [weak self] event in
            switch event {
            case .syncChanged, .calendarChanged: self?.refresh()
            default: break
            }
        }
    }

    func stop() { if let token { project.stopListening(token) } }

    func refresh() {
        state = calendar.syncState()
        conflicts = calendar.conflicts()
        let saved = calendar.account()
        if found.isEmpty { found = saved.calendars }
        account.lastSync = saved.lastSync
    }

    /// CalDAV: checks the server and lists its calendars.
    func test() async {
        busy = true
        defer { busy = false }
        do {
            let result = try await calendar.testConnection(account: account, password: password)
            message = result.message
            found = result.calendars
            error = nil
        } catch {
            self.error = atlasMessage(error)
        }
    }

    /// Google: opens the sign-in in the browser and waits for the redirect.
    func googleSignIn() async {
        busy = true
        defer { busy = false }
        do {
            let url = try calendar.googleSignIn(clientId: account.googleClientId, clientSecret: account.googleClientSecret)
            if let u = URL(string: url) { NSWorkspace.shared.open(u) }
            message = "Im Browser anmelden …"
            message = try await calendar.googleSignInFinished()
            let saved = calendar.account()
            account.user = saved.user
            found = saved.calendars
            error = nil
        } catch {
            self.error = atlasMessage(error)
        }
    }

    func cancelGoogle() { calendar.cancelGoogleSignIn() }

    /// Saves the account with the chosen calendars (then the core syncs).
    func save() {
        var a = account
        a.calendars = found
        perform {
            try calendar.saveAccount(account: a)
            if !password.isEmpty { try calendar.setPassword(password: password, remember: a.rememberPassword) }
        }
        password = ""
        if error == nil { message = "Gespeichert" }
    }

    func setFound(_ id: String, visible: Bool) {
        if let i = found.firstIndex(where: { $0.id == id }) { found[i].visible = visible }
    }

    func unlock() { perform { try calendar.setPassword(password: password, remember: account.rememberPassword) }; password = "" }
    func forgetPassword() { perform { try calendar.forgetPassword() } }
    func retryPasswordCommand() { calendar.retryPasswordCommand(); refresh() }
    func syncNow() { calendar.syncNow(); refresh() }
    func clearCache() { perform { try calendar.clearCache() } }

    func disconnect() {
        perform { try calendar.disconnect() }
        account = calendar.account()
        found = []
    }

    func resolve(_ c: Conflict, _ choice: ConflictChoice) { perform { try calendar.resolveConflict(id: c.id, choice: choice) } }

    func createCalendar(_ name: String) async {
        do {
            let info = try await calendar.createRemoteCalendar(name: name)
            found.append(info)
            message = "„\(info.name)“ angelegt"
            error = nil
        } catch {
            self.error = atlasMessage(error)
        }
        refresh()
    }

    private func perform(_ work: () throws -> Void) {
        do {
            try work()
            error = nil
        } catch {
            self.error = atlasMessage(error)
        }
        refresh()
    }
}

struct CalendarSyncView: View {
    let project: ProjectModel
    @State private var model: CalendarSyncModel?
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            if let model { SyncForm(model: model) } else { ProgressView().frame(maxHeight: .infinity) }
            Divider()
            HStack {
                Spacer()
                Button("Schließen") { dismiss() }.keyboardShortcut(.cancelAction)
            }
            .padding(10)
        }
        .frame(width: 560, height: 640)
        .onAppear { model = CalendarSyncModel(project: project) }
        .onDisappear { model?.stop() }
    }
}

private struct SyncForm: View {
    @Bindable var model: CalendarSyncModel
    @State private var confirmClear = false
    @State private var confirmDisconnect = false
    @State private var newCalendar = ""

    var body: some View {
        Form {
            Section("Konto") {
                Picker("Dienst", selection: $model.account.provider) {
                    Text("Keiner").tag(Provider.none)
                    Text("Nextcloud / CalDAV").tag(Provider.calDav)
                    Text("Google").tag(Provider.google)
                }
                .pickerStyle(.segmented)
                switch model.account.provider {
                case .calDav: caldav
                case .google: google
                case .none:
                    Text("Ohne Konto bleiben alle Einträge im Kalender des Projekts (.atlas).").font(.caption).foregroundStyle(.secondary)
                }
            }
            if model.account.provider != .none, !model.found.isEmpty {
                Section("Kalender") {
                    ForEach(model.found, id: \.id) { c in
                        Toggle(isOn: Binding(get: { c.visible }, set: { model.setFound(c.id, visible: $0) })) {
                            HStack {
                                Circle().fill(Color(nsColor: NSColor(atlasHex: c.color.isEmpty ? "#5ec8f2" : c.color))).frame(width: 9, height: 9)
                                Text(c.name + (c.writable ? "" : " (schreibgeschützt)") + (c.missing ? " – nicht mehr auf dem Server" : ""))
                            }
                        }
                    }
                    Text("Sichtbare Kalender werden angezeigt und synchronisiert.").font(.caption).foregroundStyle(.secondary)
                    if model.account.provider == .calDav, model.state.connected {
                        HStack {
                            TextField("Neuer Kalender auf dem Server", text: $newCalendar, prompt: Text("Atlas – Projektarbeit"))
                            Button("Anlegen") {
                                let name = newCalendar.isEmpty ? "Atlas – Projektarbeit" : newCalendar
                                newCalendar = ""
                                Task { await model.createCalendar(name) }
                            }
                        }
                    }
                }
            }
            if model.account.provider != .none {
                Section {
                    HStack {
                        if model.busy { ProgressView().controlSize(.small) }
                        if !model.message.isEmpty { Text(model.message).font(.callout) }
                        Spacer()
                        Button("Sichern und verbinden") { model.save() }.keyboardShortcut(.defaultAction)
                    }
                    if let error = model.error { Text(error).foregroundStyle(.red).font(.callout) }
                }
            }
            if model.state.connected { syncSection }
            if !model.conflicts.isEmpty { conflictSection }
        }
        .formStyle(.grouped)
        .confirmationDialog("Cache leeren?", isPresented: $confirmClear) {
            Button("Leeren", role: .destructive) { model.clearCache() }
        } message: {
            Text("Die zwischengespeicherten Termine vom Server werden entfernt und neu geladen."
                 + (model.state.pending > 0 ? " \(model.state.pending) noch nicht übertragene Änderung(en) gehen verloren." : ""))
        }
        .confirmationDialog("Konto trennen?", isPresented: $confirmDisconnect) {
            Button("Trennen", role: .destructive) { model.disconnect() }
        } message: {
            Text(model.state.google
                 ? "Atlas vergisst Google-Anmeldung und Cache. In Google wird nichts gelöscht; den Zugriff kannst du zusätzlich unter myaccount.google.com → Sicherheit entziehen."
                 : "Atlas vergisst Verbindung, App-Passwort und Cache. Auf dem Server wird nichts gelöscht.")
        }
    }

    @ViewBuilder
    private var caldav: some View {
        Text("Atlas nutzt CalDAV, keine Nextcloud-App nötig. Lege in Nextcloud unter Einstellungen → Sicherheit ein App-Passwort an.")
            .font(.caption).foregroundStyle(.secondary)
        TextField("Server", text: $model.account.server, prompt: Text("https://cloud.example.org"))
        TextField("Benutzer", text: $model.account.user)
        SecureField("App-Passwort", text: $model.password,
                    prompt: Text(model.state.unlocked ? "bekannt für diese Sitzung" : ""))
        Toggle("Auf diesem Rechner merken (Schlüsselbund)", isOn: $model.account.rememberPassword)
        DisclosureGroup("Erweitert") {
            TextField("Passwort-Befehl", text: $model.account.passwordCommand, prompt: Text("optional, z. B. security find-generic-password -w -s nextcloud"))
            Text("Die erste Zeile der Ausgabe ist das App-Passwort.").font(.caption).foregroundStyle(.secondary)
            TextField("CalDAV-URL", text: $model.account.davUrl, prompt: Text("nur falls die automatische Erkennung scheitert"))
        }
        HStack {
            Spacer()
            Button("Verbindung testen") { Task { await model.test() } }
                .disabled(model.busy || model.account.server.isEmpty && model.account.davUrl.isEmpty)
        }
    }

    @ViewBuilder
    private var google: some View {
        Text("Google verlangt eine eigene Anmeldung (OAuth): einmal in der Google Cloud Console einen OAuth-Client vom Typ „Desktop-App“ anlegen und Client-ID und Client-Secret hier eintragen. Atlas merkt sich danach nur ein Zugriffs-Token. Google-Kalender kennen keine Aufgaben: Aufgaben bleiben im Projektkalender.")
            .font(.caption).foregroundStyle(.secondary)
        TextField("Client-ID", text: $model.account.googleClientId)
        SecureField("Client-Secret", text: $model.account.googleClientSecret)
        if !model.account.user.isEmpty { LabeledContent("Angemeldet als", value: model.account.user) }
        HStack {
            Spacer()
            if model.busy { Button("Abbrechen") { model.cancelGoogle() } }
            Button("Mit Google anmelden") { Task { await model.googleSignIn() } }
                .disabled(model.busy || model.account.googleClientId.isEmpty)
        }
    }

    private var syncSection: some View {
        Section("Synchronisierung") {
            HStack {
                if model.state.syncing { ProgressView().controlSize(.small) }
                Text(model.state.status + (model.state.pending > 0 ? " · \(model.state.pending) Änderung(en) warten" : "")
                     + (model.state.conflicts > 0 ? " · \(model.state.conflicts) Konflikt(e)" : ""))
                    .foregroundStyle(model.state.failed ? .red : .primary)
            }
            if let last = model.account.lastSync, !last.isEmpty {
                LabeledContent("Zuletzt", value: last.replacingOccurrences(of: "T", with: " "))
            }
            if !model.state.passwordError.isEmpty {
                Text("Passwort-Befehl fehlgeschlagen: \(model.state.passwordError)").foregroundStyle(.red).font(.callout)
                Button("Passwort-Befehl erneut ausführen") { model.retryPasswordCommand() }
            }
            if !model.state.unlocked, !model.state.google {
                HStack {
                    SecureField("App-Passwort", text: $model.password)
                    Button("Entsperren") { model.unlock() }.disabled(model.password.isEmpty)
                }
            }
            if !model.state.problem.isEmpty { Text(model.state.problem).foregroundStyle(.orange).font(.callout) }
            HStack {
                Button("Jetzt synchronisieren") { model.syncNow() }.disabled(model.state.syncing)
                if !model.state.remembered.isEmpty { Button("Passwort vergessen") { model.forgetPassword() } }
                Spacer()
                Button("Cache leeren …") { confirmClear = true }
                Button("Trennen …", role: .destructive) { confirmDisconnect = true }
            }
        }
    }

    private var conflictSection: some View {
        Section("Konflikte") {
            ForEach(model.conflicts, id: \.id) { c in
                VStack(alignment: .leading, spacing: 4) {
                    Text("„\(c.title)“ (\(c.calendarName))").font(.callout.bold())
                    Text(c.message).font(.caption).foregroundStyle(.secondary)
                    HStack {
                        Button("Meine Fassung") { model.resolve(c, .keepLocal) }.disabled(c.local == nil)
                        Button("Fassung vom Server") { model.resolve(c, .keepRemote) }.disabled(c.remote == nil)
                        Button("Beide behalten") { model.resolve(c, .keepBoth) }.disabled(c.local == nil || c.remote == nil)
                    }
                }
            }
        }
    }
}
