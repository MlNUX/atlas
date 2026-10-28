// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Installed extensions: status, contributions and the commands they run.
// Extensions of a project need the user's trust first; switching them off
// or on is stored in config.toml.
Dialog {
    id: dialog
    required property AppController app
    property var actions
    property var list: []
    /// Extension waiting for the trust confirmation.
    property var pending: null

    anchors.centerIn: Overlay.overlay
    width: Math.min(760, Overlay.overlay ? Overlay.overlay.width - 60 : 760)
    height: Math.min(620, Overlay.overlay ? Overlay.overlay.height - 60 : 620)
    modal: true
    padding: 0
    topPadding: 0
    title: ""

    function reload() {
        list = JSON.parse(app.extensions())
        app.refreshExtensions()
    }
    onAboutToShow: reload()

    function statusText(s) {
        return ({ active: qsTr("Aktiv"), disabled: qsTr("Ausgeschaltet"), untrusted: qsTr("Nicht freigegeben"), broken: qsTr("Fehlerhaft") })[s] || s
    }
    function statusColor(s) {
        return ({ active: Theme.ok, disabled: Theme.textFaint, untrusted: Theme.warn, broken: Theme.error })[s] || Theme.text
    }
    function commandsOf(e) {
        return e.actions.map(a => "▶ " + a.name + (a.keybinding ? " [" + a.keybinding + "]" : "") + ":  " + a.command)
            .concat(e.processors.map(p => "👁 " + p.name + " (" + p.applies_to.join(", ") + " → " + p.output + "):  " + p.command))
            .concat(e.exporters.map(x => "⇪ " + x.name + " (→ ." + x.target + "):  " + x.command))
            .concat((e.views || []).map(v => "▣ " + v.name + (v.keybinding ? " [" + v.keybinding + "]" : "") + ":  " + qsTr("Ansicht %1").arg(v.page)
                + (v.applies_to.length ? " (" + v.applies_to.join(", ") + ")" : "")))
    }

    /// What the extension says it needs (API 2), as lines.
    function permissionsOf(e) {
        const p = e.permissions
        if (!p) return [qsTr("Format 1: keine Rechte angemeldet – die Befehle dürfen alles, was du darfst.")]
        const files = ({ none: qsTr("keine"), project: qsTr("nur das Projekt"), home: qsTr("dein persönlicher Ordner"), all: qsTr("alle") })[p.files] || p.files
        const network = p.network.length === 0 ? qsTr("keins")
            : p.network.indexOf("*") >= 0 ? qsTr("beliebige Rechner") : p.network.join(", ")
        return [
            qsTr("Startet: %1").arg(p.run.length ? p.run.join(", ") + qsTr(" und Dateien der Erweiterung") : qsTr("nur Dateien der Erweiterung")),
            qsTr("Dateien: %1").arg(files),
            qsTr("Netzwerk: %1").arg(network),
        ]
    }

    contentItem: ColumnLayout {
        spacing: 0

        // ---- header (like the settings)
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 18
            Layout.bottomMargin: 14
            spacing: 12
            Rectangle {
                Layout.preferredWidth: 36
                Layout.preferredHeight: 36
                Layout.alignment: Qt.AlignTop
                radius: 10
                color: Theme.accentSoft
                Icon { anchors.centerIn: parent; name: "grid"; size: 20; color: Theme.accent }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Text {
                    text: qsTr("Erweiterungen")
                    color: Theme.textStrong
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                }
                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    text: qsTr("Ordner mit einer extension.yaml – global in ~/.config/atlas/extensions/, für ein Projekt in .atlas/extensions/. Sie fügen Aktionen, Anzeigen für weitere Dateitypen und Exporter hinzu.")
                }
            }
            IconButton { Layout.alignment: Qt.AlignTop; iconName: "close"; tip: qsTr("Schließen (Esc)"); onClicked: dialog.close() }
        }
        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }

        ListView {
            id: extList
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: 12
            Layout.rightMargin: 6
            clip: true
            spacing: 6
            model: dialog.list
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                id: card
                required property var modelData
                property bool expanded: false
                width: ListView.view.width - 6
                height: cardColumn.implicitHeight + 20
                radius: 10
                color: Theme.header
                border.color: Theme.border
                ColumnLayout {
                    id: cardColumn
                    x: 12
                    y: 10
                    width: parent.width - 24
                    spacing: 4
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8
                        Label { text: card.modelData.title; color: Theme.textStrong; font.bold: true }
                        Label { text: card.modelData.version; color: Theme.textFaint; font.pixelSize: Theme.smallFontSize }
                        Label {
                            text: card.modelData.location === "project" ? qsTr("Projekt") : qsTr("Global")
                            color: Theme.textMuted
                            font.pixelSize: Theme.smallFontSize
                        }
                        Item { Layout.fillWidth: true }
                        Label {
                            text: dialog.statusText(card.modelData.status)
                            color: dialog.statusColor(card.modelData.status)
                            font.pixelSize: Theme.smallFontSize
                            font.bold: true
                        }
                    }
                    Label {
                        Layout.fillWidth: true
                        visible: text.length > 0
                        text: card.modelData.description
                        color: Theme.text
                        wrapMode: Text.Wrap
                    }
                    Label {
                        Layout.fillWidth: true
                        text: qsTr("%1 Aktion(en) · %2 Anzeige(n) · %3 Exporter · %4 Ansicht(en) · %5")
                            .arg(card.modelData.actions.length).arg(card.modelData.processors.length)
                            .arg(card.modelData.exporters.length).arg((card.modelData.views || []).length).arg(card.modelData.name)
                        color: Theme.textFaint
                        font.pixelSize: Theme.smallFontSize
                    }
                    Repeater {
                        model: card.modelData.errors
                        Label {
                            required property string modelData
                            Layout.fillWidth: true
                            text: "⚠ " + modelData
                            color: Theme.error
                            wrapMode: Text.Wrap
                            font.pixelSize: Theme.smallFontSize
                        }
                    }
                    Repeater {
                        model: card.expanded ? dialog.permissionsOf(card.modelData) : []
                        Label {
                            required property string modelData
                            Layout.fillWidth: true
                            text: "🔑 " + modelData
                            color: Theme.textMuted
                            wrapMode: Text.Wrap
                            font.pixelSize: Theme.smallFontSize
                        }
                    }
                    Repeater {
                        model: card.expanded ? dialog.commandsOf(card.modelData) : []
                        Label {
                            required property string modelData
                            Layout.fillWidth: true
                            text: modelData
                            color: Theme.text
                            font.family: Theme.monoFont
                            font.pixelSize: Theme.smallFontSize
                            wrapMode: Text.WrapAnywhere
                        }
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.topMargin: 2
                        spacing: 2
                        Button {
                            visible: card.modelData.status === "untrusted"
                            text: qsTr("Prüfen und freigeben …")
                            onClicked: { dialog.pending = card.modelData; trustDialog.open() }
                        }
                        Button {
                            visible: card.modelData.status === "active" || card.modelData.status === "disabled"
                            text: card.modelData.status === "active" ? qsTr("Ausschalten") : qsTr("Einschalten")
                            flat: card.modelData.status === "active"
                            onClicked: {
                                const error = dialog.app.setExtensionEnabled(card.modelData.name, card.modelData.status !== "active")
                                if (error && dialog.actions) dialog.actions.showMessage(error)
                                dialog.reload()
                            }
                        }
                        Button {
                            visible: card.modelData.settings.length > 0
                            text: qsTr("Einstellungen …")
                            flat: true
                            onClicked: settingsDialog.show(card.modelData.name)
                        }
                        Item { Layout.fillWidth: true }
                        Button {
                            text: card.expanded ? qsTr("Details ausblenden") : qsTr("Details zeigen")
                            flat: true
                            onClicked: card.expanded = !card.expanded
                        }
                        Button {
                            text: qsTr("Manifest öffnen")
                            flat: true
                            onClicked: { if (dialog.actions) dialog.actions.openRequested(card.modelData.manifest, false); dialog.close() }
                        }
                        Button {
                            text: qsTr("Ordner zeigen")
                            flat: true
                            onClicked: if (dialog.actions) dialog.actions.showInFileManager(card.modelData.dir)
                        }
                    }
                }
            }
            Label {
                anchors.centerIn: parent
                visible: extList.count === 0
                width: parent.width - 40
                wrapMode: Text.Wrap
                horizontalAlignment: Text.AlignHCenter
                color: Theme.textFaint
                text: qsTr("Keine Erweiterungen installiert.")
            }
        }

        // ---- footer: count and reload on the left, actions on the right
        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 14
            Layout.leftMargin: 18
            spacing: 8
            Text {
                text: extList.count === 0 ? qsTr("Keine installiert") : qsTr("%n installiert", "", extList.count)
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
            }
            IconButton { iconName: "refresh"; tip: qsTr("Neu laden"); onClicked: dialog.reload() }
            Item { Layout.fillWidth: true }
            Button {
                id: bundledButton
                text: qsTr("Mitgeliefert …")
                flat: true
                onClicked: bundledMenu.popup(bundledButton, 0, bundledButton.height)
                Menu {
                    id: bundledMenu
                    Instantiator {
                        model: bundledMenu.visible ? JSON.parse(dialog.app.bundledExtensions()) : []
                        delegate: MenuItem {
                            required property var modelData
                            text: (modelData.installed ? qsTr("Aktualisieren: %1") : qsTr("Installieren: %1")).arg(modelData.title)
                            onTriggered: {
                                const error = dialog.app.installBundled(modelData.name)
                                if (dialog.actions) dialog.actions.showMessage(error || qsTr("„%1“ installiert").arg(modelData.title))
                                dialog.reload()
                            }
                        }
                        onObjectAdded: (index, object) => bundledMenu.insertItem(index, object)
                        onObjectRemoved: (index, object) => bundledMenu.removeItem(object)
                    }
                }
            }
            Button { text: qsTr("Neue Erweiterung …"); onClicked: dialog.askNew() }
            Button { text: qsTr("Schließen"); highlighted: true; onClicked: dialog.close() }
        }
    }

    /// Asks for the name of a new extension, then opens its manifest.
    function askNew() {
        newName.text = ""
        newDialog.open()
        newName.forceActiveFocus()
    }
    Dialog {
        id: newDialog
        anchors.centerIn: Overlay.overlay
        parent: Overlay.overlay
        width: 420
        modal: true
        title: qsTr("Neue Erweiterung")
        standardButtons: Dialog.Ok | Dialog.Cancel
        onAccepted: {
            if (!newName.text.trim()) return
            const manifest = dialog.app.newExtension(newName.text.trim())
            if (manifest && dialog.actions) dialog.actions.openRequested(manifest, false)
            dialog.close()
        }
        ColumnLayout {
            anchors.fill: parent
            spacing: 8
            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                color: Theme.textMuted
                text: qsTr("Wie soll sie heißen? Die Vorlage enthält Beispiele als Kommentare – sie tut nichts, bis du sie ausfüllst.")
            }
            TextField {
                id: newName
                Layout.fillWidth: true
                placeholderText: qsTr("z. B. PDF-Werkzeuge")
                onAccepted: newDialog.accept()
            }
        }
    }

    // Trusting a project extension: show exactly what it may run.
    Dialog {
        id: trustDialog
        anchors.centerIn: Overlay.overlay
        width: Math.min(640, dialog.width)
        modal: true
        title: qsTr("Erweiterung freigeben?")
        standardButtons: Dialog.Cancel
        ColumnLayout {
            width: parent.width
            spacing: 8
            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: dialog.pending
                    ? qsTr("„%1“ gehört zu diesem Projekt (%2). Nach der Freigabe kann Atlas folgende Befehle ausführen – Anzeigen auch automatisch beim Öffnen passender Dateien. Gib sie nur frei, wenn du dem Projekt vertraust. Ändert sich danach eine Datei der Erweiterung, fragt Atlas erneut.")
                        .arg(dialog.pending.title).arg(dialog.pending.dir)
                    : ""
            }
            Label {
                text: qsTr("Angemeldete Rechte")
                color: Theme.textStrong
                font.bold: true
            }
            Repeater {
                model: dialog.pending ? dialog.permissionsOf(dialog.pending) : []
                Label {
                    required property string modelData
                    Layout.fillWidth: true
                    text: "🔑 " + modelData
                    wrapMode: Text.Wrap
                    color: Theme.textStrong
                }
            }
            Label {
                Layout.fillWidth: true
                visible: !!(dialog.pending && dialog.pending.permissions)
                wrapMode: Text.Wrap
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
                text: qsTr("Programme außerhalb dieser Liste lehnt Atlas ab; Dateien und Netzwerk sind Angaben der Erweiterung, ihre Skripte können sie (noch) überschreiten.")
            }
            Label {
                text: qsTr("Befehle")
                color: Theme.textStrong
                font.bold: true
            }
            Repeater {
                model: dialog.pending ? dialog.commandsOf(dialog.pending) : []
                Label {
                    required property string modelData
                    Layout.fillWidth: true
                    text: modelData
                    font.family: Theme.monoFont
                    font.pixelSize: Theme.smallFontSize
                    wrapMode: Text.WrapAnywhere
                    color: Theme.textStrong
                }
            }
            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
                text: qsTr("Ändert sich eine Datei der Erweiterung, wird erneut gefragt.")
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Button {
                    text: qsTr("Freigeben")
                    onClicked: {
                        if (dialog.pending) dialog.app.trustExtension(dialog.pending.name)
                        trustDialog.close()
                        dialog.reload()
                    }
                }
            }
        }
    }

    // Settings of one extension, drawn from its manifest. Every change is
    // stored at once (secrets in the secret store, never in a file).
    Dialog {
        id: settingsDialog
        property string name
        property var ext: null
        function show(name) {
            settingsDialog.name = name
            settingsDialog.reload()
            open()
        }
        function reload() {
            dialog.list = JSON.parse(dialog.app.extensions())
            ext = dialog.list.find(e => e.name === name) || null
        }
        anchors.centerIn: Overlay.overlay
        parent: Overlay.overlay
        width: Math.min(560, dialog.width)
        modal: true
        title: ext ? qsTr("Einstellungen: %1").arg(ext.title) : ""
        standardButtons: Dialog.Close
        ExtensionSettings {
            width: parent.width
            app: dialog.app
            actions: dialog.actions
            ext: settingsDialog.ext
            onChanged: settingsDialog.reload()
        }
    }
}
