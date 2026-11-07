// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// A view of an extension with `window: true` (e.g. the docs search): its
// page in a large window in front of Atlas, like the project search,
// instead of a tab. The page stays loaded while the window is closed, so
// it opens again where it was. Esc in the page closes it via
// `atlas.close()`.
Dialog {
    id: dialog
    required property AppController app
    property var actions: null
    /// `view:<extension>/<id>` of the page shown.
    property string viewId: ""
    property string source: ""
    /// The ExtensionView, once loaded.
    readonly property var page: loader.item

    parent: Overlay.overlay
    anchors.centerIn: Overlay.overlay
    width: parent ? parent.width * 0.86 : 1100
    height: parent ? parent.height * 0.86 : 760
    modal: true
    padding: 0
    topPadding: 0
    title: ""

    /// Shows view `id` (on `path`); the same view again keeps its page.
    function show(id, path) {
        if (viewId !== id) {
            viewId = ""
            source = path || ""
            viewId = id
        }
        open()
        if (loader.item) {
            loader.item.notifyShown()
            loader.item.focusContent()
        }
    }
    onOpened: if (loader.item) loader.item.focusContent()

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 10
            Layout.topMargin: 8
            Layout.bottomMargin: 6
            spacing: 10
            Icon { name: "search"; size: 18; color: Theme.accent }
            Text {
                text: loader.item && loader.item.info.name ? loader.item.info.name : ""
                color: Theme.textStrong
                font.pixelSize: 15
                font.weight: Font.DemiBold
            }
            Item { Layout.fillWidth: true }
            IconButton {
                iconName: "refresh"
                tip: qsTr("Neu laden (F5)")
                onClicked: if (loader.item) loader.item.webItem.reload()
            }
            IconButton { iconName: "close"; tip: qsTr("Schließen (Esc)"); onClicked: dialog.close() }
        }
        Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: Theme.border }

        Loader {
            id: loader
            Layout.fillWidth: true
            Layout.fillHeight: true
            active: dialog.viewId !== ""
            sourceComponent: ExtensionView {
                app: dialog.app
                actions: dialog.actions
                source: dialog.source
                viewId: dialog.viewId
                onCloseRequested: dialog.close()
            }
            onLoaded: if (dialog.visible) item.focusContent()
        }
    }
}
