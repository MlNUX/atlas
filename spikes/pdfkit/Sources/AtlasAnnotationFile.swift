// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import Foundation

/// `.atlas/annotations/<name>-<hash>.json` as written by `atlas-core`
/// (`crates/atlas-core/src/annotations.rs`, format version 1).
/// Later this type comes from the generated AtlasKit bindings; the spike
/// decodes the JSON directly.
struct AtlasAnnotationFile: Codable, Sendable {
    var version: Int
    var document: DocumentRef
    var annotations: [AtlasAnnotation]

    struct DocumentRef: Codable, Sendable {
        var path: String
        var blake3: String?
        var size: Int?
        var pages: Int?
        var pdf_id: String?
    }

    static func load(_ url: URL) throws -> AtlasAnnotationFile {
        try JSONDecoder().decode(AtlasAnnotationFile.self, from: Data(contentsOf: url))
    }
}

struct AtlasAnnotation: Codable, Sendable, Identifiable {
    enum Kind: String, Codable, Sendable {
        case highlight, underline, strikeout, note, ink, bookmark

        var isTextMarkup: Bool { self == .highlight || self == .underline || self == .strikeout }
    }

    var id: String
    var kind: Kind
    /// 1-based.
    var page: Int
    /// `[x, y, width, height]` in points, origin top left (see `PageGeometry`).
    var rects: [[Double]]?
    var text: String?
    var comment: String?
    /// `#rrggbb`
    var color: String?
    /// Strokes of `[x, y]` points.
    var ink: [[[Double]]]?
    var width: Double?
    var title: String?
    var group: String?
    var created: Int64
    var modified: Int64
}
