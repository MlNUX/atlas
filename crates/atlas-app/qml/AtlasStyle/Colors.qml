// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

pragma Singleton
import QtQuick

// The colours of Atlas' Theme.qml for the controls style (the style cannot
// import the Atlas module without a cycle).
QtObject {
    /// The light design; set by Theme.light.
    property bool light: false

    readonly property color surface: light ? "#eef1f6" : "#111a2b"
    readonly property color popup: light ? "#ffffff" : "#131d30"
    readonly property color field: light ? "#ffffff" : "#0d1524"
    readonly property color raised: light ? "#e9eef5" : "#152036"
    readonly property color border: light ? "#d6dde7" : "#1e2a40"
    readonly property color borderStrong: light ? "#b9c4d4" : "#2a3a57"
    readonly property color hover: light ? "#e3e9f2" : "#18233a"
    readonly property color selection: light ? "#dce7fa" : "#1a2a45"
    readonly property color activeSelection: light ? "#c8dbfa" : "#1d3a63"
    readonly property color accent: light ? "#2f6fe0" : "#3d8bfd"
    readonly property color accentHover: light ? "#4683ea" : "#5a9dfd"
    readonly property color accentDown: light ? "#235bc0" : "#2f74dc"
    readonly property color text: light ? "#283345" : "#c5d0e0"
    readonly property color textStrong: light ? "#0e1624" : "#eef3fb"
    readonly property color textMuted: light ? "#5a6577" : "#8a97ad"
    readonly property color textFaint: light ? "#8a94a5" : "#5c6981"
    readonly property color error: light ? "#d33a43" : "#f2575f"
    readonly property color dim: light ? Qt.rgba(0.1, 0.14, 0.22, 0.32) : Qt.rgba(0.02, 0.04, 0.08, 0.62)
    readonly property color tooltip: light ? "#ffffff" : "#1b2840"
    readonly property color scrollHandle: light ? "#c3ccd9" : "#26354f"
    readonly property color scrollHandleHover: light ? "#a7b3c5" : "#36496b"
    readonly property color scrollHandlePressed: light ? "#8b99ae" : "#4a6189"
    readonly property int fontSize: 13
    readonly property int radius: 8

    /// A line icon (24 grid, stroke paths) as image URL in `color`.
    function icon(paths, color) {
        return "data:image/svg+xml;utf8," + encodeURIComponent("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='"
            + color + "' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'>" + paths + "</svg>")
    }
}
