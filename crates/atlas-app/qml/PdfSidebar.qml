// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// The PDF viewer's sidebar: page thumbnails, outline and the list of annotations.
Rectangle {
    id: sidebar
    required property var viewer
    required property var pdf
    required property var annotations
    required property var annotationPopup
    visible: viewer.sidebarVisible
    Layout.preferredWidth: 220
    Layout.fillHeight: true
    color: Theme.sidebar
    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        TabBar {
            id: sideTabs
            Layout.fillWidth: true
            currentIndex: viewer.sidebarTab
            onCurrentIndexChanged: viewer.sidebarTab = currentIndex
            TabButton { text: qsTr("Seiten"); font.pixelSize: Theme.smallFontSize }
            TabButton { text: qsTr("Inhalt"); font.pixelSize: Theme.smallFontSize }
            TabButton { text: qsTr("Anmerkungen") + (annotations.count ? " " + annotations.count : ""); font.pixelSize: Theme.smallFontSize }
        }
        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: sideTabs.currentIndex
            ListView {
                id: thumbs
                clip: true
                model: pdf.status === "ready" ? pdf.pageCount : 0
                spacing: 8
                topMargin: 8
                cacheBuffer: 400
                ScrollBar.vertical: ScrollBar {}
                delegate: Item {
                    id: thumb
                    required property int index
                    width: ListView.view.width
                    height: 110 * pdf.pageHeight(index) / pdf.pageWidth(index) + 22
                    Rectangle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 110
                        height: parent.height - 22
                        color: viewer.paperColor
                        border.width: thumb.index === viewer.currentPage ? 2 : 0
                        border.color: Theme.accent
                        PdfPage {
                            anchors.fill: parent
                            anchors.margins: parent.border.width
                            source: pdf.source
                            page: thumb.index
                            inverted: viewer.dark
                            renderWidth: Math.round(110 * Screen.devicePixelRatio)
                        }
                        Text {
                            visible: (viewer.annotsByPage[thumb.index + 1] || []).some(a => a.kind === "bookmark")
                            anchors.right: parent.right
                            anchors.top: parent.top
                            text: "🔖"
                            font.pixelSize: 12
                        }
                        MouseArea { anchors.fill: parent; onClicked: viewer.goToPage(thumb.index) }
                    }
                    Text {
                        anchors.bottom: parent.bottom
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: thumb.index + 1
                        color: thumb.index === viewer.currentPage ? Theme.textStrong : Theme.textMuted
                        font.pixelSize: Theme.smallFontSize
                    }
                }
            }
            ListView {
                id: outlineList
                clip: true
                model: { try { return JSON.parse(pdf.outlineJson || "[]") } catch (e) { return [] } }
                ScrollBar.vertical: ScrollBar {}
                delegate: Rectangle {
                    id: entry
                    required property var modelData
                    width: ListView.view.width
                    height: Theme.rowHeight
                    color: entryHover.hovered ? Theme.hover : "transparent"
                    HoverHandler { id: entryHover }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        x: 8 + entry.modelData.depth * 12
                        width: parent.width - x - 30
                        text: entry.modelData.title
                        color: Theme.text
                        font.pixelSize: Theme.smallFontSize
                        elide: Text.ElideRight
                    }
                    Text {
                        anchors.right: parent.right
                        anchors.rightMargin: 6
                        anchors.verticalCenter: parent.verticalCenter
                        text: entry.modelData.page >= 0 ? entry.modelData.page + 1 : ""
                        color: Theme.textFaint
                        font.pixelSize: Theme.smallFontSize
                    }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: if (entry.modelData.page >= 0) viewer.goToPage(entry.modelData.page)
                    }
                }
                Label {
                    anchors.centerIn: parent
                    visible: outlineList.count === 0
                    text: qsTr("Kein Inhaltsverzeichnis")
                    color: Theme.textFaint
                    font.pixelSize: Theme.smallFontSize
                }
            }
            ColumnLayout {
                spacing: 0
                ListView {
                    id: annotList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    // Built only while the list is shown.
                    model: viewer.sidebarVisible && viewer.sidebarTab === 2 ? viewer.sortedAnnots : []
                    ScrollBar.vertical: ScrollBar {}
                    delegate: Rectangle {
                        id: annotRow
                        required property var modelData
                        width: ListView.view.width
                        height: annotColumn.implicitHeight + 10
                        color: annotHover.hovered ? Theme.hover : viewer.focusedAnnot === modelData.id ? Theme.selection : "transparent"
                        HoverHandler { id: annotHover }
                        Rectangle {
                            x: 0; width: 3; height: parent.height
                            color: annotRow.modelData.color || Theme.accent
                        }
                        ColumnLayout {
                            id: annotColumn
                            x: 10
                            y: 5
                            width: parent.width - 34
                            spacing: 2
                            RowLayout {
                                spacing: 4
                                Text { text: viewer.kindGlyph(annotRow.modelData.kind); color: Theme.textMuted; font.pixelSize: Theme.smallFontSize }
                                Text { text: qsTr("S. %1").arg(annotRow.modelData.page); color: Theme.textFaint; font.pixelSize: Theme.smallFontSize }
                            }
                            Text {
                                Layout.fillWidth: true
                                text: viewer.annotationLabel(annotRow.modelData)
                                color: Theme.text
                                font.pixelSize: Theme.smallFontSize
                                wrapMode: Text.Wrap
                                maximumLineCount: 3
                                elide: Text.ElideRight
                            }
                            Text {
                                Layout.fillWidth: true
                                visible: !!annotRow.modelData.comment && !!annotRow.modelData.text
                                text: "💬 " + (annotRow.modelData.comment || "")
                                color: Theme.textMuted
                                font.pixelSize: Theme.smallFontSize
                                wrapMode: Text.Wrap
                                maximumLineCount: 2
                                elide: Text.ElideRight
                            }
                        }
                        MouseArea {
                            anchors.fill: parent
                            acceptedButtons: Qt.LeftButton | Qt.RightButton
                            onClicked: (mouse) => {
                                viewer.focusAnnotation(annotRow.modelData.id)
                                if (mouse.button === Qt.RightButton) annotationPopup.show(annotRow.modelData.id, null)
                            }
                            onDoubleClicked: annotationPopup.show(annotRow.modelData.id, null)
                        }
                        IconButton {
                            anchors.right: parent.right
                            anchors.rightMargin: 4
                            anchors.top: parent.top
                            anchors.topMargin: 4
                            visible: annotHover.hovered
                            iconName: "close"
                            tip: qsTr("Löschen")
                            onClicked: viewer.removeAnnot(annotRow.modelData.id)
                        }
                    }
                    Label {
                        anchors.centerIn: parent
                        width: parent.width - 24
                        visible: annotList.count === 0
                        wrapMode: Text.Wrap
                        horizontalAlignment: Text.AlignHCenter
                        color: Theme.textFaint
                        font.pixelSize: Theme.smallFontSize
                        text: annotations.status === "error" ? annotations.error
                            : qsTr("Noch keine Anmerkungen.\nText markieren, ✎ zeichnen, 🗨 anheften oder 🔖 setzen.")
                    }
                }
                Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }
                Button {
                    Layout.fillWidth: true
                    Layout.margins: 6
                    flat: true
                    font.pixelSize: Theme.smallFontSize
                    text: qsTr("Als PDF-Kopie exportieren …")
                    enabled: annotations.count > 0
                    onClicked: viewer.exportCopy()
                    ToolTip.visible: hovered
                    ToolTip.delay: 600
                    ToolTip.text: qsTr("Schreibt eine Kopie mit eingebetteten Annotationen; die Original-PDF bleibt unverändert.")
                }
            }
        }
    }
}
