// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// General settings (stored in config.toml) and the way to the specific
// ones – external editor, calendar and Nextcloud, extensions, actions –
// each with its current state.
Dialog {
    id: dialog
    required property AppController app
    property var actions: null

    signal extensionsRequested()

    anchors.centerIn: Overlay.overlay
    width: Math.min(parent ? parent.width - 60 : 600, 600)
    modal: true
    padding: 0
    topPadding: 0
    title: ""

    readonly property int extraPlaces: dialog.app.placeNames.length - dialog.app.fixedPlaceCount
    /// Read when the dialog opens.
    property string editorText
    property int extensionCount: 0
    onAboutToShow: {
        const command = dialog.app.editorCommand()
        editorText = command ? command : qsTr("Standard: $VISUAL, $EDITOR oder das Programm des Systems")
        try { extensionCount = JSON.parse(dialog.app.extensions()).length } catch (e) { extensionCount = 0 }
    }
    readonly property var calendar: dialog.actions ? dialog.actions.calendar : null
    /// Where the places come from, for the line under the switch.
    readonly property string placesText: !dialog.app.fileManagerPlaces ? qsTr("Feste Orte von Atlas")
        : dialog.app.fixedPlaceCount === 0 ? qsTr("Aus Dolphin: %1").arg(dialog.app.placeNames.join(", ") || qsTr("keine"))
        : dialog.extraPlaces > 0 ? qsTr("Keine Dolphin-Orte gefunden – aus GTK: %1").arg(dialog.app.placeNames.slice(dialog.app.fixedPlaceCount).join(", "))
        : qsTr("Keine Dolphin-Orte gefunden – feste Orte von Atlas")

    function run(f) { dialog.close(); f() }

    contentItem: ColumnLayout {
        spacing: 0

        // ---- header
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 18
            Layout.bottomMargin: 14
            spacing: 12
            Rectangle {
                Layout.preferredWidth: 36
                Layout.preferredHeight: 36
                radius: 10
                color: Theme.accentSoft
                Icon { anchors.centerIn: parent; name: "settings"; size: 20; color: Theme.accent }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Text {
                    text: qsTr("Einstellungen")
                    color: Theme.textStrong
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                }
                Text {
                    Layout.fillWidth: true
                    text: qsTr("Gespeichert in ~/.config/atlas/config.toml")
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                }
            }
            IconButton { iconName: "close"; tip: qsTr("Schließen (Esc)"); onClicked: dialog.close() }
        }
        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.margins: 18
            spacing: 8

            SectionLabel { text: qsTr("Seitenleiste") }
            Card {
                Layout.fillWidth: true
                implicitHeight: placesRow.implicitHeight + 24
                RowLayout {
                    id: placesRow
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 12
                    Icon { Layout.alignment: Qt.AlignTop; name: "home"; size: 18; color: Theme.accent }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 3
                        Text {
                            text: qsTr("Orte wie in Dolphin")
                            color: Theme.textStrong
                            font.pixelSize: Theme.fontSize
                        }
                        Text {
                            Layout.fillWidth: true
                            wrapMode: Text.Wrap
                            color: Theme.textMuted
                            font.pixelSize: Theme.smallFontSize
                            text: qsTr("Dieselben Orte in derselben Reihenfolge wie in Dolphin; was du dort entfernst, fehlt auch hier. Netzwerk-Orte und Laufwerke nicht.")
                        }
                        Text {
                            Layout.fillWidth: true
                            Layout.topMargin: 2
                            text: dialog.placesText
                            color: Theme.text
                            font.pixelSize: Theme.smallFontSize
                            elide: Text.ElideRight
                        }
                    }
                    Toggle {
                        id: fileManagerToggle
                        Layout.alignment: Qt.AlignVCenter
                        checked: dialog.app.fileManagerPlaces
                        onToggled: dialog.app.fileManagerPlaces = !dialog.app.fileManagerPlaces
                    }
                }
            }

            SectionLabel { text: qsTr("Programme und Daten"); Layout.topMargin: 10 }
            Card {
                Layout.fillWidth: true
                implicitHeight: links.implicitHeight
                ColumnLayout {
                    id: links
                    anchors.left: parent.left
                    anchors.right: parent.right
                    spacing: 0
                    LinkRow {
                        icon: "file-code"
                        title: qsTr("Externer Editor")
                        detail: dialog.editorText
                        onClicked: dialog.run(() => dialog.actions.configureEditor())
                    }
                    LinkRow {
                        icon: "calendar"
                        title: qsTr("Kalender: Nextcloud oder Google")
                        detail: !dialog.calendar || !dialog.calendar.connected ? qsTr("Nicht verbunden – nur der lokale Projektkalender")
                            : (dialog.calendar.google ? qsTr("Google · ") : qsTr("Nextcloud · ")) + (dialog.calendar.syncStatus || qsTr("Verbunden"))
                        onClicked: dialog.run(() => dialog.actions.calendarSync.openSettings())
                    }
                    LinkRow {
                        icon: "grid"
                        title: qsTr("Erweiterungen")
                        detail: dialog.extensionCount === 0 ? qsTr("Keine installiert") : qsTr("%n installiert", "", dialog.extensionCount)
                        onClicked: dialog.run(() => dialog.extensionsRequested())
                    }
                    LinkRow {
                        icon: "play"
                        title: qsTr("Projektaktionen")
                        detail: dialog.app.projectRoot ? qsTr("Nur für %1 (.atlas/actions.yaml)").arg(dialog.app.projectName) : qsTr("Kein Projekt geöffnet")
                        enabled: !!dialog.app.projectRoot
                        onClicked: dialog.run(() => dialog.actions.editActions(false))
                    }
                    LinkRow {
                        icon: "play"
                        title: qsTr("Globale Aktionen")
                        detail: qsTr("Für alle Projekte (~/.config/atlas/actions.yaml)")
                        last: true
                        onClicked: dialog.run(() => dialog.actions.editActions(true))
                    }
                }
            }
        }

        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 14
            Layout.leftMargin: 18
            Text {
                Layout.fillWidth: true
                text: qsTr("Atlas %1").arg(dialog.app.version)
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
            }
            Button { text: qsTr("Schließen"); highlighted: true; onClicked: dialog.close() }
        }
    }

    component SectionLabel: Text {
        color: Theme.textMuted
        font.pixelSize: Theme.smallFontSize
        font.weight: Font.DemiBold
        font.capitalization: Font.AllUppercase
        font.letterSpacing: 0.6
    }

    component Card: Rectangle {
        radius: 10
        color: Theme.header
        border.color: Theme.border
    }

    /// An on/off switch.
    component Toggle: Rectangle {
        id: toggle
        property bool checked
        signal toggled()
        implicitWidth: 40
        implicitHeight: 22
        radius: 11
        color: checked ? Theme.accent : Theme.hover
        border.color: checked ? Theme.accent : Theme.border
        Behavior on color { ColorAnimation { duration: 120 } }
        Rectangle {
            width: 16
            height: 16
            radius: 8
            y: 3
            x: toggle.checked ? toggle.width - width - 3 : 3
            color: "white"
            Behavior on x { NumberAnimation { duration: 120 } }
        }
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: toggle.toggled() }
    }

    /// A row leading to another dialog: icon, title, current state, chevron.
    component LinkRow: Rectangle {
        id: row
        property string icon
        property string title
        property string detail
        property bool last: false
        signal clicked()
        Layout.fillWidth: true
        implicitHeight: 54
        color: rowHover.hovered && enabled ? Theme.hover : "transparent"
        radius: 10
        opacity: enabled ? 1 : 0.5
        HoverHandler { id: rowHover }
        MouseArea { anchors.fill: parent; enabled: row.enabled; cursorShape: Qt.PointingHandCursor; onClicked: row.clicked() }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            spacing: 12
            Icon { name: row.icon; size: 18; color: Theme.accent }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Text { text: row.title; color: Theme.textStrong; font.pixelSize: Theme.fontSize }
                Text {
                    Layout.fillWidth: true
                    textFormat: Text.PlainText
                    text: row.detail
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                }
            }
            Icon { name: "chevron-right"; size: 14; color: Theme.textFaint }
        }
        Rectangle {
            visible: !row.last
            anchors.bottom: parent.bottom
            x: 42
            width: parent.width - 54
            height: 1
            color: Theme.border
        }
    }
}
