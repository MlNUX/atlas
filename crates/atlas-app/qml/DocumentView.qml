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
    /// Viewers without their own toolbars (quick look).
    property bool compact: false
    /// The EditorGroup showing this tab (changes when the tab is moved).
    property var ownerGroup: null
    // `revision` re-evaluates the kind after the file was moved or deleted.
    property int revision: 0
    readonly property string kind: path && revision >= 0 ? app.fileKind(path) : ""
    /// Show through an extension's processor (ID; set by "Anzeigen mit …").
    property string processor: ""
    /// Files without own viewer use an automatic processor if one fits.
    readonly property bool processed: processor !== ""
        || (kind === "binary" && JSON.parse(app.processorsFor(path)).some(p => p.auto))
    readonly property string title: kind === "trash" ? qsTr("Papierkorb")
        : kind === "calendar" ? qsTr("Kalender")
        : kind === "ilias" ? "ILIAS"
        : kind === "terminal" ? (viewer.item && viewer.item.title ? viewer.item.title : qsTr("Terminal"))
        : path ? app.fileName(path) : qsTr("Leer")
    property var pendingState: ({})
    property var backStack: []
    property var forwardStack: []
    readonly property bool canGoBack: backStack.length > 0
    readonly property bool canGoForward: forwardStack.length > 0
    readonly property alias viewerItem: viewer.item

    /// Open `path` in another tab or split (e.g. a file chosen in a folder).
    /// `where`: "tab", "preview" or "split".
    /// `state` is passed to the viewer (e.g. `{annot: id}` for PDFs).
    signal openRequested(string path, int line, string where, var state)
    /// The user interacted in a way that should pin a preview tab.
    signal pinRequested()
    /// The tab should close (e.g. its terminal exited).
    signal closeRequested()

    function refresh() { revision++ }
    /// Edits not yet written to the file (text and Markdown tabs).
    function hasUnsavedChanges() {
        const v = viewer.item
        const ed = v && v.editorItem ? v.editorItem : v
        return !!ed && ed.modified === true
    }
    /// Shows a file or folder (not a terminal, the calendar or ILIAS).
    readonly property bool onDisk: path.length > 0 && !path.startsWith("terminal:") && !path.startsWith("calendar:")
        && !path.startsWith("ilias:") && kind !== "trash"
    /// Start directory of a terminal tab (`terminal:<dir>#<id>`).
    function terminalStartDir() { return path.slice(9, path.lastIndexOf("#") > 9 ? path.lastIndexOf("#") : path.length) }

    /// Shows `newPath` in this tab and remembers the old location.
    function navigate(newPath, newLine, state) {
        if (newPath === path && (newLine || 0) === line) {
            if (state && viewer.item && viewer.item.applyState) viewer.item.applyState(state)
            return
        }
        if (path) backStack = backStack.concat([{ path: path, state: viewState() }])
        forwardStack = []
        show(newPath, newLine || 0, state || {})
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
        processor = pendingState.processor || ""
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
        if (pendingState.processor) processor = pendingState.processor
        if (viewer.item && viewer.item.applyState) viewer.item.applyState(pendingState)
    }

    Rectangle { anchors.fill: parent; color: Theme.editor }

    Loader {
        id: viewer
        anchors.fill: parent
        sourceComponent: view.kind === "trash" ? trashComponent
            : view.kind === "dir" ? folderComponent
            : view.kind === "terminal" ? terminalComponent
            : view.kind === "calendar" ? calendarComponent
            : view.kind === "ilias" ? iliasComponent
            : view.processor === "atlas:html" && view.kind !== "missing" ? htmlComponent
            : view.processed && view.kind !== "missing" ? processedComponent
            : view.kind === "pdf" ? pdfComponent
            : view.kind === "markdown" ? markdownComponent
            : view.kind === "text" ? textComponent
            : view.kind === "image" ? imageComponent
            : view.kind === "missing" ? missingComponent
            : view.path ? unsupportedComponent : emptyComponent
        onLoaded: {
            // Links into PDFs carry a page (`#page=N`) as line.
            if (view.kind === "pdf" && view.line > 0) view.pendingState = Object.assign({}, view.pendingState, { page: view.line - 1 })
            // The quick look shows whole PDF pages.
            if (view.compact && view.kind === "pdf" && view.pendingState.zoom === undefined)
                view.pendingState = Object.assign({}, view.pendingState, { zoom: -1 })
            if (item.applyState) item.applyState(view.pendingState)
        }
    }

    Component {
        id: trashComponent
        TrashView { actions: view.actions }
    }
    Component {
        id: folderComponent
        FolderView {
            actions: view.actions
            app: view.app
            source: view.path
            onActivated: (path, isDir, newPane) => {
                if (isDir && !newPane) view.navigate(path, 0)
                else view.openRequested(path, 0, newPane ? "split" : "tab", {})
            }
        }
    }
    Component {
        id: terminalComponent
        // `terminal:<start directory>#<id>`; the session stores the current
        // directory as state.
        // A tab running a program (a user action) is not restored.
        TerminalPane {
            workingDirectory: (view.pendingState && view.pendingState.cwd) || view.terminalStartDir()
            command: (view.pendingState && view.pendingState.command) || []
            actions: view.actions
            onClosed: view.closeRequested()
            function viewState() { return command.length ? { cwd: cwd(), transient: true } : { cwd: cwd() } }
            Component.onCompleted: Qt.callLater(focusTerminal)
        }
    }
    Component {
        id: calendarComponent
        // `calendar:`; mode, visible date and selection are the tab's state.
        CalendarView {
            app: view.app
            actions: view.actions
            onOpenRequested: (path, line, newPane, state) => view.openRequested(path, line, newPane ? "split" : "tab", state)
            onSessionRequested: (name) => { if (view.actions) view.actions.sessionRequested(name) }
        }
    }
    Component {
        id: iliasComponent
        // `ilias:`; the open course and folders are the tab's state.
        IliasView {
            app: view.app
            actions: view.actions
        }
    }
    Component {
        id: pdfComponent
        PdfViewer {
            source: view.path
            chrome: !view.compact
            app: view.app
            actions: view.actions
            onLinkActivated: (path, line, newPane, state) => {
                if (newPane) view.openRequested(path, line, "split", state)
                else view.navigate(path, line, state)
            }
        }
    }
    Component {
        id: htmlComponent
        // An HTML file rendered ("Als Webseite anzeigen").
        HtmlView {
            source: view.path
            app: view.app
            actions: view.actions
            // (Later: this item is replaced by the switch.)
            onShowSourceRequested: Qt.callLater(() => {
                view.pendingState = Object.assign({}, view.pendingState, { processor: "" })
                view.processor = ""
            })
        }
    }
    Component {
        id: processedComponent
        ProcessedView {
            source: view.path
            app: view.app
            actions: view.actions
            processor: view.processor
            onProcessorChanged: view.processor = processor
            onLinkActivated: (path, line, newPane, state) => {
                if (newPane) view.openRequested(path, line, "split", state)
                else view.navigate(path, line, state)
            }
        }
    }
    Component {
        id: textComponent
        TextEditor {
            source: view.path
            app: view.app
            line: view.line
            actions: view.actions
            onSavedAs: (path) => { view.path = path }
        }
    }
    Component {
        id: markdownComponent
        MarkdownView {
            source: view.path
            app: view.app
            line: view.line
            onLinkActivated: (path, line, newPane, state) => {
                if (newPane) view.openRequested(path, line, "split", state)
                else view.navigate(path, line, state)
            }
            onMessage: (text) => { if (view.actions) view.actions.showMessage(text) }
            actions: view.actions
            onSavedAs: (path) => { view.path = path }
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
