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
    readonly property real pageWidth: Math.max(200, Math.min(list.width - 48, 900) * root.zoom)
    /// Rendered width in device pixels, in steps so zooming does not render every size.
    readonly property int renderWidth: Math.min(4000, Math.ceil(root.pageWidth * Screen.devicePixelRatio / 200) * 200)

    function zoomBy(factor) { root.zoom = Math.max(0.3, Math.min(4, root.zoom * factor)) }
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
            ScrollBar.vertical: ScrollBar {}
            delegate: Item {
                required property var modelData
                required property int index
                width: list.width
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
            WheelHandler {
                acceptedModifiers: Qt.ControlModifier
                onWheel: (event) => root.zoomBy(event.angleDelta.y > 0 ? 1.1 : 1 / 1.1)
            }
        }
    }
}
