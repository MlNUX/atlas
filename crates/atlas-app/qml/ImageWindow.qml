// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// A picture of a note, large in front of Atlas (button ⤢ or double click
// in the preview), like the graph: fits the dialog, the wheel zooms towards
// the mouse, +/− and 1:1, dragging moves it. Esc closes it.
Dialog {
    id: dialog
    property url source
    property string caption: ""
    /// 0 = fit the dialog, otherwise the zoom factor.
    property real zoom: 0
    readonly property real fitScale: img.implicitWidth > 0
        ? Math.min(1, Math.min((flick.width - 32) / img.implicitWidth, (flick.height - 32) / img.implicitHeight)) : 1
    readonly property real zoomScale: zoom > 0 ? zoom : fitScale
    readonly property bool ready: img.status === Image.Ready

    parent: Overlay.overlay
    anchors.centerIn: Overlay.overlay
    width: parent ? parent.width * 0.86 : 1100
    height: parent ? parent.height * 0.86 : 760
    modal: true
    padding: 0
    topPadding: 0
    title: ""

    // The picture (full size, for 1:1) leaves memory with the window.
    onClosed: source = ""

    function show(url, text) {
        source = url
        caption = text || ""
        zoom = 0
        open()
        flick.forceActiveFocus()
    }
    /// Zooms by `factor`, keeping the spot at (x, y) of the view in place.
    function zoomBy(factor, x, y) {
        const at = (v, size) => v === undefined ? size / 2 : v
        const vx = at(x, flick.width), vy = at(y, flick.height)
        const before = zoomScale
        const after = Math.max(0.05, Math.min(16, before * factor))
        const px = (flick.contentX + vx) / before, py = (flick.contentY + vy) / before
        zoom = after
        Qt.callLater(() => {
            flick.contentX = Math.max(0, Math.min(flick.contentWidth - flick.width, px * after - vx))
            flick.contentY = Math.max(0, Math.min(flick.contentHeight - flick.height, py * after - vy))
        })
    }

    contentItem: ColumnLayout {
        spacing: 0
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 16
            Layout.bottomMargin: 12
            spacing: 12
            Rectangle {
                Layout.preferredWidth: 36
                Layout.preferredHeight: 36
                radius: 10
                color: Theme.accentSoft
                Icon { anchors.centerIn: parent; name: "image"; size: 20; color: Theme.accent }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Text {
                    Layout.fillWidth: true
                    text: dialog.caption || qsTr("Bild")
                    color: Theme.textStrong
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                }
                Text {
                    Layout.fillWidth: true
                    text: (img.status === Image.Ready ? img.implicitWidth + " × " + img.implicitHeight + " px · " : "")
                          + qsTr("Mausrad zoomt, Ziehen verschiebt, Doppelklick: ganz/1:1")
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                }
            }
            IconButton { iconName: "minus"; tip: qsTr("Verkleinern (−)"); onClicked: dialog.zoomBy(0.8) }
            Label {
                text: Math.round(dialog.zoomScale * 100) + " %"
                color: Theme.text
                font.pixelSize: Theme.smallFontSize
                Layout.preferredWidth: 44
                horizontalAlignment: Text.AlignHCenter
            }
            IconButton { iconName: "plus"; tip: qsTr("Vergrößern (+)"); onClicked: dialog.zoomBy(1.25) }
            IconButton { glyph: "1:1"; tip: qsTr("Originalgröße (1)"); checkable: true; checked: dialog.zoom === 1; onClicked: dialog.zoom = 1 }
            IconButton { iconName: "maximize"; tip: qsTr("Einpassen (0)"); checkable: true; checked: dialog.zoom === 0; onClicked: dialog.zoom = 0 }
            IconButton { iconName: "close"; tip: qsTr("Schließen (Esc)"); onClicked: dialog.close() }
        }
        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }

        Flickable {
            id: flick
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            focus: true
            boundsBehavior: Flickable.StopAtBounds
            contentWidth: Math.max(width, img.implicitWidth * dialog.zoomScale)
            contentHeight: Math.max(height, img.implicitHeight * dialog.zoomScale)
            Keys.onPressed: (event) => {
                if (event.key === Qt.Key_Plus || event.key === Qt.Key_Equal) dialog.zoomBy(1.25)
                else if (event.key === Qt.Key_Minus) dialog.zoomBy(0.8)
                else if (event.key === Qt.Key_0) dialog.zoom = 0
                else if (event.key === Qt.Key_1) dialog.zoom = 1
                else return
                event.accepted = true
            }

            Image {
                id: img
                source: dialog.source
                asynchronous: true
                smooth: true
                mipmap: true
                width: implicitWidth * dialog.zoomScale
                height: implicitHeight * dialog.zoomScale
                x: (flick.contentWidth - width) / 2
                y: (flick.contentHeight - height) / 2
            }
            BusyIndicator { anchors.centerIn: parent; running: img.status === Image.Loading }
            Label {
                anchors.centerIn: parent
                visible: img.status === Image.Error
                text: qsTr("Das Bild lässt sich nicht laden.")
                color: Theme.error
            }
            // Wheel: zoom towards the mouse.
            WheelHandler {
                acceptedDevices: PointerDevice.AllDevices
                acceptedPointerTypes: PointerDevice.AllPointerTypes
                onWheel: (event) => {
                    const d = event.angleDelta.y !== 0 ? event.angleDelta.y : event.pixelDelta.y * 4
                    if (d === 0) return
                    const p = flick.mapFromItem(flick.contentItem, event.x, event.y)
                    dialog.zoomBy(Math.pow(1.0015, d), p.x, p.y)
                    event.accepted = true
                }
            }
            TapHandler { onDoubleTapped: dialog.zoom = dialog.zoom > 0 ? 0 : 1 }
        }
    }
}
