// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import SwiftUI
import WebKit

/// The `ilias:` tab (P3.11): the favourites as folders, downloads, login.
struct IliasTabView: View {
    let project: ProjectModel
    @State private var model: IliasModel?

    var body: some View {
        Group {
            if let model {
                IliasContent(model: model)
            } else {
                ProgressView()
            }
        }
        .task { model = IliasModel(project: project, ilias: project.app.core.ilias()) }
    }
}

private struct IliasContent: View {
    @Bindable var model: IliasModel
    @State private var choosingDir = false

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                Image(systemName: "graduationcap").foregroundStyle(.tint)
                Text(URL(string: model.config.baseUrl)?.host ?? "ILIAS").font(.headline)
                if model.state == .scanning { ProgressView().controlSize(.small) }
                Text(model.status).font(.callout).foregroundStyle(.secondary)
                Spacer()
                Button("Download-Ordner …") { choosingDir = true }
                    .help(model.config.downloadDir.isEmpty ? "Projektordner" : model.config.downloadDir)
                Button("Einlesen", systemImage: "arrow.clockwise") { Task { await model.scan() } }
                    .disabled(model.state == .scanning)
            }
            .padding(10)
            if let error = model.error {
                Text(error).foregroundStyle(.red).font(.callout).padding(.horizontal, 10)
            }
            Divider()
            if let index = model.index, !index.courses.isEmpty {
                List {
                    OutlineGroup(index.courses, id: \.url, children: \.childrenOrNil) { item in
                        IliasRow(model: model, item: item)
                    }
                }
            } else {
                ContentUnavailableView {
                    Label("Noch nichts eingelesen", systemImage: "graduationcap")
                } description: {
                    Text("„Einlesen“ liest die Favoriten deines ILIAS-Schreibtischs.")
                } actions: {
                    Button("Einlesen") { Task { await model.scan() } }
                }
            }
        }
        .fileImporter(isPresented: $choosingDir, allowedContentTypes: [.folder]) { result in
            if case .success(let url) = result { model.setDownloadDir(url.path) }
        }
        .sheet(isPresented: Bindable(model.login).visible) {
            IliasLoginSheet(login: model.login)
        }
    }
}

private struct IliasRow: View {
    let model: IliasModel
    let item: IliasItem

    var body: some View {
        HStack {
            Image(systemName: item.kind == "file" ? "doc" : "folder.fill")
                .foregroundStyle(item.kind == "file" ? Color.secondary : Color.accentColor)
            VStack(alignment: .leading, spacing: 1) {
                Text(item.title)
                if !item.info.isEmpty {
                    Text(item.info).font(.caption).foregroundStyle(.secondary)
                }
                if !item.error.isEmpty {
                    Text(item.error).font(.caption).foregroundStyle(.red)
                }
            }
            Spacer()
            if item.kind == "file" {
                if model.loading.contains(item.url) {
                    ProgressView().controlSize(.small)
                } else if model.isCurrent(item) {
                    Image(systemName: "checkmark.circle.fill").foregroundStyle(.green).help("Aktuell geladen")
                } else {
                    Button("Laden", systemImage: "arrow.down.circle") { Task { await model.download(item) } }
                        .labelStyle(.iconOnly)
                        .buttonStyle(.borderless)
                }
            }
        }
    }
}

private struct IliasLoginSheet: View {
    @Bindable var login: IliasLoginWindow

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Bei ILIAS anmelden").font(.headline)
                Spacer()
                Button("Schließen") { login.close() }
            }
            .padding(10)
            if !login.offerHost.isEmpty {
                HStack {
                    Text("Gespeicherte Anmeldung auf \(login.offerHost) einsetzen?")
                    Button("Erlauben") { login.allowHere() }
                }
                .padding(8).frame(maxWidth: .infinity).background(.yellow.opacity(0.2))
            }
            if !login.problem.isEmpty {
                Text(login.problem).foregroundStyle(.red).padding(8)
            }
            WebViewHost(webView: login.webView)
        }
        .frame(minWidth: 720, minHeight: 560)
    }
}

/// Shows an existing WKWebView (owned by the login model).
private struct WebViewHost: NSViewRepresentable {
    let webView: WKWebView
    func makeNSView(context: NSViewRepresentableContext<Self>) -> WKWebView { webView }
    func updateNSView(_ view: WKWebView, context: NSViewRepresentableContext<Self>) {}
}

extension IliasItem {
    /// Containers have children (also empty ones); files have none.
    var childrenOrNil: [IliasItem]? { kind == "file" ? nil : children }
}
