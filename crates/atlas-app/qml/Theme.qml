// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

pragma Singleton
import QtQuick
import AtlasStyle

// Colours and measures of Atlas' design: a window with panels as rounded
// cards and small gaps, blue accent – dark "navy" or light.
QtObject {
    /// The light design (set from `[appearance] theme` in Main.qml); the
    /// controls style follows it.
    property bool light: false
    onLightChanged: Colors.light = light

    /// Window background between the cards.
    readonly property color window: light ? "#e6eaf0" : "#0a101c"
    readonly property color sidebar: light ? "#f5f7fa" : "#0d1422"
    /// Content of documents and cards.
    readonly property color editor: light ? "#ffffff" : "#0f1726"
    readonly property color panel: light ? "#f5f7fa" : "#0d1422"
    readonly property color terminal: light ? "#ffffff" : "#0b111d"
    /// Tab bars and tool bars of cards.
    readonly property color header: light ? "#eef1f6" : "#111a2b"
    /// Raised elements on cards (inputs, list cards, chips).
    readonly property color raised: light ? "#e9eef5" : "#152036"
    readonly property color border: light ? "#d6dde7" : "#1e2a40"
    readonly property color borderStrong: light ? "#b9c4d4" : "#2a3a57"
    readonly property color hover: light ? "#e3e9f2" : "#18233a"
    readonly property color selection: light ? "#dce7fa" : "#1a2a45"
    readonly property color activeSelection: light ? "#c8dbfa" : "#1d3a63"
    readonly property color accent: light ? "#2f6fe0" : "#3d8bfd"
    readonly property color accentSoft: light ? "#dbe7fd" : "#16305a"
    readonly property color text: light ? "#283345" : "#c5d0e0"
    readonly property color textStrong: light ? "#0e1624" : "#eef3fb"
    readonly property color textMuted: light ? "#5a6577" : "#8a97ad"
    readonly property color textFaint: light ? "#8a94a5" : "#5c6981"
    readonly property color ok: light ? "#1e9455" : "#34c27a"
    readonly property color warn: light ? "#b07e12" : "#e0b33a"
    readonly property color error: light ? "#d33a43" : "#f2575f"
    readonly property color matchText: light ? "#c05f17" : "#f0a35e"
    /// Links, tags and wiki links in lists.
    readonly property color link: light ? "#1f5fbf" : "#9dd1ff"
    readonly property color pdfBackdrop: light ? "#d5dbe4" : "#1a2335"
    /// Popups that float over the window (command palette).
    readonly property color popup: light ? "#ffffff" : "#131d30"
    /// Marks of PDF quotes.
    readonly property color quote: light ? "#d9a400" : "#ffd400"
    /// Background of a day outside the shown month.
    readonly property color shade: light ? Qt.rgba(0.1, 0.15, 0.25, 0.05) : Qt.rgba(0, 0, 0, 0.18)
    /// Text on calendar entries (filled with `entryFill`).
    readonly property color entryText: light ? textStrong : "white"
    readonly property color entryTextMuted: light ? Qt.rgba(0.05, 0.09, 0.14, 0.7) : Qt.rgba(1, 1, 1, 0.75)
    /// Darkens the window behind modal popups.
    readonly property color dim: light ? Qt.rgba(0.1, 0.14, 0.22, 0.32) : Qt.rgba(0.02, 0.04, 0.08, 0.6)
    readonly property string monoFont: "monospace"
    readonly property int fontSize: 13
    readonly property int smallFontSize: 11
    readonly property int rowHeight: 26
    /// Corner radius of cards and gap between them.
    readonly property int radius: 10
    readonly property int gap: 8

    /// Colours of file kinds (icons, graph).
    readonly property var kindColors: light ? lightKindColors : darkKindColors
    readonly property var darkKindColors: ({
        pdf: "#f2575f", markdown: "#5aa2ff", text: "#9fb0c8", image: "#b98cf5", csv: "#34c27a",
        dir: "#5aa2ff", binary: "#8a97ad", missing: "#5c6981", terminal: "#8a97ad", code: "#e0b33a"
    })
    readonly property var lightKindColors: ({
        pdf: "#d33a43", markdown: "#2f6fe0", text: "#5f6f88", image: "#8a55d8", csv: "#1e9455",
        dir: "#3b82f0", binary: "#5a6577", missing: "#8a94a5", terminal: "#5a6577", code: "#b07e12"
    })

    /// Fill of a calendar entry in colour `c`: darkened by `factor` in the
    /// dark design, a pale tint of it in the light one.
    function entryFill(c, factor) {
        return light ? Qt.tint(c, Qt.rgba(1, 1, 1, factor >= 2 ? 0.78 : 0.5)) : Qt.darker(c, factor)
    }

    /// Bytes as "12,3 GB" (binary units, one decimal from GB up).
    function formatBytes(n) {
        const units = ["B", "KB", "MB", "GB", "TB"]
        let i = 0
        while (n >= 1024 && i < units.length - 1) { n /= 1024; i++ }
        return (i >= 3 ? n.toFixed(1).replace(".", ",") : Math.round(n)) + " " + units[i]
    }

    /// Icon name and colour of a file: [icon, colour].
    function fileIcon(fileName, isDir) {
        if (isDir) return ["folder-fill", kindColors.dir]
        const ext = fileName.slice(fileName.lastIndexOf(".") + 1).toLowerCase()
        if (ext === "pdf") return ["file-pdf", kindColors.pdf]
        if (["md", "markdown", "mdown", "mkd", "txt", "rst"].indexOf(ext) >= 0) return ["file-text", kindColors.markdown]
        if (["csv", "tsv", "xlsx", "ods"].indexOf(ext) >= 0) return ["file-table", kindColors.csv]
        if (["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"].indexOf(ext) >= 0) return ["file-image", kindColors.image]
        if (["rs", "py", "js", "ts", "c", "h", "cpp", "go", "java", "qml", "sh", "toml", "yaml", "yml", "json", "nix", "html", "css", "lock"].indexOf(ext) >= 0)
            return ["file-code", kindColors.code]
        return ["document", kindColors.binary]
    }
}
