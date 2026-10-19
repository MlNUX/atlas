// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

pragma Singleton
import QtQuick

// The colours of Atlas' Theme.qml for the controls style (the style cannot
// import the Atlas module without a cycle).
QtObject {
    readonly property color surface: "#111a2b"
    readonly property color popup: "#131d30"
    readonly property color field: "#0d1524"
    readonly property color raised: "#152036"
    readonly property color border: "#1e2a40"
    readonly property color borderStrong: "#2a3a57"
    readonly property color hover: "#18233a"
    readonly property color selection: "#1a2a45"
    readonly property color activeSelection: "#1d3a63"
    readonly property color accent: "#3d8bfd"
    readonly property color accentHover: "#5a9dfd"
    readonly property color accentDown: "#2f74dc"
    readonly property color text: "#c5d0e0"
    readonly property color textStrong: "#eef3fb"
    readonly property color textMuted: "#8a97ad"
    readonly property color textFaint: "#5c6981"
    readonly property color error: "#f2575f"
    readonly property color dim: Qt.rgba(0.02, 0.04, 0.08, 0.62)
    readonly property int fontSize: 13
    readonly property int radius: 8

    /// A line icon (24 grid, stroke paths) as image URL in `color`.
    function icon(paths, color) {
        return "data:image/svg+xml;utf8," + encodeURIComponent("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='"
            + color + "' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'>" + paths + "</svg>")
    }
}
