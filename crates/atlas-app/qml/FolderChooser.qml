// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Chooses a project folder: places on the left, a clickable path bar and
// the subfolders of the shown folder. For a new session (one per project
// folder) it also offers a name; without one the folder name is used.
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
    function accept() {
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
    }

    contentItem: ColumnLayout {
        spacing: 0

        // ---- header
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 18
            Layout.bottomMargin: 12
            spacing: 12
            Rectangle {
                Layout.preferredWidth: 36
                Layout.preferredHeight: 36
                radius: 10
                color: Theme.accentSoft
                Icon { anchors.centerIn: parent; name: root.mode === "session" ? "session" : "folder-open"; size: 20; color: Theme.accent }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Text {
                    Layout.fillWidth: true
                    text: root.mode === "session" ? qsTr("Neue Session") : qsTr("Projektordner öffnen")
                    color: Theme.textStrong
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                }
                Text {
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                    text: root.mode === "session" ? qsTr("Jeder Projektordner hat genau eine Session; gibt es sie schon, wird sie geöffnet.")
                        : qsTr("Atlas legt seine Daten im Unterordner .atlas des Projekts ab.")
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                }
            }
            IconButton { iconName: "close"; tip: qsTr("Abbrechen (Esc)"); onClicked: root.close() }
        }
        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Theme.border }

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
                        id: hiddenButton
                        iconName: "hidden"
                        tip: qsTr("Versteckte Ordner anzeigen")
                        checkable: true
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
                        visible: list.count === 0 && !folder.loading
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
                text: root.mode === "session" ? qsTr("Session öffnen") : qsTr("Ordner öffnen")
                highlighted: true
                enabled: !!root.chosenPath && !folder.error
                onClicked: root.accept()
            }
        }
    }
}
