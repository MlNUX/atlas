// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Atlas

// Content of one tab: a viewer chosen by file type, with its own
// back/forward history (folder navigation, followed links).
Item {
    id: view
    required property AppController app
    property var actions
    property string path
    property int line: 0
    /// Preview tabs are replaced by the next preview; pinning makes them stay.
    property bool preview: false
    // `revision` re-evaluates the kind after the file was moved or deleted.
    property int revision: 0
    readonly property string kind: path && revision >= 0 ? app.fileKind(path) : ""
    readonly property string title: kind === "trash" ? qsTr("Papierkorb") : path ? app.fileName(path) : qsTr("Leer")
    property var pendingState: ({})
    property var backStack: []
    property var forwardStack: []
    readonly property bool canGoBack: backStack.length > 0
    readonly property bool canGoForward: forwardStack.length > 0
    readonly property alias viewerItem: viewer.item

    /// Open `path` in another tab or split (e.g. a file chosen in a folder).
    /// `where`: "tab", "preview" or "split".
    signal openRequested(string path, int line, string where)
    /// The user interacted in a way that should pin a preview tab.
    signal pinRequested()

    function refresh() { revision++ }

    /// Shows `newPath` in this tab and remembers the old location.
    function navigate(newPath, newLine) {
        if (newPath === path && (newLine || 0) === line) return
        if (path) backStack = backStack.concat([{ path: path, state: viewState() }])
        forwardStack = []
        show(newPath, newLine || 0, {})
    }
    function goBack() {
        if (!canGoBack) return
        const entry = backStack[backStack.length - 1]
        backStack = backStack.slice(0, -1)
        forwardStack = forwardStack.concat([{ path: path, state: viewState() }])
        show(entry.path, 0, entry.state)
    }
    function goForward() {
        if (!canGoForward) return
        const entry = forwardStack[forwardStack.length - 1]
        forwardStack = forwardStack.slice(0, -1)
        backStack = backStack.concat([{ path: path, state: viewState() }])
        show(entry.path, 0, entry.state)
    }
    function show(newPath, newLine, state) {
        pendingState = state || {}
        line = newLine || 0
        path = newPath
        if (viewer.item && viewer.item.applyState) viewer.item.applyState(pendingState)
    }
    /// Jumps to a line/page in the current document.
    function reveal(newLine) {
        line = newLine
        if (viewer.item && viewer.item.reveal) viewer.item.reveal(newLine)
    }

    function viewState() {
        const s = { kind: view.kind }
        if (viewer.item && viewer.item.viewState) Object.assign(s, viewer.item.viewState())
        return s
    }
    function applyState(state) {
        pendingState = state || {}
        if (viewer.item && viewer.item.applyState) viewer.item.applyState(pendingState)
    }

    Rectangle { anchors.fill: parent; color: Theme.editor }

    Loader {
        id: viewer
        anchors.fill: parent
        sourceComponent: view.kind === "trash" ? trashComponent
            : view.kind === "dir" ? folderComponent
            : view.kind === "pdf" ? pdfComponent
            : view.kind === "markdown" || view.kind === "text" ? textComponent
            : view.kind === "image" ? imageComponent
            : view.kind === "missing" ? missingComponent
            : view.path ? unsupportedComponent : emptyComponent
        onLoaded: if (item.applyState) item.applyState(view.pendingState)
    }

    Component {
        id: trashComponent
        TrashView { actions: view.actions }
    }
    Component {
        id: folderComponent
        FolderView {
            actions: view.actions
            source: view.path
            onActivated: (path, isDir, newPane) => {
                if (isDir && !newPane) view.navigate(path, 0)
                else view.openRequested(path, 0, newPane ? "split" : "tab")
            }
        }
    }
    Component {
        id: pdfComponent
        PdfViewer { source: view.path; app: view.app }
    }
    Component {
        id: textComponent
        TextViewer {
            source: view.path
            app: view.app
            markdown: view.kind === "markdown"
            line: view.line
        }
    }
    Component {
        id: imageComponent
        Rectangle {
            color: Theme.pdfBackdrop
            Image {
                anchors.fill: parent
                anchors.margins: 16
                source: "file://" + view.path
                fillMode: Image.PreserveAspectFit
                asynchronous: true
            }
        }
    }
    Component {
        id: unsupportedComponent
        Label {
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            color: Theme.textMuted
            text: qsTr("Keine Vorschau für diesen Dateityp.")
        }
    }
    Component {
        id: missingComponent
        Label {
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            color: Theme.textMuted
            wrapMode: Text.Wrap
            text: qsTr("„%1“ ist nicht mehr vorhanden.\nSie wurde gelöscht, verschoben oder umbenannt.").arg(view.app.fileName(view.path))
        }
    }
    Component {
        id: emptyComponent
        Label {
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            color: Theme.textFaint
            wrapMode: Text.Wrap
            text: qsTr("Datei im Explorer wählen oder hierher ziehen.\nStrg+Klick öffnet sie in einem neuen Split.")
        }
    }
}
