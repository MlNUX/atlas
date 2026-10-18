// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import Atlas

// PDF viewer: pages, thumbnails, outline, search, zoom. The last page and
// zoom of every document are remembered (`.atlas/metadata.sqlite`).
Rectangle {
    id: root
    required property string source
    property var app: null
    /// 0 = fit width, -1 = fit page, otherwise the zoom factor.
    property real zoom: 0
    property int lastRenderMs: 0
    property bool sidebarVisible: false
    property int sidebarTab: 0
    readonly property int currentPage: Math.max(0, pages.indexAt(10, pages.contentY + pages.height / 3))

    // Search state.
    property string searchQuery: ""
    property var hits: []
    property var hitsByPage: ({})
    property int currentHit: -1
    property int searchMs: 0

    color: Theme.pdfBackdrop
    focus: true

    function viewState() {
        return { zoom: root.zoom, page: currentPage, sidebar: sidebarVisible, sidebarTab: sidebarTab }
    }
    function applyState(state) {
        let s = state || {}
        // Without a page from the session, continue where this document was
        // last read.
        if (s.page === undefined && root.app && root.source) {
            try { s = Object.assign(JSON.parse(root.app.documentState(root.source) || "{}"), s) } catch (e) {}
        }
        if (s.zoom !== undefined) root.zoom = s.zoom
        if (s.sidebar !== undefined) root.sidebarVisible = s.sidebar
        if (s.sidebarTab !== undefined) root.sidebarTab = s.sidebarTab
        restorePage = s.page || 0
        if (doc.status === "ready") Qt.callLater(root.restore)
    }
    /// Jump target from links (`datei.pdf#page=3`): 1-based page.
    function reveal(page) { if (page > 0) goToPage(page - 1) }

    property int restorePage: 0
    function restore() {
        if (restorePage > 0) goToPage(restorePage)
        restorePage = 0
    }
    function goToPage(index) {
        const i = Math.max(0, Math.min(doc.pageCount - 1, index))
        pages.positionViewAtIndex(i, ListView.Beginning)
    }

    // Remember page and zoom per document (debounced).
    Timer {
        id: rememberTimer
        interval: 800
        onTriggered: if (root.app && doc.status === "ready")
            root.app.saveDocumentState(root.source, JSON.stringify({ page: root.currentPage, zoom: root.zoom }))
    }
    onCurrentPageChanged: rememberTimer.restart()
    onZoomChanged: rememberTimer.restart()

    PdfDocument {
        id: doc
        source: root.source
        onStatusChanged: if (status === "ready") Qt.callLater(root.restore)
        onTextReady: (page, text, ms) => {
            textPopup.title = qsTr("Text von Seite %1 (%2 ms)").arg(page + 1).arg(ms)
            textPopup.text = text
            textPopup.open()
        }
        onSearchFinished: (query, hitsJson, ms) => {
            if (query !== root.searchQuery) return
            const list = JSON.parse(hitsJson)
            const byPage = {}
            list.forEach((h, i) => { h.index = i; (byPage[h.page] = byPage[h.page] || []).push(h) })
            root.hits = list
            root.hitsByPage = byPage
            root.searchMs = ms
            // Start with the first hit at or after the current page.
            const first = list.findIndex(h => h.page >= root.currentPage)
            root.showHit(list.length ? (first >= 0 ? first : 0) : -1)
        }
    }

    function search(text) {
        searchQuery = text
        if (!text) { hits = []; hitsByPage = ({}); currentHit = -1; return }
        doc.search(text)
    }
    function showHit(i) {
        currentHit = i
        if (i < 0 || i >= hits.length) return
        const h = hits[i]
        // Scroll so the hit is in the upper third.
        pages.positionViewAtIndex(h.page, ListView.Beginning)
        pages.contentY = Math.max(pages.originY, pages.contentY + h.y * zoomScale - pages.height / 3)
    }
    function nextHit(step) {
        if (hits.length) showHit((currentHit + step + hits.length) % hits.length)
    }

    readonly property real fitScale: doc.maxPageWidth > 0 ? Math.max(0.1, (pages.width - 32) / doc.maxPageWidth) : 1
    readonly property real fitPageScale: doc.pageCount > 0
        ? Math.max(0.1, Math.min(fitScale, (pages.height - 24) / doc.pageHeight(currentPage))) : 1
    readonly property real zoomScale: zoom > 0 ? zoom : zoom < 0 ? fitPageScale : fitScale
    // Rendering follows the visible scale with a short delay so resizing a
    // split does not re-render every page on every frame.
    property real renderScale: zoomScale
    onZoomScaleChanged: renderTimer.restart()
    Timer { id: renderTimer; interval: 120; onTriggered: root.renderScale = root.zoomScale }

    function zoomBy(factor) { zoom = Math.max(0.2, Math.min(6, zoomScale * factor)) }

    Keys.onPressed: (event) => {
        const ctrl = event.modifiers & Qt.ControlModifier
        if (event.key === Qt.Key_PageDown) goToPage(currentPage + 1)
        else if (event.key === Qt.Key_PageUp) goToPage(currentPage - 1)
        else if (event.key === Qt.Key_Home && ctrl) goToPage(0)
        else if (event.key === Qt.Key_End && ctrl) goToPage(doc.pageCount - 1)
        else if (ctrl && event.key === Qt.Key_F) { searchField.forceActiveFocus(); searchField.selectAll() }
        else if (event.key === Qt.Key_F3) nextHit(event.modifiers & Qt.ShiftModifier ? -1 : 1)
        else if (ctrl && (event.key === Qt.Key_Plus || event.key === Qt.Key_Equal)) zoomBy(1.2)
        else if (ctrl && event.key === Qt.Key_Minus) zoomBy(1 / 1.2)
        else if (ctrl && event.key === Qt.Key_0) zoom = 0
        else return
        event.accepted = true
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 32
            color: Theme.header
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 6
                anchors.rightMargin: 6
                spacing: 3
                IconButton {
                    glyph: "☰"
                    tip: qsTr("Seitenleiste: Miniaturen und Inhalt")
                    checkable: true
                    checked: root.sidebarVisible
                    onClicked: root.sidebarVisible = !root.sidebarVisible
                }
                TextField {
                    id: pageField
                    Layout.preferredWidth: 44
                    implicitHeight: 24
                    horizontalAlignment: Text.AlignHCenter
                    font.pixelSize: Theme.smallFontSize
                    text: doc.pageCount > 0 ? String(root.currentPage + 1) : ""
                    validator: IntValidator { bottom: 1; top: Math.max(1, doc.pageCount) }
                    onAccepted: { root.goToPage(parseInt(text) - 1); root.forceActiveFocus() }
                }
                Label {
                    text: "/ " + doc.pageCount
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                }
                IconButton { glyph: "−"; tip: qsTr("Verkleinern (Strg+−)"); onClicked: root.zoomBy(1 / 1.2) }
                Label {
                    text: Math.round(root.zoomScale * 100) + " %"
                    color: Theme.text
                    font.pixelSize: Theme.smallFontSize
                    Layout.preferredWidth: 40
                    horizontalAlignment: Text.AlignHCenter
                }
                IconButton { glyph: "+"; tip: qsTr("Vergrößern (Strg++)"); onClicked: root.zoomBy(1.2) }
                IconButton { glyph: "↔"; tip: qsTr("Seitenbreite (Strg+0)"); checkable: true; checked: root.zoom === 0; onClicked: root.zoom = 0 }
                IconButton { glyph: "▯"; tip: qsTr("Ganze Seite"); checkable: true; checked: root.zoom < 0; onClicked: root.zoom = -1 }
                Item { Layout.fillWidth: true }
                TextField {
                    id: searchField
                    Layout.preferredWidth: Math.min(220, root.width * 0.3)
                    implicitHeight: 24
                    font.pixelSize: Theme.smallFontSize
                    placeholderText: qsTr("Im Dokument suchen (Strg+F)")
                    onTextEdited: searchDebounce.restart()
                    onAccepted: { searchDebounce.stop(); if (text === root.searchQuery) root.nextHit(1); else root.search(text) }
                    Keys.onEscapePressed: { text = ""; root.search(""); root.forceActiveFocus() }
                    Timer { id: searchDebounce; interval: 300; onTriggered: root.search(searchField.text) }
                }
                Label {
                    visible: root.searchQuery.length > 0
                    text: root.hits.length ? qsTr("%1/%2").arg(root.currentHit + 1).arg(root.hits.length) : qsTr("0 Treffer")
                    color: root.hits.length ? Theme.textMuted : Theme.warn
                    font.pixelSize: Theme.smallFontSize
                }
                IconButton { glyph: "↑"; tip: qsTr("Vorheriger Treffer (Umschalt+F3)"); enabled: root.hits.length > 0; onClicked: root.nextHit(-1) }
                IconButton { glyph: "↓"; tip: qsTr("Nächster Treffer (F3)"); enabled: root.hits.length > 0; onClicked: root.nextHit(1) }
                IconButton {
                    glyph: "T"
                    tip: qsTr("Text der aktuellen Seite")
                    enabled: doc.status === "ready"
                    onClicked: doc.extractText(root.currentPage)
                }
                IconButton { glyph: "⟳"; tip: qsTr("Neu laden"); onClicked: doc.reload() }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            // Thumbnails and outline.
            Rectangle {
                visible: root.sidebarVisible
                Layout.preferredWidth: 180
                Layout.fillHeight: true
                color: Theme.sidebar
                ColumnLayout {
                    anchors.fill: parent
                    spacing: 0
                    TabBar {
                        id: sideTabs
                        Layout.fillWidth: true
                        currentIndex: root.sidebarTab
                        onCurrentIndexChanged: root.sidebarTab = currentIndex
                        TabButton { text: qsTr("Seiten"); font.pixelSize: Theme.smallFontSize }
                        TabButton { text: qsTr("Inhalt"); font.pixelSize: Theme.smallFontSize }
                    }
                    StackLayout {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        currentIndex: sideTabs.currentIndex
                        ListView {
                            id: thumbs
                            clip: true
                            model: doc.status === "ready" ? doc.pageCount : 0
                            spacing: 8
                            topMargin: 8
                            cacheBuffer: 400
                            ScrollBar.vertical: ScrollBar {}
                            delegate: Item {
                                id: thumb
                                required property int index
                                width: ListView.view.width
                                height: 110 * doc.pageHeight(index) / doc.pageWidth(index) + 22
                                Rectangle {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    width: 110
                                    height: parent.height - 22
                                    color: "white"
                                    border.width: thumb.index === root.currentPage ? 2 : 0
                                    border.color: Theme.accent
                                    PdfPage {
                                        anchors.fill: parent
                                        anchors.margins: parent.border.width
                                        source: doc.source
                                        page: thumb.index
                                        renderWidth: Math.round(110 * Screen.devicePixelRatio)
                                    }
                                    MouseArea { anchors.fill: parent; onClicked: root.goToPage(thumb.index) }
                                }
                                Text {
                                    anchors.bottom: parent.bottom
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    text: thumb.index + 1
                                    color: thumb.index === root.currentPage ? Theme.textStrong : Theme.textMuted
                                    font.pixelSize: Theme.smallFontSize
                                }
                            }
                        }
                        ListView {
                            id: outlineList
                            clip: true
                            model: { try { return JSON.parse(doc.outlineJson || "[]") } catch (e) { return [] } }
                            ScrollBar.vertical: ScrollBar {}
                            delegate: Rectangle {
                                id: entry
                                required property var modelData
                                width: ListView.view.width
                                height: Theme.rowHeight
                                color: entryHover.hovered ? Theme.hover : "transparent"
                                HoverHandler { id: entryHover }
                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    x: 8 + entry.modelData.depth * 12
                                    width: parent.width - x - 30
                                    text: entry.modelData.title
                                    color: Theme.text
                                    font.pixelSize: Theme.smallFontSize
                                    elide: Text.ElideRight
                                }
                                Text {
                                    anchors.right: parent.right
                                    anchors.rightMargin: 6
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: entry.modelData.page >= 0 ? entry.modelData.page + 1 : ""
                                    color: Theme.textFaint
                                    font.pixelSize: Theme.smallFontSize
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    onClicked: if (entry.modelData.page >= 0) root.goToPage(entry.modelData.page)
                                }
                            }
                            Label {
                                anchors.centerIn: parent
                                visible: outlineList.count === 0
                                text: qsTr("Kein Inhaltsverzeichnis")
                                color: Theme.textFaint
                                font.pixelSize: Theme.smallFontSize
                            }
                        }
                    }
                }
            }

            ListView {
                id: pages
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: doc.status === "ready" ? doc.pageCount : 0
                spacing: 12
                topMargin: 12
                bottomMargin: 12
                cacheBuffer: Math.max(0, height)
                boundsBehavior: Flickable.StopAtBounds
                flickableDirection: Flickable.AutoFlickIfNeeded
                contentWidth: Math.max(width, doc.maxPageWidth * root.zoomScale + 32)
                ScrollBar.vertical: ScrollBar {}
                ScrollBar.horizontal: ScrollBar {}

                delegate: Item {
                    id: pageDelegate
                    required property int index
                    width: pages.contentWidth
                    height: doc.pageHeight(index) * root.zoomScale

                    Rectangle {
                        id: paper
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: doc.pageWidth(pageDelegate.index) * root.zoomScale
                        height: parent.height
                        color: "white"

                        PdfPage {
                            id: page
                            anchors.fill: parent
                            source: doc.source
                            page: pageDelegate.index
                            renderWidth: Math.round(doc.pageWidth(pageDelegate.index) * root.renderScale * Screen.devicePixelRatio)
                            onRenderMsChanged: root.lastRenderMs = renderMs
                        }
                        // Search hits on this page.
                        Repeater {
                            model: root.hitsByPage[pageDelegate.index] || []
                            delegate: Rectangle {
                                required property var modelData
                                x: modelData.x * root.zoomScale - 1
                                y: modelData.y * root.zoomScale - 1
                                width: modelData.w * root.zoomScale + 2
                                height: modelData.h * root.zoomScale + 2
                                color: modelData.index === root.currentHit ? "#80ff9632" : "#50ffd200"
                                border.color: modelData.index === root.currentHit ? "#ff9632" : "transparent"
                            }
                        }
                        Text {
                            anchors.centerIn: parent
                            visible: !page.rendered
                            text: page.error ? page.error : qsTr("Seite %1").arg(pageDelegate.index + 1)
                            color: page.error ? Theme.error : "#999999"
                            width: parent.width - 20
                            wrapMode: Text.Wrap
                            horizontalAlignment: Text.AlignHCenter
                        }
                    }
                }

                WheelHandler {
                    acceptedModifiers: Qt.ControlModifier
                    onWheel: (event) => root.zoomBy(event.angleDelta.y > 0 ? 1.1 : 1 / 1.1)
                }
                TapHandler { onTapped: root.forceActiveFocus() }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 20
            color: Theme.header
            Label {
                anchors.fill: parent
                anchors.leftMargin: 8
                verticalAlignment: Text.AlignVCenter
                color: doc.status === "error" ? Theme.error : Theme.textFaint
                font.pixelSize: Theme.smallFontSize
                elide: Text.ElideRight
                text: doc.status === "error" ? doc.error
                    : doc.status === "loading" ? qsTr("Öffnen …")
                    : qsTr("Öffnen %1 ms · Rendern %2 ms%3").arg(doc.openMs).arg(root.lastRenderMs)
                        .arg(root.searchQuery ? qsTr(" · Suche %1 ms").arg(root.searchMs) : "")
            }
        }
    }

    Label {
        anchors.centerIn: parent
        visible: doc.status === "error"
        color: Theme.error
        text: doc.error
        width: parent.width - 40
        wrapMode: Text.Wrap
        horizontalAlignment: Text.AlignHCenter
    }

    Popup {
        id: textPopup
        property string title
        property string text
        anchors.centerIn: Overlay.overlay
        width: Math.min(700, root.Window.width - 80)
        height: Math.min(520, root.Window.height - 80)
        modal: true
        background: Rectangle { color: Theme.editor; border.color: Theme.border; radius: 6 }
        ColumnLayout {
            anchors.fill: parent
            Label { text: textPopup.title; color: Theme.textStrong; font.bold: true }
            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                TextArea {
                    readOnly: true
                    text: textPopup.text
                    wrapMode: Text.Wrap
                    color: Theme.text
                    selectByMouse: true
                    background: null
                }
            }
        }
    }
}
