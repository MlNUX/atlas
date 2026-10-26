// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// Contents of one folder, laid out like Dolphin: a location bar, icon or
// details view, and a status bar with the folder summary, zoom and free
// space.
Rectangle {
    id: root
    required property string source
    property alias showHidden: folder.showHidden
    required property var actions
    /// AppController (optional): places in the location bar, free space.
    property var app: null
    /// "icons" or "details".
    property string viewMode: "icons"
    /// Icon size of the icons view (the zoom slider).
    property int iconSize: 64
    readonly property var activeView: viewMode === "icons" ? grid : list
    readonly property alias folderModel: folder

    /// A row was opened: folders navigate, files open a viewer.
    signal activated(string path, bool isDir, bool newPane)

    color: Theme.editor

    function viewState() {
        return { sortKey: folder.sortKey, sortAscending: folder.sortAscending, showHidden: folder.showHidden,
                 viewMode: root.viewMode, iconSize: root.iconSize }
    }
    function applyState(state) {
        if (state.sortKey) folder.sortKey = state.sortKey
        if (state.sortAscending !== undefined) folder.sortAscending = state.sortAscending
        if (state.showHidden !== undefined) folder.showHidden = state.showHidden
        if (state.viewMode) root.viewMode = state.viewMode
        if (state.iconSize) root.iconSize = state.iconSize
    }

    FolderModel {
        id: folder
        path: root.source
        // New views start with the order chosen last (a tab's own state
        // from the session wins, see applyState).
        Component.onCompleted: {
            if (!root.app) return
            try {
                const last = JSON.parse(root.app.folderSort)
                sortKey = last.key || "name"
                sortAscending = last.ascending !== false
            } catch (e) {}
        }
    }
    // Natural sorting switched in the settings: list again.
    Connections {
        target: root.app
        ignoreUnknownSignals: true
        function onNaturalSortChanged() { folder.reload() }
    }

    function parentOf(path) {
        const i = path.lastIndexOf("/")
        return i > 0 ? path.slice(0, i) : "/"
    }

    function sortBy(key) {
        if (folder.sortKey === key) folder.sortAscending = !folder.sortAscending
        else { folder.sortKey = key; folder.sortAscending = true }
        if (root.app) root.app.rememberFolderSort(folder.sortKey, folder.sortAscending)
    }

    // Selection is keyed by path, so it survives external changes that
    // shift rows.
    property var selected: ({})
    property string anchorPath
    property bool mouseSelecting: false
    onSourceChanged: { selected = ({}); anchorPath = "" }
    // The other view gets the model anew: keep the selection meanwhile.
    property bool switching: false
    onViewModeChanged: {
        switching = true
        Qt.callLater(() => {
            root.activeView.currentIndex = folder.rowOf(root.anchorPath)
            root.activeView.forceActiveFocus()
            root.switching = false
        })
    }

    /// Keyboard focus to the list/grid (when its tab is shown).
    function focusContent() { root.activeView.forceActiveFocus() }
    function selectOnly(path) { selected = ({ [path]: true }); anchorPath = path }
    function toggle(path) {
        const s = Object.assign({}, selected)
        if (s[path]) delete s[path]; else s[path] = true
        selected = s
        anchorPath = path
    }
    function selectRange(path) {
        const a = folder.rowOf(anchorPath), b = folder.rowOf(path)
        if (a < 0) { selectOnly(path); return }
        const s = {}
        for (let i = Math.min(a, b); i <= Math.max(a, b); ++i) s[folder.pathAt(i)] = true
        selected = s
    }
    function selectAll() {
        const s = {}
        for (let i = 0; i < folder.count; ++i) s[folder.pathAt(i)] = true
        selected = s
    }
    /// Selected paths in list order (existing ones only).
    function selectedPaths() {
        const out = []
        for (let i = 0; i < folder.count; ++i) {
            const p = folder.pathAt(i)
            if (selected[p]) out.push(p)
        }
        return out
    }

    function openRow(row, newPane) {
        if (row < 0 || row >= folder.count) return
        root.activated(folder.pathAt(row), folder.isDirAt(row), newPane)
    }

    /// Click on an entry of either view.
    function itemClicked(index, path, mouse) {
        root.activeView.forceActiveFocus()
        if (mouse.button === Qt.MiddleButton) {
            root.openRow(index, true)
            return
        }
        root.mouseSelecting = true
        root.activeView.currentIndex = index
        root.mouseSelecting = false
        if (mouse.modifiers & Qt.ControlModifier) root.toggle(path)
        else if (mouse.modifiers & Qt.ShiftModifier) root.selectRange(path)
        // Right click on a selected entry keeps the selection.
        else if (!(mouse.button === Qt.RightButton && root.selected[path])) root.selectOnly(path)
        if (mouse.button === Qt.RightButton) root.actions.showMenu(root.selectedPaths(), root.source)
    }
    /// Click on the empty area of either view.
    function backgroundClicked(mouse) {
        root.activeView.forceActiveFocus()
        root.selected = ({})
        if (mouse.button === Qt.RightButton) root.actions.showMenu([], root.source)
    }

    function handleKey(event) {
        const ctrl = event.modifiers & Qt.ControlModifier
        const current = root.activeView.currentIndex
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            if (event.modifiers & Qt.AltModifier) {
                if (root.selectedPaths().length === 1) root.actions.showProperties(root.selectedPaths()[0])
            } else root.openRow(current, ctrl)
        } else if ((event.key === Qt.Key_Up && (event.modifiers & Qt.AltModifier)) || event.key === Qt.Key_Backspace) {
            root.activated(root.parentOf(root.source), true, false)
        } else if (event.key === Qt.Key_Space && current >= 0) {
            root.actions.quickLook(folder.pathAt(current), (step) => {
                const next = root.activeView.currentIndex + step
                if (next < 0 || next >= folder.count) return ""
                root.activeView.currentIndex = next
                root.activeView.positionViewAtIndex(next, ListView.Contain)
                return folder.pathAt(next)
            })
        } else if (event.key === Qt.Key_F2 && root.selectedPaths().length === 1) {
            root.actions.rename(root.selectedPaths()[0])
        } else if (event.key === Qt.Key_Delete) {
            root.actions.trash(root.selectedPaths())
        } else if (ctrl && event.key === Qt.Key_C) {
            root.actions.copy(root.selectedPaths())
        } else if (ctrl && event.key === Qt.Key_X) {
            root.actions.cut(root.selectedPaths())
        } else if (ctrl && event.key === Qt.Key_V) {
            root.actions.paste(root.source)
        } else if (ctrl && event.key === Qt.Key_A) {
            root.selectAll()
        } else if (ctrl && event.key === Qt.Key_L) {
            pathBar.startEditing()
        } else if (ctrl && (event.key === Qt.Key_Plus || event.key === Qt.Key_Equal)) {
            root.iconSize = Math.min(zoom.to, root.iconSize + 16)
        } else if (ctrl && event.key === Qt.Key_Minus) {
            root.iconSize = Math.max(zoom.from, root.iconSize - 16)
        } else {
            return
        }
        event.accepted = true
    }

    // Narrow splits drop the type and date columns before the name gets cramped.
    readonly property int sizeWidth: width < 300 ? 0 : 90
    readonly property int dateWidth: width < 420 ? 0 : 140
    readonly property int kindWidth: width < 600 ? 0 : 150

    /// Icon of an entry: images show themselves (like Dolphin's previews).
    /// Only formats Qt can read here get a preview (e.g. WebP needs the
    /// qtimageformats plugin); others keep their icon.
    function isPdf(name) { return /\.pdf$/i.test(name) }
    function isImage(name) {
        const dot = name.lastIndexOf(".")
        return dot > 0 && !!root.app && root.app.imageFormats.indexOf(name.slice(dot + 1).toLowerCase()) >= 0
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ---- location bar
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 44
            color: Theme.header
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 8
                spacing: 4
                IconButton {
                    iconName: "arrow-up"
                    tip: qsTr("Übergeordneter Ordner (Alt+↑)")
                    enabled: root.source !== "/"
                    onClicked: root.activated(root.parentOf(root.source), true, false)
                }
                PathBar {
                    id: pathBar
                    Layout.fillWidth: true
                    app: root.app
                    path: root.source
                    onNavigateRequested: (path) => root.activated(path, true, false)
                }
                Item { Layout.preferredWidth: 4 }
                Rectangle {
                    Layout.preferredHeight: 30
                    Layout.preferredWidth: modes.implicitWidth + 4
                    radius: 8
                    color: Theme.editor
                    border.color: Theme.border
                    Row {
                        id: modes
                        anchors.centerIn: parent
                        spacing: 2
                        Repeater {
                            model: [{ mode: "icons", icon: "grid", tip: qsTr("Symbole") },
                                    { mode: "details", icon: "list", tip: qsTr("Details") }]
                            IconButton {
                                required property var modelData
                                implicitWidth: 30
                                implicitHeight: 26
                                iconName: modelData.icon
                                tip: modelData.tip
                                checkable: true
                                checked: root.viewMode === modelData.mode
                                onClicked: root.viewMode = modelData.mode
                            }
                        }
                    }
                }
                IconButton {
                    iconName: "hidden"
                    tip: qsTr("Versteckte Dateien anzeigen")
                    checkable: true
                    checked: folder.showHidden
                    onToggled: folder.showHidden = checked
                }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
        }

        // ---- details: column headers
        Rectangle {
            visible: root.viewMode === "details"
            Layout.fillWidth: true
            Layout.preferredHeight: 28
            color: Theme.editor
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.border }
            Row {
                anchors.fill: parent
                anchors.leftMargin: 14
                Repeater {
                    model: [
                        { key: "name", label: qsTr("Name"), width: -1 },
                        { key: "size", label: qsTr("Größe"), width: root.sizeWidth },
                        { key: "modified", label: qsTr("Geändert"), width: root.dateWidth },
                        { key: "kind", label: qsTr("Typ"), width: root.kindWidth }
                    ]
                    delegate: Item {
                        id: column
                        required property var modelData
                        width: modelData.key === "name" ? parent.width - root.sizeWidth - root.dateWidth - root.kindWidth - 14
                            : modelData.width
                        visible: width > 0
                        height: parent.height
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            x: column.modelData.key === "name" ? 30 : 0
                            text: column.modelData.label
                                + (folder.sortKey === column.modelData.key ? (folder.sortAscending ? "  ▴" : "  ▾") : "")
                            color: folder.sortKey === column.modelData.key ? Theme.textStrong : Theme.textMuted
                            font.pixelSize: Theme.smallFontSize
                        }
                        MouseArea { anchors.fill: parent; onClicked: root.sortBy(column.modelData.key) }
                    }
                }
            }
        }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            // ---- details view
            ListView {
                id: list
                anchors.fill: parent
                currentIndex: -1
                anchors.margins: 4
                visible: root.viewMode === "details"
                clip: true
                model: root.viewMode === "details" ? folder : null
                focus: root.viewMode === "details"
                reuseItems: true
                keyNavigationEnabled: true
                boundsBehavior: Flickable.StopAtBounds
                highlightMoveDuration: 0
                ScrollBar.vertical: ScrollBar {}
                Keys.onPressed: (event) => root.handleKey(event)
                // Keyboard navigation moves the selection like a click would.
                onCurrentIndexChanged: if (currentIndex >= 0 && !root.mouseSelecting && !root.switching) root.selectOnly(folder.pathAt(currentIndex))

                // The empty area below the rows: clears the selection, right
                // click offers new file/folder and paste.
                footer: MouseArea {
                    width: list.width
                    height: Math.max(Theme.rowHeight, list.height - list.count * 30)
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    onClicked: (mouse) => root.backgroundClicked(mouse)
                    DropArea {
                        anchors.fill: parent
                        keys: ["text/uri-list"]
                        onDropped: (drop) => root.actions.dropInto(root.source, drop)
                    }
                }

                delegate: Item {
                    id: row
                    required property int index
                    required property string name
                    required property bool isDir
                    required property bool isSymlink
                    required property bool isHidden
                    required property string sizeText
                    required property real modified
                    required property string kind
                    required property string path
                    readonly property var icon: Theme.fileIcon(name, isDir)
                    width: ListView.view.width
                    height: 30

                    Rectangle {
                        anchors.fill: parent
                        anchors.topMargin: 1
                        anchors.bottomMargin: 1
                        radius: 6
                        color: root.selected[row.path] ? (list.activeFocus ? Theme.activeSelection : Theme.selection)
                             : rowHover.hovered ? Theme.hover : "transparent"
                        border.color: root.selected[row.path] && list.activeFocus ? Qt.rgba(0.24, 0.55, 0.99, 0.45) : "transparent"
                    }
                    HoverHandler { id: rowHover }

                    Row {
                        anchors.fill: parent
                        anchors.leftMargin: 10
                        Item {
                            width: parent.width - root.sizeWidth - root.dateWidth - root.kindWidth - 10
                            height: parent.height
                            Icon {
                                id: rowIcon
                                anchors.verticalCenter: parent.verticalCenter
                                name: row.icon[0]
                                size: 18
                                color: row.isHidden ? Theme.textFaint : row.icon[1]
                            }
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                x: 30
                                width: parent.width - 34
                                text: row.name + (row.isSymlink ? "  ↪" : "")
                                color: row.isHidden ? Theme.textMuted : Theme.textStrong
                                font.pixelSize: Theme.fontSize
                                elide: Text.ElideRight
                            }
                        }
                        Text {
                            width: root.sizeWidth
                            visible: width > 0
                            anchors.verticalCenter: parent.verticalCenter
                            text: row.isDir ? "" : row.sizeText
                            color: Theme.textMuted
                            font.pixelSize: Theme.smallFontSize
                        }
                        Text {
                            width: root.dateWidth
                            visible: width > 0
                            anchors.verticalCenter: parent.verticalCenter
                            text: Qt.formatDateTime(new Date(row.modified), "dd.MM.yyyy  HH:mm")
                            color: Theme.textMuted
                            font.pixelSize: Theme.smallFontSize
                        }
                        Text {
                            width: root.kindWidth
                            visible: width > 0
                            anchors.verticalCenter: parent.verticalCenter
                            text: row.kind
                            color: Theme.textMuted
                            font.pixelSize: Theme.smallFontSize
                            elide: Text.ElideRight
                        }
                    }

                    // Drag source (all selected entries if this one is selected).
                    Item { id: dragProxy }
                    Drag.active: rowMouse.drag.active
                    Drag.dragType: Drag.Automatic
                    Drag.supportedActions: Qt.CopyAction | Qt.MoveAction
                    Drag.proposedAction: Qt.MoveAction
                    Drag.mimeData: ({ "text/uri-list": root.actions.uriList(root.selected[row.path] ? root.selectedPaths() : [row.path]) })

                    // Drop target: into this folder, or the shown folder.
                    DropArea {
                        anchors.fill: parent
                        keys: ["text/uri-list"]
                        onDropped: (drop) => root.actions.dropInto(row.isDir ? row.path : root.source, drop)
                        Rectangle {
                            anchors.fill: parent
                            visible: parent.containsDrag && row.isDir
                            radius: 6
                            color: "transparent"
                            border.color: Theme.accent
                        }
                    }

                    MouseArea {
                        id: rowMouse
                        drag.target: dragProxy
                        drag.threshold: 8
                        anchors.fill: parent
                        acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
                        onClicked: (mouse) => root.itemClicked(row.index, row.path, mouse)
                        onDoubleClicked: (mouse) => {
                            if (mouse.button === Qt.LeftButton) root.openRow(row.index, mouse.modifiers & Qt.ControlModifier)
                        }
                    }
                }
            }
            TouchpadFling {
                anchors.fill: list
                visible: list.visible
                flickable: list
                active: !root.app || root.app.touchpadFling
                app: root.app
            }

            // ---- icons view
            GridView {
                id: grid
                anchors.fill: parent
                currentIndex: -1
                anchors.margins: 8
                visible: root.viewMode === "icons"
                clip: true
                model: root.viewMode === "icons" ? folder : null
                focus: root.viewMode === "icons"
                keyNavigationEnabled: true
                boundsBehavior: Flickable.StopAtBounds
                highlightMoveDuration: 0
                // Cells stretch so the columns fill the width.
                readonly property int columns: Math.max(1, Math.floor(width / (root.iconSize + 56)))
                cellWidth: Math.floor(width / columns)
                cellHeight: root.iconSize + 58
                ScrollBar.vertical: ScrollBar {}
                Keys.onPressed: (event) => root.handleKey(event)
                onCurrentIndexChanged: if (currentIndex >= 0 && !root.mouseSelecting && !root.switching) root.selectOnly(folder.pathAt(currentIndex))

                footer: MouseArea {
                    width: grid.width
                    height: Math.max(grid.cellHeight, grid.height - Math.ceil(grid.count / grid.columns) * grid.cellHeight)
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    onClicked: (mouse) => root.backgroundClicked(mouse)
                    DropArea {
                        anchors.fill: parent
                        keys: ["text/uri-list"]
                        onDropped: (drop) => root.actions.dropInto(root.source, drop)
                    }
                }

                delegate: Item {
                    id: cell
                    /// The PDF preview is drawn (for the smoke test).
                    readonly property bool pdfPreview: !!pdfThumb.item && pdfThumb.item.ready
                    required property int index
                    required property string name
                    required property bool isDir
                    required property bool isSymlink
                    required property bool isHidden
                    required property string path
                    readonly property var icon: Theme.fileIcon(name, isDir)
                    readonly property bool isSelected: !!root.selected[path]
                    width: grid.cellWidth
                    height: grid.cellHeight

                    Rectangle {
                        id: tile
                        anchors.fill: parent
                        anchors.margins: 4
                        radius: 8
                        color: cell.isSelected ? (grid.activeFocus ? Theme.activeSelection : Theme.selection)
                             : cellHover.hovered ? Theme.hover : "transparent"
                        border.color: cell.isSelected && grid.activeFocus ? Qt.rgba(0.24, 0.55, 0.99, 0.45) : "transparent"
                    }
                    HoverHandler { id: cellHover }

                    Item {
                        id: iconBox
                        anchors.horizontalCenter: parent.horizontalCenter
                        y: 10
                        width: root.iconSize
                        height: root.iconSize
                        Icon {
                            anchors.centerIn: parent
                            visible: !thumb.visible && !cell.pdfPreview
                            name: cell.icon[0]
                            size: root.iconSize
                            color: cell.isHidden ? Theme.textFaint : cell.icon[1]
                            opacity: cell.isHidden ? 0.7 : 1
                        }
                        Image {
                            id: thumb
                            anchors.fill: parent
                            visible: status === Image.Ready
                            source: !cell.isDir && root.iconSize >= 48 && !root.isPdf(cell.name) && root.isImage(cell.name) ? "file://" + cell.path : ""
                            sourceSize.width: root.iconSize * 2
                            sourceSize.height: root.iconSize * 2
                            fillMode: Image.PreserveAspectFit
                            asynchronous: true
                            smooth: true
                        }
                        // PDFs: the first page, rendered by Atlas' own PDF engine
                        // (Qt's PDF image plugin fails on many files), as a
                        // sheet of paper.
                        Loader {
                            id: pdfThumb
                            anchors.fill: parent
                            active: !cell.isDir && root.iconSize >= 48 && root.isPdf(cell.name)
                            sourceComponent: Item {
                                readonly property bool ready: page.rendered && fit > 0
                                readonly property real pw: doc.pageCount > 0 ? doc.pageWidth(0) : 0
                                readonly property real ph: doc.pageCount > 0 ? doc.pageHeight(0) : 0
                                readonly property real fit: pw > 0 && ph > 0 ? Math.min(width / pw, height / ph) : 0
                                PdfDocument { id: doc; source: cell.path }
                                Rectangle {
                                    visible: parent.ready
                                    anchors.centerIn: parent
                                    width: page.width + 2
                                    height: page.height + 2
                                    color: "white"
                                    border.color: Theme.border
                                }
                                PdfPage {
                                    id: page
                                    anchors.centerIn: parent
                                    width: Math.round(parent.pw * parent.fit)
                                    height: Math.round(parent.ph * parent.fit)
                                    visible: parent.ready
                                    source: parent.fit > 0 ? cell.path : ""
                                    page: 0
                                    renderWidth: Math.round(root.iconSize * 2)
                                }
                            }
                        }
                        Text {
                            visible: cell.isSymlink
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            text: "↪"
                            color: Theme.textStrong
                            font.pixelSize: Math.max(10, root.iconSize / 4)
                        }
                    }
                    Text {
                        anchors.top: iconBox.bottom
                        anchors.topMargin: 6
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: parent.width - 16
                        horizontalAlignment: Text.AlignHCenter
                        text: cell.name
                        color: cell.isHidden ? Theme.textMuted : Theme.textStrong
                        font.pixelSize: Theme.fontSize
                        wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                        maximumLineCount: cell.isSelected ? 4 : 2
                        elide: Text.ElideRight
                    }

                    Item { id: cellDragProxy }
                    Drag.active: cellMouse.drag.active
                    Drag.dragType: Drag.Automatic
                    Drag.supportedActions: Qt.CopyAction | Qt.MoveAction
                    Drag.proposedAction: Qt.MoveAction
                    Drag.mimeData: ({ "text/uri-list": root.actions.uriList(cell.isSelected ? root.selectedPaths() : [cell.path]) })

                    DropArea {
                        anchors.fill: parent
                        keys: ["text/uri-list"]
                        onDropped: (drop) => root.actions.dropInto(cell.isDir ? cell.path : root.source, drop)
                        Rectangle {
                            anchors.fill: parent
                            anchors.margins: 4
                            visible: parent.containsDrag && cell.isDir
                            radius: 8
                            color: "transparent"
                            border.color: Theme.accent
                        }
                    }

                    MouseArea {
                        id: cellMouse
                        drag.target: cellDragProxy
                        drag.threshold: 8
                        anchors.fill: parent
                        acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
                        onClicked: (mouse) => root.itemClicked(cell.index, cell.path, mouse)
                        onDoubleClicked: (mouse) => {
                            if (mouse.button === Qt.LeftButton) root.openRow(cell.index, mouse.modifiers & Qt.ControlModifier)
                        }
                    }
                }
            }
            // Momentum after touchpad swipes (#4).
            TouchpadFling {
                anchors.fill: grid
                visible: grid.visible
                flickable: grid
                active: !root.app || root.app.touchpadFling
                app: root.app
            }

            // Ctrl+wheel zooms the icons, like in Dolphin.
            // (All devices: Qt on Wayland reports wheels as a "touchpad".)
            WheelHandler {
                acceptedDevices: PointerDevice.AllDevices
                acceptedPointerTypes: PointerDevice.AllPointerTypes
                acceptedModifiers: Qt.ControlModifier
                property real pending: 0
                onWheel: (event) => {
                    event.accepted = true
                    // One size step per notch, also from small touchpad deltas.
                    pending += (event.angleDelta.y || event.pixelDelta.y * 2.4) / 120
                    const steps = pending > 0 ? Math.floor(pending) : Math.ceil(pending)
                    if (!steps) return
                    pending -= steps
                    root.iconSize = Math.max(zoom.from, Math.min(zoom.to, root.iconSize + 16 * steps))
                }
            }

            Label {
                anchors.centerIn: parent
                visible: folder.count === 0 && !folder.loading
                text: folder.error ? folder.error : qsTr("Dieser Ordner ist leer.")
                color: folder.error ? Theme.error : Theme.textFaint
            }
        }

        // ---- status bar
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 30
            color: Theme.header
            Rectangle { width: parent.width; height: 1; color: Theme.border }
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 10
                spacing: 12
                Text {
                    Layout.fillWidth: true
                    readonly property var summary: folder.summary ? JSON.parse(folder.summary) : ({ dirs: 0, files: 0, sizeText: "" })
                    readonly property var picked: root.selectedPaths()
                    text: picked.length > 0
                        ? qsTr("%n ausgewählt", "", picked.length) + (folder.sizeTextOf(picked) !== "0 B" ? " (" + folder.sizeTextOf(picked) + ")" : "")
                        : [summary.dirs ? qsTr("%n Ordner", "", summary.dirs) : "",
                           summary.files ? (summary.files === 1 ? qsTr("1 Datei") : qsTr("%1 Dateien").arg(summary.files)) + " (" + summary.sizeText + ")" : ""]
                          .filter(t => t).join(", ") || qsTr("Leer")
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                }
                Icon { visible: root.viewMode === "icons"; name: "minus"; size: 12; color: Theme.textFaint }
                Slider {
                    id: zoom
                    visible: root.viewMode === "icons"
                    Layout.preferredWidth: 110
                    from: 32
                    to: 128
                    stepSize: 16
                    value: root.iconSize
                    onMoved: root.iconSize = value
                }
                Icon { visible: root.viewMode === "icons"; name: "plus"; size: 12; color: Theme.textFaint }
                Text {
                    id: freeSpace
                    visible: text.length > 0
                    property var usage: ({})
                    function refresh() {
                        try { usage = root.app && root.source ? JSON.parse(root.app.diskUsage(root.source) || "{}") : {} } catch (e) { usage = {} }
                    }
                    text: usage.total ? qsTr("%1 frei").arg(Theme.formatBytes(usage.free)) : ""
                    color: Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    Component.onCompleted: refresh()
                    Connections { target: root; function onSourceChanged() { freeSpace.refresh() } }
                }
            }
        }
    }
}
