// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Templates as T

T.DialogButtonBox {
    id: control
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset, contentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset, contentHeight + topPadding + bottomPadding)
    spacing: 8
    padding: 20
    topPadding: 12
    alignment: Qt.AlignRight
    font.pixelSize: Colors.fontSize

    /// Qt's standard button texts in German (Qt translations are not loaded).
    readonly property var german: ({ "OK": "OK", "Cancel": "Abbrechen", "Close": "Schließen", "Save": "Speichern",
        "Apply": "Anwenden", "Yes": "Ja", "No": "Nein", "&Yes": "Ja", "&No": "Nein", "Discard": "Verwerfen",
        "Reset": "Zurücksetzen", "Help": "Hilfe", "Open": "Öffnen", "Abort": "Abbrechen", "Retry": "Wiederholen", "Ignore": "Ignorieren" })

    delegate: Button {
        width: Math.min(implicitWidth, control.width / Math.max(1, control.count))
        onTextChanged: { const t = control.german[text]; if (t !== undefined && t !== text) text = t }
    }

    contentItem: ListView {
        implicitWidth: contentWidth
        model: control.contentModel
        spacing: control.spacing
        orientation: ListView.Horizontal
        boundsBehavior: Flickable.StopAtBounds
        snapMode: ListView.SnapToItem
    }

    background: Item {}
}
