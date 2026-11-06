// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Right context bar for the active document, made of blocks (annotations,
// links, graph, actions, today, tasks, extension cards …) laid out by the
// profile of the active tab's file type (ContextProfiles.qml, concept in
// docs/concept/kontextleiste.html): tabs, the overview and blocks pinned to
// the bottom edge. Blocks collapse, move and hide right here; „Leiste
// anpassen“ opens the editing mode (ContextEditor.qml).
Rectangle {
    id: inspector
    required property AppController app
    required property var editor
    required property var actions
    /// Index of the shown tab in `tabIds` (0: usually „Übersicht“).
    property int tab: 0
    readonly property alias linksPanel: linksPanel
    readonly property alias tabBar: tabFlick
    readonly property alias modules: contextModules
    readonly property alias pomodoro: pomodoroTimer
    readonly property alias editorPanel: contextEditor

    signal openRequested(string path, int line, string where, var state)
    /// A view or command from an extension card ({id, name}).
    signal keyActionRequested(var binding)
    /// A tag of the note was clicked: search it in the project.
    signal tagSearchRequested(string tag)

    color: Theme.panel
    radius: Theme.radius
    border.color: Theme.border

    // ------------------------------------------------------------ context

    readonly property var activeTab: editor.activeTab
    /// File of the active tab ("" for calendar, terminals, folders).
    readonly property string document: activeTab && activeTab.path && ["pdf", "markdown", "text", "image", "sheet", "binary"].indexOf(activeTab.kind) >= 0
        ? activeTab.path : lastDocument
    property string lastDocument: ""
    /// The PDF whose annotations are shown: the active one, otherwise the
    /// last PDF that was active.
    property string pdfPath: ""
    onActiveTabChanged: remember()
    Connections { target: inspector.activeTab; ignoreUnknownSignals: true; function onPathChanged() { inspector.remember() } }
    function remember() {
        if (!activeTab || !activeTab.path) return
        if (["pdf", "markdown", "text", "image", "sheet", "binary"].indexOf(activeTab.kind) >= 0) lastDocument = activeTab.path
        if (activeTab.kind === "pdf") pdfPath = activeTab.path
    }
    readonly property var pdfViewer: pdfPath && editor.groups.length >= 0 ? editor.viewerFor(pdfPath) : null
    readonly property var annotations: {
        const store = pdfViewer && pdfViewer.annotationStore ? pdfViewer.annotationStore : null
        if (!store) return []
        try { return JSON.parse(store.markupJson || "[]") } catch (e) { return [] }
    }
    /// User actions for the document, then for the project: {id, name,
    /// description, source, paths}.
    readonly property var actionList: {
        const t = Timing.now()
        const out = []
        const seen = {}
        for (const paths of document ? [[document], []] : [[]])
            for (const a of actions.actionsFor(paths))
                if (!seen[a.id] && a.menu) { seen[a.id] = true; out.push(Object.assign({ paths: paths }, a)) }
        Timing.took("Kontextleiste: Aktionen", t)
        return out
    }

    function when(unix) {
        if (!unix) return ""
        const d = new Date(unix * 1000), now = new Date()
        const pad = (n) => (n < 10 ? "0" : "") + n
        const time = pad(d.getHours()) + ":" + pad(d.getMinutes())
        if (d.toDateString() === now.toDateString()) return qsTr("Heute, %1").arg(time)
        return d.getDate() + "." + (d.getMonth() + 1) + "." + d.getFullYear() + ", " + time
    }
    function openAnnotation(a) { openRequested(pdfPath, a.page, "tab", { annot: a.id }) }

    /// Built-in actions for the document next to the user's own.
    readonly property var builtins: {
        const t = Timing.now()
        const out = []
        const kind = document ? app.fileKind(document) : ""
        if (kind === "pdf" && pdfViewer) {
            out.push({ icon: "quote", name: qsTr("Zitat übernehmen"), enabled: !!pdfViewer.selectionText,
                       note: qsTr("Erst Text in der PDF markieren"),
                       tip: qsTr("Markierte Stelle als Zitat in eine Notiz (Strg+Q)"), run: () => pdfViewer.quoteSelection() })
            // Only worth it with marks, comments, ink or bookmarks.
            out.push({ icon: "export", name: qsTr("PDF-Kopie exportieren"), enabled: pdfViewer.annotationStore.count > 0,
                       note: qsTr("Noch keine Anmerkungen"),
                       tip: pdfViewer.annotationStore.count > 0 ? qsTr("Kopie mit Anmerkungen; das Original bleibt unverändert")
                                                                : qsTr("Erst etwas markieren oder kommentieren – ohne Anmerkungen wäre die Kopie gleich dem Original"),
                       run: () => pdfViewer.exportCopy() })
        }
        if (document && app.isRunnable(document))
            out.push({ icon: "play", name: qsTr("Ausführen"), enabled: true, tip: qsTr("Skript im Terminal starten (im Ordner des Skripts)"),
                       run: () => actions.runScript(document) })
        if (document && actions.isMarkdown(document))
            out.push({ icon: "export", name: qsTr("Als PDF exportieren"), enabled: !actions.markdownPdf.busy,
                       note: qsTr("Export läuft …"),
                       tip: qsTr("PDF neben der Notiz, gesetzt wie die Vorschau (Formeln, Diagramme)"),
                       run: () => actions.markdownToPdf(document) })
        if (document && actions.isTex(document))
            out.push({ icon: "export", name: qsTr("Zu PDF übersetzen"), enabled: true,
                       tip: qsTr("Mit latexmk (sonst pdflatex) im Terminal; die PDF entsteht neben der .tex-Datei"),
                       run: () => actions.texToPdf(document) })
        if (document && actions.isHtml(document))
            out.push({ icon: "eye", name: qsTr("Als Webseite anzeigen"), enabled: true, tip: qsTr("HTML gerendert anzeigen (lädt beim Speichern neu)"),
                       run: () => actions.renderHtml(document) })
        if (document && actions.isDelimited(document))
            out.push({ icon: "file-table", name: qsTr("Als Tabelle anzeigen"), enabled: true,
                       tip: qsTr("Spalten und Zeilen wie in Excel (Trennzeichen und Zeichensatz werden erkannt)"),
                       run: () => actions.showAsTable(document) })
        if (document && app.atlasDir && app.exists(document))
            out.push({ icon: "link", name: qsTr("Verknüpfen mit …"), enabled: true, tip: qsTr("Mit einer anderen Datei verbinden, z. B. zwei PDFs (im Graph sichtbar)"),
                       run: () => app.requestRelate(document) })
        if (kind === "markdown" && app.hasChecklist(document))
            out.push({ icon: "check-square", name: qsTr("Checkliste als Aufgaben"), enabled: true, tip: qsTr("Punkte in den Kalender übernehmen"),
                       run: () => actions.calendarSync.importChecklist(document) })
        if (document)
            out.push({ icon: "calendar", name: qsTr("Als Aufgabe einplanen"), enabled: true, tip: qsTr("Aufgabe im Kalender anlegen"),
                       run: () => actions.planAsTask(document) })
        Timing.took("Kontextleiste: eingebaute Aktionen", t)
        return out
    }

    // ------------------------------------------------------------ layout

    readonly property var config: { try { return JSON.parse(app.contextModules || "{}") } catch (e) { return {} } }
    /// Extensions with views or commands ({name, title, shown, entries}).
    readonly property var extensionList: {
        app.contextModules; app.untrustedExtensions
        try { return JSON.parse(app.contextExtensions()).filter(e => e.entries.length > 0) } catch (e) { return [] }
    }
    /// File type of the active tab and the profile used for it.
    readonly property string kind: ContextProfiles.kindOf(activeTab)
    readonly property string profileName: ContextProfiles.profileFor(config, kind)
    readonly property var layout: ContextProfiles.layout(config, profileName, extensionList)
    /// Menus of the blocks change the file type's own profile, unless
    /// „Gilt für: alle Dateitypen“ was chosen (until the type changes).
    property bool scopeAll: false
    onKindChanged: scopeAll = false
    readonly property string editTarget: profileName !== "default" && !scopeAll ? profileName : "default"
    readonly property bool canChooseScope: config.by_kind !== false && kind !== "default"

    /// Tabs: „Übersicht“ (unless empty while there are other tabs), then
    /// the profile's.
    readonly property var tabIds: (layout.overview.length > 0 || layout.tabs.length === 0 ? ["overview"] : []).concat(layout.tabs)
    readonly property string tabId: tabIds[Math.max(0, Math.min(tab, tabIds.length - 1))] || "overview"
    /// The tab chosen by the user, kept when the profile changes.
    property string wantedTab: "overview"
    onTabChanged: if (tabIds[tab]) wantedTab = tabIds[tab]
    onTabIdsChanged: tab = Math.max(0, tabIds.indexOf(wantedTab))
    /// Shows tab `id` (e.g. "actions", "links"); the overview if the
    /// profile has no such tab.
    function showTab(id) { tab = Math.max(0, tabIds.indexOf(id)) }
    function tabTitle(id) { return id === "overview" ? qsTr("Übersicht") : (layout.pages[id] || { title: id }).title }
    function tabIcon(id) {
        if (id === "overview") return "grid"
        if (ContextProfiles.catalog[id]) return ContextProfiles.catalog[id].icon
        const first = ((layout.pages[id] || {}).blocks || [])[0]
        return first ? (ContextProfiles.info(first, extensionList) || { icon: "list" }).icon : "list"
    }
    /// Blocks of the shown page (a tab other than the overview).
    readonly property var page: tabId === "overview" ? null : layout.pages[tabId] || null
    readonly property var pageIds: page ? shownIn(page.blocks) : []
    /// Where a block can be moved to: the overview and every tab.
    readonly property var moveTargets: [{ id: "overview", title: qsTr("Übersicht") }]
        .concat(tabIds.filter(t => t !== "overview").map(t => ({ id: t, title: tabTitle(t) })))

    /// Empty blocks: shown, only their header, or not at all.
    readonly property string emptyMode: config.empty || "collapse"
    function isEmpty(id) {
        if (id === "annotations") return annotations.length === 0
        if (id === "outline") return outline.length === 0
        if (id === "tags") return noteTags.length === 0
        if (id === "actions") return builtins.length === 0 && userActions.length === 0
        if (id === "builtin-actions") return builtins.length === 0
        if (id === "fileinfo") return !document
        return false
    }
    /// Empty blocks opened by hand (until the file changes).
    property var openedEmpty: ({})
    onDocumentChanged: openedEmpty = ({})
    function isCollapsed(id) {
        if (layout.collapsed.indexOf(id) >= 0) return true
        return emptyMode === "collapse" && isEmpty(id) && !openedEmpty[id]
    }
    function shownIn(ids) { return ids.filter(id => !(emptyMode === "hide" && isEmpty(id))) }
    readonly property var overviewIds: shownIn(layout.overview)
    readonly property var pinnedIds: shownIn(layout.pinned)

    // ---- changes: to the file type's own profile, or to all profiles
    function change(fn) {
        if (editTarget === "default") ContextProfiles.editAll(app, config, fn)
        else ContextProfiles.edit(app, config, editTarget, fn)
    }
    function toggleCollapsed(id) {
        if (layout.collapsed.indexOf(id) < 0 && emptyMode === "collapse" && isEmpty(id)) {
            const o = Object.assign({}, openedEmpty)
            o[id] = !o[id]
            openedEmpty = o
            return
        }
        change(l => ContextProfiles.toggle(l.collapsed, id, l.collapsed.indexOf(id) < 0))
    }
    function moveBlock(id, area, delta) {
        const shown = area === "pinned" ? pinnedIds : area === "overview" ? overviewIds : pageIds
        change(l => ContextProfiles.move(l, area, id, delta, shown))
        Qt.callLater(focusHeader, id)
    }
    function blockToTab(id) {
        change(l => { ContextProfiles.unplace(l, id); l.tabs.push(id) })
        Qt.callLater(showTab, id)
    }
    function blockTo(id, area) {
        change(l => {
            ContextProfiles.unplace(l, id)
            // A shown extension card not placed so far keeps its place.
            l[area].push(id)
        })
    }
    /// Moves a block from area `from` (overview, pinned or a tab) to `to`.
    function moveBlockTo(id, from, to) {
        change(l => ContextProfiles.moveTo(l, id, from, to))
    }
    /// Takes a block off a tab (with undo); its page stays.
    function removeFromPage(id, page) {
        const snap = ContextProfiles.snapshot(config)
        change(l => { const list = ContextProfiles.areaList(l, page); const i = list.indexOf(id); if (i >= 0) list.splice(i, 1) })
        undoHide = { snapshot: snap, name: (ContextProfiles.info(id, extensionList) || { name: id }).name }
        undoTimer.restart()
    }
    /// A new tab of one's own (named by the user), shown at once.
    function addPage(title, firstBlock, from) {
        let id = ""
        change(l => {
            id = ContextProfiles.newPage(l, title)
            if (firstBlock) ContextProfiles.moveTo(l, firstBlock, from, id)
        })
        Qt.callLater(showTab, id)
    }
    function renamePage(id, title) { change(l => ContextProfiles.renamePage(l, id, title)) }
    property var undoHide: null
    function hideBlock(id) {
        const snap = ContextProfiles.snapshot(config)
        change(l => ContextProfiles.toggle(l.hidden, id, true))
        undoHide = { snapshot: snap, name: (ContextProfiles.info(id, extensionList) || { name: id }).name }
        undoTimer.restart()
    }
    /// Removes tab `id` (the block keeps its place in the overview, if any).
    function hideTab(id) {
        const snap = ContextProfiles.snapshot(config)
        const title = tabTitle(id)
        change(l => ContextProfiles.removeTab(l, id))
        undoHide = { snapshot: snap, name: qsTr("Reiter %1").arg(title) }
        undoTimer.restart()
    }
    function undoHiding() {
        if (!undoHide) return
        ContextProfiles.restore(app, config, undoHide.snapshot)
        undoHide = null
    }
    Timer { id: undoTimer; interval: 8000; onTriggered: inspector.undoHide = null }
    function chooseScope(own) {
        if (own) {
            if (!ContextProfiles.isSet(config, kind)) ContextProfiles.save(app, config, kind, ContextProfiles.lists(config, "default"))
            scopeAll = false
        } else scopeAll = true
    }
    /// Opens the editing mode for the profile of the active file.
    function customize(name) { contextEditor.start(name || profileName) }
    function focusHeader(id) {
        for (let i = 0; i < overviewRepeater.count; ++i) {
            const b = overviewRepeater.itemAt(i)
            if (b && b.blockId === id) { b.header.forceActiveFocus(); return }
        }
    }

    // ---- data of the new blocks
    /// Headings of the note or bookmarks of the PDF: [{title, depth, page, line}].
    readonly property var outline: {
        app.notesRevision
        if (activeTab && activeTab.kind === "pdf" && pdfViewer) {
            try { return JSON.parse(pdfViewer.outlineJson || "[]").map(o => ({ title: o.title, depth: o.depth, page: o.page, line: -1 })) } catch (e) { return [] }
        }
        if (document && actions.isMarkdown(document)) {
            const out = []
            let fence = false
            const lines = app.readTextFile(document).split("\n")
            for (let i = 0; i < lines.length; ++i) {
                if (/^\s*(```|~~~)/.test(lines[i])) fence = !fence
                const m = !fence && /^(#{1,6})\s+(.*?)\s*#*\s*$/.exec(lines[i])
                if (m && m[2]) out.push({ title: m[2], depth: m[1].length - 1, page: -1, line: i + 1 })
            }
            const top = Math.min(...out.map(o => o.depth))
            return out.map(o => Object.assign(o, { depth: o.depth - top }))
        }
        return []
    }
    /// The outline entry of the current PDF page (the last one at or before it).
    readonly property int outlineCurrent: {
        if (!pdfViewer || !activeTab || activeTab.kind !== "pdf") return -1
        let at = -1
        outline.forEach((o, i) => { if (o.page >= 0 && o.page <= pdfViewer.currentPage) at = i })
        return at
    }
    function openOutline(o) {
        if (o.page >= 0 && pdfViewer) pdfViewer.goToPage(o.page)
        else if (o.line > 0) openRequested(document, o.line, "tab", {})
    }
    readonly property var noteTags: {
        app.notesRevision
        return document && actions.isMarkdown(document) ? app.tagsOfNote(document) : []
    }
    readonly property var fileInfo: {
        app.notesRevision
        if (!document || !actions.ops) return null
        try { const i = JSON.parse(actions.ops.properties(document)); return i.error ? null : i } catch (e) { return null }
    }

    // --------------------------------------------------------------- view

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 1
        spacing: 0

        // Tabs with an accent underline; when the bar is too narrow for the
        // names (or as set), the tabs not shown show only their icon.
        Item {
            id: tabArea
            visible: !contextEditor.active
            Layout.fillWidth: true
            Layout.preferredHeight: 42 + (tabFlick.contentWidth > tabFlick.width ? 6 : 0)
            FontMetrics { id: tabMetrics; font.pixelSize: Theme.fontSize }
            readonly property real textWidth: inspector.tabIds.reduce((w, id) => w + tabMetrics.advanceWidth(inspector.tabTitle(id)) + 18, 0)
            readonly property bool icons: inspector.config.tab_labels === "icons"
                || (inspector.config.tab_labels !== "text" && textWidth > tabFlick.width)
            Flickable {
                id: tabFlick
                anchors.fill: parent
                anchors.leftMargin: 10
                anchors.rightMargin: 34
                contentWidth: tabRow.width
                contentHeight: height
                flickableDirection: Flickable.HorizontalFlick
                boundsBehavior: Flickable.StopAtBounds
                clip: true
                ScrollBar.horizontal: ScrollBar {
                    policy: tabFlick.contentWidth > tabFlick.width ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff
                    height: 5
                }
                // The wheel scrolls sideways (all devices: Wayland reports
                // wheels as a touchpad).
                WheelHandler {
                    acceptedDevices: PointerDevice.AllDevices
                    acceptedPointerTypes: PointerDevice.AllPointerTypes
                    onWheel: (event) => {
                        const d = (event.angleDelta.y || event.angleDelta.x) / 2 || event.pixelDelta.y || event.pixelDelta.x
                        tabFlick.contentX = Math.max(0, Math.min(tabFlick.contentWidth - tabFlick.width, tabFlick.contentX - d))
                    }
                }
                /// Scrolls the tab at `index` into view.
                function reveal(index) {
                    const item = tabRepeater.itemAt(index)
                    if (!item) return
                    if (item.x < contentX) contentX = item.x
                    else if (item.x + item.width > contentX + width) contentX = Math.min(contentWidth - width, item.x + item.width - width)
                }
                Row {
                    id: tabRow
                    y: 2
                    spacing: 2
                    Repeater {
                        id: tabRepeater
                        model: inspector.tabIds
                        Item {
                            id: tabItem
                            required property string modelData
                            required property int index
                            readonly property bool current: inspector.tab === index
                            readonly property bool iconOnly: tabArea.icons && !current
                            width: iconOnly ? 32 : tabLabel.implicitWidth + 16
                            height: 40
                            Text {
                                id: tabLabel
                                visible: !tabItem.iconOnly
                                anchors.centerIn: parent
                                text: inspector.tabTitle(tabItem.modelData)
                                color: tabItem.current ? Theme.accent : tabHover.hovered ? Theme.textStrong : Theme.text
                                font.pixelSize: Theme.fontSize
                            }
                            Icon {
                                visible: tabItem.iconOnly
                                anchors.centerIn: parent
                                name: inspector.tabIcon(tabItem.modelData)
                                size: 16
                                color: tabHover.hovered ? Theme.textStrong : Theme.text
                            }
                            Rectangle {
                                anchors.bottom: parent.bottom
                                width: parent.width
                                height: 2
                                radius: 1
                                color: Theme.accent
                                visible: tabItem.current
                            }
                            HoverHandler { id: tabHover }
                            ToolTip.visible: tabItem.iconOnly && tabHover.hovered
                            ToolTip.delay: 400
                            ToolTip.text: inspector.tabTitle(tabItem.modelData)
                            MouseArea {
                                anchors.fill: parent
                                acceptedButtons: Qt.LeftButton | Qt.RightButton
                                onClicked: (mouse) => {
                                    if (mouse.button === Qt.RightButton && tabItem.modelData !== "overview") {
                                        tabMenu.blockId = tabItem.modelData
                                        tabMenu.popup()
                                    } else inspector.tab = tabItem.index
                                }
                            }
                        }
                    }
                }
            }
            IconButton {
                anchors.right: parent.right
                anchors.rightMargin: 4
                y: 8
                iconName: "settings"
                iconSize: 14
                tip: qsTr("Leiste anpassen")
                onClicked: inspector.customize()
            }
            Connections {
                target: inspector
                function onTabChanged() { Qt.callLater(tabFlick.reveal, inspector.tab) }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }
        // Which profile applies, shown briefly when it changes.
        Rectangle {
            id: profileStrip
            visible: !contextEditor.active && opacity > 0
            opacity: 0
            Layout.fillWidth: true
            Layout.preferredHeight: visible ? 22 : 0
            color: Theme.accentSoft
            Text {
                anchors.centerIn: parent
                text: qsTr("Profil: %1").arg(ContextProfiles.title(inspector.profileName))
                color: Theme.accent
                font.pixelSize: Theme.smallFontSize
            }
            Behavior on opacity { NumberAnimation { duration: 200 } }
            Timer { id: stripTimer; interval: 1600; onTriggered: profileStrip.opacity = 0 }
            Connections {
                target: inspector
                function onProfileNameChanged() { profileStrip.opacity = 1; stripTimer.restart() }
            }
        }

        StackLayout {
            visible: !contextEditor.active
            Layout.fillWidth: true
            Layout.fillHeight: true
            // A page with a single big block (links, graph) shows it at full
            // height; any other page its blocks like the overview.
            currentIndex: inspector.tabId === "overview" ? 0
                : inspector.page && inspector.page.full ? (inspector.page.blocks[0] === "links" ? 1 : 2) : 3

            // ---- overview: the blocks
            ScrollView {
                id: overview
                contentWidth: availableWidth
                clip: true
                ColumnLayout {
                    width: overview.availableWidth
                    spacing: 8
                    Item { Layout.preferredHeight: 2 }
                    Repeater {
                        id: overviewRepeater
                        model: inspector.overviewIds
                        delegate: BlockView { area: "overview" }
                    }
                    Label {
                        visible: inspector.overviewIds.length === 0
                        Layout.fillWidth: true
                        Layout.margins: 14
                        wrapMode: Text.Wrap
                        color: Theme.textFaint
                        font.pixelSize: Theme.smallFontSize
                        text: qsTr("Keine Bausteine in der Übersicht. „Leiste anpassen“ fügt welche hinzu.")
                    }
                    Item { Layout.preferredHeight: 8 }
                }
            }

            // ---- links
            ContextPanel {
                id: linksPanel
                lazy: true
                app: inspector.app
                path: inspector.document
                tab: 0
                showTabs: false
                color: "transparent"
                onOpenRequested: (path, line, newPane, state) => inspector.openRequested(path, line, newPane ? "split" : "tab", state)
            }

            // ---- graph
            ContextPanel {
                lazy: true
                app: inspector.app
                path: inspector.document
                tab: 1
                showTabs: false
                color: "transparent"
                onOpenRequested: (path, line, newPane, state) => inspector.openRequested(path, line, newPane ? "split" : "tab", state)
            }

            // ---- a page of blocks (Aktionen, tabs of one's own, a block as a tab)
            ScrollView {
                id: pageView
                contentWidth: availableWidth
                clip: true
                ColumnLayout {
                    width: pageView.availableWidth
                    spacing: 8
                    Item { Layout.preferredHeight: 2 }
                    Repeater {
                        model: inspector.pageIds
                        delegate: BlockView { area: inspector.tabId }
                    }
                    Label {
                        visible: inspector.pageIds.length === 0
                        Layout.fillWidth: true
                        Layout.margins: 14
                        wrapMode: Text.Wrap
                        color: Theme.textFaint
                        font.pixelSize: Theme.smallFontSize
                        text: qsTr("Dieser Reiter ist leer. „Leiste anpassen“ fügt Bausteine hinzu, oder „Verschieben nach …“ im ⋯-Menü eines Bausteins.")
                    }
                    Item { Layout.preferredHeight: 8 }
                }
            }
        }

        // The editing mode replaces tabs and overview.
        ContextEditor {
            id: contextEditor
            visible: active
            Layout.fillWidth: true
            Layout.fillHeight: true
            app: inspector.app
            config: inspector.config
            extensions: inspector.extensionList
            currentProfile: inspector.profileName
        }

        // „Ausgeblendet · Rückgängig“
        Rectangle {
            visible: !!inspector.undoHide && !contextEditor.active
            Layout.fillWidth: true
            Layout.leftMargin: 10
            Layout.rightMargin: 10
            Layout.topMargin: 6
            Layout.preferredHeight: 32
            radius: 8
            color: Theme.raised
            border.color: Theme.border
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 10
                anchors.rightMargin: 4
                Text {
                    Layout.fillWidth: true
                    text: inspector.undoHide ? qsTr("„%1“ ausgeblendet").arg(inspector.undoHide.name) : ""
                    color: Theme.text
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                }
                Button { flat: true; text: qsTr("Rückgängig"); onClicked: inspector.undoHiding() }
            }
        }

        // Pinned to the bottom edge, on every tab.
        ColumnLayout {
            visible: !contextEditor.active && inspector.pinnedIds.length > 0
            Layout.fillWidth: true
            Layout.topMargin: 6
            spacing: 6
            Repeater {
                model: inspector.pinnedIds
                delegate: Loader {
                    id: pinnedLoader
                    required property string modelData
                    required property int index
                    Layout.fillWidth: true
                    Layout.leftMargin: modelData === "pomodoro" ? 10 : 0
                    Layout.rightMargin: modelData === "pomodoro" ? 10 : 0
                    Layout.bottomMargin: index === inspector.pinnedIds.length - 1 ? 10 : 0
                    Layout.preferredHeight: item ? item.implicitHeight : 0
                    sourceComponent: modelData === "pomodoro" ? pomodoroSlot : pinnedBlock
                    Component {
                        id: pinnedBlock
                        BlockView { modelData: pinnedLoader.modelData; index: pinnedLoader.index; area: "pinned" }
                    }
                }
            }
        }
    }

    // Data of „Heute“, „Diese Woche“ and „Aufgaben“ (loaded only when placed).
    ContextModules {
        id: contextModules
        app: inspector.app
        actions: inspector.actions
        showCalendar: inspector.overviewIds.concat(inspector.pinnedIds).indexOf("today") >= 0
        showWeek: inspector.overviewIds.concat(inspector.pinnedIds).indexOf("week") >= 0
        showTasks: inspector.overviewIds.concat(inspector.pinnedIds).indexOf("tasks") >= 0
    }

    // The one pomodoro timer: it keeps running while it moves between the
    // bottom edge and the overview, or is hidden.
    Item { id: pomodoroHome; visible: false }
    PomodoroTimer {
        id: pomodoroTimer
        parent: pomodoroHome
        app: inspector.app
        actions: inspector.actions
    }
    Component {
        id: pomodoroSlot
        Item {
            id: slot
            implicitHeight: pomodoroTimer.implicitHeight
            Component.onCompleted: {
                pomodoroTimer.parent = slot
                pomodoroTimer.x = 0
                pomodoroTimer.width = Qt.binding(() => slot.width)
            }
            Component.onDestruction: if (pomodoroTimer.parent === slot) pomodoroTimer.parent = pomodoroHome
        }
    }

    Menu {
        id: tabMenu
        property string blockId: ""
        readonly property var page: inspector.layout.pages[blockId] || { blocks: [] }
        readonly property bool single: page.blocks.length === 1 && page.blocks[0] === blockId
        MenuItem { text: qsTr("Umbenennen …"); onTriggered: pageDialog.ask(tabMenu.blockId) }
        MenuItem {
            text: qsTr("In die Übersicht")
            visible: tabMenu.single
            height: visible ? implicitHeight : 0
            onTriggered: inspector.blockTo(tabMenu.blockId, "overview")
        }
        MenuItem { text: qsTr("Reiter ausblenden"); onTriggered: inspector.hideTab(tabMenu.blockId) }
        MenuSeparator {}
        MenuItem { text: qsTr("Neuer Reiter …"); onTriggered: pageDialog.ask("") }
        MenuItem { text: qsTr("Leiste anpassen …"); onTriggered: inspector.customize() }
    }

    /// Name of a new tab, or a new name for one: with `block`/`from`, the
    /// block moves to the new tab.
    Popup {
        id: pageDialog
        property string pageId: ""
        property string block: ""
        property string from: ""
        parent: Overlay.overlay
        anchors.centerIn: parent
        width: 320
        modal: true
        focus: true
        function ask(id, block, from) {
            pageId = id
            pageDialog.block = block || ""
            pageDialog.from = from || ""
            nameField.text = id ? inspector.tabTitle(id) : ""
            open()
            nameField.forceActiveFocus()
            nameField.selectAll()
        }
        function accept() {
            const title = nameField.text.trim()
            if (!title) return
            if (pageId) inspector.renamePage(pageId, title)
            else inspector.addPage(title, block, from)
            close()
        }
        ColumnLayout {
            anchors.fill: parent
            spacing: 10
            Label {
                text: pageDialog.pageId ? qsTr("Reiter umbenennen") : qsTr("Neuer Reiter")
                color: Theme.textStrong
                font.bold: true
            }
            TextField {
                id: nameField
                Layout.fillWidth: true
                placeholderText: qsTr("Name, z. B. Lesen")
                onAccepted: pageDialog.accept()
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Button { flat: true; text: qsTr("Abbrechen"); onClicked: pageDialog.close() }
                Button { highlighted: true; text: pageDialog.pageId ? qsTr("Umbenennen") : qsTr("Anlegen"); enabled: nameField.text.trim() !== ""; onClicked: pageDialog.accept() }
            }
        }
    }
    readonly property alias pageDialog: pageDialog

    /// Content of a block (the views each block shows below its header).
    function contentOf(id) {
        if (id.startsWith("ext:")) return extensionContent
        return ({ annotations: annotationsContent, outline: outlineContent, links: linksContent, tags: tagsContent,
                  fileinfo: fileinfoContent, graph: graphContent, actions: actionsContent,
                  "builtin-actions": builtinActionsContent, "user-actions": userActionsContent, today: contextModules.todayView,
                  week: contextModules.weekView, tasks: contextModules.tasksView, pomodoro: pomodoroSlot })[id] || null
    }
    /// Number or date next to a block's name.
    function countOf(id) {
        switch (id) {
        case "annotations": return annotations.length ? String(annotations.length) : ""
        case "outline": return outline.length ? String(outline.length) : ""
        case "tasks": return contextModules.openTasks.length ? String(contextModules.openTasks.length) : ""
        case "today": return contextModules.todayText
        case "week": return contextModules.weekCount ? String(contextModules.weekCount) : ""
        case "tags": return noteTags.length ? String(noteTags.length) : ""
        default: return ""
        }
    }

    /// A block placed in the overview or at the bottom.
    component BlockView: ContextBlock {
        id: view
        required property string modelData
        required property int index
        blockId: modelData
        info: ContextProfiles.info(modelData, inspector.extensionList) || { name: modelData, icon: "grid" }
        content: inspector.contentOf(modelData)
        count: inspector.countOf(modelData)
        empty: inspector.isEmpty(modelData)
        collapsed: inspector.isCollapsed(modelData)
        readonly property var siblings: area === "pinned" ? inspector.pinnedIds : area === "overview" ? inspector.overviewIds : inspector.pageIds
        canMoveUp: siblings.indexOf(modelData) > 0
        canMoveDown: siblings.indexOf(modelData) >= 0 && siblings.indexOf(modelData) < siblings.length - 1
        profileTitle: ContextProfiles.title(inspector.kind)
        ownProfile: inspector.editTarget === inspector.kind
        canChooseScope: inspector.canChooseScope
        onToggleCollapsed: inspector.toggleCollapsed(modelData)
        onMoveBy: (delta) => inspector.moveBlock(modelData, area, delta)
        onToTab: inspector.blockToTab(modelData)
        onToPinned: inspector.blockTo(modelData, "pinned")
        onToOverview: inspector.blockTo(modelData, "overview")
        onHide: area === "overview" || area === "pinned" ? inspector.hideBlock(modelData) : inspector.removeFromPage(modelData, area)
        // Not where it is already (graph is on the „Graph“ tab).
        targets: inspector.moveTargets.filter(t => t.id !== area
            && (t.id === "overview" ? inspector.layout.overview : (inspector.layout.pages[t.id] || { blocks: [] }).blocks).indexOf(modelData) < 0)
        onMoveTo: (target) => target === "" ? inspector.pageDialog.ask("", modelData, area) : inspector.moveBlockTo(modelData, area, target)
        onCustomize: inspector.customize()
        onScopeChosen: (own) => inspector.chooseScope(own)
        tools: [
            IconButton {
                visible: view.modelData === "graph"
                width: visible ? implicitWidth : 0
                iconName: "maximize"
                iconSize: 13
                tip: qsTr("Groß anzeigen (eigenes Fenster)")
                onClicked: graphDialog.show(inspector.document)
            },
            IconButton {
                visible: view.modelData === "today" || view.modelData === "week"
                width: visible ? implicitWidth : 0
                iconName: "chevron-right"
                iconSize: 13
                tip: qsTr("Kalender öffnen")
                onClicked: inspector.actions.calendarRequested({})
            }
        ]
    }

    // ---- block contents

    Component {
        id: annotationsContent
        ColumnLayout {
            spacing: 6
            Repeater {
                model: inspector.annotations.slice(0, 6)
                AnnotationCard { Layout.leftMargin: 8; Layout.rightMargin: 8 }
            }
            Label {
                Layout.fillWidth: true
                Layout.leftMargin: 12
                Layout.rightMargin: 12
                visible: inspector.annotations.length === 0
                wrapMode: Text.Wrap
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
                text: inspector.pdfPath
                    ? qsTr("Noch keine Anmerkungen in %1. Text in der PDF markieren und kommentieren.").arg(inspector.app.fileName(inspector.pdfPath))
                    : qsTr("Eine PDF öffnen, um ihre Markierungen und Kommentare hier zu sehen.")
            }
            Label {
                Layout.leftMargin: 12
                visible: inspector.annotations.length > 6
                text: qsTr("Alle %1 Anmerkungen in der Seitenleiste der PDF").arg(inspector.annotations.length)
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
            }
        }
    }
    Component {
        id: outlineContent
        Column {
            Repeater {
                model: inspector.outline.slice(0, 60)
                delegate: Rectangle {
                    id: entry
                    required property var modelData
                    required property int index
                    readonly property bool current: index === inspector.outlineCurrent
                    width: parent ? parent.width : 0
                    height: 26
                    color: current ? Theme.selection : entryHover.hovered ? Theme.hover : "transparent"
                    HoverHandler { id: entryHover }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        x: 14 + entry.modelData.depth * 12
                        width: parent.width - x - 36
                        text: entry.modelData.title
                        textFormat: Text.PlainText
                        color: entry.current ? Theme.textStrong : Theme.text
                        font.pixelSize: Theme.smallFontSize + 1
                        font.bold: entry.current
                        elide: Text.ElideRight
                    }
                    Text {
                        anchors.right: parent.right
                        anchors.rightMargin: 12
                        anchors.verticalCenter: parent.verticalCenter
                        text: entry.modelData.page >= 0 ? entry.modelData.page + 1 : ""
                        color: Theme.textFaint
                        font.pixelSize: Theme.smallFontSize
                    }
                    MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: inspector.openOutline(entry.modelData) }
                }
            }
            Label {
                visible: inspector.outline.length === 0
                x: 12
                width: parent ? parent.width - 24 : 0
                wrapMode: Text.Wrap
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
                text: qsTr("Keine Gliederung: Lesezeichen einer PDF oder Überschriften einer Notiz erscheinen hier.")
            }
        }
    }
    Component {
        id: linksContent
        ContextPanel {
            implicitHeight: 280
            app: inspector.app
            path: inspector.document
            tab: 0
            showTabs: false
            compact: true
            color: "transparent"
            onOpenRequested: (path, line, newPane, state) => inspector.openRequested(path, line, newPane ? "split" : "tab", state)
        }
    }
    Component {
        id: tagsContent
        Item {
            implicitHeight: tagFlow.implicitHeight + 6
            Flow {
                id: tagFlow
                x: 12
                width: parent.width - 24
                spacing: 6
                Repeater {
                    model: inspector.noteTags
                    delegate: Rectangle {
                        id: chip
                        required property string modelData
                        width: chipText.implicitWidth + 16
                        height: 24
                        radius: 12
                        color: chipHover.hovered ? Theme.accentSoft : Theme.raised
                        border.color: Theme.border
                        HoverHandler { id: chipHover }
                        Text { id: chipText; anchors.centerIn: parent; text: "#" + chip.modelData; color: Theme.link; font.pixelSize: Theme.smallFontSize }
                        ToolTip.visible: chipHover.hovered
                        ToolTip.delay: 600
                        ToolTip.text: qsTr("Notizen mit #%1 suchen").arg(chip.modelData)
                        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: inspector.tagSearchRequested(chip.modelData) }
                    }
                }
                Label {
                    visible: inspector.noteTags.length === 0
                    color: Theme.textFaint
                    font.pixelSize: Theme.smallFontSize
                    text: qsTr("Keine Tags. In einer Notiz: #tag im Text oder tags: im Kopf.")
                }
            }
        }
    }
    Component {
        id: fileinfoContent
        Item {
            implicitHeight: infoGrid.implicitHeight + 4
        GridLayout {
            id: infoGrid
            x: 12
            width: parent.width - 24
            columns: 2
            columnSpacing: 10
            rowSpacing: 3
            readonly property var i: inspector.fileInfo || {}
            readonly property var rows: !inspector.fileInfo ? [] : [
                [qsTr("Größe"), i.sizeText + (inspector.pdfViewer && inspector.activeTab && inspector.activeTab.kind === "pdf"
                                              ? " · " + qsTr("%n Seite(n)", "", inspector.pdfViewer.pageCount) : "")],
                [qsTr("Geändert"), inspector.when(Math.round(i.modified / 1000))],
                [qsTr("Rechte"), i.modeText || ""],
                [qsTr("Pfad"), inspector.app.relativePath(i.path) || i.path]
            ]
            Repeater {
                model: parent.rows.length * 2
                delegate: Text {
                    required property int index
                    readonly property var row: parent.rows[Math.floor(index / 2)]
                    Layout.fillWidth: index % 2 === 1
                    text: row ? row[index % 2] : ""
                    textFormat: Text.PlainText
                    color: index % 2 ? Theme.text : Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideMiddle
                }
            }
        }
        }
    }
    Component {
        id: graphContent
        ContextPanel {
            implicitHeight: 240
            app: inspector.app
            path: inspector.document
            tab: 1
            showTabs: false
            compact: true
            color: "transparent"
            onOpenRequested: (path, line, newPane, state) => inspector.openRequested(path, line, newPane ? "split" : "tab", state)
        }
    }
    Component {
        id: builtinActionsContent
        ColumnLayout {
            spacing: 4
            Label {
                visible: !!inspector.document
                Layout.leftMargin: 12
                Layout.rightMargin: 12
                Layout.fillWidth: true
                text: inspector.document ? qsTr("Für %1").arg(inspector.app.fileName(inspector.document)) : ""
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
                elide: Text.ElideMiddle
            }
            ActionList { Layout.leftMargin: 4; Layout.rightMargin: 4; items: inspector.builtins }
            Label {
                visible: inspector.builtins.length === 0
                Layout.leftMargin: 12
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
                text: qsTr("Keine eingebauten Aktionen für diese Datei.")
            }
        }
    }
    Component {
        id: userActionsContent
        ColumnLayout {
            spacing: 4
            ActionList { Layout.leftMargin: 4; Layout.rightMargin: 4; items: inspector.userActions }
            Label {
                visible: inspector.userActions.length === 0
                Layout.fillWidth: true
                Layout.leftMargin: 12
                Layout.rightMargin: 12
                wrapMode: Text.Wrap
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
                text: qsTr("Noch keine eigenen Aktionen für diese Datei. Lege welche für das Projekt an oder installiere Erweiterungen.")
            }
            // (Wraps in a narrow bar instead of widening it.)
            Flow {
                Layout.fillWidth: true
                Layout.leftMargin: 4
                Layout.rightMargin: 4
                spacing: 2
                Button { flat: true; text: qsTr("Projektaktionen bearbeiten …"); onClicked: inspector.actions.editActions(false) }
                Button { flat: true; text: qsTr("Erweiterungen …"); onClicked: inspector.actions.extensionsRequested() }
            }
        }
    }
    Component {
        id: actionsContent
        ColumnLayout {
            spacing: 4
            ActionList { Layout.leftMargin: 4; Layout.rightMargin: 4; items: inspector.builtins.concat(inspector.userActions.slice(0, 4)) }
            Label {
                visible: inspector.builtins.length === 0 && inspector.userActions.length === 0
                Layout.leftMargin: 12
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
                text: qsTr("Keine Aktionen für diese Datei.")
            }
        }
    }
    Component {
        id: extensionContent
        ExtensionCards {
            // The block's id ("ext:<name>"), from the BlockView.
            only: parent && parent.parent && parent.parent.blockId ? parent.parent.blockId.slice(4) : ""
            app: inspector.app
            onKeyActionRequested: (binding) => inspector.keyActionRequested(binding)
        }
    }

    GraphDialog {
        id: graphDialog
        parent: Overlay.overlay
        app: inspector.app
        onOpenRequested: (path, line, newPane, state) => inspector.openRequested(path, line, newPane ? "split" : "tab", state)
    }
    readonly property alias graphWindow: graphDialog

    component AnnotationCard: Rectangle {
        required property var modelData
        Layout.fillWidth: true
        Layout.leftMargin: 10
        Layout.rightMargin: 10
        implicitHeight: cardColumn.implicitHeight + 20
        radius: 8
        color: cardHover.hovered ? Theme.hover : Theme.header
        border.color: Theme.border
        HoverHandler { id: cardHover }
        Rectangle {
            x: 12
            y: 14
            width: 14
            height: 14
            radius: 7
            color: modelData.color || "#ffd400"
        }
        ColumnLayout {
            id: cardColumn
            x: 38
            y: 10
            width: parent.width - 38 - 48
            spacing: 3
            Label {
                Layout.fillWidth: true
                text: modelData.comment || modelData.text || modelData.title || qsTr("Lesezeichen")
                color: Theme.textStrong
                font.pixelSize: Theme.fontSize
                wrapMode: Text.Wrap
                maximumLineCount: 3
                elide: Text.ElideRight
            }
            Label {
                Layout.fillWidth: true
                visible: !!modelData.comment && !!modelData.text
                text: "„" + (modelData.text || "") + "“"
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
                maximumLineCount: 2
                wrapMode: Text.Wrap
                elide: Text.ElideRight
            }
            Label {
                text: inspector.when(modelData.modified || modelData.created)
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
            }
        }
        Label {
            anchors.right: parent.right
            anchors.rightMargin: 12
            y: 12
            text: qsTr("S. %1").arg(modelData.page)
            color: Theme.textMuted
            font.pixelSize: Theme.smallFontSize
        }
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: inspector.openAnnotation(parent.modelData) }
    }

    /// The user's actions for the document, as rows of an action list.
    readonly property var userActions: actionList.map(a => ({
        icon: "play", name: a.name, enabled: true, note: "", detail: a.description || a.source, tip: a.description || a.source,
        run: () => inspector.actions.runAction(a.id, a.paths, 0)
    }))

    component SectionLabel: Text {
        Layout.leftMargin: 14
        Layout.rightMargin: 14
        Layout.fillWidth: true
        color: Theme.textMuted
        font.pixelSize: Theme.smallFontSize
        font.weight: Font.DemiBold
        font.capitalization: Font.AllUppercase
        font.letterSpacing: 0.5
        elide: Text.ElideMiddle
    }

    /// Actions as one card with a row each: icon, name and a second line
    /// (why it is not available, or what it does).
    component ActionList: Rectangle {
        id: list
        property var items: []
        visible: items.length > 0
        Layout.fillWidth: true
        Layout.leftMargin: 10
        Layout.rightMargin: 10
        implicitHeight: rows.implicitHeight
        radius: 10
        color: Theme.header
        border.color: Theme.border
        Column {
            id: rows
            width: parent.width
            Repeater {
                model: list.items
                Item {
                    id: row
                    required property var modelData
                    required property int index
                    readonly property string second: !modelData.enabled ? (modelData.note || "") : (modelData.detail || "")
                    width: rows.width
                    height: second ? 50 : 40
                    Rectangle {
                        anchors.fill: parent
                        anchors.margins: 3
                        radius: 7
                        color: rowHover.hovered && row.modelData.enabled ? Theme.hover : "transparent"
                    }
                    HoverHandler { id: rowHover }
                    Icon {
                        x: 14
                        anchors.verticalCenter: parent.verticalCenter
                        name: row.modelData.icon
                        size: 17
                        color: row.modelData.enabled ? Theme.accent : Theme.textFaint
                    }
                    Column {
                        x: 42
                        width: row.width - 54
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 2
                        Text {
                            width: parent.width
                            text: row.modelData.name
                            color: row.modelData.enabled ? Theme.textStrong : Theme.textFaint
                            font.pixelSize: Theme.fontSize
                            elide: Text.ElideRight
                        }
                        Text {
                            visible: row.second !== ""
                            width: parent.width
                            text: row.second
                            color: Theme.textFaint
                            font.pixelSize: Theme.smallFontSize
                            elide: Text.ElideRight
                        }
                    }
                    Rectangle {
                        visible: row.index < list.items.length - 1
                        anchors.bottom: parent.bottom
                        x: 42
                        width: parent.width - 54
                        height: 1
                        color: Theme.border
                    }
                    ToolTip.visible: rowHover.hovered && !!row.modelData.tip
                    ToolTip.delay: 600
                    ToolTip.text: row.modelData.tip || ""
                    MouseArea {
                        anchors.fill: parent
                        enabled: row.modelData.enabled
                        cursorShape: Qt.PointingHandCursor
                        onClicked: row.modelData.run()
                    }
                }
            }
        }
    }
}
