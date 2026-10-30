// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// The settings of one extension, drawn from its manifest (`settings:`).
// Every change is stored at once – secrets in the secret store, never in a
// file – and reported with `changed()` so the owner can re-read the list.
ColumnLayout {
    id: form
    required property AppController app
    /// One entry of `app.extensions()`.
    required property var ext
    property var actions: null

    signal changed()

    spacing: 10

    function store(key, value) {
        const error = form.app.setExtensionSetting(form.ext.name, key, JSON.stringify(value))
        if (error && form.actions) form.actions.showMessage(error)
        form.changed()
    }

    DoubleValidator { id: numberCheck; locale: "C" }

    Repeater {
        model: form.ext ? form.ext.settings : []
        ColumnLayout {
            id: row
            required property var modelData
            Layout.fillWidth: true
            spacing: 3
            RowLayout {
                Layout.fillWidth: true
                Label { text: row.modelData.title; color: Theme.textStrong; Layout.fillWidth: true; elide: Text.ElideRight }
                Label {
                    text: "ATLAS_SETTING_" + row.modelData.key.toUpperCase().replace(/-/g, "_")
                    color: Theme.textFaint
                    font.family: Theme.monoFont
                    font.pixelSize: Theme.smallFontSize
                }
            }
            Label {
                Layout.fillWidth: true
                visible: text.length > 0
                text: row.modelData.description
                color: Theme.textMuted
                wrapMode: Text.Wrap
                font.pixelSize: Theme.smallFontSize
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 6
                TextField {
                    Layout.fillWidth: true
                    visible: ["string", "path", "number", "secret"].indexOf(row.modelData.type) >= 0
                    echoMode: row.modelData.type === "secret" ? TextInput.Password : TextInput.Normal
                    text: row.modelData.type === "secret" ? "" : String(row.modelData.value)
                    placeholderText: row.modelData.type === "secret"
                        ? (row.modelData.value ? qsTr("gespeichert – neu eingeben zum Ersetzen") : qsTr("nicht gesetzt"))
                        : String(row.modelData.default)
                    validator: row.modelData.type === "number" ? numberCheck : null
                    onEditingFinished: {
                        const t = text.trim()
                        if (row.modelData.type === "secret") {
                            if (t) { form.store(row.modelData.key, t); text = "" }
                        } else if (row.modelData.type === "number") {
                            if (t && t !== String(row.modelData.value)) form.store(row.modelData.key, Number(t.replace(",", ".")))
                        } else if (text !== String(row.modelData.value)) {
                            form.store(row.modelData.key, text)
                        }
                    }
                }
                CheckBox {
                    visible: row.modelData.type === "bool"
                    checked: row.modelData.value === true
                    text: checked ? qsTr("An") : qsTr("Aus")
                    onToggled: form.store(row.modelData.key, checked)
                }
                ComboBox {
                    Layout.fillWidth: true
                    visible: row.modelData.type === "choice"
                    model: row.modelData.choices
                    currentIndex: row.modelData.choices.indexOf(String(row.modelData.value))
                    onActivated: (i) => form.store(row.modelData.key, row.modelData.choices[i])
                }
                Item { Layout.fillWidth: row.modelData.type === "bool" }
                Button {
                    flat: true
                    text: row.modelData.type === "secret" ? qsTr("Vergessen") : qsTr("Standard")
                    enabled: row.modelData.type === "secret" ? row.modelData.value === true
                        : JSON.stringify(row.modelData.value) !== JSON.stringify(row.modelData.default)
                    onClicked: form.store(row.modelData.key, null)
                }
            }
        }
    }
}
