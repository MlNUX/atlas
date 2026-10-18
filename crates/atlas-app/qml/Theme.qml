// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later
pragma Singleton

import QtQuick

// Dark, calm, dense desktop palette (see docs/CONCEPT.md, preview 05/08).
QtObject {
    readonly property color window: "#181818"
    readonly property color sidebar: "#181818"
    readonly property color editor: "#1f1f1f"
    readonly property color panel: "#181818"
    readonly property color terminal: "#181818"
    readonly property color header: "#1f1f1f"
    readonly property color border: "#2b2b2b"
    readonly property color hover: "#2a2d2e"
    readonly property color selection: "#37373d"
    readonly property color activeSelection: "#04395e"
    readonly property color accent: "#0078d4"
    readonly property color text: "#cccccc"
    readonly property color textStrong: "#e7e7e7"
    readonly property color textMuted: "#9d9d9d"
    readonly property color textFaint: "#6e7681"
    readonly property color ok: "#2ea043"
    readonly property color warn: "#cca700"
    readonly property color error: "#f85149"
    readonly property color matchText: "#f0a35e"
    readonly property color pdfBackdrop: "#2b2b2b"

    readonly property string monoFont: "monospace"
    readonly property int fontSize: 13
    readonly property int smallFontSize: 11
    readonly property int rowHeight: 22
}
