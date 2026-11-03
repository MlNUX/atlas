// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Editing mode of the context bar („Leiste anpassen“): the bar shows its
// composition as compact rows – tabs as chips, the overview and the blocks
// pinned to the bottom, each with a grip to drag and an eye to hide.
// „Baustein hinzufügen“ opens the gallery. Changes apply at once;
// „Zurücksetzen“ removes the profile, „Fertig“ or Esc end the mode.
FocusScope {
    id: ed
    required property AppController app
    property var config: ({})
    property var extensions: []
    /// Profile of the active file (where editing starts).
    property string currentProfile: "default"

    property bool active: false
    /// The profile being edited.
    property string profile: "default"
    readonly property bool profileSet: ContextProfiles.isSet(config, profile)
    readonly property var lists: ContextProfiles.lists(config, profile)

    function start(name) {
        profile = name || currentProfile
        active = true
        forceActiveFocus()
    }
    function finish() { active = false }
    function change(fn) { ContextProfiles.edit(app, config, profile, fn) }
    function info(id) { return ContextProfiles.info(id, extensions) || { name: id, icon: "grid" } }
    function known(id) { return !!ContextProfiles.catalog[id] || (id.startsWith("ext:") && extensions.some(e => "ext:" + e.name === id)) }
    /// Rows of an area: its blocks (also the hidden ones, so they keep
    /// their place), for the overview plus extension cards not placed yet.
    function rowsOf(area) {
        const l = lists
        const ids = l[area].filter(known)
        if (area === "overview") {
            const placed = (id) => ["tabs", "overview", "pinned", "hidden"].some(k => l[k].indexOf(id) >= 0)
            for (const e of extensions) if (e.shown && !placed("ext:" + e.name)) ids.push("ext:" + e.name)
        }
        return ids
    }
    readonly property var overviewRows: rowsOf("overview")
    readonly property var pinnedRows: rowsOf("pinned")
    function isHidden(id) { return lists.hidden.indexOf(id) >= 0 }
    /// Where `id` stands: "tabs", "overview", "pinned" or "".
    function areaOf(id) {
        for (const k of ["tabs", "overview", "pinned"]) if (lists[k].indexOf(id) >= 0) return k
        return overviewRows.indexOf(id) >= 0 ? "overview" : ""
    }

    // ---- changes
    function setHidden(id, hidden) { change(l => ContextProfiles.toggle(l.hidden, id, hidden)) }
    /// Puts `id` into `area` at `index` (the shown rows count, so extension
    /// cards not stored yet are stored with their place).
    function place(id, area, index, show) {
        const rows = (area === "overview" ? overviewRows : area === "pinned" ? pinnedRows : lists.tabs).slice()
        const from = rows.indexOf(id)
        if (from >= 0) { rows.splice(from, 1); if (from < index) --index }
        rows.splice(Math.max(0, Math.min(index, rows.length)), 0, id)
        change(l => {
            ContextProfiles.unplace(l, id)
            if (show) ContextProfiles.toggle(l.hidden, id, false)
            for (const x of rows) if (x !== id) ContextProfiles.unplace(l, x)
            l[area] = rows.concat(l[area].filter(x => rows.indexOf(x) < 0))
        })
    }
    function addBlock(id) { place(id, "overview", overviewRows.length, true) }
    function moveTab(id, delta) { change(l => ContextProfiles.move(l, "tabs", id, delta)) }
    function removeTab(id) { change(l => { ContextProfiles.unplace(l, id); l.overview.push(id) }) }
    function reset() {
        app.setContextProfile(profile, "")
        if (profile === "default") {
            // Without a stored „Allgemein“ the old switches would apply
            // again: store the built-in layout.
            ContextProfiles.save(app, JSON.parse(app.contextModules || "{}"), "default", ContextProfiles.builtin)
        }
    }
    /// Takes the suggestion over as the profile (a profile not set yet).
    function adopt() { ContextProfiles.edit(app, config, profile, () => {}) }

    Keys.onEscapePressed: finish()

    // ---- drag and drop of rows
    property string dragId: ""
    property string dropArea: ""
    property int dropIndex: -1
    /// Finds the drop place for a pointer at `y` (in `rowsColumn`).
    function dragTo(y) {
        let best = null
        const consider = (area, repeater, count) => {
            for (let i = 0; i <= count; ++i) {
                const item = i < count ? repeater.itemAt(i) : null
                const prev = i > 0 ? repeater.itemAt(i - 1) : null
                const top = item ? item.mapToItem(rowsColumn, 0, 0).y
                    : prev ? prev.mapToItem(rowsColumn, 0, prev.height).y
                    : (area === "pinned" ? pinnedHeader.mapToItem(rowsColumn, 0, pinnedHeader.height).y : overviewHeader.mapToItem(rowsColumn, 0, overviewHeader.height).y)
                const d = Math.abs(y - top)
                if (!best || d < best.d) best = { area: area, index: i, d: d }
            }
        }
        consider("overview", overviewRepeater, overviewRows.length)
        if ((ContextProfiles.catalog[dragId] || {}).pin) consider("pinned", pinnedRepeater, pinnedRows.length)
        dropArea = best ? best.area : ""
        dropIndex = best ? best.index : -1
    }
    function drop() {
        if (dragId && dropArea) place(dragId, dropArea, dropIndex)
        dragId = ""
        dropArea = ""
        dropIndex = -1
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ---- header: title and profile
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.rightMargin: 8
            Layout.preferredHeight: 42
            spacing: 6
            Icon { name: "settings"; size: 15; color: Theme.accent }
            Text { text: qsTr("Leiste anpassen"); color: Theme.textStrong; font.pixelSize: Theme.fontSize; font.bold: true }
            Item { Layout.fillWidth: true }
            ComboBox {
                id: profileBox
                Layout.preferredWidth: 120
                model: ContextProfiles.names.map(n => ContextProfiles.title(n))
                currentIndex: ContextProfiles.names.indexOf(ed.profile)
                onActivated: (i) => ed.profile = ContextProfiles.names[i]
                ToolTip.visible: hovered
                ToolTip.text: qsTr("Welches Profil bearbeitet wird")
            }
        }
        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }
        Label {
            visible: !ed.profileSet
            Layout.fillWidth: true
            Layout.margins: 10
            wrapMode: Text.Wrap
            color: Theme.textMuted
            font.pixelSize: Theme.smallFontSize
            text: ed.config.by_kind === false
                ? qsTr("Profile nach Dateityp sind ausgeschaltet (Einstellungen): überall gilt „Allgemein“.")
                : qsTr("„%1“ ist noch nicht eingestellt, es gilt „Allgemein“. Unten steht ein Vorschlag; die erste Änderung legt das Profil an.").arg(ContextProfiles.title(ed.profile))
        }
        Button {
            visible: !ed.profileSet
            Layout.leftMargin: 6
            flat: true
            text: qsTr("Vorschlag übernehmen")
            onClicked: ed.adopt()
        }

        ScrollView {
            id: scroll
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true

            ColumnLayout {
                id: rowsColumn
                width: scroll.availableWidth
                spacing: 4

                // ---- tabs as chips
                SectionText { text: qsTr("Reiter"); Layout.topMargin: 8 }
                Flow {
                    Layout.fillWidth: true
                    Layout.leftMargin: 10
                    Layout.rightMargin: 10
                    spacing: 6
                    Chip { label: qsTr("Übersicht"); fixed: true }
                    Repeater {
                        model: ed.lists.tabs.filter(id => ed.known(id))
                        delegate: Chip {
                            required property string modelData
                            required property int index
                            label: ed.info(modelData).name
                            icon: ed.info(modelData).icon
                            canLeft: index > 0
                            canRight: index < ed.lists.tabs.length - 1
                            onMoveLeft: ed.moveTab(modelData, -1)
                            onMoveRight: ed.moveTab(modelData, 1)
                            onRemove: ed.removeTab(modelData)
                        }
                    }
                    Button {
                        flat: true
                        text: qsTr("+ Reiter")
                        onClicked: tabMenu.popup()
                        Menu {
                            id: tabMenu
                            Repeater {
                                model: Object.keys(ContextProfiles.catalog).filter(id => ContextProfiles.catalog[id].tab && ed.lists.tabs.indexOf(id) < 0)
                                MenuItem {
                                    required property string modelData
                                    text: ed.info(modelData).name
                                    onTriggered: ed.place(modelData, "tabs", ed.lists.tabs.length)
                                }
                            }
                        }
                    }
                }

                // ---- overview
                SectionText {
                    id: overviewHeader
                    Layout.topMargin: 10
                    text: qsTr("Übersicht · %n Baustein(e)", "", ed.overviewRows.filter(id => !ed.isHidden(id)).length)
                }
                Repeater {
                    id: overviewRepeater
                    model: ed.overviewRows
                    delegate: BlockRow { area: "overview" }
                }
                DropLine { visible: ed.dropArea === "overview" && ed.dropIndex === ed.overviewRows.length }
                Button {
                    Layout.leftMargin: 6
                    flat: true
                    text: qsTr("+ Baustein hinzufügen")
                    onClicked: gallery.open()
                }

                // ---- pinned
                SectionText {
                    id: pinnedHeader
                    Layout.topMargin: 10
                    text: qsTr("Unten angeheftet · auf jedem Reiter")
                }
                Repeater {
                    id: pinnedRepeater
                    model: ed.pinnedRows
                    delegate: BlockRow { area: "pinned" }
                }
                DropLine { visible: ed.dropArea === "pinned" && ed.dropIndex === ed.pinnedRows.length }
                Label {
                    visible: ed.pinnedRows.length === 0
                    Layout.leftMargin: 14
                    color: Theme.textFaint
                    font.pixelSize: Theme.smallFontSize
                    text: qsTr("Hierher ziehen: Heute, Aufgaben, Lernzeit")
                }
                Item { Layout.preferredHeight: 10 }
            }
        }

        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 8
            Button {
                flat: true
                text: qsTr("Zurücksetzen")
                enabled: ed.profileSet
                ToolTip.visible: hovered
                ToolTip.text: ed.profile === "default" ? qsTr("„Allgemein“ wie ausgeliefert")
                                                       : qsTr("Profil löschen: es gilt wieder „Allgemein“")
                onClicked: ed.reset()
            }
            Item { Layout.fillWidth: true }
            Button { highlighted: true; text: qsTr("Fertig"); onClicked: ed.finish() }
        }
    }

    // ---- gallery of all blocks
    Popup {
        id: gallery
        parent: Overlay.overlay
        anchors.centerIn: parent
        width: Math.min(420, parent ? parent.width - 40 : 420)
        height: Math.min(520, parent ? parent.height - 80 : 520)
        modal: true
        focus: true
        onOpened: { search.text = ""; search.forceActiveFocus() }
        readonly property var entries: {
            const q = search.text.trim().toLowerCase()
            const ids = Object.keys(ContextProfiles.catalog).concat(ed.extensions.map(e => "ext:" + e.name))
            return ids.map(id => Object.assign({ id: id }, ed.info(id)))
                      .filter(e => !q || (e.name + " " + (e.detail || "")).toLowerCase().indexOf(q) >= 0)
        }
        ColumnLayout {
            anchors.fill: parent
            spacing: 6
            TextField { id: search; Layout.fillWidth: true; placeholderText: qsTr("Bausteine suchen …") }
            ScrollView {
                id: galleryScroll
                Layout.fillWidth: true
                Layout.fillHeight: true
                contentWidth: availableWidth
                clip: true
                ColumnLayout {
                    width: galleryScroll.availableWidth
                    spacing: 2
                    Repeater {
                        model: ContextProfiles.groups
                        delegate: ColumnLayout {
                            id: group
                            required property var modelData
                            readonly property var items: gallery.entries.filter(e => e.group === group.modelData[0])
                            visible: items.length > 0
                            Layout.fillWidth: true
                            spacing: 2
                            SectionText { text: group.modelData[1]; Layout.topMargin: 6 }
                            Repeater {
                                model: group.items
                                delegate: RowLayout {
                                    id: entry
                                    required property var modelData
                                    readonly property string where: ed.isHidden(modelData.id) ? "hidden" : ed.areaOf(modelData.id)
                                    Layout.fillWidth: true
                                    Layout.leftMargin: 8
                                    Layout.rightMargin: 8
                                    spacing: 8
                                    Icon { visible: !entry.modelData.letter; name: entry.modelData.icon; size: 16; color: Theme.accent }
                                    Text { visible: !!entry.modelData.letter; text: entry.modelData.letter || ""; color: Theme.accent; font.bold: true }
                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 0
                                        Text { text: entry.modelData.name; color: Theme.textStrong; font.pixelSize: Theme.fontSize }
                                        Text {
                                            Layout.fillWidth: true
                                            text: entry.modelData.detail || ""
                                            color: Theme.textMuted
                                            font.pixelSize: Theme.smallFontSize
                                            wrapMode: Text.Wrap
                                        }
                                    }
                                    Text {
                                        visible: entry.where !== ""
                                        text: ({ tabs: qsTr("als Reiter"), overview: qsTr("in der Übersicht"), pinned: qsTr("unten"),
                                                 hidden: qsTr("ausgeblendet") })[entry.where] || ""
                                        color: Theme.textFaint
                                        font.pixelSize: Theme.smallFontSize
                                    }
                                    Button {
                                        visible: entry.where === "" || entry.where === "hidden"
                                        flat: true
                                        text: entry.where === "hidden" ? qsTr("Zeigen") : qsTr("Hinzufügen")
                                        onClicked: entry.where === "hidden" && ed.areaOf(entry.modelData.id) !== ""
                                                   ? ed.setHidden(entry.modelData.id, false) : ed.addBlock(entry.modelData.id)
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Button { Layout.alignment: Qt.AlignRight; text: qsTr("Schließen"); onClicked: gallery.close() }
        }
    }

    // ---- pieces

    component SectionText: Text {
        Layout.leftMargin: 14
        Layout.rightMargin: 14
        Layout.fillWidth: true
        color: Theme.textMuted
        font.pixelSize: Theme.smallFontSize
        font.weight: Font.DemiBold
        font.capitalization: Font.AllUppercase
        font.letterSpacing: 0.5
        elide: Text.ElideRight
    }
    component DropLine: Rectangle {
        Layout.fillWidth: true
        Layout.leftMargin: 10
        Layout.rightMargin: 10
        Layout.preferredHeight: 2
        color: Theme.accent
    }
    /// A tab as a chip: arrows to move it, × back to the overview.
    component Chip: Rectangle {
        id: chip
        property string label
        property string icon: "grid"
        property bool fixed: false
        property bool dimmed: false
        property bool canLeft: false
        property bool canRight: false
        signal moveLeft()
        signal moveRight()
        signal remove()
        width: chipRow.implicitWidth + 16
        height: 28
        radius: 14
        color: fixed ? Theme.accentSoft : Theme.raised
        border.color: Theme.border
        opacity: dimmed ? 0.5 : 1
        HoverHandler { id: chipHover }
        Row {
            id: chipRow
            anchors.centerIn: parent
            spacing: 2
            IconButton { visible: !chip.fixed && chipHover.hovered && chip.canLeft; iconName: "chevron-left"; iconSize: 11; width: 18; height: 18; tip: qsTr("Nach links"); onClicked: chip.moveLeft() }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: chip.label
                color: chip.fixed ? Theme.accent : Theme.textStrong
                font.pixelSize: Theme.smallFontSize + 1
            }
            IconButton { visible: !chip.fixed && chipHover.hovered && chip.canRight; iconName: "chevron-right"; iconSize: 11; width: 18; height: 18; tip: qsTr("Nach rechts"); onClicked: chip.moveRight() }
            IconButton { visible: !chip.fixed; iconName: "close"; iconSize: 10; width: 18; height: 18; tip: qsTr("Kein Reiter mehr (in die Übersicht)"); onClicked: chip.remove() }
        }
    }
    /// A block as a compact row: grip, icon, name, eye.
    component BlockRow: ColumnLayout {
        id: row
        required property string modelData
        required property int index
        property string area
        readonly property bool hidden: ed.isHidden(modelData)
        Layout.fillWidth: true
        spacing: 0
        DropLine { visible: ed.dropArea === row.area && ed.dropIndex === row.index && ed.dragId !== row.modelData }
        Rectangle {
            Layout.fillWidth: true
            Layout.leftMargin: 10
            Layout.rightMargin: 10
            Layout.preferredHeight: 34
            radius: 8
            color: ed.dragId === row.modelData ? Theme.accentSoft : rowHover.hovered || rowFocus.activeFocus ? Theme.hover : Theme.header
            border.color: rowFocus.activeFocus ? Theme.accent : Theme.border
            opacity: row.hidden ? 0.5 : 1
            HoverHandler { id: rowHover }
            Item {
                id: rowFocus
                anchors.fill: parent
                activeFocusOnTab: true
                Keys.onPressed: (event) => {
                    const list = row.area === "pinned" ? ed.pinnedRows : ed.overviewRows
                    if ((event.modifiers & Qt.AltModifier) && (event.key === Qt.Key_Up || event.key === Qt.Key_Down)) {
                        const to = row.index + (event.key === Qt.Key_Up ? -1 : 2)
                        if (to >= 0 && to <= list.length) ed.place(row.modelData, row.area, to)
                        event.accepted = true
                    } else if (event.key === Qt.Key_Space) { ed.setHidden(row.modelData, !row.hidden); event.accepted = true }
                }
            }
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 4
                anchors.rightMargin: 4
                spacing: 6
                // Grip: drag to move (also between overview and bottom).
                Item {
                    Layout.preferredWidth: 22
                    Layout.fillHeight: true
                    Icon { anchors.centerIn: parent; name: "menu"; size: 13; color: Theme.textMuted }
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                        preventStealing: true
                        onPressed: ed.dragId = row.modelData
                        onPositionChanged: (mouse) => ed.dragTo(mapToItem(rowsColumn, mouse.x, mouse.y).y)
                        onReleased: ed.drop()
                        onCanceled: { ed.dragId = ""; ed.dropArea = "" }
                    }
                }
                Text {
                    visible: !!ed.info(row.modelData).letter
                    text: ed.info(row.modelData).letter || ""
                    color: Theme.accent
                    font.bold: true
                    font.pixelSize: Theme.smallFontSize
                }
                Icon { visible: !ed.info(row.modelData).letter; name: ed.info(row.modelData).icon; size: 14; color: Theme.accent }
                Text {
                    Layout.fillWidth: true
                    text: ed.info(row.modelData).name
                    textFormat: Text.PlainText
                    color: Theme.textStrong
                    font.pixelSize: Theme.fontSize
                    elide: Text.ElideRight
                }
                IconButton {
                    iconName: row.hidden ? "hidden" : "eye"
                    iconSize: 14
                    tip: row.hidden ? qsTr("Zeigen") : qsTr("Ausblenden (behält den Platz)")
                    onClicked: ed.setHidden(row.modelData, !row.hidden)
                }
            }
        }
    }
}
