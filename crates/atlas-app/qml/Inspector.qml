// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Right context bar for the active document: annotations of the PDF in
// use, links (backlinks with filters), the project graph and user actions.
// The first tab is an overview with all three as cards.
Rectangle {
    id: inspector
    required property AppController app
    required property var editor
    required property var actions
    /// 0 = Übersicht (modules, annotations, graph, actions), 1 = Verknüpfungen, 2 = Graph,
    /// 3 = Aktionen.
    property int tab: 0
    readonly property alias linksPanel: linksPanel
    readonly property alias tabBar: tabFlick
    readonly property alias modules: contextModules
    readonly property alias pomodoro: pomodoroTimer

    signal openRequested(string path, int line, string where, var state)

    color: Theme.panel
    radius: Theme.radius
    border.color: Theme.border

    // ------------------------------------------------------------ context

    readonly property var activeTab: editor.activeTab
    /// File of the active tab ("" for calendar, terminals, folders).
    readonly property string document: activeTab && activeTab.path && ["pdf", "markdown", "text", "image", "binary"].indexOf(activeTab.kind) >= 0
        ? activeTab.path : lastDocument
    property string lastDocument: ""
    /// The PDF whose annotations are shown: the active one, otherwise the
    /// last PDF that was active.
    property string pdfPath: ""
    onActiveTabChanged: remember()
    Connections { target: inspector.activeTab; ignoreUnknownSignals: true; function onPathChanged() { inspector.remember() } }
    function remember() {
        if (!activeTab || !activeTab.path) return
        if (["pdf", "markdown", "text", "image", "binary"].indexOf(activeTab.kind) >= 0) lastDocument = activeTab.path
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
        const out = []
        const seen = {}
        for (const paths of document ? [[document], []] : [[]])
            for (const a of actions.actionsFor(paths))
                if (!seen[a.id] && a.menu) { seen[a.id] = true; out.push(Object.assign({ paths: paths }, a)) }
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
        if (document && app.atlasDir && app.exists(document))
            out.push({ icon: "link", name: qsTr("Verknüpfen mit …"), enabled: true, tip: qsTr("Mit einer anderen Datei verbinden, z. B. zwei PDFs (im Graph sichtbar)"),
                       run: () => app.requestRelate(document) })
        if (kind === "markdown" && app.hasChecklist(document))
            out.push({ icon: "check-square", name: qsTr("Checkliste als Aufgaben"), enabled: true, tip: qsTr("Punkte in den Kalender übernehmen"),
                       run: () => actions.calendarSync.importChecklist(document) })
        if (document)
            out.push({ icon: "calendar", name: qsTr("Als Aufgabe einplanen"), enabled: true, tip: qsTr("Aufgabe im Kalender anlegen"),
                       run: () => actions.planAsTask(document) })
        return out
    }

    // --------------------------------------------------------------- view

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 1
        spacing: 0

        // Tabs with an accent underline, always written out; a narrow panel
        // scrolls them sideways (thin scroll bar, mouse wheel).
        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: 42 + (tabFlick.contentWidth > tabFlick.width ? 6 : 0)
            Flickable {
                id: tabFlick
                anchors.fill: parent
                anchors.leftMargin: 10
                anchors.rightMargin: 4
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
                        model: [qsTr("Übersicht"), qsTr("Verknüpfungen"), qsTr("Graph"), qsTr("Aktionen")]
                        Item {
                            required property string modelData
                            required property int index
                            width: tabLabel.implicitWidth + 16
                            height: 40
                            Text {
                                id: tabLabel
                                anchors.centerIn: parent
                                text: parent.modelData
                                color: inspector.tab === parent.index ? Theme.accent : tabHover.hovered ? Theme.textStrong : Theme.text
                                font.pixelSize: Theme.fontSize
                            }
                            Rectangle {
                                anchors.bottom: parent.bottom
                                width: parent.width
                                height: 2
                                radius: 1
                                color: Theme.accent
                                visible: inspector.tab === parent.index
                            }
                            HoverHandler { id: tabHover }
                            MouseArea { anchors.fill: parent; onClicked: inspector.tab = parent.index }
                        }
                    }
                }
            }
            Connections {
                target: inspector
                function onTabChanged() { Qt.callLater(tabFlick.reveal, inspector.tab) }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: inspector.tab

            // ---- overview: annotations, graph, actions
            ScrollView {
                id: overview
                contentWidth: availableWidth
                clip: true
                ColumnLayout {
                    width: overview.availableWidth
                    spacing: 10
                    Item { Layout.preferredHeight: 2 }
                    // Today's entries and tasks (each can be switched off in
                    // the settings).
                    ContextModules {
                        id: contextModules
                        Layout.fillWidth: true
                        visible: showCalendar || showTasks
                        app: inspector.app
                        actions: inspector.actions
                    }
                    Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border; visible: contextModules.visible }
                    Label { Layout.leftMargin: 14; text: qsTr("Anmerkungen"); color: Theme.textStrong; font.pixelSize: Theme.fontSize; font.bold: true }
                    Repeater {
                        model: inspector.annotations.slice(0, 6)
                        AnnotationCard { }
                    }
                    Label {
                        Layout.fillWidth: true
                        Layout.margins: 14
                        visible: inspector.annotations.length === 0
                        wrapMode: Text.Wrap
                        color: Theme.textFaint
                        font.pixelSize: Theme.smallFontSize
                        text: inspector.pdfPath
                            ? qsTr("Noch keine Anmerkungen in %1. Text in der PDF markieren und kommentieren.").arg(inspector.app.fileName(inspector.pdfPath))
                            : qsTr("Eine PDF öffnen, um ihre Markierungen und Kommentare hier zu sehen.")
                    }
                    Label {
                        Layout.leftMargin: 14
                        visible: inspector.annotations.length > 6
                        text: qsTr("Alle %1 Anmerkungen in der Seitenleiste der PDF").arg(inspector.annotations.length)
                        color: Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                    }

                    Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.leftMargin: 14
                        Layout.rightMargin: 8
                        Label { Layout.fillWidth: true; text: qsTr("Graph"); color: Theme.textStrong; font.pixelSize: Theme.fontSize; font.bold: true }
                        IconButton { iconName: "maximize"; iconSize: 14; tip: qsTr("Groß anzeigen (eigenes Fenster)"); onClicked: graphDialog.show(inspector.document) }
                    }
                    ContextPanel {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 240
                        app: inspector.app
                        path: inspector.document
                        tab: 1
                        showTabs: false
                        compact: true
                        color: "transparent"
                        onOpenRequested: (path, line, newPane, state) => inspector.openRequested(path, line, newPane ? "split" : "tab", state)
                    }

                    Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }
                    Label { Layout.leftMargin: 14; text: qsTr("Aktionen"); color: Theme.textStrong; font.pixelSize: Theme.fontSize; font.bold: true }
                    ActionList { items: inspector.builtins.concat(inspector.userActions.slice(0, 4)) }
                    Item { Layout.preferredHeight: 8 }
                }
            }

            // ---- links
            ContextPanel {
                id: linksPanel
                app: inspector.app
                path: inspector.document
                tab: 0
                showTabs: false
                color: "transparent"
                onOpenRequested: (path, line, newPane, state) => inspector.openRequested(path, line, newPane ? "split" : "tab", state)
            }

            // ---- graph
            ContextPanel {
                app: inspector.app
                path: inspector.document
                tab: 1
                showTabs: false
                color: "transparent"
                onOpenRequested: (path, line, newPane, state) => inspector.openRequested(path, line, newPane ? "split" : "tab", state)
            }

            // ---- actions
            ScrollView {
                id: actionsView
                contentWidth: availableWidth
                clip: true
                ColumnLayout {
                    width: actionsView.availableWidth
                    spacing: 8
                    Item { Layout.preferredHeight: 4 }
                    SectionLabel {
                        visible: inspector.builtins.length > 0
                        text: inspector.document ? qsTr("Für %1").arg(inspector.app.fileName(inspector.document)) : ""
                    }
                    ActionList { items: inspector.builtins }
                    SectionLabel { text: qsTr("Eigene Aktionen"); Layout.topMargin: 6 }
                    ActionList { items: inspector.userActions }
                    Label {
                        visible: inspector.userActions.length === 0
                        Layout.fillWidth: true
                        Layout.leftMargin: 14
                        Layout.rightMargin: 14
                        wrapMode: Text.Wrap
                        color: Theme.textFaint
                        font.pixelSize: Theme.smallFontSize
                        text: qsTr("Noch keine eigenen Aktionen für diese Datei. Lege welche für das Projekt an oder installiere Erweiterungen.")
                    }
                    // (Wraps in a narrow bar instead of widening it.)
                    Flow {
                        Layout.fillWidth: true
                        Layout.leftMargin: 6
                        Layout.rightMargin: 6
                        spacing: 2
                        Button { flat: true; text: qsTr("Projektaktionen bearbeiten …"); onClicked: inspector.actions.editActions(false) }
                        Button { flat: true; text: qsTr("Erweiterungen …"); onClicked: inspector.actions.extensionsRequested() }
                    }
                }
            }
        }

        // Pomodoro, fixed to the bottom edge on every tab.
        PomodoroTimer {
            id: pomodoroTimer
            Layout.fillWidth: true
            Layout.margins: 10
            visible: pomodoroTimer.settings.pomodoro !== false
            app: inspector.app
            actions: inspector.actions
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
