// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// General settings (stored in config.toml) and the way to the specific
// ones: external editor, calendar and Nextcloud, extensions.
Dialog {
    id: dialog
    required property AppController app
    property var actions: null

    signal extensionsRequested()

    anchors.centerIn: Overlay.overlay
    width: Math.min(parent ? parent.width - 60 : 560, 560)
    modal: true
    title: qsTr("Einstellungen")
    standardButtons: Dialog.Close

    readonly property int extraPlaces: dialog.app.placeNames.length - dialog.app.fixedPlaceCount

    ColumnLayout {
        anchors.fill: parent
        spacing: 10

        Label { text: qsTr("SEITENLEISTE"); font.bold: true; color: Theme.text; font.pixelSize: Theme.smallFontSize }
        CheckBox {
            id: fileManagerBox
            Layout.fillWidth: true
            text: qsTr("Orte aus Dolphin und anderen Dateimanagern anzeigen")
            checked: dialog.app.fileManagerPlaces
            onToggled: dialog.app.fileManagerPlaces = checked
        }
        Label {
            Layout.fillWidth: true
            Layout.leftMargin: 28
            wrapMode: Text.Wrap
            color: Theme.textMuted
            font.pixelSize: Theme.smallFontSize
            text: qsTr("Unter den festen Orten erscheinen die lokalen Orte aus Dolphin/KDE (~/.local/share/user-places.xbel) und GTK-Dateimanagern (~/.config/gtk-3.0/bookmarks). Ausgeblendete Einträge, Netzwerk-Orte und Laufwerke werden nicht übernommen; Änderungen in Dolphin erscheinen sofort.")
                + (dialog.app.fileManagerPlaces
                    ? "\n" + (dialog.extraPlaces > 0 ? qsTr("Übernommen: %1").arg(dialog.app.placeNames.slice(dialog.app.fixedPlaceCount).join(", "))
                                                       : qsTr("Keine weiteren Orte gefunden."))
                    : "")
        }

        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border; Layout.topMargin: 4 }
        Label { text: qsTr("WEITERE EINSTELLUNGEN"); font.bold: true; color: Theme.text; font.pixelSize: Theme.smallFontSize }
        Flow {
            Layout.fillWidth: true
            spacing: 8
            Button { text: qsTr("Externer Editor …"); onClicked: { dialog.close(); dialog.actions.configureEditor() } }
            Button { text: qsTr("Kalender und Nextcloud …"); onClicked: { dialog.close(); dialog.actions.calendarSync.openSettings() } }
            Button { text: qsTr("Erweiterungen …"); onClicked: { dialog.close(); dialog.extensionsRequested() } }
        }
    }
}
