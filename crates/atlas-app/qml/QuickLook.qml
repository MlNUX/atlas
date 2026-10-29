// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Large temporary preview (Space, Ctrl+click in the tree): no tab, no
// split. A header with page and zoom controls, the document and an info
// column; arrow keys move through the list it was opened from, Enter opens
// a permanent tab, Space/Esc close.
Popup {
    id: root
    required property AppController app
    property var actions
    property string path
    /// `step => path` provided by the list the preview was opened from.
    property var navigate: null
    property bool showInfo: true
    property var info: ({})

    signal openRequested(string path)
    signal splitRequested(string path)

    readonly property var pdf: view.kind === "pdf" && view.viewerItem && view.viewerItem.pageCount !== undefined ? view.viewerItem : null
    readonly property var markdown: view.kind === "markdown" && view.viewerItem && view.viewerItem.info ? view.viewerItem : null
    readonly property alias documentView: view

    function show(path, navigate) {
        root.navigate = navigate || null
        root.path = path
        open()
        view.forceActiveFocus()
    }
    function step(delta) {
        if (!navigate) return
        const next = navigate(delta)
        if (next) root.path = next
    }
    function openAsTab() { const p = root.path; close(); root.openRequested(p) }
    function openAsSplit() { const p = root.path; close(); root.splitRequested(p) }
    function kindLabel(kind) {
        return ({ pdf: qsTr("PDF-Dokument"), markdown: qsTr("Markdown-Notiz"), text: qsTr("Textdatei"),
                  image: qsTr("Bild"), dir: qsTr("Ordner"), missing: qsTr("Nicht gefunden") })[kind] || qsTr("Datei")
    }
    function dateText(ms) { return ms ? Qt.formatDateTime(new Date(ms), "dd.MM.yyyy  HH:mm") : "–" }

    onPathChanged: info = path && actions ? JSON.parse(actions.ops.properties(path)) : ({})

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: parent ? Math.min(parent.width - 80, 1180) : 900
    height: parent ? parent.height - 120 : 640
    modal: true
    focus: true
    padding: 0
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    // The card ends above the key hints, which float below it.
    readonly property int hintsHeight: 54
    background: Item {
        Rectangle {
            anchors.fill: parent
            anchors.bottomMargin: root.hintsHeight
            color: Theme.panel
            border.color: Theme.borderStrong
            radius: 14
        }
    }
    Overlay.modal: Rectangle { color: Theme.dim }

    contentItem: ColumnLayout {
        spacing: 0

        // ---- header: title, pages, zoom
        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: 50
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 18
                anchors.rightMargin: 10
                spacing: 6
                Text {
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    text: qsTr("Schnellansicht – %1").arg(root.app.fileName(root.path))
                    color: Theme.textStrong
                    font.pixelSize: 15
                    font.weight: Font.DemiBold
                    elide: Text.ElideMiddle
                }
                Row {
                    visible: !!root.pdf
                    spacing: 4
                    IconButton {
                        iconName: "chevron-left"
                        tip: qsTr("Vorherige Seite")
                        enabled: !!root.pdf && root.pdf.currentPage > 0
                        onClicked: root.pdf.goToPage(root.pdf.currentPage - 1)
                    }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 64
                        horizontalAlignment: Text.AlignHCenter
                        text: root.pdf ? (root.pdf.currentPage + 1) + " / " + root.pdf.pageCount : ""
                        color: Theme.text
                        font.pixelSize: Theme.fontSize
                    }
                    IconButton {
                        iconName: "chevron-right"
                        tip: qsTr("Nächste Seite")
                        enabled: !!root.pdf && root.pdf.currentPage < root.pdf.pageCount - 1
                        onClicked: root.pdf.goToPage(root.pdf.currentPage + 1)
                    }
                }
                Rectangle { visible: !!root.pdf; width: 1; height: 22; color: Theme.border; Layout.leftMargin: 8; Layout.rightMargin: 8 }
                Row {
                    visible: !!root.pdf
                    spacing: 4
                    IconButton { iconName: "minus"; tip: qsTr("Verkleinern"); onClicked: root.pdf.zoomBy(1 / 1.2) }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 52
                        horizontalAlignment: Text.AlignHCenter
                        text: root.pdf ? Math.round(root.pdf.zoomScale * 100) + " %" : ""
                        color: Theme.text
                        font.pixelSize: Theme.fontSize
                    }
                    IconButton { iconName: "plus"; tip: qsTr("Vergrößern"); onClicked: root.pdf.zoomBy(1.2) }
                    IconButton {
                        iconName: "maximize"
                        tip: qsTr("Ganze Seite")
                        checkable: true
                        checked: !!root.pdf && root.pdf.zoom < 0
                        onClicked: root.pdf.zoom = root.pdf.zoom < 0 ? 0 : -1
                    }
                }
                Rectangle { visible: !!root.pdf; width: 1; height: 22; color: Theme.border; Layout.leftMargin: 8; Layout.rightMargin: 8 }
                IconButton {
                    iconName: "sidebar-right"
                    tip: qsTr("Infospalte")
                    checkable: true
                    checked: root.showInfo
                    onClicked: root.showInfo = !root.showInfo
                }
                IconButton { iconName: "close"; tip: qsTr("Schließen (Esc)"); onClicked: root.close() }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            DocumentView {
                id: view
                Layout.fillWidth: true
                Layout.fillHeight: true
                app: root.app
                actions: root.actions
                path: root.visible ? root.path : ""
                compact: true
                focus: true
                onOpenRequested: (p) => { root.close(); root.openRequested(p) }
                // Keys go to the preview first; the shortcuts below only see
                // what it does not use.
                Keys.priority: Keys.AfterItem
                Keys.onPressed: (event) => {
                    if (event.key === Qt.Key_Space || event.key === Qt.Key_Escape) root.close()
                    else if (event.key === Qt.Key_Right || event.key === Qt.Key_Down) root.step(1)
                    else if (event.key === Qt.Key_Left || event.key === Qt.Key_Up) root.step(-1)
                    else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) root.openAsTab()
                    else return
                    event.accepted = true
                }
            }

            // ---- info column
            Rectangle {
                visible: root.showInfo
                Layout.preferredWidth: 290
                Layout.fillHeight: true
                color: Theme.panel
                radius: 14
                Rectangle { width: 1; height: parent.height; color: Theme.border }
                Rectangle { x: 1; width: 14; height: parent.height; color: Theme.panel }

                Flickable {
                    anchors.fill: parent
                    anchors.margins: 20
                    contentHeight: infoColumn.implicitHeight
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds

                    ColumnLayout {
                        id: infoColumn
                        width: parent.width
                        spacing: 14

                        RowLayout {
                            id: titleRow
                            spacing: 12
                            Layout.fillWidth: true
                            readonly property var icon: Theme.fileIcon(root.app.fileName(root.path), view.kind === "dir")
                            Rectangle {
                                Layout.preferredWidth: 38
                                Layout.preferredHeight: 38
                                radius: 9
                                color: Theme.raised
                                border.color: Theme.border
                                Icon { anchors.centerIn: parent; name: titleRow.icon[0]; size: 20; color: titleRow.icon[1] }
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2
                                Text {
                                    Layout.fillWidth: true
                                    text: root.app.fileName(root.path)
                                    color: Theme.textStrong
                                    font.pixelSize: 15
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideMiddle
                                }
                                Text {
                                    Layout.fillWidth: true
                                    text: root.kindLabel(view.kind) + (root.info.sizeText ? "  ·  " + root.info.sizeText : "")
                                    color: Theme.textMuted
                                    font.pixelSize: Theme.smallFontSize
                                }
                            }
                        }

                        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }

                        GridLayout {
                            Layout.fillWidth: true
                            columns: 2
                            columnSpacing: 14
                            rowSpacing: 9
                            Repeater {
                                model: {
                                    const rows = [[qsTr("Geändert"), root.dateText(root.info.modified)],
                                                  [qsTr("Ordner"), root.app.relativePath(root.path.slice(0, root.path.lastIndexOf("/"))) || "/"]]
                                    if (root.pdf) {
                                        rows.push([qsTr("Seiten"), String(root.pdf.pageCount)])
                                        const mm = root.pdf.pageSizeMm(root.pdf.currentPage)
                                        if (mm) rows.push([qsTr("Abmessungen"), mm.w + " × " + mm.h + " mm"])
                                        rows.push([qsTr("Anmerkungen"), String(root.pdf.annots.length)])
                                    }
                                    if (root.markdown) {
                                        rows.push([qsTr("Überschriften"), String((root.markdown.info.headings || []).length)])
                                        rows.push([qsTr("Schlagwörter"), (root.markdown.info.tags || []).map(t => "#" + t).join("  ") || "–"])
                                    }
                                    return rows.reduce((all, r) => all.concat(r), [])
                                }
                                Text {
                                    required property string modelData
                                    required property int index
                                    Layout.fillWidth: index % 2 === 1
                                    Layout.alignment: Qt.AlignTop
                                    text: modelData
                                    color: index % 2 ? Theme.text : Theme.textMuted
                                    font.pixelSize: Theme.fontSize
                                    wrapMode: index % 2 ? Text.Wrap : Text.NoWrap
                                }
                            }
                        }

                        // PDF annotations, like the notes of the mockup.
                        ColumnLayout {
                            visible: !!root.pdf && root.pdf.annots.length > 0
                            Layout.fillWidth: true
                            spacing: 8
                            Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }
                            Text { text: qsTr("Anmerkungen"); color: Theme.textStrong; font.pixelSize: Theme.fontSize; font.weight: Font.DemiBold }
                            Repeater {
                                model: root.pdf ? root.pdf.sortedAnnots.slice(0, 4) : []
                                Rectangle {
                                    id: annot
                                    required property var modelData
                                    Layout.fillWidth: true
                                    implicitHeight: annotText.implicitHeight + 16
                                    radius: 8
                                    color: Theme.editor
                                    border.color: annotHover.hovered ? Theme.borderStrong : Theme.border
                                    Rectangle { x: 0; width: 3; height: parent.height; radius: 2; color: annot.modelData.color || Theme.warn }
                                    Text {
                                        id: annotText
                                        anchors.fill: parent
                                        anchors.margins: 8
                                        anchors.leftMargin: 12
                                        text: root.pdf ? root.pdf.annotationLabel(annot.modelData) : ""
                                        color: Theme.text
                                        font.pixelSize: Theme.smallFontSize
                                        wrapMode: Text.Wrap
                                        maximumLineCount: 3
                                        elide: Text.ElideRight
                                    }
                                    HoverHandler { id: annotHover }
                                    MouseArea { anchors.fill: parent; onClicked: root.pdf.focusAnnotation(annot.modelData.id) }
                                }
                            }
                        }

                        Item { Layout.preferredHeight: 4 }
                        Button {
                            Layout.fillWidth: true
                            text: qsTr("Im Tab öffnen")
                            highlighted: true
                            onClicked: root.openAsTab()
                        }
                        Button {
                            Layout.fillWidth: true
                            text: qsTr("Im Split öffnen")
                            onClicked: root.openAsSplit()
                        }
                    }
                }
            }
        }

        // ---- key hints below the preview
        Rectangle {
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: 14
            Layout.preferredWidth: hints.implicitWidth + 16
            Layout.preferredHeight: 40
            radius: 10
            color: Theme.panel
            border.color: Theme.border
            Row {
                id: hints
                anchors.centerIn: parent
                spacing: 10
                Repeater {
                    model: [[qsTr("Leertaste"), qsTr("schließen")], [qsTr("Enter"), qsTr("im Tab öffnen")],
                            ["← →", qsTr("blättern")], [qsTr("Esc"), qsTr("schließen")]]
                    Row {
                        required property var modelData
                        required property int index
                        spacing: 8
                        Rectangle { visible: index > 0; width: 1; height: 18; color: Theme.border; anchors.verticalCenter: parent.verticalCenter }
                        Rectangle {
                            anchors.verticalCenter: parent.verticalCenter
                            width: key.implicitWidth + 14
                            height: 24
                            radius: 6
                            color: Theme.raised
                            border.color: Theme.borderStrong
                            Text { id: key; anchors.centerIn: parent; text: modelData[0]; color: Theme.textStrong; font.pixelSize: Theme.smallFontSize }
                        }
                        Text { anchors.verticalCenter: parent.verticalCenter; text: modelData[1]; color: Theme.textMuted; font.pixelSize: Theme.smallFontSize }
                    }
                }
            }
        }

    }
}
