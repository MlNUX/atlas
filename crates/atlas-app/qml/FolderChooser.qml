// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Chooses a project folder: places on the left, a clickable path bar and
// the subfolders of the shown folder; "Neuer Ordner" creates one in the
// shown folder and selects it. For a new session (one per project folder)
// it also offers a name; without one the folder name is used.
Dialog {
    id: root
    required property AppController app
    /// "open" (open a project) or "session" (new session in a project).
    property string mode: "open"
    /// The folder shown; the chosen folder unless a row is selected.
    property string folderPath
    property string selectedPath
    readonly property string chosenPath: selectedPath || folderPath
    property alias sessionName: nameField.text
    readonly property alias folders: folder
    readonly property alias pathBar: pathEdit

    /// Texts for the mode "pick" (choose any folder, e.g. for downloads).
    property string pickTitle: qsTr("Ordner wählen")
    property string pickHint: ""
    property string pickButton: qsTr("Diesen Ordner wählen")

    signal chosen(string path, string sessionName)

    function ask(newMode, startPath, name) {
        mode = newMode
        nameField.text = name || ""
        navigate(startPath || app.projectRoot || app.placePaths[0] || "/")
        pathEdit.editing = false
        open()
        if (mode === "session") { nameField.forceActiveFocus(); nameField.selectAll() }
        else list.forceActiveFocus()
    }
    function navigate(path) {
        if (!path) return
        creating = false
        folderPath = path.length > 1 && path.endsWith("/") ? path.slice(0, -1) : path
        selectedPath = ""
        list.currentIndex = -1
    }
    function parentOf(path) {
        const i = path.lastIndexOf("/")
        return i > 0 ? path.slice(0, i) : "/"
    }
    function nameOf(path) { return path === "/" ? "/" : path.slice(path.lastIndexOf("/") + 1) }
    function segments(path) { return pathEdit.segments(path) }
    /// A new folder needs a name (the input row above the list).
    property bool creating: false
    /// A folder created here, selected once the list shows it.
    property string pendingSelect: ""
    function startCreate() {
        creating = true
        newFolderName.text = ""
        newFolderError.text = ""
        newFolderName.forceActiveFocus()
    }
    function cancelCreate() {
        creating = false
        list.forceActiveFocus()
    }
    /// Creates folder `name` in the shown folder and selects it; its path,
    /// or "" (the reason is shown under the name).
    function createFolder(name) {
        name = (name || "").trim()
        const problem = name ? ops.checkName(name) : qsTr("Bitte einen Namen eingeben")
        if (problem) { newFolderError.text = problem; return "" }
        const created = ops.newFolder(root.folderPath, name)
        if (!created) return ""
        creating = false
        pendingSelect = created
        selectedPath = created
        folder.reload()
        list.forceActiveFocus()
        return created
    }
    function accept() {
        if (creating) { createFolder(newFolderName.text); return }
        if (!chosenPath || folder.error) return
        chosen(chosenPath, nameField.text.trim())
        close()
    }

    anchors.centerIn: Overlay.overlay
    width: Math.min(parent ? parent.width - 60 : 860, 860)
    height: Math.min(parent ? parent.height - 60 : 560, 560)
    modal: true
    padding: 0
    topPadding: 0
    title: ""

    FolderModel {
        id: folder
        path: root.visible ? root.folderPath : ""
        foldersOnly: true
        showHidden: hiddenButton.checked
        // The folder just created: select it as soon as it is listed.
        onCountChanged: {
            if (!root.pendingSelect) return
            const i = folder.rowOf(root.pendingSelect)
            if (i < 0) return
            list.currentIndex = i
            list.positionViewAtIndex(i, ListView.Contain)
            root.pendingSelect = ""
        }
    }
    FileOps {
        id: ops
        onFailed: (message) => { newFolderError.text = message }
    }

    contentItem: ColumnLayout {
        spacing: 0

        // ---- header
        DialogHeader {
            icon: root.mode === "session" ? "session" : root.mode === "pick" ? "downloads" : "folder-open"
            title: root.mode === "session" ? qsTr("Neue Session") : root.mode === "pick" ? root.pickTitle : qsTr("Projektordner öffnen")
            subtitle: root.mode === "session" ? qsTr("Jeder Projektordner hat genau eine Session; gibt es sie schon, wird sie geöffnet.")
                : root.mode === "pick" ? root.pickHint
                : qsTr("Atlas legt seine Daten im Unterordner .atlas des Projekts ab.")
            bottomMargin: 12
            IconButton { iconName: "close"; tip: qsTr("Abbrechen (Esc)"); onClicked: root.close() }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            // ---- places
            Rectangle {
                Layout.preferredWidth: 190
                Layout.fillHeight: true
                color: Theme.sidebar
                ColumnLayout {
                    anchors.fill: parent
                    anchors.topMargin: 8
                    spacing: 0
                    Repeater {
                        model: (root.app.projectRoot ? [{ name: root.app.projectName || root.nameOf(root.app.projectRoot), path: root.app.projectRoot, icon: "logo", project: true }] : [])
                            .concat(root.app.placeNames.map((n, i) => ({ name: n, path: root.app.placePaths[i], icon: root.app.placeIcons[i] })))
                            .filter(p => p.icon !== "trash")
                        delegate: Item {
                            id: place
                            required property var modelData
                            readonly property bool current: root.folderPath === modelData.path
                            Layout.fillWidth: true
                            Layout.preferredHeight: 32
                            Rectangle {
                                anchors.fill: parent
                                anchors.leftMargin: 8
                                anchors.rightMargin: 8
                                anchors.topMargin: 1
                                anchors.bottomMargin: 1
                                radius: 7
                                color: place.current ? Theme.activeSelection : placeHover.hovered ? Theme.hover : "transparent"
                            }
                            HoverHandler { id: placeHover }
                            Row {
                                anchors.verticalCenter: parent.verticalCenter
                                x: 18
                                spacing: 10
                                Icon {
                                    anchors.verticalCenter: parent.verticalCenter
                                    name: place.modelData.icon || "folder"
                                    size: 17
                                    color: place.current || place.modelData.project ? Theme.accent : Theme.textMuted
                                }
                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 130
                                    text: place.modelData.name
                                    color: place.current ? Theme.textStrong : Theme.text
                                    font.pixelSize: Theme.fontSize
                                    elide: Text.ElideRight
                                }
                            }
                            MouseArea { anchors.fill: parent; onClicked: root.navigate(place.modelData.path) }
                        }
                    }
                    Item { Layout.fillHeight: true }
                }
                Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: Theme.border }
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 0

                // ---- path bar
                RowLayout {
                    Layout.fillWidth: true
                    Layout.margins: 10
                    spacing: 6
                    IconButton {
                        iconName: "arrow-left"
                        tip: qsTr("Übergeordneter Ordner (Rücktaste)")
                        enabled: root.folderPath !== "/"
                        onClicked: root.navigate(root.parentOf(root.folderPath))
                    }
                    PathBar {
                        id: pathEdit
                        Layout.fillWidth: true
                        app: root.app
                        path: root.folderPath
                        onNavigateRequested: (path) => root.navigate(path)
                    }
                    IconButton {
                        objectName: "newFolderButton"
                        iconName: "folder-plus"
                        tip: qsTr("Neuer Ordner hier (Strg+Umschalt+N)")
                        enabled: !folder.error
                        onClicked: root.startCreate()
                    }
                    IconButton {
                        id: hiddenButton
                        iconName: "hidden"
                        tip: qsTr("Versteckte Ordner anzeigen")
                        checkable: true
                    }
                }

                // ---- a new folder in the shown one
                Rectangle {
                    visible: root.creating
                    Layout.fillWidth: true
                    Layout.leftMargin: 10
                    Layout.rightMargin: 10
                    Layout.bottomMargin: 6
                    implicitHeight: newFolderColumn.implicitHeight + 16
                    radius: 7
                    color: Theme.raised
                    border.color: Theme.accentAlpha(0.5)
                    ColumnLayout {
                        id: newFolderColumn
                        anchors.fill: parent
                        anchors.margins: 8
                        spacing: 4
                        RowLayout {
                            spacing: 8
                            Icon { name: "folder-plus"; size: 18; color: Theme.accent }
                            TextField {
                                id: newFolderName
                                objectName: "newFolderName"
                                Layout.fillWidth: true
                                placeholderText: qsTr("Name des neuen Ordners in „%1“").arg(root.nameOf(root.folderPath))
                                onTextChanged: newFolderError.text = text.trim() ? ops.checkName(text.trim()) : ""
                                onAccepted: root.createFolder(text)
                                Keys.onEscapePressed: (event) => { root.cancelCreate(); event.accepted = true }
                            }
                            Button {
                                text: qsTr("Anlegen")
                                highlighted: true
                                enabled: newFolderName.text.trim().length > 0 && !newFolderError.text
                                onClicked: root.createFolder(newFolderName.text)
                            }
                            Button { text: qsTr("Abbrechen"); onClicked: root.cancelCreate() }
                        }
                        Text {
                            id: newFolderError
                            visible: text.length > 0
                            Layout.fillWidth: true
                            color: Theme.error
                            font.pixelSize: Theme.smallFontSize
                            wrapMode: Text.Wrap
                        }
                    }
                }

                // ---- subfolders
                ListView {
                    id: list
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.leftMargin: 10
                    Layout.rightMargin: 10
                    clip: true
                    model: folder
                    currentIndex: -1
                    keyNavigationEnabled: true
                    highlightMoveDuration: 0
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar {}
                    onCurrentIndexChanged: root.selectedPath = currentIndex >= 0 ? folder.pathAt(currentIndex) : ""
                    Keys.onReturnPressed: root.accept()
                    Keys.onEnterPressed: root.accept()
                    Keys.onPressed: (event) => {
                        if (event.key === Qt.Key_Backspace || (event.key === Qt.Key_Up && (event.modifiers & Qt.AltModifier))) {
                            root.navigate(root.parentOf(root.folderPath))
                        } else if (event.key === Qt.Key_Right && currentIndex >= 0) {
                            root.navigate(folder.pathAt(currentIndex))
                        } else if (event.key === Qt.Key_L && (event.modifiers & Qt.ControlModifier)) {
                            pathEdit.startEditing()
                        } else if (event.key === Qt.Key_N && (event.modifiers & Qt.ControlModifier) && (event.modifiers & Qt.ShiftModifier)) {
                            root.startCreate()
                        } else return
                        event.accepted = true
                    }

                    delegate: Rectangle {
                        id: row
                        required property int index
                        required property string name
                        required property string path
                        required property bool isHidden
                        required property real modified
                        readonly property bool isCurrent: ListView.isCurrentItem
                        width: ListView.view.width
                        height: 34
                        radius: 7
                        color: isCurrent ? Theme.activeSelection : rowHover.hovered ? Theme.hover : "transparent"
                        HoverHandler { id: rowHover }
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 10
                            anchors.rightMargin: 10
                            spacing: 10
                            Icon {
                                name: row.isCurrent ? "folder-open" : "folder-fill"
                                size: 18
                                color: row.isHidden ? Theme.textFaint : Theme.accent
                            }
                            Text {
                                Layout.fillWidth: true
                                text: row.name
                                color: row.isHidden ? Theme.textMuted : Theme.textStrong
                                font.pixelSize: Theme.fontSize
                                elide: Text.ElideRight
                            }
                            Text {
                                text: Qt.formatDateTime(new Date(row.modified), "dd.MM.yyyy")
                                color: Theme.textFaint
                                font.pixelSize: Theme.smallFontSize
                            }
                            Icon { name: "chevron-right"; size: 14; color: Theme.textFaint }
                        }
                        MouseArea {
                            anchors.fill: parent
                            onClicked: { list.forceActiveFocus(); list.currentIndex = row.index }
                            onDoubleClicked: root.navigate(row.path)
                        }
                    }

                    Text {
                        anchors.centerIn: parent
                        visible: list.count === 0 && !folder.loading && !root.creating
                        text: folder.error ? folder.error : qsTr("Keine Unterordner")
                        color: folder.error ? Theme.error : Theme.textFaint
                        font.pixelSize: Theme.fontSize
                    }
                }
            }
        }

        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }

        // ---- footer
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 14
            spacing: 10
            Label {
                visible: root.mode === "session"
                text: qsTr("Name")
                color: Theme.textMuted
            }
            TextField {
                id: nameField
                visible: root.mode === "session"
                Layout.preferredWidth: 200
                placeholderText: root.nameOf(root.chosenPath)
                onAccepted: root.accept()
            }
            Icon { name: "folder"; size: 16; color: Theme.textMuted }
            Text {
                Layout.fillWidth: true
                text: root.chosenPath
                color: Theme.text
                font.pixelSize: Theme.smallFontSize
                elide: Text.ElideMiddle
            }
            Button { text: qsTr("Abbrechen"); onClicked: root.close() }
            Button {
                text: root.mode === "session" ? qsTr("Session öffnen") : root.mode === "pick" ? root.pickButton : qsTr("Ordner öffnen")
                highlighted: true
                enabled: !!root.chosenPath && !folder.error
                onClicked: root.accept()
            }
        }
    }
}
