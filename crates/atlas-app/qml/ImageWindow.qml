// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import Atlas

// A picture of a note in a window of its own (button ⤢ or double click in
// the preview): fits the window, zooms with the wheel (towards the mouse),
// +/− and 100 %, moves by dragging. Esc closes it.
Window {
    id: win
    property url source
    property string caption: ""
    /// 0 = fit the window, otherwise the zoom factor.
    property real zoom: 0
    readonly property real fitScale: img.implicitWidth > 0
        ? Math.min(1, Math.min((flick.width - 32) / img.implicitWidth, (flick.height - 32) / img.implicitHeight)) : 1
    readonly property real scale: zoom > 0 ? zoom : fitScale
    readonly property bool ready: img.status === Image.Ready

    title: caption || qsTr("Bild")
    width: Math.min(Screen.desktopAvailableWidth * 0.85, 1400)
    height: Math.min(Screen.desktopAvailableHeight * 0.85, 1000)
    color: "#070b12"
    transientParent: null

    function show(url, text) {
        source = url
        caption = text || ""
        zoom = 0
        visible = true
        raise()
        requestActivate()
        flick.forceActiveFocus()
    }
    /// Zooms by `factor`, keeping the spot at (x, y) of the view in place.
    function zoomBy(factor, x, y) {
        const before = scale
        const after = Math.max(0.05, Math.min(16, before * factor))
        const px = (flick.contentX + (x === undefined ? flick.width / 2 : x)) / before
        const py = (flick.contentY + (y === undefined ? flick.height / 2 : y)) / before
        zoom = after
        Qt.callLater(() => {
            flick.contentX = Math.max(0, Math.min(flick.contentWidth - flick.width, px * after - (x === undefined ? flick.width / 2 : x)))
            flick.contentY = Math.max(0, Math.min(flick.contentHeight - flick.height, py * after - (y === undefined ? flick.height / 2 : y)))
        })
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Flickable {
            id: flick
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            focus: true
            boundsBehavior: Flickable.StopAtBounds
            contentWidth: Math.max(width, img.implicitWidth * win.scale)
            contentHeight: Math.max(height, img.implicitHeight * win.scale)
            Keys.onPressed: (event) => {
                if (event.key === Qt.Key_Escape) win.close()
                else if (event.key === Qt.Key_Plus || event.key === Qt.Key_Equal) win.zoomBy(1.25)
                else if (event.key === Qt.Key_Minus) win.zoomBy(0.8)
                else if (event.key === Qt.Key_0) win.zoom = 0
                else if (event.key === Qt.Key_1) win.zoom = 1
                else return
                event.accepted = true
            }

            Image {
                id: img
                source: win.source
                asynchronous: true
                smooth: true
                mipmap: true
                width: implicitWidth * win.scale
                height: implicitHeight * win.scale
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
            // Wheel: zoom towards the mouse (no scrolling needed for a picture).
            WheelHandler {
                acceptedDevices: PointerDevice.AllDevices
                acceptedPointerTypes: PointerDevice.AllPointerTypes
                onWheel: (event) => {
                    const d = event.angleDelta.y !== 0 ? event.angleDelta.y : event.pixelDelta.y * 4
                    if (d === 0) return
                    const p = flick.mapFromItem(flick.contentItem, event.x, event.y)
                    win.zoomBy(Math.pow(1.0015, d), p.x, p.y)
                    event.accepted = true
                }
            }
            TapHandler {
                onDoubleTapped: win.zoom = win.zoom > 0 ? 0 : 1
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 40
            color: Theme.header
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 8
                spacing: 6
                Label {
                    Layout.fillWidth: true
                    text: win.caption
                    elide: Text.ElideRight
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                }
                Label {
                    text: img.status === Image.Ready ? img.implicitWidth + " × " + img.implicitHeight + " px" : ""
                    color: Theme.textFaint
                    font.pixelSize: Theme.smallFontSize
                }
                IconButton { iconName: "minus"; tip: qsTr("Verkleinern (−)"); onClicked: win.zoomBy(0.8) }
                Label {
                    text: Math.round(win.scale * 100) + " %"
                    color: Theme.text
                    font.pixelSize: Theme.smallFontSize
                    Layout.preferredWidth: 44
                    horizontalAlignment: Text.AlignHCenter
                }
                IconButton { iconName: "plus"; tip: qsTr("Vergrößern (+)"); onClicked: win.zoomBy(1.25) }
                IconButton { glyph: "1:1"; tip: qsTr("Originalgröße (1)"); checkable: true; checked: win.zoom === 1; onClicked: win.zoom = 1 }
                IconButton { glyph: "⤢"; tip: qsTr("Ans Fenster anpassen (0)"); checkable: true; checked: win.zoom === 0; onClicked: win.zoom = 0 }
                IconButton { iconName: "close"; tip: qsTr("Schließen (Esc)"); onClicked: win.close() }
            }
        }
    }
}
