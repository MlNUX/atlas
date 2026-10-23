// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// ILIAS tab (`ilias:`): the courses of the dashboard as folders – courses,
// groups, sessions and folders, with their files. Only a table of
// contents: a click on a file downloads it (IliasService) and opens it.
Rectangle {
    id: root
    required property AppController app
    property var actions: null
    readonly property var service: actions ? actions.ilias : null
    color: Theme.editor

    /// Keys ("fold123") of the containers from the course down.
    property var trail: []
    property int currentIndex: -1

    function viewState() { return { trail: trail } }
    function applyState(s) { if (s && s.trail) trail = s.trail }

    /// The containers along `trail` (missing ones end it).
    readonly property var chain: {
        const out = []
        let list = service && service.revision >= 0 ? (service.index.courses || []) : []
        for (const key of trail) {
            const node = list.find(n => n.kind + n.id === key)
            if (!node) break
            out.push(node)
            list = node.children || []
        }
        return out
    }
    readonly property var items: {
        const list = chain.length ? (chain[chain.length - 1].children || []) : (service ? service.index.courses || [] : [])
        // Folders first, then files; each by name (natural order).
        const container = (n) => n.kind === "file" ? 1 : 0
        return list.slice().sort((a, b) => container(a) - container(b) || naturalCompare(a.name, b.name))
    }
    /// Names with numbers by value ("Blatt 2" before "Blatt 10"); Qt's
    /// localeCompare has no `numeric` option.
    function naturalCompare(a, b) {
        const split = (s) => s.toLowerCase().match(/\d+|\D+/g) || []
        const x = split(a), y = split(b)
        for (let i = 0; i < Math.min(x.length, y.length); ++i) {
            const p = x[i], q = y[i]
            if (p === q) continue
            const np = /^\d/.test(p), nq = /^\d/.test(q)
            if (np && nq) return (+p - +q) || p.length - q.length
            return p.localeCompare(q)
        }
        return x.length - y.length
    }
    function enter(node) { trail = trail.slice(0, chain.length).concat([node.kind + node.id]); currentIndex = -1 }
    function up(levels) { trail = chain.slice(0, Math.max(0, chain.length - (levels || 1))).map(n => n.kind + n.id); currentIndex = -1 }
    function activate(node) {
        if (!node) return
        if (node.kind === "file") service.open(node, chain.map(n => n.name))
        else enter(node)
    }

    function kindText(kind) {
        return ({ crs: qsTr("Kurs"), grp: qsTr("Gruppe"), sess: qsTr("Sitzung"), fold: qsTr("Ordner"), file: qsTr("Datei") })[kind] || ""
    }
    /// Icon and colour of an entry; files by the type in their name or info.
    function iconOf(node) {
        if (node.kind === "crs") return ["school", "#5aa2ff"]
        if (node.kind === "grp") return ["session", "#c792ea"]
        if (node.kind === "sess") return ["calendar", "#7bd88f"]
        if (node.kind !== "file") return ["folder-fill", "#e0b33a"]
        const t = (node.name + " " + (node.info || "")).toLowerCase()
        if (/\bpdf\b|\.pdf/.test(t)) return ["file-pdf", "#f2575f"]
        if (/\b(png|jpe?g|gif|svg|webp)\b/.test(t)) return ["file-image", "#7bd88f"]
        if (/\b(xlsx?|csv|ods)\b/.test(t)) return ["file-table", "#7bd88f"]
        if (/\b(py|java|c|cpp|rs|js|ts|m|ipynb|zip)\b/.test(t)) return ["file-code", "#c792ea"]
        return ["file-text", "#8a97ad"]
    }
    function count(node) {
        const c = node.children || []
        const files = c.filter(n => n.kind === "file").length
        return c.length - files > 0 ? qsTr("%1 Ordner · %2 Dateien").arg(c.length - files).arg(files) : qsTr("%n Datei(en)", "", files)
    }

    Keys.onPressed: (event) => {
        if (event.key === Qt.Key_Backspace || (event.key === Qt.Key_Up && (event.modifiers & Qt.AltModifier))) { up(); event.accepted = true }
        else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) { activate(items[currentIndex]); event.accepted = true }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ---- location and actions
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 44
            color: Theme.header
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 8
                spacing: 4
                IconButton { iconName: "arrow-up"; tip: qsTr("Eine Ebene höher (Rücktaste)"); enabled: root.chain.length > 0; onClicked: root.up() }
                // Breadcrumbs: ILIAS › course › folder …
                Flickable {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 30
                    contentWidth: crumbs.width
                    clip: true
                    flickableDirection: Flickable.HorizontalFlick
                    Row {
                        id: crumbs
                        height: parent.height
                        spacing: 2
                        Repeater {
                            model: [{ name: "ILIAS", depth: 0 }].concat(root.chain.map((n, i) => ({ name: n.name, depth: i + 1 })))
                            Row {
                                required property var modelData
                                required property int index
                                height: crumbs.height
                                spacing: 2
                                Text {
                                    visible: parent.index > 0
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: "›"
                                    color: Theme.textFaint
                                }
                                Rectangle {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: crumbText.implicitWidth + 14
                                    height: 26
                                    radius: 6
                                    color: crumbHover.hovered ? Theme.hover : "transparent"
                                    Text {
                                        id: crumbText
                                        anchors.centerIn: parent
                                        text: parent.parent.modelData.name
                                        color: parent.parent.index === root.chain.length ? Theme.textStrong : Theme.text
                                        font.pixelSize: Theme.fontSize
                                        font.weight: parent.parent.index === root.chain.length ? Font.DemiBold : Font.Normal
                                    }
                                    HoverHandler { id: crumbHover }
                                    MouseArea { anchors.fill: parent; onClicked: root.up(root.chain.length - parent.parent.modelData.depth) }
                                }
                            }
                        }
                    }
                }
                Text {
                    Layout.maximumWidth: 320
                    text: root.service ? root.service.status : ""
                    color: root.service && root.service.state === "error" ? Theme.error : Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                }
                BusyIndicator { Layout.preferredWidth: 22; Layout.preferredHeight: 22; running: !!root.service && (root.service.busy || !!root.service.current); visible: running }
                IconButton {
                    iconName: "refresh"
                    tip: qsTr("Kurse neu einlesen")
                    enabled: !!root.service && !root.service.busy
                    onClicked: root.service.refresh()
                }
                IconButton {
                    iconName: "file-text"
                    tip: qsTr("ILIAS-Protokoll öffnen (Fehlersuche)")
                    onClicked: root.service.openLog()
                }
                IconButton {
                    iconName: "downloads"
                    tip: root.service && root.service.downloadDir ? qsTr("Download-Ordner: %1 (ändern …)").arg(root.service.downloadDir) : qsTr("Download-Ordner wählen …")
                    onClicked: root.chooseDir()
                }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }

        // ---- the list
        ListView {
            id: list
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            focus: true
            model: root.items
            currentIndex: root.currentIndex
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                id: row
                required property var modelData
                required property int index
                readonly property var icon: root.iconOf(modelData)
                readonly property string fileState: root.service && root.service.revision >= 0 && modelData.kind === "file" ? root.service.fileState(modelData) : ""
                readonly property bool loading: !!root.service && !!root.service.current && root.service.current.item.url === modelData.url
                width: ListView.view.width
                height: 48
                color: index === root.currentIndex ? Theme.selection : rowHover.hovered ? Theme.hover : "transparent"
                HoverHandler { id: rowHover }
                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    onClicked: (mouse) => {
                        root.currentIndex = row.index
                        list.forceActiveFocus()
                        if (mouse.button === Qt.RightButton) { itemMenu.node = row.modelData; itemMenu.popup(); return }
                        root.activate(row.modelData)
                    }
                }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 16
                    anchors.rightMargin: 16
                    spacing: 12
                    Icon { name: row.icon[0]; size: 22; color: row.icon[1] }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 1
                        Text {
                            Layout.fillWidth: true
                            text: row.modelData.name
                            textFormat: Text.PlainText
                            color: Theme.textStrong
                            font.pixelSize: Theme.fontSize
                            elide: Text.ElideRight
                        }
                        Text {
                            Layout.fillWidth: true
                            text: row.modelData.kind === "file" ? (row.modelData.info || qsTr("Datei"))
                                : root.kindText(row.modelData.kind) + (row.modelData.children ? " · " + root.count(row.modelData) : "")
                                  + (row.modelData.error ? qsTr(" · nicht lesbar") : "")
                            textFormat: Text.PlainText
                            color: Theme.textMuted
                            font.pixelSize: Theme.smallFontSize
                            elide: Text.ElideRight
                        }
                    }
                    // Download progress, or what is here already.
                    ProgressBar {
                        visible: row.loading
                        Layout.preferredWidth: 90
                        indeterminate: !root.service.current || !root.service.current.total
                        value: root.service.current && root.service.current.total ? root.service.current.received / root.service.current.total : 0
                    }
                    Rectangle {
                        visible: !row.loading && row.fileState !== ""
                        Layout.preferredHeight: 20
                        Layout.preferredWidth: stateText.implicitWidth + 14
                        radius: 10
                        color: row.fileState === "changed" ? Qt.rgba(0.88, 0.70, 0.23, 0.18) : Qt.rgba(0.18, 0.63, 0.26, 0.16)
                        Text {
                            id: stateText
                            anchors.centerIn: parent
                            text: row.fileState === "changed" ? qsTr("neue Version") : qsTr("geladen")
                            color: row.fileState === "changed" ? Theme.warn : Theme.ok
                            font.pixelSize: 11
                        }
                    }
                    Icon { visible: row.modelData.kind !== "file"; name: "chevron-right"; size: 14; color: Theme.textFaint }
                }
                Rectangle { anchors.bottom: parent.bottom; x: 50; width: parent.width - 66; height: 1; color: Theme.border; opacity: 0.6 }
            }

            // Nothing to show yet: read in, or log in first.
            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(parent.width - 60, 460)
                visible: list.count === 0
                spacing: 12
                Icon { Layout.alignment: Qt.AlignHCenter; name: "school"; size: 40; color: Theme.accent }
                Text {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    text: !root.service ? "" : root.chain.length ? qsTr("Hier ist nichts.")
                        : root.service.state === "login" ? qsTr("Bitte bei ILIAS anmelden – danach liest Atlas deine Kurse ein.")
                        : root.service.busy ? qsTr("Kurse werden eingelesen …")
                        : qsTr("Deine Kurse vom ILIAS-Dashboard erscheinen hier als Ordner, mit allen Dateien. Ein Klick auf eine Datei lädt sie herunter und öffnet sie.")
                    color: Theme.text
                    font.pixelSize: Theme.fontSize
                }
                Button {
                    Layout.alignment: Qt.AlignHCenter
                    visible: !!root.service && !root.chain.length && !root.service.busy
                    highlighted: true
                    text: root.service && root.service.state === "login" ? qsTr("Anmelden …") : qsTr("Kurse einlesen")
                    onClicked: root.service.state === "login" ? root.service.login() : root.service.refresh()
                }
            }
        }

        // ---- status bar
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 26
            color: Theme.header
            Text {
                anchors.verticalCenter: parent.verticalCenter
                x: 12
                width: parent.width - 24
                elide: Text.ElideRight
                text: !root.service ? "" : (root.service.index.scanned
                    ? qsTr("Eingelesen %1").arg(new Date(root.service.index.scanned).toLocaleString(Qt.locale(), Locale.ShortFormat)) : qsTr("Noch nicht eingelesen"))
                    + (root.service.downloadDir ? qsTr(" · Downloads: %1").arg(root.service.downloadDir) : "")
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
            }
        }
    }

    Menu {
        id: itemMenu
        property var node: null
        MenuItem {
            text: itemMenu.node && itemMenu.node.kind === "file" ? qsTr("Öffnen") : qsTr("Öffnen")
            onTriggered: root.activate(itemMenu.node)
        }
        MenuItem {
            text: qsTr("Neu herunterladen")
            visible: !!itemMenu.node && itemMenu.node.kind === "file"
            height: visible ? implicitHeight : 0
            onTriggered: root.service.open(itemMenu.node, root.chain.map(n => n.name), true)
        }
        MenuItem {
            text: qsTr("Im Browser öffnen")
            onTriggered: Qt.openUrlExternally(itemMenu.node.url)
        }
        MenuItem {
            readonly property var e: itemMenu.node && root.service ? root.service.entry(itemMenu.node) : null
            text: qsTr("Im Ordner zeigen")
            visible: !!e
            height: visible ? implicitHeight : 0
            onTriggered: if (root.actions) root.actions.openRequested(e.path.slice(0, e.path.lastIndexOf("/")), false)
        }
    }

    // The download folder: asked before the first download.
    function chooseDir() { dirChooser.ask("pick", service && service.downloadDir ? service.downloadDir : "", "") }
    Connections {
        target: root.service
        function onDownloadDirNeeded() { if (root.visible) root.chooseDir() }
    }
    FolderChooser {
        id: dirChooser
        app: root.app
        pickTitle: qsTr("Download-Ordner für ILIAS")
        pickHint: qsTr("Darin legt Atlas pro Kurs einen Ordner an, mit derselben Struktur wie in ILIAS.")
        pickButton: qsTr("Hier speichern")
        onChosen: (path) => {
            root.app.setIliasConfig("download_dir", path)
            root.service.resume()
        }
    }
}
