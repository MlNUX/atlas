// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AppKit
import AtlasKit
import SwiftUI

/// The window of one project: sidebar | editor area | context bar (inspector).
struct ProjectWindow: View {
    @Bindable var project: ProjectModel

    var body: some View {
        NavigationSplitView {
            FilesSidebar(project: project)
                .navigationSplitViewColumnWidth(min: 200, ideal: 260)
        } detail: {
            EditorAreaView(project: project)
        }
        .inspector(isPresented: $project.inspectorVisible) {
            ContextBar(project: project)
                .inspectorColumnWidth(min: 240, ideal: 300, max: 420)
        }
        .toolbar {
            ToolbarItem(placement: .status) {
                if !project.indexReady {
                    ProgressView().controlSize(.small).help(project.indexText)
                }
            }
            ToolbarItem {
                Button("Schnell öffnen", systemImage: "magnifyingglass") { project.quickOpenVisible = true }
                    .help("Datei schnell öffnen (⌘P)")
            }
            ToolbarItem {
                Button("Kontextleiste", systemImage: "sidebar.trailing") { project.inspectorVisible.toggle() }
            }
        }
        .sheet(isPresented: $project.quickOpenVisible) {
            QuickOpenView(project: project)
        }
        .sheet(isPresented: $project.searchVisible) {
            SearchWindowView(project: project)
        }
        .modifier(ActionsWindowModifier(project: project))
        .overlay(alignment: .bottom) {
            if !project.notice.isEmpty {
                Text(project.notice)
                    .padding(.horizontal, 14).padding(.vertical, 8)
                    .glassEffect()
                    .padding(.bottom, 14)
                    .transition(.opacity)
            }
        }
        .animation(.default, value: project.notice)
        .onDisappear { project.close() }
        .task {
            // CI: `-AtlasWindowSize 1280x780` fits the window onto the runner's screen.
            guard let size = UserDefaults.standard.string(forKey: "AtlasWindowSize")?.split(separator: "x"),
                  size.count == 2, let w = Double(size[0]), let h = Double(size[1]) else { return }
            try? await Task.sleep(for: .milliseconds(500))
            guard let window = NSApp.keyWindow ?? NSApp.windows.first(where: \.isVisible),
                  let screen = window.screen ?? NSScreen.main else { return }
            let top = screen.visibleFrame.maxY
            window.setFrame(NSRect(x: screen.visibleFrame.minX, y: top - h, width: w, height: h), display: true)
            // What limits the size (read by ci-app.sh).
            let info = "frame \(window.frame) minSize \(window.minSize) contentMinSize \(window.contentMinSize) "
                + "fitting \(window.contentView?.fittingSize ?? .zero) screen \(screen.visibleFrame)\n"
            try? info.write(toFile: "/tmp/atlas-window.txt", atomically: true, encoding: .utf8)
        }
    }
}

/// Splits with their tab bars.
struct EditorAreaView: View {
    let project: ProjectModel

    var body: some View {
        let editor = project.editor
        Group {
            if editor.orientation == .horizontal {
                HSplitView { groups(editor) }
            } else {
                VSplitView { groups(editor) }
            }
        }
    }

    @ViewBuilder
    private func groups(_ editor: EditorAreaModel) -> some View {
        ForEach(Array(editor.groups.enumerated()), id: \.element.id) { index, group in
            EditorGroupView(project: project, index: index, group: group, active: index == editor.active)
                .frame(minWidth: 240, minHeight: 160)
        }
    }
}

struct EditorGroupView: View {
    let project: ProjectModel
    let index: Int
    let group: EditorGroup
    let active: Bool

    var body: some View {
        VStack(spacing: 0) {
            TabStrip(project: project, index: index, group: group, active: active)
            Divider()
            if group.tabs.indices.contains(group.current) {
                FeatureRouter.view(for: group.tabs[group.current], project: project)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ContentUnavailableView("Keine Datei geöffnet", systemImage: "doc",
                                       description: Text("Datei in der Seitenleiste wählen oder ⌘P drücken"))
            }
        }
        .contentShape(Rectangle())
        .onTapGesture { project.editor.activate(group: index) }
    }
}

struct TabStrip: View {
    let project: ProjectModel
    let index: Int
    let group: EditorGroup
    let active: Bool

    private func tabFill(selected: Bool) -> Color {
        guard selected else { return .clear }
        return active ? Color.accentColor.opacity(0.18) : Color.secondary.opacity(0.15)
    }

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 2) {
                ForEach(Array(group.tabs.enumerated()), id: \.element.id) { i, tab in
                    let selected = i == group.current
                    HStack(spacing: 4) {
                        Text(FeatureRouter.title(for: tab))
                            .lineLimit(1)
                        Button {
                            project.editor.close(group: index, tab: i)
                        } label: {
                            Image(systemName: "xmark").font(.caption2)
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel("Tab schließen")
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(tabFill(selected: selected), in: .rect(cornerRadius: 6))
                    .onTapGesture { project.editor.select(group: index, tab: i) }
                }
            }
            .padding(.horizontal, 6)
            .padding(.vertical, 4)
        }
    }
}
