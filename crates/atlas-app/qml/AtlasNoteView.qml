// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import Atlas

// A notebook written with Atlas Tablet (`.atlasnote`): pages below each
// other, rendered by atlas-core into the cache. Read-only; the file is
// edited on the tablet.
Rectangle {
    id: root
    required property string source
    property var app: null
    /// 1 = page width up to 900 px; Ctrl+wheel and the buttons change it.
    property real zoom: 1
    property int revision: 0
    color: Theme.pdfBackdrop

    readonly property var info: root.app && root.source && root.revision >= 0
        ? JSON.parse(root.app.atlasNoteInfo(root.source)) : ({ pages: [], error: "" })
    /// Page width at 100 %: the view width up to 900 px.
    readonly property real baseWidth: Math.max(200, Math.min(list.width - 48, 900))
    readonly property real pageWidth: root.baseWidth * root.zoom
    /// Zoom the pages are rendered for: follows `zoom` once zooming pauses, so a
    /// Ctrl+wheel turn scales the shown pictures and renders only once at the end.
    property real renderZoom: 1
    onZoomChanged: renderTimer.restart()
    Timer { id: renderTimer; interval: 180; onTriggered: root.renderZoom = root.zoom }
    /// Rendered width in device pixels, in steps so zooming does not render every size.
    readonly property int renderWidth: Math.min(4000, Math.ceil(root.baseWidth * root.renderZoom * Screen.devicePixelRatio / 200) * 200)

    function zoomBy(factor) { root.zoom = Math.max(0.3, Math.min(5, root.zoom * factor)) }
    /// Wheel notches, fractional for touchpads and high-resolution wheels (as in PdfViewer).
    function wheelSteps(event) {
        const d = event.angleDelta.y || event.angleDelta.x
        if (d) return d / 120
        const p = event.pixelDelta.y || event.pixelDelta.x
        return p / 50
    }
    function applyState(state) { if (state && state.zoom) root.zoom = state.zoom }
    function viewState() { return { zoom: root.zoom } }
    function focusContent() { list.forceActiveFocus() }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 36
            color: Theme.header
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 8
                spacing: 8
                Icon { name: "pen"; color: Theme.accent }
                Label {
                    text: root.info.pages.length === 1 ? qsTr("Atlas-Notiz · 1 Seite") : qsTr("Atlas-Notiz · %1 Seiten").arg(root.info.pages.length)
                    color: Theme.textStrong
                    font.pixelSize: Theme.smallFontSize
                }
                Label {
                    text: qsTr("Nur Ansicht – bearbeitet wird auf dem Tablet")
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                IconButton { iconName: "minus"; tip: qsTr("Verkleinern (Strg+Mausrad)"); onClicked: root.zoomBy(1 / 1.2) }
                Label { text: Math.round(root.zoom * 100) + " %"; color: Theme.text; font.pixelSize: Theme.smallFontSize }
                IconButton { iconName: "plus"; tip: qsTr("Vergrößern (Strg+Mausrad)"); onClicked: root.zoomBy(1.2) }
                IconButton { iconName: "refresh"; tip: qsTr("Neu laden"); onClicked: root.revision++ }
            }
        }
        Label {
            visible: root.info.error.length > 0
            Layout.fillWidth: true
            Layout.margins: 16
            wrapMode: Text.Wrap
            color: Theme.error
            text: qsTr("Die Atlas-Notiz kann nicht angezeigt werden: %1").arg(root.info.error)
        }
        ListView {
            id: list
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 16
            topMargin: 16
            bottomMargin: 16
            model: root.info.pages
            boundsBehavior: Flickable.StopAtBounds
            // Zoomed in wider than the view: scroll sideways too.
            flickableDirection: Flickable.AutoFlickIfNeeded
            contentWidth: Math.max(width, root.pageWidth + 48)
            ScrollBar.vertical: ScrollBar {}
            ScrollBar.horizontal: ScrollBar {}
            delegate: Item {
                required property var modelData
                required property int index
                width: list.contentWidth
                height: root.pageWidth * modelData.height / 1000
                Rectangle {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: root.pageWidth
                    height: parent.height
                    color: "#242528"
                    border.color: Theme.border
                    Image {
                        anchors.fill: parent
                        // Renders on first use and then comes from the cache.
                        source: {
                            const url = root.app ? root.app.atlasNotePage(root.source, index, root.renderWidth) : ""
                            return url ? url + "#" + root.revision : ""
                        }
                        fillMode: Image.Stretch
                        smooth: true
                        mipmap: true
                        cache: false
                    }
                    Label {
                        anchors.right: parent.right
                        anchors.bottom: parent.bottom
                        anchors.margins: 6
                        text: (index + 1) + " / " + root.info.pages.length
                        color: Theme.textFaint
                        font.pixelSize: Theme.smallFontSize
                    }
                }
            }
            // Ctrl+wheel zooms and keeps the spot under the mouse in place. Qt on
            // Wayland reports mouse wheels as touchpads with finger pointers and
            // high-resolution wheels with pixel deltas only: accept every device.
            WheelHandler {
                acceptedDevices: PointerDevice.AllDevices
                acceptedPointerTypes: PointerDevice.AllPointerTypes
                acceptedModifiers: Qt.ControlModifier
                onWheel: (event) => {
                    event.accepted = true
                    const steps = root.wheelSteps(event)
                    if (!steps) return
                    list.cancelFlick()
                    // The handler sits in the content: convert to view coordinates.
                    const at = list.mapFromItem(list.contentItem, event.x, event.y)
                    const before = root.pageWidth
                    const offset = (w) => (Math.max(list.width, w + 48) - w) / 2
                    const fx = (list.contentX - list.originX + at.x - offset(before)) / before
                    const cy = list.contentY + at.y
                    let index = list.indexAt(list.contentWidth / 2, cy)
                    if (index < 0) index = list.indexAt(list.contentWidth / 2, cy - list.spacing)
                    const item = index >= 0 ? list.itemAtIndex(index) : null
                    const along = item && item.height > 0 ? (cy - item.y) / item.height : 0
                    root.zoomBy(Math.pow(1.1, steps))
                    const place = () => {
                        const width = root.pageWidth
                        const maxX = Math.max(0, list.contentWidth - list.width)
                        list.contentX = Math.max(0, Math.min(maxX, offset(width) + fx * width - at.x)) + list.originX
                        if (!item) return
                        list.forceLayout()
                        const now = list.itemAtIndex(index)
                        if (!now) return
                        const minY = list.originY - list.topMargin
                        const maxY = Math.max(minY, list.originY + list.contentHeight + list.bottomMargin - list.height)
                        list.contentY = Math.max(minY, Math.min(maxY, now.y + along * now.height - at.y))
                    }
                    place()
                    Qt.callLater(place)
                }
            }
        }
    }
}
