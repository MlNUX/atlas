// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Properties of a file or folder: details, Unix permissions, owner and
// group. Changes use the user's existing rights only (mode and group of own
// files, owner only as root).
Dialog {
    id: dialog
    required property FileOps ops
    property string path
    property var info: ({})
    property int mode: 0
    property var groups: []
    property string folderSize: ""

    readonly property bool editable: !!info.canChangeMode
    readonly property bool changed: mode !== info.mode
        || (groupBox.currentIndex >= 0 && groups[groupBox.currentIndex] && groups[groupBox.currentIndex].gid !== info.gid)
        || (ownerField.text.trim() !== "" && ownerField.text.trim() !== info.owner)

    anchors.centerIn: Overlay.overlay
    width: 520
    modal: true
    title: qsTr("Eigenschaften")

    function show(path) {
        dialog.path = path
        reload()
        open()
    }
    function reload() {
        info = JSON.parse(ops.properties(path))
        mode = info.mode || 0
        ownerField.text = info.owner || ""
        // Own groups, plus the current one if the user is not a member.
        const list = JSON.parse(ops.ownGroups())
        if (info.gid !== undefined && !list.some(g => g.gid === info.gid)) list.unshift({ gid: info.gid, name: info.group, foreign: true })
        groups = list
        groupBox.currentIndex = list.findIndex(g => g.gid === info.gid)
        folderSize = info.isDir ? qsTr("wird berechnet …") : ""
        if (info.isDir) ops.computeSize(path)
    }
    function bit(b) { return (mode & b) !== 0 }
    function setBit(b, on) { mode = on ? (mode | b) : (mode & ~b) }
    function octal(m) { return ("0000" + m.toString(8)).slice(-4) }
    function modeText(m) {
        const chars = "rwxrwxrwx"
        let s = ""
        for (let i = 0; i < 9; ++i) s += (m & (0o400 >> i)) ? chars[i] : "-"
        return s
    }
    function date(ms) { return ms ? new Date(ms).toLocaleString(Qt.locale(), Locale.ShortFormat) : "" }

    function apply() {
        let ok = true
        if (mode !== info.mode) ok = ops.setMode(path, mode) && ok
        const g = groups[groupBox.currentIndex]
        const owner = ownerField.text.trim() !== info.owner ? ownerField.text.trim() : ""
        if ((g && g.gid !== info.gid) || owner) ok = ops.setOwnership(path, owner, g && g.gid !== info.gid ? g.gid : -1) && ok
        reload()
        return ok
    }

    Connections {
        target: dialog.ops
        function onSizeComputed(path, bytes, files, dirs) {
            if (path !== dialog.path) return
            dialog.folderSize = qsTr("%1 · %2 Dateien, %3 Ordner").arg(Qt.locale().formattedDataSize(bytes)).arg(files).arg(dirs)
        }
    }

    footer: DialogButtonBox {
        Button {
            text: qsTr("Anwenden")
            enabled: dialog.changed
            DialogButtonBox.buttonRole: DialogButtonBox.ApplyRole
        }
        Button { text: qsTr("Schließen"); DialogButtonBox.buttonRole: DialogButtonBox.RejectRole }
        onApplied: dialog.apply()
        onRejected: dialog.close()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 12

        Label {
            visible: !!dialog.info.error
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            color: Theme.error
            text: dialog.info.error || ""
        }

        GridLayout {
            columns: 2
            columnSpacing: 16
            rowSpacing: 4
            Layout.fillWidth: true
            Label { text: qsTr("Name"); color: Theme.textMuted }
            Label { text: dialog.info.name || ""; color: Theme.textStrong; font.bold: true; elide: Text.ElideMiddle; Layout.fillWidth: true }
            Label { text: qsTr("Ort"); color: Theme.textMuted }
            Label { text: dialog.path; color: Theme.text; elide: Text.ElideMiddle; Layout.fillWidth: true }
            Label { text: qsTr("Verweist auf"); color: Theme.textMuted; visible: !!dialog.info.isSymlink }
            Label { text: dialog.info.linkTarget || ""; color: Theme.text; visible: !!dialog.info.isSymlink; elide: Text.ElideMiddle; Layout.fillWidth: true }
            Label { text: qsTr("Größe"); color: Theme.textMuted }
            Label { text: dialog.info.isDir ? dialog.folderSize : (dialog.info.sizeText || ""); color: Theme.text }
            Label { text: qsTr("Geändert"); color: Theme.textMuted }
            Label { text: dialog.date(dialog.info.modified); color: Theme.text }
            Label { text: qsTr("Zugriff"); color: Theme.textMuted }
            Label { text: dialog.date(dialog.info.accessed); color: Theme.text }
            Label { text: qsTr("Inode / Links"); color: Theme.textMuted }
            Label { text: (dialog.info.inode || "") + " / " + (dialog.info.links || ""); color: Theme.textFaint }
        }

        Rectangle { Layout.fillWidth: true; height: 1; color: Theme.border }

        Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            visible: !dialog.editable && !dialog.info.error
            color: Theme.warn
            text: dialog.info.readOnlyFs ? qsTr("Das Dateisystem ist schreibgeschützt.")
                : qsTr("Nur %1 (Besitzer) oder root kann Rechte und Gruppe ändern.").arg(dialog.info.owner || "")
        }

        // ---- permissions
        ColumnLayout {
            spacing: 0
            enabled: dialog.editable
            RowLayout {
                spacing: 0
                Item { Layout.preferredWidth: 90 }
                Repeater {
                    model: [qsTr("Lesen"), qsTr("Schreiben"), dialog.info.isDir ? qsTr("Betreten") : qsTr("Ausführen")]
                    Label {
                        required property string modelData
                        text: modelData
                        color: Theme.textMuted
                        horizontalAlignment: Text.AlignHCenter
                        Layout.preferredWidth: 90
                    }
                }
            }
            Repeater {
                model: [
                    { label: qsTr("Besitzer"), shift: 6 },
                    { label: qsTr("Gruppe"), shift: 3 },
                    { label: qsTr("Andere"), shift: 0 },
                ]
                delegate: RowLayout {
                    id: row
                    required property var modelData
                    readonly property int shift: modelData.shift
                    spacing: 0
                    Label { text: row.modelData.label; color: Theme.text; Layout.preferredWidth: 90 }
                    Repeater {
                        model: [4, 2, 1]
                        Item {
                            id: cell
                            required property int modelData
                            readonly property int mask: modelData << row.shift
                            Layout.preferredWidth: 90
                            implicitHeight: box.implicitHeight
                            CheckBox {
                                id: box
                                anchors.horizontalCenter: parent.horizontalCenter
                                checked: dialog.bit(cell.mask)
                                onToggled: dialog.setBit(cell.mask, checked)
                            }
                        }
                    }
                }
            }
        }
        RowLayout {
            enabled: dialog.editable
            spacing: 10
            CheckBox { text: "setuid"; checked: dialog.bit(0o4000); onToggled: dialog.setBit(0o4000, checked) }
            CheckBox { text: "setgid"; checked: dialog.bit(0o2000); onToggled: dialog.setBit(0o2000, checked) }
            CheckBox { text: qsTr("Sticky"); checked: dialog.bit(0o1000); onToggled: dialog.setBit(0o1000, checked) }
            Item { Layout.fillWidth: true }
            TextField {
                Layout.preferredWidth: 70
                text: dialog.octal(dialog.mode)
                validator: RegularExpressionValidator { regularExpression: /[0-7]{3,4}/ }
                onEditingFinished: if (acceptableInput) dialog.mode = parseInt(text, 8)
                font.family: Theme.monoFont
            }
            Label { text: dialog.modeText(dialog.mode); font.family: Theme.monoFont; color: Theme.textMuted }
        }

        Rectangle { Layout.fillWidth: true; height: 1; color: Theme.border }

        // ---- owner and group
        GridLayout {
            columns: 2
            columnSpacing: 16
            Layout.fillWidth: true
            Label { text: qsTr("Besitzer"); color: Theme.textMuted }
            TextField {
                id: ownerField
                Layout.fillWidth: true
                readOnly: !dialog.ops.canChangeOwner()
                ToolTip.visible: hovered && readOnly
                ToolTip.text: qsTr("Den Besitzer kann nur root ändern.")
            }
            Label { text: qsTr("Gruppe"); color: Theme.textMuted }
            ComboBox {
                id: groupBox
                Layout.fillWidth: true
                enabled: dialog.editable
                model: dialog.groups.map(g => g.name + (g.foreign ? qsTr(" (nicht Ihre Gruppe)") : ""))
            }
        }
    }
}
