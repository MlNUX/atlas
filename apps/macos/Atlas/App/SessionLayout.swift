// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import Foundation

/// Any JSON value, kept as it is (unknown keys of the Linux layout survive
/// a round trip through the Mac app, E12).
enum JSONValue: Codable, Equatable, Sendable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSONValue])
    case object([String: JSONValue])

    init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() { self = .null }
        else if let b = try? c.decode(Bool.self) { self = .bool(b) }
        else if let n = try? c.decode(Double.self) { self = .number(n) }
        else if let s = try? c.decode(String.self) { self = .string(s) }
        else if let a = try? c.decode([JSONValue].self) { self = .array(a) }
        else { self = .object(try c.decode([String: JSONValue].self)) }
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .null: try c.encodeNil()
        case .bool(let b): try c.encode(b)
        case .number(let n):
            if n.rounded() == n, abs(n) < 1e15 { try c.encode(Int64(n)) } else { try c.encode(n) }
        case .string(let s): try c.encode(s)
        case .array(let a): try c.encode(a)
        case .object(let o): try c.encode(o)
        }
    }

    subscript(key: String) -> JSONValue? {
        if case .object(let o) = self { return o[key] }
        return nil
    }

    var string: String? { if case .string(let s) = self { s } else { nil } }
    var int: Int? { if case .number(let n) = self { Int(n) } else { nil } }
    var bool: Bool? { if case .bool(let b) = self { b } else { nil } }
    var array: [JSONValue]? { if case .array(let a) = self { a } else { nil } }
    var object: [String: JSONValue]? { if case .object(let o) = self { o } else { nil } }
}

/// One tab: a file (absolute path) or a tab that is no file
/// (`terminal:<dir>#<n>`, `calendar:`, `ilias:`).
struct EditorTab: Identifiable, Equatable, Sendable {
    let id = UUID()
    var path: String
    var line: Int = 0
    var preview = false
    /// The viewer's state and keys this app does not know.
    var extra: [String: JSONValue] = [:]

    static func == (a: EditorTab, b: EditorTab) -> Bool {
        a.path == b.path && a.line == b.line && a.preview == b.preview && a.extra == b.extra
    }

    /// Shown through an extension: `view:<ext>/<id>` or a processor id
    /// (Linux: `processor` in the tab state).
    var processor: String { extra["processor"]?.string ?? "" }

    var isVirtual: Bool {
        path.hasPrefix("terminal:") || path.hasPrefix("calendar:") || path.hasPrefix("ilias:")
    }
}

/// A split with its tabs.
struct EditorGroup: Identifiable, Equatable, Sendable {
    let id = UUID()
    var tabs: [EditorTab] = []
    var current = 0
    var temporary = false
    var extra: [String: JSONValue] = [:]

    static func == (a: EditorGroup, b: EditorGroup) -> Bool {
        a.tabs == b.tabs && a.current == b.current && a.temporary == b.temporary && a.extra == b.extra
    }
}

enum SplitOrientation: String, Sendable {
    case horizontal, vertical
}

/// The session layout (`layout` in `.atlas/sessions/*.json`): tabs and
/// splits of the editor area plus everything else the Linux app stores
/// (terminal panel, sidebar …), kept untouched.
struct SessionLayout: Equatable, Sendable {
    var groups: [EditorGroup] = [EditorGroup()]
    var active = 0
    var orientation: SplitOrientation = .horizontal
    var editorExtra: [String: JSONValue] = [:]
    var extra: [String: JSONValue] = [:]

    static let version = 2

    /// Reads the core's layout JSON (paths relative to `root`); empty or
    /// broken JSON gives one empty split.
    init(json: String, root: String) {
        guard let data = json.data(using: .utf8),
              let value = try? JSONDecoder().decode(JSONValue.self, from: data),
              var top = value.object
        else { return }
        var editor = top.removeValue(forKey: "editor")?.object ?? [:]
        top.removeValue(forKey: "version")
        extra = top
        orientation = editor.removeValue(forKey: "orientation")?.string == "vertical" ? .vertical : .horizontal
        let active = editor.removeValue(forKey: "active")?.int ?? 0
        var groupValues = editor.removeValue(forKey: "groups")?.array ?? []
        // Phase 0 layouts had one document per split ("panes").
        if let panes = editor.removeValue(forKey: "panes")?.array, groupValues.isEmpty {
            groupValues = panes.map { .object(["tabs": .array([$0]), "current": .number(0)]) }
        }
        editorExtra = editor
        groups = groupValues.compactMap { $0.object }.map { g in
            var g = g
            let tabs = (g.removeValue(forKey: "tabs")?.array ?? []).compactMap { $0.object }.map { t in
                var t = t
                let path = t.removeValue(forKey: "path")?.string ?? ""
                return EditorTab(
                    path: Self.absolute(path, root: root),
                    line: t.removeValue(forKey: "line")?.int ?? 0,
                    preview: t.removeValue(forKey: "preview")?.bool ?? false,
                    extra: t)
            }.filter { !$0.path.isEmpty }
            let current = g.removeValue(forKey: "current")?.int ?? 0
            let temporary = g.removeValue(forKey: "temporary")?.bool ?? false
            return EditorGroup(tabs: tabs, current: max(0, min(current, tabs.count - 1)), temporary: temporary, extra: g)
        }
        if groups.isEmpty { groups = [EditorGroup()] }
        self.active = max(0, min(active, groups.count - 1))
    }

    init() {}

    /// The layout JSON for the core (paths relative to `root`).
    func json(root: String) -> String {
        var editor = editorExtra
        editor["orientation"] = .string(orientation.rawValue)
        editor["active"] = .number(Double(active))
        editor["groups"] = .array(groups.map { g in
            var o = g.extra
            o["current"] = .number(Double(g.current))
            o["temporary"] = .bool(g.temporary)
            o["tabs"] = .array(g.tabs.map { t in
                var o = t.extra
                o["path"] = .string(Self.relative(t.path, root: root))
                o["line"] = .number(Double(t.line))
                o["preview"] = .bool(t.preview)
                return .object(o)
            })
            return .object(o)
        })
        var top = extra
        top["version"] = .number(Double(Self.version))
        top["editor"] = .object(editor)
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        return (try? encoder.encode(JSONValue.object(top))).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
    }

    /// Files the layout refers to (absolute; tabs that are no files left out).
    func documents(root: String) -> [String] {
        groups.flatMap { $0.tabs }.filter { !$0.isVirtual }.map(\.path)
    }

    static func absolute(_ path: String, root: String) -> String {
        if path.isEmpty || path.hasPrefix("/") || path.hasPrefix("terminal:") || path.hasPrefix("calendar:")
            || path.hasPrefix("ilias:") { return path }
        return (root as NSString).appendingPathComponent(path)
    }

    static func relative(_ path: String, root: String) -> String {
        let prefix = root.hasSuffix("/") ? root : root + "/"
        return path.hasPrefix(prefix) ? String(path.dropFirst(prefix.count)) : path
    }
}
