// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import AtlasKit
import SwiftUI

/// „Graph“ (P3.6; Linux: `context_graph`): the file in the middle, what it
/// links or quotes on the first ring, their neighbours on the second. A
/// click opens a file.
struct ContextGraphBlock: View {
    let project: ProjectModel
    let path: String?
    @Binding var empty: Bool
    @State private var graph: Graph?

    var body: some View {
        Group {
            if let graph, graph.nodes.count > 1 {
                GeometryReader { geo in
                    let places = Self.layout(graph, in: geo.size)
                    ZStack {
                        Canvas { context, _ in
                            for e in graph.edges {
                                guard let a = places[Int(e.from)], let b = places[Int(e.to)] else { continue }
                                var line = Path()
                                line.move(to: a)
                                line.addLine(to: b)
                                let style = e.related ? StrokeStyle(lineWidth: 1, dash: [3, 3]) : StrokeStyle(lineWidth: min(3, 0.8 + CGFloat(e.count) * 0.4))
                                context.stroke(line, with: .color(e.quotes > 0 ? .orange.opacity(0.6) : .secondary.opacity(0.5)), style: style)
                            }
                        }
                        ForEach(Array(graph.nodes.enumerated()), id: \.offset) { i, node in
                            if let p = places[i] {
                                Button {
                                    if !node.missing, node.depth > 0 { project.editor.open(path: node.path, mode: .tab) }
                                } label: {
                                    Text(node.title)
                                        .font(node.depth == 0 ? .caption.bold() : .caption2)
                                        .lineLimit(1)
                                        .frame(maxWidth: 110)
                                        .padding(.horizontal, 5).padding(.vertical, 2)
                                        .background(node.depth == 0 ? Color.accentColor.opacity(0.3) : Color(nsColor: .controlBackgroundColor),
                                                    in: .capsule)
                                        .overlay(Capsule().stroke(node.missing ? Color.orange : Color.secondary.opacity(0.4), lineWidth: 0.5))
                                }
                                .buttonStyle(.plain)
                                .help(node.path)
                                .position(p)
                            }
                        }
                    }
                }
                .frame(minHeight: 260)
            } else {
                Text("Keine verbundenen Dateien.").font(.caption).foregroundStyle(.secondary)
            }
        }
        .task(id: "\(path ?? "")#\(project.notesRevision)") {
            graph = path.map { project.core.notes().graph(path: $0, depth: 2) }
            empty = (graph?.nodes.count ?? 0) <= 1
        }
    }

    /// Positions by depth on rings around the middle, in node order.
    static func layout(_ graph: Graph, in size: CGSize) -> [Int: CGPoint] {
        let center = CGPoint(x: size.width / 2, y: size.height / 2)
        let radius = min(size.width, size.height) / 2 - 18
        var byDepth: [Int: [Int]] = [:]
        for (i, n) in graph.nodes.enumerated() where n.depth <= 2 { byDepth[Int(n.depth), default: []].append(i) }
        var out: [Int: CGPoint] = [:]
        for (depth, nodes) in byDepth {
            if depth == 0 {
                for i in nodes { out[i] = center }
                continue
            }
            let r = radius * (depth == 1 ? 0.55 : 1)
            let offset = depth == 2 ? Double.pi / Double(max(nodes.count, 1)) : 0
            for (k, i) in nodes.enumerated() {
                let angle = 2 * Double.pi * Double(k) / Double(nodes.count) - Double.pi / 2 + offset
                out[i] = CGPoint(x: center.x + r * cos(angle), y: center.y + r * sin(angle))
            }
        }
        return out
    }
}
