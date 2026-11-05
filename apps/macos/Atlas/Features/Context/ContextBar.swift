// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import SwiftUI

/// The context bar in the inspector (P3.6): tabs („Übersicht“ and pages),
/// blocks for the current file, pinned blocks at the bottom; edit mode with
/// moving, hiding, pages and the gallery. Router: `App/ProjectWindow.swift`.
struct ContextBar: View {
    let project: ProjectModel
    @State private var model: ContextModel?

    var body: some View {
        Group {
            if let model {
                ContextBarContent(model: model)
            } else {
                ProgressView()
            }
        }
        .onAppear {
            if model == nil { model = ContextModel(project: project) }
        }
        .onDisappear { model?.stop() }
        .onChange(of: project.editor.currentTab?.path) { model?.reload() }
        .onChange(of: project.app.settingsRevision) { model?.reload() }
        .onChange(of: project.contextEditing) {
            model?.editing = project.contextEditing
        }
        .onChange(of: model?.editing) {
            if let editing = model?.editing, editing != project.contextEditing { project.contextEditing = editing }
        }
    }
}

private struct ContextBarContent: View {
    @Bindable var model: ContextModel
    @State private var naming: NamingPage?

    struct NamingPage: Identifiable {
        let id = UUID()
        let page: String?
        var title: String
    }

    var body: some View {
        VStack(spacing: 0) {
            tabBar
            Divider()
            if model.editing { editBanner }
            if let error = model.error {
                Text(error).font(.caption).foregroundStyle(.red).padding(6)
            }
            ScrollView {
                VStack(spacing: 10) {
                    ForEach(model.blocks, id: \.self) { id in
                        BlockFrame(model: model, id: id, area: model.area, full: model.fullPage && model.blocks.count == 1)
                    }
                    if model.blocks.isEmpty {
                        Text(model.editing ? "Leer – unten aus der Galerie hinzufügen." : "Keine Bausteine.")
                            .font(.callout).foregroundStyle(.secondary).padding(.top, 20)
                    }
                    if model.editing { ContextGallery(model: model) }
                }
                .padding(10)
            }
            if !model.layout.pinned.isEmpty {
                Divider()
                VStack(spacing: 8) {
                    ForEach(model.layout.pinned, id: \.self) { id in
                        BlockFrame(model: model, id: id, area: .pinned, full: false)
                    }
                }
                .padding(10)
            }
        }
        .alert(naming?.page == nil ? "Neue Seite" : "Seite umbenennen", isPresented: Binding(
            get: { naming != nil }, set: { if !$0 { naming = nil } })
        ) {
            TextField("Titel", text: Binding(get: { naming?.title ?? "" }, set: { naming?.title = $0 }))
            Button("OK") {
                guard let n = naming else { return }
                let title = n.title.trimmingCharacters(in: .whitespaces)
                if !title.isEmpty {
                    if let page = n.page { model.renamePage(page, title) } else { model.newPage(title) }
                }
                naming = nil
            }
            Button("Abbrechen", role: .cancel) { naming = nil }
        }
    }

    // MARK: tabs

    private var tabs: [String] { ["overview"] + model.layout.tabs }

    private func tabLabel(_ id: String, text: Bool) -> some View {
        let title = id == "overview" ? "Übersicht" : model.tabTitle(id)
        let symbol = id == "overview" ? "square.grid.2x2" : model.tabSymbol(id)
        return Group {
            if text { Label(title, systemImage: symbol) } else { Image(systemName: symbol) }
        }
        .help(title)
        .accessibilityLabel(title)
    }

    private func tabRow(text: Bool) -> some View {
        HStack(spacing: 2) {
            ForEach(tabs, id: \.self) { id in
                Button { model.page = id } label: {
                    tabLabel(id, text: text)
                        .lineLimit(1)
                        .padding(.horizontal, 8).padding(.vertical, 4)
                        .background(model.page == id ? Color.accentColor.opacity(0.18) : .clear, in: .rect(cornerRadius: 6))
                }
                .buttonStyle(.plain)
            }
        }
    }

    private var tabBar: some View {
        HStack(spacing: 4) {
            switch model.options.tabLabels {
            case "text": ScrollView(.horizontal, showsIndicators: false) { tabRow(text: true) }
            case "icons": tabRow(text: false)
            default: ViewThatFits(in: .horizontal) { tabRow(text: true); tabRow(text: false) }
            }
            Spacer(minLength: 0)
            Menu {
                Toggle("Bearbeiten", isOn: $model.editing)
                Divider()
                Button("Neue Seite …") { naming = NamingPage(page: nil, title: "") }
                if model.page != "overview" {
                    if model.ownPages.contains(where: { $0.id == model.page }) {
                        Button("Seite umbenennen …") { naming = NamingPage(page: model.page, title: model.tabTitle(model.page)) }
                    }
                    Button("Reiter entfernen") { model.removeTab(model.page) }
                }
                Divider()
                Button("„\(ContextModel.profileTitle(model.profile))“ zurücksetzen") { model.reset() }
                if model.undo != nil { Button("Rückgängig") { model.restoreUndo() } }
            } label: {
                Image(systemName: "ellipsis.circle")
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
            .accessibilityLabel("Kontextleiste anpassen")
        }
        .padding(.horizontal, 8).padding(.vertical, 6)
    }

    private var editBanner: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text("Bearbeiten: \(ContextModel.profileTitle(model.profile))").font(.callout.bold())
                Spacer()
                if model.undo != nil { Button("Rückgängig") { model.restoreUndo() } }
                Button("Fertig") { model.editing = false }.keyboardShortcut(.defaultAction)
            }
            Toggle("Gilt für alle Dateitypen", isOn: $model.applyToAll)
                .toggleStyle(.checkbox).font(.caption)
        }
        .padding(8)
        .background(Color.accentColor.opacity(0.1))
    }
}

/// One block: header (collapse; in the edit mode move, hide, take out) and
/// its content. Hidden blocks show only in the edit mode; empty ones as
/// the options say (`show`, `collapse`, `hide`).
private struct BlockFrame: View {
    let model: ContextModel
    let id: String
    let area: ContextArea
    let full: Bool
    @State private var empty = false

    var body: some View {
        let hidden = model.isHidden(id)
        let collapsed = model.isCollapsed(id)
        let emptyMode = model.editing ? "show" : model.options.empty
        if !(hidden && !model.editing) && !(empty && emptyMode == "hide") {
            VStack(alignment: .leading, spacing: 6) {
                header(hidden: hidden, collapsed: collapsed)
                if !collapsed && !(empty && emptyMode == "collapse") {
                    BlockContent(model: model, id: id, empty: $empty)
                        .frame(maxWidth: .infinity, minHeight: full ? 300 : nil, alignment: .topLeading)
                        .opacity(hidden ? 0.4 : 1)
                }
            }
            .padding(8)
            .background(.background.secondary, in: .rect(cornerRadius: 8))
        }
    }

    private func header(hidden: Bool, collapsed: Bool) -> some View {
        HStack(spacing: 6) {
            Button { model.setCollapsed(id, !collapsed) } label: {
                Image(systemName: collapsed ? "chevron.right" : "chevron.down").font(.caption2).frame(width: 10)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(collapsed ? "Aufklappen" : "Einklappen")
            Image(systemName: model.symbol(id)).foregroundStyle(.secondary)
            Text(model.title(id)).font(.callout.weight(.semibold)).lineLimit(1)
            Spacer()
            if model.editing {
                Button { model.moveBy(id, in: area, -1) } label: { Image(systemName: "arrow.up") }
                    .help("Nach oben (⌥↑)")
                Button { model.moveBy(id, in: area, 1) } label: { Image(systemName: "arrow.down") }
                    .help("Nach unten (⌥↓)")
                Button { model.setHidden(id, !hidden) } label: { Image(systemName: hidden ? "eye.slash" : "eye") }
                    .help(hidden ? "Einblenden" : "Ausblenden")
                Menu {
                    moveTargets
                    Divider()
                    Button("Herausnehmen", role: .destructive) { model.unplace(id) }
                } label: {
                    Image(systemName: "ellipsis")
                }
                .menuStyle(.borderlessButton)
                .menuIndicator(.hidden)
                .fixedSize()
            }
        }
        .buttonStyle(.borderless)
    }

    @ViewBuilder
    private var moveTargets: some View {
        let here = areaKey(area)
        if here != "overview" { Button("In die Übersicht") { model.move(id, from: area, to: .overview) } }
        if here != "pinned", model.catalog[id]?.pin ?? false {
            Button("Unten anheften") { model.move(id, from: area, to: .pinned) }
        }
        if model.catalog[id]?.tab ?? false, !model.layout.tabs.contains(id) {
            Button("Als eigener Reiter") {
                model.move(id, from: area, to: .tabs)
                model.page = id
            }
        }
        ForEach(model.ownPages.filter { "page:\($0.id)" != here }, id: \.id) { p in
            Button("Auf „\(model.tabTitle(p.id))“") { model.move(id, from: area, to: .page(id: p.id)) }
        }
    }

    private func areaKey(_ a: ContextArea) -> String {
        switch a {
        case .tabs: "tabs"
        case .overview: "overview"
        case .pinned: "pinned"
        case .hidden: "hidden"
        case .page(let id): "page:\(id)"
        }
    }
}

/// Edit mode: blocks not placed in this profile, by group, to add.
private struct ContextGallery: View {
    let model: ContextModel

    var body: some View {
        let unplaced = model.unplaced
        VStack(alignment: .leading, spacing: 8) {
            Text("Galerie").font(.headline)
            if unplaced.isEmpty {
                Text("Alle Bausteine sind schon in der Leiste.").font(.caption).foregroundStyle(.secondary)
            }
            ForEach(ContextModel.groups, id: \.0) { group, title in
                let ids = unplaced.filter { model.group(of: $0) == group }
                if !ids.isEmpty {
                    Text(title).font(.caption.bold()).foregroundStyle(.secondary)
                    ForEach(ids, id: \.self) { id in
                        HStack(alignment: .top) {
                            Image(systemName: model.symbol(id)).frame(width: 18)
                            VStack(alignment: .leading, spacing: 1) {
                                Text(model.title(id))
                                Text(model.detail(id)).font(.caption).foregroundStyle(.secondary)
                            }
                            Spacer()
                            if model.catalog[id]?.tab ?? false {
                                Button { model.add(id, asTab: true) } label: { Image(systemName: "rectangle.badge.plus") }
                                    .help("Als eigener Reiter")
                            }
                            Button { model.add(id) } label: { Image(systemName: "plus.circle") }
                                .help("Hier hinzufügen")
                        }
                        .buttonStyle(.borderless)
                    }
                }
            }
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.accentColor.opacity(0.06), in: .rect(cornerRadius: 8))
    }
}
