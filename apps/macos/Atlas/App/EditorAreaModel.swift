// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import Foundation
import Observation

/// Tabs and splits of the editor area (up to four splits, as on Linux).
@MainActor
@Observable
final class EditorAreaModel {
    static let maxGroups = 4

    enum OpenMode {
        /// A tab in the active split (an existing tab of the file is reused).
        case tab
        /// A new split to the right of / below the active one.
        case split
    }

    private(set) var layout: SessionLayout
    /// Called after every change (the project saves the session).
    var onChange: (() -> Void)?

    init(layout: SessionLayout) {
        self.layout = layout
    }

    var groups: [EditorGroup] { layout.groups }
    var active: Int { layout.active }
    var orientation: SplitOrientation { layout.orientation }

    /// The tab shown in the active split, if any.
    var currentTab: EditorTab? {
        let g = layout.groups[layout.active]
        return g.tabs.indices.contains(g.current) ? g.tabs[g.current] : nil
    }

    /// Opens `path`; with `processor` (an extension's view or processor) in
    /// a tab of its own next to the plain one.
    func open(path: String, line: Int = 0, mode: OpenMode = .tab, processor: String = "") {
        var new = EditorTab(path: path, line: line)
        if !processor.isEmpty { new.extra["processor"] = .string(processor) }
        if mode == .split, layout.groups.count < Self.maxGroups, !layout.groups[layout.active].tabs.isEmpty {
            layout.groups.insert(EditorGroup(tabs: [new]), at: layout.active + 1)
            layout.active += 1
            changed()
            return
        }
        var g = layout.groups[layout.active]
        if let i = g.tabs.firstIndex(where: { $0.path == path && $0.processor == processor }) {
            g.current = i
            if line > 0 { g.tabs[i].line = line }
        } else {
            g.tabs.insert(new, at: g.tabs.isEmpty ? 0 : g.current + 1)
            g.current = g.tabs.count == 1 ? 0 : g.current + 1
        }
        layout.groups[layout.active] = g
        changed()
    }

    func select(group: Int, tab: Int) {
        guard layout.groups.indices.contains(group), layout.groups[group].tabs.indices.contains(tab) else { return }
        layout.active = group
        layout.groups[group].current = tab
        changed()
    }

    func activate(group: Int) {
        guard layout.groups.indices.contains(group), layout.active != group else { return }
        layout.active = group
        changed()
    }

    /// Closes a tab; an empty split closes too (the last one stays).
    func close(group: Int, tab: Int) {
        guard layout.groups.indices.contains(group), layout.groups[group].tabs.indices.contains(tab) else { return }
        layout.groups[group].tabs.remove(at: tab)
        let g = layout.groups[group]
        if g.tabs.isEmpty, layout.groups.count > 1 {
            layout.groups.remove(at: group)
            layout.active = min(layout.active, layout.groups.count - 1)
        } else {
            layout.groups[group].current = min(g.current, max(0, g.tabs.count - 1))
        }
        changed()
    }

    func closeCurrentTab() {
        close(group: layout.active, tab: layout.groups[layout.active].current)
    }

    /// Moves the current tab of the active split into a new split.
    func splitCurrentTab() {
        guard let tab = currentTab, layout.groups.count < Self.maxGroups else { return }
        layout.groups[layout.active].tabs.remove(at: layout.groups[layout.active].current)
        layout.groups[layout.active].current = max(0, layout.groups[layout.active].current - 1)
        layout.groups.insert(EditorGroup(tabs: [tab]), at: layout.active + 1)
        if layout.groups[layout.active].tabs.isEmpty {
            layout.groups.remove(at: layout.active)
        } else {
            layout.active += 1
        }
        changed()
    }

    func toggleOrientation() {
        layout.orientation = layout.orientation == .horizontal ? .vertical : .horizontal
        changed()
    }

    /// Tabs follow files and folders that were moved or renamed.
    func pathMoved(from: String, to: String) {
        for g in layout.groups.indices {
            for t in layout.groups[g].tabs.indices {
                let p = layout.groups[g].tabs[t].path
                if p == from { layout.groups[g].tabs[t].path = to }
                else if p.hasPrefix(from + "/") { layout.groups[g].tabs[t].path = to + p.dropFirst(from.count) }
            }
        }
        changed()
    }

    private func changed() {
        onChange?()
    }
}
