// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

pragma Singleton
import QtQuick

// Colours and measures of the dark "navy" design: a dark window, panels as
// rounded cards with small gaps, blue accent.
QtObject {
    /// Window background between the cards.
    readonly property color window: "#0a101c"
    readonly property color sidebar: "#0d1422"
    /// Content of documents and cards.
    readonly property color editor: "#0f1726"
    readonly property color panel: "#0d1422"
    readonly property color terminal: "#0b111d"
    /// Tab bars and tool bars of cards.
    readonly property color header: "#111a2b"
    /// Raised elements on cards (inputs, list cards, chips).
    readonly property color raised: "#152036"
    readonly property color border: "#1e2a40"
    readonly property color borderStrong: "#2a3a57"
    readonly property color hover: "#18233a"
    readonly property color selection: "#1a2a45"
    readonly property color activeSelection: "#1d3a63"
    readonly property color accent: "#3d8bfd"
    readonly property color accentSoft: "#16305a"
    readonly property color text: "#c5d0e0"
    readonly property color textStrong: "#eef3fb"
    readonly property color textMuted: "#8a97ad"
    readonly property color textFaint: "#5c6981"
    readonly property color ok: "#34c27a"
    readonly property color warn: "#e0b33a"
    readonly property color error: "#f2575f"
    readonly property color matchText: "#f0a35e"
    readonly property color pdfBackdrop: "#1a2335"
    readonly property string monoFont: "monospace"
    readonly property int fontSize: 13
    readonly property int smallFontSize: 11
    readonly property int rowHeight: 26
    /// Corner radius of cards and gap between them.
    readonly property int radius: 10
    readonly property int gap: 8

    /// Colours of file kinds (icons, graph).
    readonly property var kindColors: ({
        pdf: "#f2575f", markdown: "#5aa2ff", text: "#9fb0c8", image: "#b98cf5", csv: "#34c27a",
        dir: "#5aa2ff", binary: "#8a97ad", missing: "#5c6981", terminal: "#8a97ad", atlasnote: "#a678ed"
    })

    /// Bytes as "12,3 GB" (binary units, one decimal from GB up).
    function formatBytes(n) {
        const units = ["B", "KB", "MB", "GB", "TB"]
        let i = 0
        while (n >= 1024 && i < units.length - 1) { n /= 1024; i++ }
        return (i >= 3 ? n.toFixed(1).replace(".", ",") : Math.round(n)) + " " + units[i]
    }

    /// Icon name and colour of a file: [icon, colour].
    function fileIcon(fileName, isDir) {
        if (isDir) return ["folder-fill", "#5aa2ff"]
        const ext = fileName.slice(fileName.lastIndexOf(".") + 1).toLowerCase()
        if (ext === "pdf") return ["file-pdf", kindColors.pdf]
        if (ext === "atlasnote" || fileName.toLowerCase().endsWith(".atlasnote.zip")) return ["pen", kindColors.atlasnote]
        if (["md", "markdown", "mdown", "mkd", "txt", "rst"].indexOf(ext) >= 0) return ["file-text", kindColors.markdown]
        if (["csv", "tsv", "xlsx", "ods"].indexOf(ext) >= 0) return ["file-table", kindColors.csv]
        if (["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"].indexOf(ext) >= 0) return ["file-image", kindColors.image]
        if (["rs", "py", "js", "ts", "c", "h", "cpp", "go", "java", "qml", "sh", "toml", "yaml", "yml", "json", "nix", "html", "css", "lock"].indexOf(ext) >= 0)
            return ["file-code", "#e0b33a"]
        return ["document", "#8a97ad"]
    }
}
