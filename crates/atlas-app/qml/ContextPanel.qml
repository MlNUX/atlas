// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Context of a Markdown file or PDF: notes linking to it (filterable by
// text, kind of link, tag and folder) and a local graph of its neighbours –
// notes, PDFs and other files – with quotes of PDF annotations highlighted.
Rectangle {
    id: panel
    required property AppController app
    required property string path
    /// 0 = backlinks, 1 = graph.
    property int tab: 0
    /// Own tab bar (off when a surrounding panel switches the tab).
    property bool showTabs: true
    /// Graph only, without depth switch and legend (overview cards).
    property bool compact: false
    property int depth: 1
    /// Backlinks: "file" (links to `path`) or "project" (all marked text of
    /// the project's PDFs and where notes quote it, by PDF).
    property string scope: "file"
    readonly property bool projectScope: scope === "project"
    /// The list of backlinks (for tests).
    readonly property alias backlinkView: backList
    property var backlinkList: []
    property var graph: ({ nodes: [], edges: [], truncated: 0 })
    readonly property int count: backlinkList.length

    /// `newPane` for Ctrl/middle click; `state` for the viewer.
    signal openRequested(string path, int line, bool newPane, var state)

    color: Theme.sidebar

    function refresh() {
        // (`scope`, not `projectScope`: this also runs from onScopeChanged.)
        if (scope === "project" && tab === 0) { backlinkList = JSON.parse(app.projectQuotes()); return }
        if (!path) { backlinkList = []; graph = ({ nodes: [], edges: [], truncated: 0 }); return }
        backlinkList = JSON.parse(app.backlinks(path))
        if (visible && tab === 1) graph = JSON.parse(app.contextGraph(path, depth))
    }
    onPathChanged: refresh()
    onTabChanged: refresh()
    onDepthChanged: refresh()
    onScopeChanged: refresh()
    onVisibleChanged: if (visible) refresh()
    Component.onCompleted: refresh()
    Connections {
        target: panel.app
        function onNotesRevisionChanged() { refreshTimer.restart() }
        // New or changed marks in any PDF of the project.
        function onAnnotationsRevisionChanged() { if (panel.scope === "project") refreshTimer.restart() }
    }
    Timer { id: refreshTimer; interval: 200; onTriggered: panel.refresh() }

    // ------------------------------------------------------------ backlinks

    property string filterText: ""
    /// "", "wiki", "link", "quote", "page", "heading", "image"
    property string filterKind: ""
    property string filterTag: ""
    property string filterFolder: ""

    readonly property var tagChoices: {
        const seen = {}
        for (const b of backlinkList) for (const t of b.tags || []) seen[t.toLowerCase()] = t
        return Object.keys(seen).sort().map(k => seen[k])
    }
    readonly property var folderChoices: {
        const seen = {}
        for (const b of backlinkList) {
            const i = b.relative.lastIndexOf("/")
            seen[i > 0 ? b.relative.slice(0, i) : "/"] = true
        }
        return Object.keys(seen).sort()
    }

    function matches(b) {
        const k = projectScope ? "" : filterKind
        if (k === "wiki" && b.kind !== "wiki") return false
        if (k === "link" && (b.kind === "wiki" || b.kind === "image")) return false
        if (k === "image" && b.kind !== "image") return false
        if ((k === "quote" || k === "page" || k === "heading") && b.anchorKind !== (k === "page" ? "page" : k)) return false
        if (filterTag) {
            const want = filterTag.toLowerCase()
            if (!(b.tags || []).some(t => t.toLowerCase() === want || t.toLowerCase().startsWith(want + "/"))) return false
        }
        if (filterFolder) {
            const i = b.relative.lastIndexOf("/")
            if ((i > 0 ? b.relative.slice(0, i) : "/") !== filterFolder) return false
        }
        if (filterText) {
            const q = filterText.toLowerCase()
            const hay = (b.relative + " " + (b.title || "") + " " + b.context + " " + (b.quote || "") + " " + (b.targetRelative || "")).toLowerCase()
            if (q.split(/\s+/).some(w => w && hay.indexOf(w) < 0)) return false
        }
        return true
    }
    /// Header row per source note (project quotes: per PDF), then its links.
    readonly property var rows: {
        const out = []
        let last = ""
        for (const b of backlinkList) {
            if (!matches(b)) continue
            const key = projectScope ? b.target : b.source
            if (key !== last) {
                out.push(projectScope
                    ? { header: true, source: b.target, relative: b.targetRelative, title: app.fileName(b.target), tags: [], pdf: true }
                    : { header: true, source: b.source, relative: b.relative, title: b.title || "", tags: b.tags || [] })
                last = key
            }
            out.push(Object.assign({ header: false }, b))
        }
        return out
    }
    /// PDFs among the project quotes (for the summary).
    readonly property int quotedPdfs: projectScope ? new Set(backlinkList.map(b => b.target)).size : 0
    readonly property int shownCount: rows.filter(r => !r.header).length

    // ------------------------------------------------------------- graph

    readonly property var kindColors: Theme.kindColors

    /// Radial layout: the file in the middle, direct neighbours on a ring,
    /// second-degree neighbours outside, near the neighbour they hang on.
    function layout(w, h) {
        const nodes = graph.nodes
        const pos = new Array(nodes.length)
        if (!nodes.length) return pos
        const cx = w / 2, cy = h / 2
        const r1 = Math.max(60, Math.min(w, h) * 0.30)
        const r2 = Math.min(Math.min(w, h) * 0.47, r1 * 1.65)
        pos[0] = { x: cx, y: cy, angle: 0 }
        const ring1 = []
        for (let i = 1; i < nodes.length; ++i) if (nodes[i].depth === 1) ring1.push(i)
        ring1.forEach((n, k) => {
            const a = -Math.PI / 2 + 2 * Math.PI * k / ring1.length
            pos[n] = { x: cx + r1 * Math.cos(a), y: cy + r1 * Math.sin(a), angle: a }
        })
        // Children of each ring-1 node.
        const children = {}
        for (let i = 1; i < nodes.length; ++i) {
            if (nodes[i].depth !== 2) continue
            const e = graph.edges.find(e => (e.from === i && nodes[e.to].depth === 1) || (e.to === i && nodes[e.from].depth === 1))
            const parent = e ? (e.from === i ? e.to : e.from) : ring1[0]
            ;(children[parent] = children[parent] || []).push(i)
        }
        const spread = ring1.length > 0 ? Math.min(Math.PI / 3, 2 * Math.PI / ring1.length) : Math.PI
        for (const p in children) {
            const list = children[p]
            const base = pos[p] ? pos[p].angle : 0
            list.forEach((n, k) => {
                const a = base + (list.length === 1 ? 0 : spread * (k / (list.length - 1) - 0.5))
                pos[n] = { x: cx + r2 * Math.cos(a), y: cy + r2 * Math.sin(a), angle: a }
            })
        }
        return pos
    }
    property var positions: []
    function relayout() { positions = layout(graphArea.width, graphArea.height); edgeCanvas.requestPaint() }
    onGraphChanged: relayout()

    // ---------------------------------------------------------------- view

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        TabBar {
            id: tabs
            visible: panel.showTabs
            Layout.fillWidth: true
            currentIndex: panel.tab
            onCurrentIndexChanged: panel.tab = currentIndex
            background: Rectangle { color: Theme.sidebar }
            TabButton { text: qsTr("Rückverweise %1").arg(panel.count); font.pixelSize: Theme.smallFontSize }
            TabButton { text: qsTr("Graph"); font.pixelSize: Theme.smallFontSize }
        }
        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: tabs.currentIndex

            // ---- backlinks with filters
            ColumnLayout {
                spacing: 4
                Row {
                    id: scopeSwitch
                    Layout.leftMargin: 6
                    Layout.topMargin: 6
                    spacing: 4
                    Repeater {
                        model: [{ scope: "file", text: qsTr("Dieses Dokument") }, { scope: "project", text: qsTr("Zitate im Projekt") }]
                        Rectangle {
                            id: scopeButton
                            required property var modelData
                            readonly property bool current: panel.scope === modelData.scope
                            width: scopeLabel.implicitWidth + 18
                            height: 24
                            radius: 6
                            color: current ? Theme.accentSoft : scopeHover.hovered ? Theme.hover : "transparent"
                            border.color: current ? Qt.rgba(0.24, 0.55, 0.99, 0.5) : Theme.border
                            Text {
                                id: scopeLabel
                                anchors.centerIn: parent
                                text: scopeButton.modelData.text
                                color: scopeButton.current ? Theme.textStrong : Theme.textMuted
                                font.pixelSize: Theme.smallFontSize
                            }
                            HoverHandler { id: scopeHover }
                            MouseArea { anchors.fill: parent; onClicked: panel.scope = scopeButton.modelData.scope }
                        }
                    }
                }
                TextField {
                    Layout.fillWidth: true
                    Layout.leftMargin: 6
                    Layout.rightMargin: 6
                    Layout.topMargin: 6
                    implicitHeight: 24
                    font.pixelSize: Theme.smallFontSize
                    placeholderText: qsTr("Filtern …")
                    onTextChanged: panel.filterText = text.trim()
                }
                // (Layouts fill the height by default: with every filter hidden
                // this row took the list's space.)
                RowLayout {
                    visible: !panel.projectScope || panel.tagChoices.length > 0 || panel.folderChoices.length > 1
                    Layout.fillWidth: true
                    Layout.fillHeight: false
                    Layout.leftMargin: 6
                    Layout.rightMargin: 6
                    spacing: 4
                    ComboBox {
                        Layout.fillWidth: true
                        visible: !panel.projectScope
                        implicitHeight: 24
                        font.pixelSize: Theme.smallFontSize
                        textRole: "text"
                        valueRole: "value"
                        model: [
                            { text: qsTr("Alle Arten"), value: "" },
                            { text: qsTr("Wiki-Links"), value: "wiki" },
                            { text: qsTr("Markdown-Links"), value: "link" },
                            { text: qsTr("Zitate"), value: "quote" },
                            { text: qsTr("Mit Seite"), value: "page" },
                            { text: qsTr("Auf Überschrift"), value: "heading" },
                            { text: qsTr("Bilder"), value: "image" },
                        ]
                        onActivated: panel.filterKind = currentValue
                    }
                    ComboBox {
                        Layout.fillWidth: true
                        implicitHeight: 24
                        font.pixelSize: Theme.smallFontSize
                        visible: panel.tagChoices.length > 0
                        model: [qsTr("Alle Tags")].concat(panel.tagChoices.map(t => "#" + t))
                        onActivated: (index) => panel.filterTag = index > 0 ? panel.tagChoices[index - 1] : ""
                    }
                    ComboBox {
                        Layout.fillWidth: true
                        implicitHeight: 24
                        font.pixelSize: Theme.smallFontSize
                        visible: panel.folderChoices.length > 1
                        model: [qsTr("Alle Ordner")].concat(panel.folderChoices)
                        onActivated: (index) => panel.filterFolder = index > 0 ? panel.folderChoices[index - 1] : ""
                    }
                }
                ListView {
                    id: backList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: panel.rows
                    ScrollBar.vertical: ScrollBar {}
                    delegate: Rectangle {
                        id: row
                        required property var modelData
                        width: ListView.view.width
                        height: rowColumn.implicitHeight + (modelData.header ? 10 : 8)
                        color: !modelData.header && rowHover.hovered ? Theme.hover : "transparent"
                        HoverHandler { id: rowHover }
                        ColumnLayout {
                            id: rowColumn
                            x: row.modelData.header ? 8 : 18
                            y: row.modelData.header ? 6 : 4
                            width: parent.width - x - 8
                            spacing: 1
                            Label {
                                visible: row.modelData.header
                                Layout.fillWidth: true
                                text: row.modelData.title || panel.app.fileName(row.modelData.source || "")
                                color: Theme.textStrong
                                font.bold: true
                                font.pixelSize: Theme.smallFontSize
                                elide: Text.ElideRight
                            }
                            Label {
                                visible: row.modelData.header
                                Layout.fillWidth: true
                                text: row.modelData.relative + ((row.modelData.tags || []).length ? "  ·  #" + row.modelData.tags.join(" #") : "")
                                color: Theme.textFaint
                                font.pixelSize: Theme.smallFontSize
                                elide: Text.ElideMiddle
                            }
                            RowLayout {
                                visible: !row.modelData.header
                                Layout.fillWidth: true
                                spacing: 6
                                Label {
                                    visible: !panel.projectScope
                                    text: qsTr("Z. %1").arg(row.modelData.line || 0)
                                    color: Theme.textFaint
                                    font.pixelSize: Theme.smallFontSize
                                }
                                Label {
                                    visible: !!row.modelData.anchor
                                    text: row.modelData.anchor || ""
                                    color: row.modelData.anchorKind === "quote" ? "#ffd400" : "#9dd1ff"
                                    font.pixelSize: Theme.smallFontSize
                                }
                                Label {
                                    visible: row.modelData.kind === "wiki" || row.modelData.kind === "image"
                                    text: row.modelData.kind === "wiki" ? "[[ ]]" : "🖼"
                                    color: Theme.textMuted
                                    font.pixelSize: Theme.smallFontSize
                                }
                                Item { Layout.fillWidth: true }
                            }
                            // Quotes show the quoted text (gold bar), other links
                            // their line.
                            Item {
                                visible: !row.modelData.header
                                Layout.fillWidth: true
                                implicitHeight: contextLabel.implicitHeight + (row.modelData.quote ? 4 : 0)
                                Rectangle {
                                    visible: !!row.modelData.quote
                                    width: 2
                                    height: parent.height
                                    radius: 1
                                    color: row.modelData.color || "#ffd400"
                                }
                                Label {
                                    id: contextLabel
                                    x: row.modelData.quote ? 8 : 0
                                    y: row.modelData.quote ? 2 : 0
                                    width: parent.width - x
                                    text: row.modelData.quote ? "„" + row.modelData.quote + "“" : (row.modelData.context || "")
                                    color: row.modelData.quote ? Theme.textStrong : Theme.text
                                    font.pixelSize: Theme.smallFontSize
                                    font.italic: !!row.modelData.quote
                                    wrapMode: Text.Wrap
                                    maximumLineCount: row.modelData.quote ? 4 : 3
                                    elide: Text.ElideRight
                                }
                            }
                            // Project quotes: comment and where notes quote the mark.
                            Label {
                                visible: panel.projectScope && !row.modelData.header && !!row.modelData.comment
                                Layout.fillWidth: true
                                text: "💬 " + (row.modelData.comment || "")
                                color: Theme.text
                                font.pixelSize: Theme.smallFontSize
                                wrapMode: Text.Wrap
                                maximumLineCount: 2
                                elide: Text.ElideRight
                            }
                            Label {
                                visible: panel.projectScope && !row.modelData.header
                                Layout.fillWidth: true
                                readonly property var cited: row.modelData.cited || []
                                text: cited.length
                                    ? qsTr("zitiert in %1").arg(cited.map(c => c.relative + " (Z. " + c.line + ")").join(", "))
                                    : qsTr("noch in keiner Notiz zitiert")
                                color: cited.length ? Theme.textMuted : Theme.textFaint
                                font.pixelSize: Theme.smallFontSize
                                elide: Text.ElideRight
                            }
                        }
                        MouseArea {
                            anchors.fill: parent
                            acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                            onClicked: (mouse) => {
                                const newPane = mouse.button === Qt.MiddleButton || (mouse.modifiers & Qt.ControlModifier)
                                const d = row.modelData
                                // Project quotes: to the marked passage in the PDF.
                                if (panel.projectScope && !d.header) panel.openRequested(d.target, d.page, newPane, { annot: d.annot })
                                else panel.openRequested(d.source, d.header ? 0 : d.line, newPane, {})
                            }
                        }
                        // Project quotes: the note quoting the mark.
                        IconButton {
                            visible: panel.projectScope && !row.modelData.header && (row.modelData.cited || []).length > 0
                                && (rowHover.hovered || hovered)
                            anchors.right: parent.right
                            anchors.rightMargin: 6
                            y: 2
                            implicitHeight: 22
                            glyph: qsTr("Notiz ↗")
                            tip: qsTr("%1 an der Stelle des Zitats öffnen").arg(((row.modelData.cited || [])[0] || {}).relative || "")
                            onClicked: {
                                const c = row.modelData.cited[0]
                                panel.openRequested(c.source, c.line, false, {})
                            }
                        }
                    }
                    Label {
                        anchors.centerIn: parent
                        width: parent.width - 24
                        visible: backList.count === 0
                        wrapMode: Text.Wrap
                        horizontalAlignment: Text.AlignHCenter
                        color: Theme.textFaint
                        font.pixelSize: Theme.smallFontSize
                        text: panel.projectScope
                            ? (panel.count === 0 ? qsTr("Noch keine Zitate im Projekt.\nMarkiere Text in einer PDF – die Markierung erscheint hier.")
                                                 : qsTr("Kein Zitat passt zum Filter."))
                            : panel.count === 0 ? qsTr("Keine Notiz verweist auf diese Datei.") : qsTr("Kein Rückverweis passt zum Filter.")
                    }
                }
                Label {
                    Layout.fillWidth: true
                    Layout.margins: 6
                    visible: panel.count > 0 && (panel.shownCount !== panel.count || panel.projectScope)
                    text: (panel.projectScope ? qsTr("%1 Zitate aus %2 PDFs · %3 in Notizen").arg(panel.count).arg(panel.quotedPdfs)
                                                  .arg(panel.backlinkList.filter(b => (b.cited || []).length).length) + (panel.shownCount !== panel.count ? " · " : "") : "")
                        + (panel.shownCount !== panel.count ? qsTr("%1 von %2 angezeigt").arg(panel.shownCount).arg(panel.count) : "")
                    color: Theme.textFaint
                    font.pixelSize: Theme.smallFontSize
                }
            }

            // ---- local graph
            ColumnLayout {
                spacing: 0
                RowLayout {
                    visible: !panel.compact
                    Layout.fillWidth: true
                    Layout.margins: 6
                    spacing: 4
                    IconButton { glyph: "1"; tip: qsTr("Direkte Nachbarn"); checkable: true; checked: panel.depth === 1; onClicked: panel.depth = 1 }
                    IconButton { glyph: "2"; tip: qsTr("Auch Nachbarn der Nachbarn"); checkable: true; checked: panel.depth === 2; onClicked: panel.depth = 2 }
                    Item { Layout.fillWidth: true }
                    Label {
                        text: qsTr("%n Datei(en)", "", Math.max(0, panel.graph.nodes.length - 1))
                            + (panel.graph.truncated ? qsTr(" · %1 ausgelassen").arg(panel.graph.truncated) : "")
                        color: Theme.textFaint
                        font.pixelSize: Theme.smallFontSize
                    }
                }
                Item {
                    id: graphArea
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    onWidthChanged: panel.relayout()
                    onHeightChanged: panel.relayout()

                    Canvas {
                        id: edgeCanvas
                        anchors.fill: parent
                        onPaint: {
                            const ctx = getContext("2d")
                            ctx.reset()
                            const pos = panel.positions
                            for (const e of panel.graph.edges) {
                                const a = pos[e.from], b = pos[e.to]
                                if (!a || !b) continue
                                ctx.beginPath()
                                ctx.strokeStyle = e.quotes > 0 ? "rgba(255, 212, 0, 0.75)" : "rgba(120, 140, 175, 0.45)"
                                ctx.lineWidth = Math.min(4, 1 + (e.count - 1) * 0.6) + (e.quotes > 0 ? 0.5 : 0)
                                ctx.moveTo(a.x, a.y)
                                ctx.lineTo(b.x, b.y)
                                ctx.stroke()
                                // Arrow head at the target (links go from notes).
                                const ang = Math.atan2(b.y - a.y, b.x - a.x)
                                const tip = { x: b.x - Math.cos(ang) * 11, y: b.y - Math.sin(ang) * 11 }
                                ctx.beginPath()
                                ctx.fillStyle = ctx.strokeStyle
                                ctx.moveTo(tip.x, tip.y)
                                ctx.lineTo(tip.x - Math.cos(ang - 0.4) * 7, tip.y - Math.sin(ang - 0.4) * 7)
                                ctx.lineTo(tip.x - Math.cos(ang + 0.4) * 7, tip.y - Math.sin(ang + 0.4) * 7)
                                ctx.closePath()
                                ctx.fill()
                            }
                        }
                    }

                    Repeater {
                        // Alone, the file itself would sit on the hint below.
                        model: panel.graph.nodes.length > 1 ? panel.graph.nodes : []
                        delegate: Item {
                            id: node
                            required property var modelData
                            required property int index
                            readonly property var p: panel.positions[index] || { x: 0, y: 0 }
                            readonly property real radius: index === 0 ? 17 : modelData.depth === 1 ? 10 : 6
                            x: p.x - width / 2
                            y: p.y - radius
                            width: 110
                            height: radius * 2 + 16
                            Rectangle {
                                anchors.horizontalCenter: parent.horizontalCenter
                                width: node.radius * 2
                                height: width
                                radius: width / 2
                                color: node.modelData.missing ? "transparent" : (panel.kindColors[node.modelData.kind] || "#9da5b4")
                                border.width: node.index === 0 ? 3 : node.modelData.missing ? 1 : 0
                                border.color: node.modelData.missing ? Theme.textFaint : Qt.rgba(0.24, 0.55, 0.99, 0.35)
                                opacity: node.modelData.depth === 2 ? 0.75 : 1
                                Icon {
                                    anchors.centerIn: parent
                                    visible: node.radius >= 10 && !node.modelData.missing
                                    name: Theme.fileIcon(node.modelData.label + (node.modelData.kind === "pdf" ? ".pdf" : node.modelData.kind === "markdown" ? ".md" : ""), node.modelData.kind === "dir")[0]
                                    color: "white"
                                    size: node.index === 0 ? 18 : 12
                                }
                            }
                            Label {
                                anchors.horizontalCenter: parent.horizontalCenter
                                y: node.radius * 2 + 1
                                width: parent.width
                                horizontalAlignment: Text.AlignHCenter
                                text: node.modelData.label
                                color: node.index === 0 ? Theme.textStrong : node.modelData.missing ? Theme.textFaint : Theme.text
                                font.pixelSize: node.modelData.depth === 2 ? 10 : Theme.smallFontSize
                                font.bold: node.index === 0
                                font.strikeout: node.modelData.missing
                                elide: Text.ElideMiddle
                            }
                            HoverHandler { id: nodeHover }
                            ToolTip.visible: nodeHover.hovered
                            ToolTip.delay: 400
                            ToolTip.text: node.modelData.relative + (node.modelData.missing ? qsTr(" (fehlt)") : "")
                            MouseArea {
                                anchors.fill: parent
                                acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                                cursorShape: node.index === 0 || node.modelData.missing ? Qt.ArrowCursor : Qt.PointingHandCursor
                                onClicked: (mouse) => {
                                    if (node.index === 0 || node.modelData.missing) return
                                    panel.openRequested(node.modelData.path, 0,
                                        mouse.button === Qt.MiddleButton || (mouse.modifiers & Qt.ControlModifier), {})
                                }
                            }
                        }
                    }
                    Label {
                        anchors.centerIn: parent
                        width: parent.width - 24
                        visible: panel.graph.nodes.length <= 1
                        wrapMode: Text.Wrap
                        horizontalAlignment: Text.AlignHCenter
                        color: Theme.textFaint
                        font.pixelSize: Theme.smallFontSize
                        text: qsTr("Keine Verknüpfungen.\nLinks in Notizen – auch Wiki-Links und Zitate aus PDFs – erscheinen hier.")
                    }
                }
                // Legend.
                Flow {
                    visible: !panel.compact
                    Layout.fillWidth: true
                    Layout.margins: 6
                    spacing: 10
                    Repeater {
                        model: [
                            { kind: "markdown", text: qsTr("Notiz") },
                            { kind: "pdf", text: qsTr("PDF") },
                            { kind: "text", text: qsTr("Datei") },
                            { kind: "quote", text: qsTr("Zitat") },
                        ]
                        Row {
                            required property var modelData
                            spacing: 4
                            Rectangle {
                                anchors.verticalCenter: parent.verticalCenter
                                width: modelData.kind === "quote" ? 14 : 8
                                height: modelData.kind === "quote" ? 2 : 8
                                radius: modelData.kind === "quote" ? 0 : 4
                                color: modelData.kind === "quote" ? "#ffd400" : panel.kindColors[modelData.kind]
                            }
                            Label { text: modelData.text; color: Theme.textFaint; font.pixelSize: 10 }
                        }
                    }
                }
            }
        }
    }
}
