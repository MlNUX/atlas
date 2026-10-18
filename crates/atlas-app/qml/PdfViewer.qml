// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import Atlas

// PDF viewer used for the Poppler/PDFium comparison: the engine can be
// switched at runtime, open/render times are shown in the toolbar.
Rectangle {
    id: root
    required property string source
    property alias engine: doc.engine
    /// 0 = fit width
    property real zoom: 0
    property int lastRenderMs: 0
    readonly property int currentPage: pages.indexAt(10, pages.contentY + pages.height / 3)

    color: Theme.pdfBackdrop

    function viewState() {
        return { engine: doc.engine, zoom: root.zoom, page: Math.max(0, root.currentPage) }
    }
    function applyState(state) {
        if (state.engine) doc.engine = state.engine
        if (state.zoom !== undefined) root.zoom = state.zoom
        if (state.page) restorePage = state.page
    }
    property int restorePage: 0

    PdfDocument {
        id: doc
        source: root.source
        engine: "poppler"
        onStatusChanged: if (status === "ready" && root.restorePage > 0) {
            Qt.callLater(() => { pages.positionViewAtIndex(root.restorePage, ListView.Beginning); root.restorePage = 0 })
        }
        onTextReady: (page, text, ms) => {
            textPopup.title = qsTr("Text von Seite %1 – %2 (%3 ms)").arg(page + 1).arg(doc.engine).arg(ms)
            textPopup.text = text
            textPopup.open()
        }
    }

    readonly property real fitScale: doc.maxPageWidth > 0 ? Math.max(0.1, (pages.width - 32) / doc.maxPageWidth) : 1
    readonly property real scale: root.zoom > 0 ? root.zoom : fitScale
    // Rendering follows the visible scale with a short delay so resizing a
    // split does not re-render every page on every frame.
    property real renderScale: scale
    onScaleChanged: renderTimer.restart()
    Timer { id: renderTimer; interval: 120; onTriggered: root.renderScale = root.scale }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 30
            color: Theme.header
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 8
                spacing: 4
                ComboBox {
                    id: engineBox
                    model: ["poppler", "pdfium"]
                    implicitHeight: 24
                    currentIndex: Math.max(0, model.indexOf(doc.engine))
                    onActivated: doc.engine = currentText
                    displayText: currentText === "pdfium" ? "PDFium" : "Poppler"
                }
                IconButton { glyph: "−"; tip: qsTr("Verkleinern"); onClicked: root.zoom = Math.max(0.2, root.scale / 1.2) }
                Label {
                    text: Math.round(root.scale * 100) + " %"
                    color: Theme.text
                    font.pixelSize: Theme.smallFontSize
                    Layout.preferredWidth: 44
                    horizontalAlignment: Text.AlignHCenter
                }
                IconButton { glyph: "+"; tip: qsTr("Vergrößern"); onClicked: root.zoom = Math.min(6, root.scale * 1.2) }
                IconButton { glyph: "↔"; tip: qsTr("Seitenbreite"); checkable: true; checked: root.zoom === 0; onClicked: root.zoom = 0 }
                Label {
                    text: doc.pageCount > 0 ? qsTr("Seite %1 / %2").arg(Math.max(1, root.currentPage + 1)).arg(doc.pageCount) : ""
                    color: Theme.text
                    font.pixelSize: Theme.smallFontSize
                    Layout.leftMargin: 8
                }
                Label {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignRight
                    color: doc.status === "error" ? Theme.error : Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                    text: doc.status === "error" ? doc.error
                        : doc.status === "loading" ? qsTr("Öffnen …")
                        : qsTr("Öffnen %1 ms · Rendern %2 ms").arg(doc.openMs).arg(root.lastRenderMs)
                }
                IconButton {
                    glyph: "T"
                    tip: qsTr("Text der aktuellen Seite extrahieren")
                    enabled: doc.status === "ready"
                    onClicked: doc.extractText(Math.max(0, root.currentPage))
                }
                IconButton { glyph: "⟳"; tip: qsTr("Neu laden"); onClicked: doc.reload() }
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
            contentWidth: Math.max(width, doc.maxPageWidth * root.scale + 32)
            ScrollBar.vertical: ScrollBar {}
            ScrollBar.horizontal: ScrollBar {}

            delegate: Item {
                id: pageDelegate
                required property int index
                width: pages.contentWidth
                height: doc.pageHeight(index) * root.scale

                Rectangle {
                    id: paper
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: doc.pageWidth(pageDelegate.index) * root.scale
                    height: parent.height
                    color: "white"

                    PdfPage {
                        id: page
                        anchors.fill: parent
                        source: doc.source
                        engine: doc.engine
                        page: pageDelegate.index
                        renderWidth: Math.round(doc.pageWidth(pageDelegate.index) * root.renderScale * Screen.devicePixelRatio)
                        onRenderMsChanged: root.lastRenderMs = renderMs
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
                onWheel: (event) => {
                    const factor = event.angleDelta.y > 0 ? 1.1 : 1 / 1.1
                    root.zoom = Math.max(0.2, Math.min(6, root.scale * factor))
                }
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
