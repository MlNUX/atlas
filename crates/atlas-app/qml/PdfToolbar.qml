// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// The PDF viewer's toolbar: pages, zoom, tools, colours, search.
ToolStrip {
    id: toolbar
    required property var viewer
    required property var pdf
    required property var annotations
    required property var contextView
    property alias searchField: searchField
    Layout.preferredHeight: 32
    visible: viewer.chrome && !viewer.presenting
    leftMargin: 6
    rightMargin: 6
    spacing: 3
    line: "none"
    IconButton {
        iconName: "list"
        tip: qsTr("Seitenleiste: Miniaturen, Inhalt und Annotationen")
        checkable: true
        checked: viewer.sidebarVisible
        onClicked: viewer.sidebarVisible = !viewer.sidebarVisible
    }
    TextField {
        id: pageField
        Layout.preferredWidth: 44
        implicitHeight: 24
        horizontalAlignment: Text.AlignHCenter
        font.pixelSize: Theme.smallFontSize
        text: pdf.pageCount > 0 ? String(viewer.currentPage + 1) : ""
        validator: IntValidator { bottom: 1; top: Math.max(1, pdf.pageCount) }
        onAccepted: { viewer.goToPage(parseInt(text) - 1); viewer.forceActiveFocus() }
    }
    Label {
        text: "/ " + pdf.pageCount
        color: Theme.textMuted
        font.pixelSize: Theme.smallFontSize
    }
    IconButton { iconName: "minus"; tip: qsTr("Verkleinern (Strg+−)"); onClicked: viewer.zoomBy(1 / 1.2) }
    Label {
        text: Math.round(viewer.zoomScale * 100) + " %"
        color: Theme.text
        font.pixelSize: Theme.smallFontSize
        Layout.preferredWidth: 40
        horizontalAlignment: Text.AlignHCenter
    }
    IconButton { iconName: "plus"; tip: qsTr("Vergrößern (Strg++)"); onClicked: viewer.zoomBy(1.2) }
    IconButton { glyph: "↔"; tip: qsTr("Seitenbreite (Strg+0)"); checkable: true; checked: viewer.zoom === 0; onClicked: viewer.zoom = 0 }
    IconButton { glyph: "▯"; tip: qsTr("Ganze Seite"); checkable: true; checked: viewer.zoom < 0; onClicked: viewer.zoom = -1 }
    IconButton {
        iconName: "moon"
        tip: viewer.dark ? qsTr("Dunkelmodus aus: Originalfarben") : qsTr("Dunkelmodus: Farben umkehren (für alle PDFs)")
        checkable: true
        checked: viewer.dark
        enabled: !!viewer.app
        onClicked: viewer.app.pdfDark = !viewer.app.pdfDark
    }
    Rectangle { Layout.preferredWidth: 1; Layout.preferredHeight: 18; color: Theme.border }
    IconButton {
        glyph: "I"
        tip: qsTr("Text auswählen und markieren")
        checkable: true
        checked: viewer.tool === "select"
        onClicked: viewer.tool = "select"
    }
    IconButton {
        iconName: "pen"
        tip: qsTr("Freihand zeichnen")
        checkable: true
        checked: viewer.tool === "ink"
        onClicked: { viewer.tool = viewer.tool === "ink" ? "select" : "ink"; viewer.clearSelection() }
    }
    Rectangle {
        visible: viewer.tool === "ink"
        Layout.preferredWidth: 16
        Layout.preferredHeight: 16
        radius: 8
        color: viewer.inkColor
        border.color: Theme.border
        MouseArea { anchors.fill: parent; onClicked: inkMenu.popup() }
        Menu {
            id: inkMenu
            Repeater {
                model: viewer.markColors
                MenuItem {
                    required property string modelData
                    text: "●"
                    onTriggered: viewer.inkColor = modelData
                    contentItem: Text { text: "●  " + parent.modelData; color: parent.modelData }
                }
            }
            MenuSeparator {}
            MenuItem { text: qsTr("Dünn"); onTriggered: viewer.inkWidth = 1 }
            MenuItem { text: qsTr("Mittel"); onTriggered: viewer.inkWidth = 2 }
            MenuItem { text: qsTr("Breit"); onTriggered: viewer.inkWidth = 4 }
        }
    }
    IconButton {
        iconName: "eraser"
        tip: qsTr("Radierer: ganze Striche löschen (über sie fahren)")
        checkable: true
        checked: viewer.tool === "erase"
        onClicked: { viewer.tool = viewer.tool === "erase" ? "select" : "erase"; viewer.clearSelection() }
    }
    IconButton {
        glyph: "🗨"
        tip: qsTr("Notiz auf der Seite anheften")
        checkable: true
        checked: viewer.tool === "note"
        onClicked: { viewer.tool = viewer.tool === "note" ? "select" : "note"; viewer.clearSelection() }
    }
    IconButton {
        iconName: "image"
        tip: qsTr("Bild als Zitat: Rahmen um Abbildung oder Tabelle ziehen, dann Notiz wählen")
        checkable: true
        checked: viewer.tool === "figure"
        onClicked: { viewer.tool = viewer.tool === "figure" ? "select" : "figure"; viewer.clearSelection() }
    }
    IconButton {
        glyph: "∑"
        tip: qsTr("Formel als LaTeX: Rahmen um die Formel ziehen (läuft auf diesem Rechner)")
        checkable: true
        checked: viewer.tool === "latex"
        onClicked: { viewer.tool = viewer.tool === "latex" ? "select" : "latex"; viewer.clearSelection() }
    }
    IconButton {
        iconName: "undo"
        tip: qsTr("Rückgängig (Strg+Z)")
        enabled: viewer.canUndo
        onClicked: viewer.undo()
    }
    IconButton {
        iconName: "redo"
        tip: qsTr("Wiederholen (Strg+Umschalt+Z)")
        enabled: viewer.canRedo
        onClicked: viewer.redo()
    }
    IconButton {
        iconName: "bookmark"
        tip: viewer.pageBookmarked ? qsTr("Lesezeichen entfernen (Strg+D)") : qsTr("Lesezeichen für diese Seite (Strg+D)")
        checkable: true
        checked: viewer.pageBookmarked
        enabled: annotations.status !== "loading" && annotations.status !== "error"
        onClicked: viewer.toggleBookmark()
    }
    Item { Layout.fillWidth: true }
    TextField {
        id: searchField
        Layout.preferredWidth: Math.min(220, viewer.width * 0.3)
        implicitHeight: 24
        font.pixelSize: Theme.smallFontSize
        placeholderText: qsTr("Im Dokument suchen (Strg+F)")
        onTextEdited: searchDebounce.restart()
        onAccepted: { searchDebounce.stop(); if (text === viewer.searchQuery) viewer.nextHit(1); else viewer.search(text) }
        Keys.onEscapePressed: { text = ""; viewer.search(""); viewer.forceActiveFocus() }
        Timer { id: searchDebounce; interval: 300; onTriggered: viewer.search(searchField.text) }
    }
    Label {
        visible: viewer.searchQuery.length > 0
        text: viewer.hits.length ? qsTr("%1/%2").arg(viewer.currentHit + 1).arg(viewer.hits.length) : qsTr("0 Treffer")
        color: viewer.hits.length ? Theme.textMuted : Theme.warn
        font.pixelSize: Theme.smallFontSize
    }
    IconButton { glyph: "↑"; tip: qsTr("Vorheriger Treffer (Umschalt+F3)"); enabled: viewer.hits.length > 0; onClicked: viewer.nextHit(-1) }
    IconButton { glyph: "↓"; tip: qsTr("Nächster Treffer (F3)"); enabled: viewer.hits.length > 0; onClicked: viewer.nextHit(1) }
    IconButton {
        glyph: "T"
        tip: qsTr("Text der aktuellen Seite, Formeln als LaTeX")
        enabled: pdf.status === "ready"
        onClicked: pdf.extractText(viewer.currentPage)
    }
    IconButton { iconName: "refresh"; tip: qsTr("Neu laden"); onClicked: { pdf.reload(); annotations.reload() } }
    IconButton {
        iconName: "play"
        tip: qsTr("Präsentieren ab dieser Seite (Umschalt+F5; F5: von vorn)")
        onClicked: viewer.startPresentation(false)
    }
    Rectangle { Layout.preferredWidth: 1; Layout.preferredHeight: 18; color: Theme.border }
    IconButton {
        iconName: "backlinks"; glyph: String(contextView.count)
        tip: qsTr("Notizen, die auf diese PDF verweisen oder daraus zitieren")
        checkable: true
        checked: viewer.showContext && contextView.tab === 0
        onClicked: { viewer.showContext = !(viewer.showContext && contextView.tab === 0); contextView.tab = 0 }
    }
    IconButton {
        iconName: "graph"
        tip: qsTr("Graph: Notizen, PDFs und Dateien des Projekts")
        checkable: true
        checked: viewer.showContext && contextView.tab === 1
        onClicked: { viewer.showContext = !(viewer.showContext && contextView.tab === 1); contextView.tab = 1 }
    }
}
