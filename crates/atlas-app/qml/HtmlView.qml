// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtWebEngine
import Atlas

// An HTML file rendered (action "Als Webseite anzeigen"). Reloads when the
// file is saved. Pages may run scripts and load files next to them, but not
// reach the network; links to other sites open in the system browser.
Rectangle {
    id: root
    required property AppController app
    property var actions: null
    required property string source
    property real zoom: 1

    /// Back to the source in this tab.
    signal showSourceRequested()

    color: Theme.editor

    readonly property url fileUrl: "file://" + source.split("/").map(encodeURIComponent).join("/")
    readonly property alias webItem: web
    property real modified: 0

    function viewState() { return { processor: "atlas:html", zoom: root.zoom } }
    function applyState(state) { if (state.zoom) root.zoom = state.zoom }
    function reload() { web.reload() }

    function modifiedTime() {
        try { return JSON.parse(root.actions.ops.properties(root.source)).modified || 0 } catch (e) { return 0 }
    }
    Component.onCompleted: modified = modifiedTime()

    // Saved in an editor (Atlas or another): show the new version.
    Timer {
        interval: 1000
        repeat: true
        running: root.visible && !!root.actions
        onTriggered: {
            const m = root.modifiedTime()
            if (m && m !== root.modified) {
                root.modified = m
                web.reload()
            }
        }
    }

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
                Label {
                    Layout.fillWidth: true
                    text: web.title && web.title !== root.app.fileName(root.source)
                        ? qsTr("Webseite: %1").arg(web.title) : qsTr("Webseite")
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                }
                IconButton { iconName: "minus"; tip: qsTr("Verkleinern"); onClicked: root.zoom = Math.max(0.3, root.zoom / 1.1) }
                Label {
                    text: Math.round(root.zoom * 100) + " %"
                    color: Theme.text
                    font.pixelSize: Theme.smallFontSize
                    MouseArea { anchors.fill: parent; onClicked: root.zoom = 1 }
                }
                IconButton { iconName: "plus"; tip: qsTr("Vergrößern"); onClicked: root.zoom = Math.min(5, root.zoom * 1.1) }
                IconButton { iconName: "refresh"; tip: qsTr("Neu laden (F5)"); onClicked: web.reload() }
                IconButton { iconName: "file-code"; tip: qsTr("Quelltext anzeigen"); onClicked: root.showSourceRequested() }
                IconButton { iconName: "external"; tip: qsTr("Im Browser öffnen"); onClicked: Qt.openUrlExternally(root.fileUrl) }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }

        WebEngineView {
            id: web
            Layout.fillWidth: true
            Layout.fillHeight: true
            url: root.fileUrl
            zoomFactor: root.zoom
            backgroundColor: "white"
            settings.javascriptEnabled: true
            settings.localContentCanAccessFileUrls: true
            settings.localContentCanAccessRemoteUrls: false
            settings.javascriptCanOpenWindows: false
            settings.focusOnNavigationEnabled: false
            // Local pages (also linked ones) stay here; the web goes to the
            // system browser.
            onNavigationRequested: (request) => {
                const url = request.url.toString()
                if (url.startsWith("file:") || url.startsWith("data:") || url.startsWith("about:")) return
                request.reject()
                if (request.navigationType === WebEngineNavigationRequest.LinkClickedNavigation) Qt.openUrlExternally(request.url)
            }
            onNewWindowRequested: (request) => Qt.openUrlExternally(request.requestedUrl)
            Keys.onPressed: (event) => {
                if (event.key === Qt.Key_F5) { web.reload(); event.accepted = true }
            }
        }
    }
}
