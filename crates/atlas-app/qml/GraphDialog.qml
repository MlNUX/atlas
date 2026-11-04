// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// The project graph in its own large window (from "Groß anzeigen"), around
// `path`. A click on a node opens the file and closes the window;
// Ctrl/middle click opens it in a split.
Dialog {
    id: dialog
    required property AppController app
    property string path

    signal openRequested(string path, int line, bool newPane, var state)

    anchors.centerIn: Overlay.overlay
    width: parent ? parent.width * 0.86 : 1100
    height: parent ? parent.height * 0.86 : 760
    modal: true
    padding: 0
    topPadding: 0
    title: ""

    function show(file) {
        path = file
        open()
    }

    contentItem: ColumnLayout {
        spacing: 0
        DialogHeader {
            icon: "graph"
            title: qsTr("Graph")
            subtitle: dialog.path ? qsTr("Rund um %1 – Strg+Mausrad zoomt, Ziehen verschiebt, Klick öffnet").arg(dialog.app.relativePath(dialog.path))
                                  : qsTr("Kein Dokument geöffnet")
            elide: Text.ElideMiddle
            margins: 16
            bottomMargin: 12
            IconButton { iconName: "close"; tip: qsTr("Schließen (Esc)"); onClicked: dialog.close() }
        }
        ContextPanel {
            id: graph
            Layout.fillWidth: true
            Layout.fillHeight: true
            // Built only while shown (the layout runs on every refresh).
            visible: dialog.visible
            app: dialog.app
            path: dialog.visible ? dialog.path : ""
            tab: 1
            showTabs: false
            color: "transparent"
            onOpenRequested: (file, line, newPane, state) => {
                dialog.close()
                dialog.openRequested(file, line, newPane, state)
            }
        }
    }
}
