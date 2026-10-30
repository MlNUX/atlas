// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Context of a Markdown file or PDF: notes linking to it (filterable by
// text, kind of link, tag and folder) and the graph of the project – notes,
// PDFs and other files, this file highlighted in the middle – with quotes
// of PDF annotations highlighted.
Rectangle {
    id: panel
    required property AppController app
    required property string path
    /// 0 = backlinks, 1 = graph.
    property int tab: 0
    /// Own tab bar (off when a surrounding panel switches the tab).
    property bool showTabs: true
    /// Graph only, without tool bar and legend (overview cards).
    property bool compact: false
    /// Backlinks: "file" (links to `path`) or "project" (all marked text of
    /// the project's PDFs and where notes quote it, by PDF).
    property string scope: "file"
    readonly property bool projectScope: scope === "project"
    /// The list of backlinks (for tests).
    readonly property alias backlinkView: backList
    readonly property alias graphView: graphArea
    property var backlinkList: []
    /// Files related to `path` by hand ("Verknüpfen mit …").
    property var relatedList: []
    property var graph: ({ nodes: [], edges: [], truncated: 0 })
    readonly property int count: backlinkList.length

    /// `newPane` for Ctrl/middle click; `state` for the viewer.
    signal openRequested(string path, int line, bool newPane, var state)

    color: Theme.sidebar

    /// Only filled while visible (a hidden tab of the context bar): opening
    /// a file does not look up what nobody sees; showing it fills it.
    property bool lazy: false
    function refresh() {
        if (lazy && !visible) return
        const t = Timing.now()
        // (`scope`, not `projectScope`: this also runs from onScopeChanged.)
        if (scope === "project" && tab === 0) { backlinkList = JSON.parse(app.projectQuotes()); return }
        if (!path) { backlinkList = []; relatedList = []; graphJson = ""; graph = ({ nodes: [], edges: [], truncated: 0 }); return }
        backlinkList = JSON.parse(app.backlinks(path))
        relatedList = JSON.parse(app.relatedFiles(path))
        Timing.took("Kontext: Rückverweise und Verknüpfungen", t)
        if (visible && tab === 1) graphTimer.restart()
    }
    // The graph (up to 500 files, laid out in JavaScript) comes a moment
    // after the rest: opening a file switches the tab at once instead of
    // waiting for it, and clicking through files lays it out only once.
    Timer {
        id: graphTimer
        interval: 120
        onTriggered: {
            if (!panel.path || !panel.visible || panel.tab !== 1) return
            const t = Timing.now()
            const json = panel.app.contextGraph(panel.path, 0)
            if (json === panel.graphJson) return
            const graph = JSON.parse(json)
            Timing.took("Graph: contextGraph (Rust), " + graph.nodes.length + " Dateien", t)
            const t2 = Timing.now()
            panel.graphJson = json
            panel.graph = graph
            Timing.took("Graph: Layout und Elemente", t2)
        }
    }
    onPathChanged: refresh()
    onTabChanged: refresh()
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

    /// Depth of nodes not linked with this file.
    readonly property int unconnected: 255

    /// Layout in graph coordinates (the file at 0,0). Files with links are
    /// placed by a force layout: linked files pull together, all push
    /// apart, a weak pull to the middle keeps separate groups close. Files
    /// without any link fill the space around them evenly (a sunflower
    /// spiral), so many of them neither overlap nor push the rest away.
    /// Deterministic: the same graph always gets the same picture.
    function layout() {
        const nodes = graph.nodes, n = nodes.length
        const pos = []
        if (!n) return pos
        const degree = new Array(n).fill(0)
        for (const e of graph.edges) { degree[e.from]++; degree[e.to]++ }
        const linked = [], loose = []
        for (let i = 1; i < n; ++i) (degree[i] > 0 ? linked : loose).push(i)
        const ideal = 80
        // Start: rings by distance from the file, other groups outside.
        pos[0] = { x: 0, y: 0 }
        const rings = {}
        for (const i of linked) (rings[nodes[i].depth] = rings[nodes[i].depth] || []).push(i)
        Object.keys(rings).map(Number).sort((a, b) => a - b).forEach((d, ring) => {
            rings[d].forEach((node, k) => {
                const a = -Math.PI / 2 + 2 * Math.PI * k / rings[d].length + ring * 0.4
                const r = ideal * (ring + 1)
                pos[node] = { x: r * Math.cos(a), y: r * Math.sin(a) }
            })
        })
        const moving = [0].concat(linked)
        const steps = moving.length > 150 ? 80 : moving.length > 60 ? 150 : 250
        let heat = ideal / 2
        for (let step = 0; step < steps; ++step) {
            const disp = {}
            for (const i of moving) disp[i] = { x: 0, y: 0 }
            for (let a = 0; a < moving.length; ++a) {
                for (let b = a + 1; b < moving.length; ++b) {
                    const i = moving[a], j = moving[b]
                    let dx = pos[i].x - pos[j].x, dy = pos[i].y - pos[j].y
                    let d2 = dx * dx + dy * dy
                    if (d2 < 0.01) { dx = 0.1 * (i - j); dy = 0.1; d2 = dx * dx + dy * dy }
                    const f = ideal * ideal / d2
                    disp[i].x += dx * f; disp[i].y += dy * f
                    disp[j].x -= dx * f; disp[j].y -= dy * f
                }
            }
            for (const e of graph.edges) {
                const dx = pos[e.from].x - pos[e.to].x, dy = pos[e.from].y - pos[e.to].y
                const f = Math.sqrt(dx * dx + dy * dy) / ideal
                disp[e.from].x -= dx * f; disp[e.from].y -= dy * f
                disp[e.to].x += dx * f; disp[e.to].y += dy * f
            }
            for (const i of linked) {
                // Pull to the middle, growing with the distance.
                const r = Math.sqrt(pos[i].x * pos[i].x + pos[i].y * pos[i].y)
                disp[i].x -= pos[i].x * r / (ideal * 12)
                disp[i].y -= pos[i].y * r / (ideal * 12)
                const len = Math.sqrt(disp[i].x * disp[i].x + disp[i].y * disp[i].y) || 1
                const move = Math.min(len, heat)
                pos[i] = { x: pos[i].x + disp[i].x / len * move, y: pos[i].y + disp[i].y / len * move }
            }
            heat = Math.max(1, heat * 0.97)
        }
        // Files without links: a sunflower spiral around the linked part.
        let inner = 0
        for (const i of linked) inner = Math.max(inner, Math.sqrt(pos[i].x * pos[i].x + pos[i].y * pos[i].y))
        inner += linked.length ? ideal * 0.8 : ideal * 0.6
        const spacing = ideal * 0.6
        const golden = Math.PI * (3 - Math.sqrt(5))
        loose.forEach((node, k) => {
            // Equal area per node: r² grows by spacing² per node.
            const r = Math.sqrt(inner * inner + (k + 0.5) * spacing * spacing * 1.1)
            const a = k * golden
            pos[node] = { x: r * Math.cos(a), y: r * Math.sin(a) }
        })
        return pos
    }
    /// Graph coordinates of the nodes, and the view: scale and offset of
    /// the graph's 0,0 in the area (changed by zooming and dragging).
    property var world: []
    property real zoom: 1
    property real panX: 0
    property real panY: 0
    /// Fits the whole graph into the area.
    function fit() {
        const w = graphArea.width, h = graphArea.height
        if (!world.length || w <= 0 || h <= 0) return
        let x0 = 0, x1 = 0, y0 = 0, y1 = 0
        for (const p of world) { x0 = Math.min(x0, p.x); x1 = Math.max(x1, p.x); y0 = Math.min(y0, p.y); y1 = Math.max(y1, p.y) }
        // Room for the labels below and beside the nodes.
        zoom = Math.min(1.6, (w - 130) / Math.max(1, x1 - x0), (h - 60) / Math.max(1, y1 - y0))
        panX = w / 2 - (x0 + x1) / 2 * zoom
        panY = h / 2 - (y0 + y1) / 2 * zoom
    }
    function zoomAt(factor, x, y) {
        const z = Math.max(0.1, Math.min(6, zoom * factor))
        panX = x - (x - panX) * z / zoom
        panY = y - (y - panY) * z / zoom
        zoom = z
    }
    readonly property var positions: world.map(p => ({ x: panX + p.x * zoom, y: panY + p.y * zoom }))
    onPositionsChanged: edgeCanvas.requestPaint()
    /// The graph as Rust gave it (key of the layout cache).
    property string graphJson: ""
    /// Layouts of graphs seen lately, by graphJson: switching back to a file
    /// does not lay out its graph again (deterministic anyway).
    property var layoutCache: ({})
    property var layoutKeys: []
    function relayout() {
        const key = graphJson
        let pos = key ? layoutCache[key] : undefined
        if (!pos) {
            pos = layout()
            if (key) {
                layoutCache[key] = pos
                layoutKeys.push(key)
                if (layoutKeys.length > 16) delete layoutCache[layoutKeys.shift()]
            }
        }
        world = pos
        fit()
    }
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
                // Files related by hand, with "+" to add one.
                ColumnLayout {
                    visible: !panel.projectScope && !!panel.path && !!panel.app.atlasDir
                    Layout.fillWidth: true
                    Layout.fillHeight: false
                    Layout.leftMargin: 6
                    Layout.rightMargin: 6
                    Layout.topMargin: 6
                    spacing: 0
                    RowLayout {
                        Layout.fillWidth: true
                        Text {
                            Layout.fillWidth: true
                            text: panel.relatedList.length ? qsTr("Verknüpft mit (%1)").arg(panel.relatedList.length) : qsTr("Mit keiner Datei verknüpft")
                            color: Theme.textMuted
                            font.pixelSize: Theme.smallFontSize
                            font.weight: Font.DemiBold
                        }
                        IconButton {
                            iconName: "plus"
                            iconSize: 14
                            implicitWidth: 24
                            implicitHeight: 24
                            tip: qsTr("Verknüpfen mit … (z. B. eine andere PDF)")
                            onClicked: panel.app.requestRelate(panel.path)
                        }
                    }
                    Repeater {
                        model: panel.relatedList
                        Rectangle {
                            id: relatedRow
                            required property var modelData
                            Layout.fillWidth: true
                            implicitHeight: 26
                            radius: 5
                            color: relatedHover.hovered ? Theme.hover : "transparent"
                            HoverHandler { id: relatedHover }
                            MouseArea {
                                anchors.fill: parent
                                acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                                cursorShape: relatedRow.modelData.missing ? Qt.ArrowCursor : Qt.PointingHandCursor
                                onClicked: (mouse) => {
                                    if (!relatedRow.modelData.missing)
                                        panel.openRequested(relatedRow.modelData.path, 0, mouse.button === Qt.MiddleButton || (mouse.modifiers & Qt.ControlModifier), {})
                                }
                            }
                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 4
                                spacing: 6
                                Icon {
                                    readonly property var look: Theme.fileIcon(relatedRow.modelData.relative, relatedRow.modelData.kind === "dir")
                                    name: look[0]
                                    color: look[1]
                                    size: 14
                                }
                                Text {
                                    Layout.fillWidth: true
                                    text: relatedRow.modelData.relative + (relatedRow.modelData.missing ? qsTr(" (fehlt)") : "")
                                    color: relatedRow.modelData.missing ? Theme.textFaint : Theme.text
                                    font.pixelSize: Theme.smallFontSize
                                    font.strikeout: relatedRow.modelData.missing
                                    elide: Text.ElideMiddle
                                }
                                IconButton {
                                    visible: relatedHover.hovered
                                    iconName: "close"
                                    iconSize: 12
                                    implicitWidth: 22
                                    implicitHeight: 22
                                    tip: qsTr("Verknüpfung lösen")
                                    onClicked: panel.app.unrelateFiles(panel.path, relatedRow.modelData.path)
                                }
                            }
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
                                    color: row.modelData.anchorKind === "quote" ? Theme.quote : Theme.link
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
                    Label {
                        Layout.fillWidth: true
                        leftPadding: 4
                        text: qsTr("%n Datei(en)", "", panel.graph.nodes.length)
                            + (panel.graph.truncated ? qsTr(" · %1 ausgelassen").arg(panel.graph.truncated) : "")
                        color: Theme.textFaint
                        font.pixelSize: Theme.smallFontSize
                        elide: Text.ElideRight
                    }
                    IconButton { iconName: "minus"; tip: qsTr("Verkleinern (Strg+Mausrad)"); onClicked: panel.zoomAt(1 / 1.25, graphArea.width / 2, graphArea.height / 2) }
                    IconButton { iconName: "plus"; tip: qsTr("Vergrößern (Strg+Mausrad)"); onClicked: panel.zoomAt(1.25, graphArea.width / 2, graphArea.height / 2) }
                    IconButton { iconName: "focus"; tip: qsTr("Ganzen Graphen zeigen"); onClicked: panel.fit() }
                }
                Item {
                    id: graphArea
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    onWidthChanged: panel.fit()
                    onHeightChanged: panel.fit()

                    // Drag the empty area to move, Ctrl+wheel to zoom (the
                    // plain wheel scrolls the surrounding panel).
                    DragHandler {
                        id: panDrag
                        target: null
                        property real startX
                        property real startY
                        onActiveChanged: if (active) { startX = panel.panX; startY = panel.panY }
                        onTranslationChanged: if (active) { panel.panX = startX + translation.x; panel.panY = startY + translation.y }
                    }
                    WheelHandler {
                        acceptedModifiers: Qt.ControlModifier
                        acceptedDevices: PointerDevice.AllDevices
                        acceptedPointerTypes: PointerDevice.AllPointerTypes
                        onWheel: (event) => {
                            const steps = event.angleDelta.y !== 0 ? event.angleDelta.y / 120 : event.pixelDelta.y / 50
                            panel.zoomAt(Math.pow(1.2, steps), point.position.x, point.position.y)
                        }
                    }

                    Canvas {
                        id: edgeCanvas
                        anchors.fill: parent
                        Connections { target: Theme; function onLightChanged() { edgeCanvas.requestPaint() } }
                        onPaint: {
                            const ctx = getContext("2d")
                            ctx.reset()
                            const pos = panel.positions
                            for (const e of panel.graph.edges) {
                                const a = pos[e.from], b = pos[e.to]
                                if (!a || !b) continue
                                const scale = Math.max(0.4, Math.min(1, panel.zoom * 1.2))
                                // Related by hand only: dashed, green, no direction.
                                if (e.related && e.count === 0) {
                                    ctx.beginPath()
                                    ctx.strokeStyle = Theme.light ? "rgba(30, 148, 85, 0.85)" : "rgba(52, 194, 122, 0.85)"
                                    ctx.lineWidth = Math.max(1.6, 2.5 * scale)
                                    ctx.setLineDash([5, 3])
                                    ctx.moveTo(a.x, a.y)
                                    ctx.lineTo(b.x, b.y)
                                    ctx.stroke()
                                    ctx.setLineDash([])
                                    continue
                                }
                                ctx.beginPath()
                                ctx.strokeStyle = e.quotes > 0 ? (Theme.light ? "rgba(217, 164, 0, 0.85)" : "rgba(255, 212, 0, 0.75)") : (Theme.light ? "rgba(90, 105, 130, 0.5)" : "rgba(120, 140, 175, 0.45)")
                                ctx.lineWidth = (Math.min(4, 1 + (e.count - 1) * 0.6) + (e.quotes > 0 ? 0.5 : 0)) * scale
                                ctx.moveTo(a.x, a.y)
                                ctx.lineTo(b.x, b.y)
                                ctx.stroke()
                                // Arrow head at the target (links go from notes).
                                const ang = Math.atan2(b.y - a.y, b.x - a.x)
                                const tip = { x: b.x - Math.cos(ang) * 11 * scale, y: b.y - Math.sin(ang) * 11 * scale }
                                ctx.beginPath()
                                ctx.fillStyle = ctx.strokeStyle
                                ctx.moveTo(tip.x, tip.y)
                                ctx.lineTo(tip.x - Math.cos(ang - 0.4) * 7 * scale, tip.y - Math.sin(ang - 0.4) * 7 * scale)
                                ctx.lineTo(tip.x - Math.cos(ang + 0.4) * 7 * scale, tip.y - Math.sin(ang + 0.4) * 7 * scale)
                                ctx.closePath()
                                ctx.fill()
                            }
                        }
                    }

                    Repeater {
                        // Alone, the file itself would sit on the hint below.
                        // A count, not the list: with as many files as before
                        // (the same project) the nodes stay and only take the
                        // new data, instead of all being built again.
                        model: panel.graph.nodes.length > 1 ? panel.graph.nodes.length : 0
                        delegate: Item {
                            id: node
                            required property int index
                            readonly property var modelData: panel.graph.nodes[index] || ({ label: "", relative: "", path: "", kind: "", depth: 0 })
                            readonly property var p: panel.positions[index] || { x: 0, y: 0 }
                            readonly property bool far: modelData.depth === panel.unconnected
                            // Smaller when zoomed out, so dense parts stay apart.
                            // Folder nodes are bigger: files hang on them.
                            readonly property bool folder: modelData.kind === "dir"
                            readonly property real radius: (index === 0 ? 17 : folder ? 13 : modelData.depth === 1 ? 10 : far ? 6 : 7)
                                * Math.max(index === 0 ? 0.7 : 0.35, Math.min(1, panel.zoom * 1.2))
                            // Names of distant nodes only when there is room
                            // or on hover.
                            readonly property bool labelled: index === 0 || folder || nodeHover.hovered || panel.zoom >= 1.3
                                || (panel.graph.nodes.length <= 30 && panel.zoom >= 0.5) || (modelData.depth === 1 && panel.zoom >= 0.6)
                            z: nodeHover.hovered ? 2 : index === 0 ? 1 : 0
                            x: p.x - width / 2
                            y: p.y - radius
                            width: 110
                            height: radius * 2 + 16
                            Rectangle {
                                anchors.horizontalCenter: parent.horizontalCenter
                                width: node.radius * 2
                                height: width
                                radius: width / 2
                                color: node.modelData.missing ? "transparent" : (panel.kindColors[node.modelData.kind] || Theme.textMuted)
                                border.width: node.index === 0 ? 3 : node.modelData.missing ? 1 : 0
                                border.color: node.modelData.missing ? Theme.textFaint : Qt.rgba(0.24, 0.55, 0.99, 0.35)
                                opacity: node.far ? 0.55 : node.modelData.depth >= 2 ? 0.8 : 1
                                Icon {
                                    anchors.centerIn: parent
                                    visible: node.radius >= 8 && !node.modelData.missing
                                    name: Theme.fileIcon(node.modelData.label + (node.modelData.kind === "pdf" ? ".pdf" : node.modelData.kind === "markdown" ? ".md" : ""), node.modelData.kind === "dir")[0]
                                    color: "white"
                                    size: Math.round(node.radius * (node.index === 0 ? 1.05 : 1.2))
                                }
                            }
                            Label {
                                anchors.horizontalCenter: parent.horizontalCenter
                                y: node.radius * 2 + 1
                                width: parent.width
                                visible: node.labelled
                                horizontalAlignment: Text.AlignHCenter
                                text: node.modelData.label
                                color: node.index === 0 ? Theme.textStrong : node.modelData.missing ? Theme.textFaint : Theme.text
                                font.pixelSize: node.index === 0 || node.modelData.depth === 1 ? Theme.smallFontSize : 10
                                opacity: node.far && !nodeHover.hovered ? 0.7 : 1
                                font.bold: node.index === 0 || node.folder
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
                            { kind: "related", text: qsTr("Verknüpfung") },
                        ]
                        Row {
                            required property var modelData
                            spacing: 4
                            Rectangle {
                                anchors.verticalCenter: parent.verticalCenter
                                readonly property bool line: modelData.kind === "quote" || modelData.kind === "related"
                                width: line ? 14 : 8
                                height: line ? 2 : 8
                                radius: line ? 0 : 4
                                color: modelData.kind === "quote" ? Theme.quote : modelData.kind === "related" ? Theme.ok : panel.kindColors[modelData.kind]
                            }
                            Label { text: modelData.text; color: Theme.textFaint; font.pixelSize: 10 }
                        }
                    }
                }
            }
        }
    }
}
