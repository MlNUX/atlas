// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Right context bar for the active document: annotations of the PDF in
// use, links (backlinks with filters), the local graph and user actions.
// The first tab is an overview with all three as cards.
Rectangle {
    id: inspector
    required property AppController app
    required property var editor
    required property var actions
    /// 0 = Anmerkungen (overview), 1 = Verknüpfungen, 2 = Lokaler Graph,
    /// 3 = Aktionen.
    property int tab: 0
    readonly property alias linksPanel: linksPanel
    readonly property alias tabBar: tabFlick

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
        try { return JSON.parse(store.annotationsJson || "[]").filter(a => a.kind !== "ink") } catch (e) { return [] }
    }
    /// User actions for the document, then for the project: {id, name,
    /// description, source, paths}.
    readonly property var actionList: {
        const out = []
        const seen = {}
        for (const paths of document ? [[document], []] : [[]])
            for (const a of actions.actionsFor(paths))
                if (!seen[a.id]) { seen[a.id] = true; out.push(Object.assign({ paths: paths }, a)) }
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
                       tip: qsTr("Markierte Stelle als Zitat in eine Notiz (Strg+Q)"), run: () => pdfViewer.quoteSelection() })
            out.push({ icon: "export", name: qsTr("PDF-Kopie exportieren"), enabled: true,
                       tip: qsTr("Kopie mit Anmerkungen; das Original bleibt unverändert"), run: () => pdfViewer.exportCopy() })
        }
        if (document && app.isRunnable(document))
            out.push({ icon: "play", name: qsTr("Ausführen"), enabled: true, tip: qsTr("Skript im Terminal starten (im Ordner des Skripts)"),
                       run: () => actions.runScript(document) })
        if (document && actions.isHtml(document))
            out.push({ icon: "eye", name: qsTr("Als Webseite anzeigen"), enabled: true, tip: qsTr("HTML gerendert anzeigen (lädt beim Speichern neu)"),
                       run: () => actions.renderHtml(document) })
        if (kind === "markdown")
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
                        model: [qsTr("Anmerkungen"), qsTr("Verknüpfungen"), qsTr("Lokaler Graph"), qsTr("Aktionen")]
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
                        Label { Layout.fillWidth: true; text: qsTr("Lokaler Graph"); color: Theme.textStrong; font.pixelSize: Theme.fontSize; font.bold: true }
                        IconButton { iconName: "maximize"; iconSize: 14; tip: qsTr("Groß anzeigen"); onClicked: inspector.tab = 2 }
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
                    Label { Layout.leftMargin: 14; text: qsTr("Eigene Aktionen"); color: Theme.textStrong; font.pixelSize: Theme.fontSize; font.bold: true }
                    Repeater {
                        model: inspector.builtins.concat(inspector.actionList.slice(0, 4).map(a => ({
                            icon: "play", name: a.name, enabled: true, tip: a.description || a.source,
                            run: () => inspector.actions.runAction(a.id, a.paths, 0)
                        })))
                        ActionCard { }
                    }
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
                    Label {
                        Layout.leftMargin: 14
                        visible: !!inspector.document
                        text: inspector.app.fileName(inspector.document)
                        color: Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                    }
                    Repeater {
                        model: inspector.builtins.concat(inspector.actionList.map(a => ({
                            icon: "play", name: a.name, enabled: true, tip: a.description || a.source,
                            run: () => inspector.actions.runAction(a.id, a.paths, 0)
                        })))
                        ActionCard { }
                    }
                    RowLayout {
                        Layout.leftMargin: 10
                        Layout.rightMargin: 10
                        Button { flat: true; text: qsTr("Projektaktionen bearbeiten …"); onClicked: inspector.actions.editActions(false) }
                        Button { flat: true; text: qsTr("Erweiterungen …"); onClicked: inspector.actions.extensionsRequested() }
                    }
                }
            }
        }
    }

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

    component ActionCard: Rectangle {
        id: card
        required property var modelData
        Layout.fillWidth: true
        Layout.leftMargin: 10
        Layout.rightMargin: 10
        implicitHeight: 38
        radius: 8
        color: !modelData.enabled ? "transparent" : actionHover.hovered ? Theme.hover : Theme.header
        border.color: Theme.border
        opacity: modelData.enabled ? 1 : 0.5
        HoverHandler { id: actionHover }
        Row {
            anchors.verticalCenter: parent.verticalCenter
            x: 14
            spacing: 14
            Icon { name: card.modelData.icon; size: 17; color: Theme.text; anchors.verticalCenter: parent.verticalCenter }
            Label {
                text: card.modelData.name
                color: Theme.textStrong
                font.pixelSize: Theme.fontSize
                width: card.width - 60
                elide: Text.ElideRight
                anchors.verticalCenter: parent.verticalCenter
            }
        }
        ToolTip.visible: actionHover.hovered && !!modelData.tip
        ToolTip.delay: 600
        ToolTip.text: modelData.tip || ""
        MouseArea { anchors.fill: parent; enabled: card.modelData.enabled; cursorShape: Qt.PointingHandCursor; onClicked: card.modelData.run() }
    }
}
