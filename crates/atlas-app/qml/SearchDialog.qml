// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Project wide search in its own large window in front of Atlas (like the
// graph): ripgrep over the text files (respects .gitignore) and the text
// of PDFs. Filters for document types, case, whole words and regular
// expressions; hits are marked in the list, the preview and the opened
// document. Enter opens the selected hit, Ctrl+Enter in a split.
Dialog {
    id: dialog
    required property SearchModel model
    required property string projectRoot

    signal openRequested(string path, int line, bool newPane, var state)

    parent: Overlay.overlay
    anchors.centerIn: Overlay.overlay
    width: parent ? parent.width * 0.86 : 1100
    height: parent ? parent.height * 0.86 : 760
    modal: true
    padding: 0
    topPadding: 0
    title: ""

    /// Document types (keys of `atlas_core::search::KINDS`); none = all.
    readonly property var kindList: [
        { key: "markdown", label: qsTr("Markdown"), icon: "file-text" },
        { key: "pdf", label: qsTr("PDF"), icon: "file-pdf" },
        { key: "latex", label: qsTr("LaTeX"), icon: "file-code" },
        { key: "code", label: qsTr("Code"), icon: "file-code" },
        { key: "text", label: qsTr("Text"), icon: "file-text" }
    ]
    property var kinds: []
    property var collapsed: ({})
    property int currentRow: -1
    /// Query and options of the results shown (Enter opens a hit only
    /// when nothing changed since).
    property string shownSignature: ""

    onOpened: focusQuery()
    // The first hit is selected (preview, Enter).
    Connections {
        target: dialog.model
        function onCountChanged() { if (dialog.currentRow < 0 && dialog.model.count > 0) dialog.currentRow = 0 }
    }

    // ---------------------------------------------------------------- API

    function focusQuery() {
        if (!visible) open()
        query.forceActiveFocus()
        query.selectAll()
    }
    function setQuery(text) {
        if (!visible) open()
        query.text = text
        run()
        query.forceActiveFocus()
    }
    /// Finds TODO and FIXME markers in the project.
    function searchTodos() {
        if (!visible) open()
        query.text = "\\b(TODO|FIXME)\\b"
        regex.checked = true
        caseSensitive.checked = true
        wholeWord.checked = false
        run()
    }
    function viewState() {
        return { query: query.text, regex: regex.checked, caseSensitive: caseSensitive.checked,
                 wholeWord: wholeWord.checked, globs: globs.text, buildDirs: buildDirs.checked, kinds: kinds }
    }
    function applyState(state) {
        query.text = state.query || ""
        regex.checked = !!state.regex
        caseSensitive.checked = !!state.caseSensitive
        wholeWord.checked = !!state.wholeWord
        globs.text = state.globs || ""
        buildDirs.checked = !!state.buildDirs
        kinds = Array.isArray(state.kinds) ? state.kinds : []
        if (query.text) run()
        else model.clear()
    }

    function options() {
        return { regex: regex.checked, caseSensitive: caseSensitive.checked, wholeWord: wholeWord.checked,
                 globs: globs.text, buildDirs: buildDirs.checked, kinds: kinds }
    }
    function signature() { return JSON.stringify([query.text, options(), projectRoot]) }
    function run() {
        debounce.stop()
        collapsed = ({})
        currentRow = -1
        shownSignature = signature()
        model.search(projectRoot, query.text, JSON.stringify(options()))
    }
    function toggleKind(key) {
        const i = kinds.indexOf(key)
        kinds = i >= 0 ? kinds.filter(k => k !== key) : kinds.concat([key])
        debounce.restart()
    }

    // ------------------------------------------------------------ results

    function entry(row) {
        try { return JSON.parse(model.entry(row)) } catch (e) { return {} }
    }
    /// Moves the selection by `step` hits, past folded files.
    function moveSelection(step) {
        let row = currentRow
        for (let i = 0; i < model.count; ++i) {
            row += step
            if (row < 0 || row >= model.count) return
            if (!collapsed[entry(row).path]) break
        }
        currentRow = row
        results.positionViewAtIndex(row, ListView.Contain)
    }
    function openRow(row, newPane) {
        const e = entry(row)
        if (!e.path) return
        const r = (e.ranges && e.ranges[0]) || [0, 0]
        const found = e.text.slice(r[0], r[1]) || query.text
        const state = e.pdf ? { find: { text: found, page: e.line } }
            : { find: { text: found, line: e.line, start: r[0], end: r[1], caseSensitive: caseSensitive.checked } }
        close()
        openRequested(e.path, e.line, newPane, state)
    }
    /// Enter: opens the selected hit, or searches first when the query
    /// or an option changed.
    function acceptQuery(newPane) {
        if (shownSignature === signature() && model.count > 0) openRow(Math.max(0, currentRow), newPane)
        else run()
    }
    function toggleFile(path) {
        const c = Object.assign({}, collapsed)
        if (c[path]) delete c[path]
        else c[path] = true
        collapsed = c
    }

    function escapeHtml(s) {
        return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;")
    }
    /// `text` (from `from`, at most `length` characters) with the ranges
    /// `[[start, end], …]` marked, as rich text; "…" marks a cut (not one
    /// of the first `lead` characters, the indentation).
    function marked(text, ranges, from, length, current, lead) {
        const to = Math.min(text.length, from + length)
        let html = from > (lead || 0) ? "…" : ""
        let at = from
        for (const r of ranges) {
            const s = Math.max(r[0], at), e = Math.min(r[1], to)
            if (e <= s) continue
            html += escapeHtml(text.slice(at, s))
            html += "<span style=\"background-color:" + (current ? "#c98a00" : "#6b5100") + "; color:#fff4d6\">"
                + escapeHtml(text.slice(s, e)) + "</span>"
            at = e
        }
        html += escapeHtml(text.slice(at, to))
        return (to < text.length ? html + "…" : html).replace(/ {2}/g, " &nbsp;")
    }
    function rangesOf(list) {
        const out = []
        const parts = list ? list.split(",") : []
        for (let i = 0; i + 1 < parts.length; i += 2) out.push([+parts[i], +parts[i + 1]])
        return out
    }
    /// The query as a JavaScript expression for the preview (all
    /// occurrences in the lines around the hit).
    function previewRegex() {
        if (!query.text) return null
        let source = regex.checked ? query.text : query.text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
        if (wholeWord.checked) source = "\\b(?:" + source + ")\\b"
        try { return new RegExp(source, caseSensitive.checked ? "g" : "gi") } catch (e) { return null }
    }
    function rangesIn(line, re) {
        const out = []
        if (!re) return out
        re.lastIndex = 0
        let m
        while ((m = re.exec(line)) !== null && out.length < 200) {
            if (m[0].length === 0) { re.lastIndex++; continue }
            out.push([m.index, m.index + m[0].length])
        }
        return out
    }
    function fileIcon(path, pdf) {
        if (pdf || /\.pdf$/i.test(path)) return "file-pdf"
        if (/\.(md|markdown|txt|org|rst|qmd|rmd)$/i.test(path)) return "file-text"
        if (/\.(csv|tsv)$/i.test(path)) return "file-table"
        return "file-code"
    }

    Timer {
        id: debounce
        interval: 250
        onTriggered: dialog.run()
    }

    // Toggle with a label (filters and options).
    component Chip: AbstractButton {
        id: chip
        property string iconName
        property string tip
        checkable: true
        focusPolicy: Qt.NoFocus
        implicitHeight: 28
        implicitWidth: row.implicitWidth + 20
        hoverEnabled: true
        ToolTip.visible: hovered && tip.length > 0
        ToolTip.text: tip
        ToolTip.delay: 500
        background: Rectangle {
            radius: 14
            color: chip.checked ? Theme.accentSoft : chip.hovered ? Theme.hover : "transparent"
            border.width: 1
            border.color: chip.checked ? Theme.accent : Theme.border
        }
        contentItem: Item {
            RowLayout {
                id: row
                anchors.centerIn: parent
                spacing: 6
                Icon { visible: chip.iconName.length > 0; name: chip.iconName; size: 14; color: chip.checked ? Theme.accent : Theme.textMuted }
                Text {
                    text: chip.text
                    color: chip.checked ? Theme.textStrong : Theme.text
                    font.pixelSize: Theme.smallFontSize
                }
            }
        }
    }

    contentItem: ColumnLayout {
        spacing: 0

        // ---- title
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 16
            Layout.bottomMargin: 10
            spacing: 12
            Rectangle {
                Layout.preferredWidth: 36
                Layout.preferredHeight: 36
                radius: 10
                color: Theme.accentSoft
                Icon { anchors.centerIn: parent; name: "search"; size: 20; color: Theme.accent }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Text { text: qsTr("Suche im Projekt"); color: Theme.textStrong; font.pixelSize: 16; font.weight: Font.DemiBold }
                Text {
                    Layout.fillWidth: true
                    text: qsTr("Textdateien und PDFs · .gitignore wird berücksichtigt · ↑↓ wählen, Enter öffnet, Strg+Enter im Split")
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                }
            }
            IconButton { iconName: "close"; tip: qsTr("Schließen (Esc)"); onClicked: dialog.close() }
        }

        // ---- query and options
        ColumnLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                spacing: 8
                TextField {
                    id: query
                    objectName: "searchQuery"
                    Layout.fillWidth: true
                    placeholderText: qsTr("Suchen in %1 …").arg(dialog.projectRoot ? dialog.projectRoot.split("/").pop() : qsTr("Projekt"))
                    selectByMouse: true
                    font.pixelSize: Theme.fontSize + 2
                    leftPadding: 34
                    Icon { anchors.left: parent.left; anchors.leftMargin: 10; anchors.verticalCenter: parent.verticalCenter; name: "search"; size: 16 }
                    onTextEdited: debounce.restart()
                    Keys.onUpPressed: dialog.moveSelection(-1)
                    Keys.onDownPressed: dialog.moveSelection(1)
                    Keys.onPressed: (event) => {
                        if (event.key === Qt.Key_PageDown) { dialog.moveSelection(10); event.accepted = true }
                        else if (event.key === Qt.Key_PageUp) { dialog.moveSelection(-10); event.accepted = true }
                    }
                    Keys.onReturnPressed: (event) => dialog.acceptQuery((event.modifiers & Qt.ControlModifier) !== 0)
                    Keys.onEnterPressed: (event) => dialog.acceptQuery((event.modifiers & Qt.ControlModifier) !== 0)
                    Keys.onEscapePressed: {
                        if (dialog.model.running) dialog.model.cancel()
                        else dialog.close()
                    }
                }
                Chip { id: caseSensitive; objectName: "searchCase"; text: "Aa"; tip: qsTr("Groß-/Kleinschreibung beachten (aus: wird ignoriert)"); onToggled: debounce.restart() }
                Chip { id: wholeWord; objectName: "searchWord"; text: qsTr("Ganzes Wort"); tip: qsTr("Nur ganze Wörter finden"); onToggled: debounce.restart() }
                Chip { id: regex; objectName: "searchRegex"; text: ".*"; tip: qsTr("Regulärer Ausdruck"); onToggled: debounce.restart() }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 6
                Chip {
                    text: qsTr("Alle")
                    checked: dialog.kinds.length === 0
                    onClicked: { dialog.kinds = []; debounce.restart() }
                }
                Repeater {
                    model: dialog.kindList
                    Chip {
                        required property var modelData
                        objectName: "searchKind-" + modelData.key
                        text: modelData.label
                        iconName: modelData.icon
                        checked: dialog.kinds.indexOf(modelData.key) >= 0
                        onClicked: dialog.toggleKind(modelData.key)
                    }
                }
                Item { Layout.fillWidth: true }
                TextField {
                    id: globs
                    Layout.preferredWidth: 240
                    placeholderText: qsTr("Dateimuster, z. B. notes/**, !alt/**")
                    selectByMouse: true
                    font.pixelSize: Theme.smallFontSize
                    onTextEdited: debounce.restart()
                }
                Chip { id: buildDirs; text: qsTr("Build-Ordner"); tip: qsTr("Build- und Abhängigkeitsordner einbeziehen (target, node_modules, …)"); onToggled: debounce.restart() }
            }

            Label {
                Layout.fillWidth: true
                Layout.topMargin: 2
                Layout.bottomMargin: 8
                color: dialog.model.error ? Theme.error : Theme.textMuted
                font.pixelSize: Theme.smallFontSize
                elide: Text.ElideRight
                text: dialog.model.error ? dialog.model.error
                    : dialog.model.running ? qsTr("Suche läuft … %1 Treffer in %2 Dateien").arg(dialog.model.count).arg(dialog.model.files)
                    : query.text.length === 0 ? qsTr("Suchbegriff eingeben")
                    : dialog.model.count === 0 ? qsTr("Keine Treffer")
                    : qsTr("%1 Treffer in %2 Dateien · %3 ms%4").arg(dialog.model.count).arg(dialog.model.files)
                        .arg(dialog.model.elapsedMs).arg(dialog.model.truncated ? qsTr(" (gekürzt)") : "")
            }
        }

        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }

        // ---- results and preview
        SplitView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Horizontal

            ListView {
                id: results
                objectName: "searchResults"
                SplitView.preferredWidth: dialog.width * 0.55
                SplitView.minimumWidth: 280
                clip: true
                model: dialog.model
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar {}

                delegate: Column {
                    id: hit
                    required property int index
                    required property string path
                    required property string relativePath
                    required property int line
                    required property string text
                    required property string matchRanges
                    required property bool firstInFile
                    required property bool isPdf
                    required property int fileHits
                    readonly property bool folded: !!dialog.collapsed[path]
                    readonly property bool current: index === dialog.currentRow
                    width: ListView.view.width

                    // File header.
                    Rectangle {
                        visible: hit.firstInFile
                        width: parent.width
                        height: visible ? 30 : 0
                        color: headerArea.containsMouse ? Theme.hover : Theme.panel
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 10
                            anchors.rightMargin: 12
                            spacing: 6
                            Icon { name: hit.folded ? "chevron-right" : "chevron-down"; size: 12 }
                            Icon { name: dialog.fileIcon(hit.path, hit.isPdf); size: 15; color: hit.isPdf ? "#e5534b" : Theme.textMuted }
                            Text {
                                text: hit.relativePath.split("/").pop()
                                color: Theme.textStrong
                                font.pixelSize: Theme.fontSize
                                font.weight: Font.DemiBold
                            }
                            Text {
                                Layout.fillWidth: true
                                text: hit.relativePath.indexOf("/") >= 0 ? hit.relativePath.slice(0, hit.relativePath.lastIndexOf("/")) : ""
                                color: Theme.textFaint
                                font.pixelSize: Theme.smallFontSize
                                elide: Text.ElideMiddle
                            }
                            Rectangle {
                                Layout.preferredHeight: 18
                                Layout.preferredWidth: Math.max(22, hitCount.implicitWidth + 12)
                                radius: 9
                                color: Theme.raised
                                Text { id: hitCount; anchors.centerIn: parent; text: hit.fileHits; color: Theme.textMuted; font.pixelSize: Theme.smallFontSize }
                            }
                        }
                        MouseArea {
                            id: headerArea
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: dialog.toggleFile(hit.path)
                        }
                    }
                    // The hit.
                    Rectangle {
                        visible: !hit.folded
                        width: parent.width
                        height: visible ? 24 : 0
                        color: hit.current ? Theme.selection : area.containsMouse ? Theme.hover : "transparent"
                        Rectangle { visible: hit.current; width: 3; height: parent.height; color: Theme.accent }
                        Text {
                            id: lineNumber
                            x: 28
                            width: 52
                            height: parent.height
                            verticalAlignment: Text.AlignVCenter
                            horizontalAlignment: Text.AlignRight
                            text: hit.isPdf ? qsTr("S. %1").arg(hit.line) : hit.line
                            color: Theme.textFaint
                            font.pixelSize: Theme.smallFontSize
                            font.family: Theme.monoFont
                        }
                        Text {
                            anchors.left: lineNumber.right
                            anchors.leftMargin: 10
                            anchors.right: parent.right
                            anchors.rightMargin: 8
                            height: parent.height
                            verticalAlignment: Text.AlignVCenter
                            clip: true
                            textFormat: Text.RichText
                            wrapMode: Text.NoWrap
                            color: Theme.text
                            font.pixelSize: Theme.fontSize
                            text: {
                                const ranges = dialog.rangesOf(hit.matchRanges)
                                const first = ranges.length ? ranges[0][0] : 0
                                // Leading space trimmed, some context before the hit.
                                const indent = hit.text.length - hit.text.replace(/^\s+/, "").length
                                return dialog.marked(hit.text, ranges, Math.max(indent, first - 40), 260, hit.current, indent)
                            }
                        }
                        MouseArea {
                            id: area
                            anchors.fill: parent
                            hoverEnabled: true
                            acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                            onClicked: (mouse) => {
                                if (mouse.button === Qt.MiddleButton || (mouse.modifiers & Qt.ControlModifier)) dialog.openRow(hit.index, true)
                                else { dialog.currentRow = hit.index; query.forceActiveFocus() }
                            }
                            onDoubleClicked: dialog.openRow(hit.index, false)
                        }
                    }
                }

                // Empty states.
                Column {
                    anchors.centerIn: parent
                    visible: results.count === 0 && !dialog.model.running
                    spacing: 8
                    width: Math.min(parent.width - 48, 420)
                    Icon { anchors.horizontalCenter: parent.horizontalCenter; name: "search"; size: 36; color: Theme.textFaint }
                    Text {
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                        color: Theme.textMuted
                        text: query.text.length === 0
                            ? qsTr("Durchsucht Notizen, Code und den Text aller PDFs im Projekt. Mit den Filtern oben auf Dokumenttypen beschränken.")
                            : qsTr("Keine Treffer für „%1“.").arg(query.text)
                    }
                }
            }

            // Preview of the selected hit.
            Rectangle {
                id: preview
                SplitView.fillWidth: true
                SplitView.minimumWidth: 240
                color: Theme.editor
                readonly property var hitEntry: dialog.currentRow >= 0 && dialog.model.count > dialog.currentRow ? dialog.entry(dialog.currentRow) : ({})
                readonly property var lines: {
                    const e = hitEntry
                    if (!e.path) return []
                    try { return JSON.parse(dialog.model.context(e.path, e.line, 6, e.pdf)) } catch (err) { return [] }
                }
                readonly property var re: dialog.previewRegex()

                ColumnLayout {
                    anchors.fill: parent
                    spacing: 0
                    visible: !!preview.hitEntry.path
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.margins: 12
                        spacing: 8
                        Icon { name: dialog.fileIcon(preview.hitEntry.path || "", preview.hitEntry.pdf); size: 16 }
                        Text {
                            Layout.fillWidth: true
                            text: (preview.hitEntry.relative || "")
                                + (preview.hitEntry.pdf ? qsTr(" · Seite %1").arg(preview.hitEntry.line)
                                                                    : qsTr(" · Zeile %1").arg(preview.hitEntry.line))
                            color: Theme.textStrong
                            font.pixelSize: Theme.smallFontSize
                            elide: Text.ElideMiddle
                        }
                        Button {
                            text: qsTr("Öffnen")
                            onClicked: dialog.openRow(dialog.currentRow, false)
                        }
                        Button {
                            text: qsTr("Im Split")
                            onClicked: dialog.openRow(dialog.currentRow, true)
                        }
                    }
                    Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }
                    ListView {
                        id: previewLines
                        objectName: "searchPreview"
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        Layout.topMargin: 6
                        clip: true
                        model: preview.lines
                        boundsBehavior: Flickable.StopAtBounds
                        ScrollBar.vertical: ScrollBar {}
                        readonly property var hitEntry: preview.hitEntry
                        readonly property var re: preview.re
                        // A PDF page: show the line of the hit.
                        onModelChanged: Qt.callLater(() => {
                            if (!hitEntry.pdf) return
                            const i = (model || []).findIndex(l => l.text.indexOf(hitEntry.text.trim().slice(0, 40)) >= 0)
                            if (i >= 0) positionViewAtIndex(i, ListView.Center)
                        })
                        delegate: Rectangle {
                            id: previewLine
                            required property var modelData
                            readonly property bool isHit: !previewLines.hitEntry.pdf && modelData.line === previewLines.hitEntry.line
                            width: ListView.view.width
                            height: Math.max(22, lineText.implicitHeight + 4)
                            color: isHit ? Theme.accentSoft : "transparent"
                            Text {
                                id: previewNumber
                                visible: !previewLines.hitEntry.pdf
                                width: visible ? 44 : 4
                                anchors.top: parent.top
                                anchors.topMargin: 2
                                horizontalAlignment: Text.AlignRight
                                text: previewLine.modelData.line
                                color: previewLine.isHit ? Theme.accent : Theme.textFaint
                                font.family: Theme.monoFont
                                font.pixelSize: Theme.smallFontSize
                            }
                            Text {
                                id: lineText
                                anchors.left: previewNumber.right
                                anchors.leftMargin: 12
                                anchors.right: parent.right
                                anchors.rightMargin: 12
                                anchors.top: parent.top
                                anchors.topMargin: 2
                                textFormat: Text.RichText
                                wrapMode: Text.Wrap
                                color: Theme.text
                                font.family: previewLines.hitEntry.pdf ? dialog.font.family : Theme.monoFont
                                font.pixelSize: Theme.fontSize
                                text: {
                                    const t = previewLine.modelData.text
                                    const ranges = previewLine.isHit ? previewLines.hitEntry.ranges : dialog.rangesIn(t, previewLines.re)
                                    return dialog.marked(t, ranges, 0, t.length, previewLine.isHit)
                                }
                            }
                        }
                    }
                }
                Text {
                    anchors.centerIn: parent
                    visible: !preview.hitEntry.path && dialog.model.count > 0
                    text: qsTr("Treffer wählen für die Vorschau")
                    color: Theme.textFaint
                }
            }
        }
    }
}
