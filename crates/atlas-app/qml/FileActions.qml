// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Atlas

// File operations shared by project tree, file lists and trash view:
// context menu, dialogs, clipboard and status messages.
Item {
    id: actions

    readonly property alias ops: ops
    readonly property alias propertiesDialog: propertiesDialog
    readonly property alias busy: ops.busy
    /// Last result or progress text for the status bar.
    property string message: ops.busy ? ops.status : lastMessage
    property string lastMessage
    required property AppController app

    signal openRequested(string path, bool newPane)
    /// A file or folder was created (e.g. to select or open it).
    signal created(string path)
    /// Space: large preview; `navigate(step)` returns the neighbour path.
    signal quickLookRequested(string path, var navigate)
    /// Run `command` (program + arguments) in an Atlas terminal in `cwd`.
    signal runInTerminal(var command, string cwd)
    /// Same, as a terminal tab in the editor area.
    signal runInTerminalTab(var command, string cwd)
    /// Open a terminal in `dir`; `where`: "panel", "tab" or "split".
    signal openTerminalRequested(string dir, string where)
    /// Put `text` (terminal output) into a note; `cwd` suggests a folder.
    signal sendToNoteRequested(string text, string cwd)
    /// Put a Markdown block into a note: `makeBlock(notePath)` builds it
    /// (e.g. a PDF quote whose link depends on the note).
    signal blockToNoteRequested(string heading, var makeBlock, string cwd, string doneText, string newNoteName)

    FileOps {
        id: ops
        onFinished: (message) => actions.showMessage(message)
        onFailed: (message) => {
            errorDialog.text = message
            errorDialog.open()
        }
    }

    Timer { id: messageTimer; interval: 6000; onTriggered: actions.lastMessage = "" }
    function showMessage(text) {
        lastMessage = text
        messageTimer.restart()
    }

    function baseName(path) { return path.slice(path.lastIndexOf("/") + 1) }

    // ------------------------------------------------------------ operations

    /// Opens `path` at `line` in the configured external editor.
    function openInEditor(path, line) {
        const r = JSON.parse(app.openInEditor(path, line || 0))
        if (r.mode === "system") Qt.openUrlExternally("file://" + encodeURI(path))
        else if (r.mode === "terminal") actions.runInTerminal(r.command, ops.parentDir(path))
        else if (r.mode === "error") {
            errorDialog.text = qsTr("Externer Editor: %1").arg(r.error)
            errorDialog.open()
        } else showMessage(qsTr("Geöffnet mit %1").arg(r.command[0]))
    }

    function configureEditor() { editorDialog.open() }

    // ---------------------------------------------------- user defined actions

    /// Actions for `paths` (files, one folder, or [] for the project).
    function actionsFor(paths) { return JSON.parse(app.actionsFor(paths)) }

    /// Runs action `id` on `paths` visibly in a terminal; asks first if the
    /// action wants it or the command looks destructive.
    function runAction(id, paths, line) {
        const p = JSON.parse(app.prepareAction(id, paths, line || 0))
        if (p.error) {
            errorDialog.text = p.error
            errorDialog.open()
            return
        }
        const run = () => {
            if (p.placement === "tab") actions.runInTerminalTab(p.argv, p.cwd)
            else actions.runInTerminal(p.argv, p.cwd)
            showMessage(qsTr("Aktion „%1“ gestartet").arg(p.name))
        }
        if (!p.confirm) { run(); return }
        confirmDialog.ask(
            qsTr("Aktion ausführen?"),
            qsTr("„%1“ führt in %2 aus:\n\n%3").arg(p.name).arg(p.cwd).arg(p.display)
                + (p.destructive ? qsTr("\n\nDer Befehl kann Dateien löschen oder überschreiben.") : ""),
            qsTr("Ausführen"),
            run)
    }

    /// Opens the project's (or the global) action file, created from a
    /// template if needed.
    function editActions(global) {
        const file = app.actionsFile(global)
        if (file) actions.openRequested(file, false)
        else showMessage(qsTr("Aktionsdatei kann nicht angelegt werden"))
        const errors = JSON.parse(app.actionErrors())
        if (errors.length) showMessage(qsTr("Aktionen: %1").arg(errors[0].message))
    }

    function showProperties(path) { propertiesDialog.show(path) }
    function openWithSystem(path) { Qt.openUrlExternally("file://" + encodeURI(path)) }
    function showInFileManager(path) {
        if (!ops.showInFileManager(path)) Qt.openUrlExternally("file://" + encodeURI(ops.parentDir(path)))
    }

    function sendToNote(text, cwd) {
        if (text.trim()) sendToNoteRequested(text, cwd)
        else showMessage(qsTr("Keine Ausgabe zum Übernehmen"))
    }

    function sendBlockToNote(heading, makeBlock, cwd, doneText, newNoteName) {
        blockToNoteRequested(heading, makeBlock, cwd || app.projectRoot, doneText || "", newNoteName || "")
    }

    function newFile(dir) {
        nameDialog.ask(qsTr("Neue Datei in „%1“").arg(baseName(dir)), qsTr("Neue Datei.md"), false, (name) => {
            const path = ops.newFile(dir, name)
            if (path) { actions.created(path); actions.openRequested(path, false) }
        })
    }

    function newFolder(dir) {
        nameDialog.ask(qsTr("Neuer Ordner in „%1“").arg(baseName(dir)), qsTr("Neuer Ordner"), true, (name) => {
            const path = ops.newFolder(dir, name)
            if (path) actions.created(path)
        })
    }

    function rename(path) {
        nameDialog.ask(qsTr("Umbenennen"), baseName(path), ops.isDir(path), (name) => {
            if (name !== baseName(path)) ops.rename(path, name)
        })
    }

    function copy(paths) {
        ops.setClipboard(paths, false)
        showMessage(qsTr("%n Element(e) kopiert – mit Strg+V einfügen", "", paths.length))
    }

    function cut(paths) {
        ops.setClipboard(paths, true)
        showMessage(qsTr("%n Element(e) ausgeschnitten – mit Strg+V einfügen", "", paths.length))
    }

    function canPaste() { return ops.clipboardFiles().length > 0 }

    function paste(dir) {
        const files = ops.clipboardFiles()
        if (files.length === 0) return
        transfer(ops.clipboardIsCut() ? "move" : "copy", files, dir)
    }

    /// Copy or move with a question if names clash.
    function transfer(kind, paths, dest) {
        // Pasting a copy into its own folder never clashes: it creates "… (Kopie)".
        const clashes = ops.conflicts(paths, dest).filter(p => kind === "move" || paths.indexOf(p) < 0)
        if (clashes.length === 0) {
            ops.transfer(kind, paths, dest, "abort")
            return
        }
        conflictDialog.kind = kind
        conflictDialog.paths = paths
        conflictDialog.dest = dest
        conflictDialog.clashes = clashes
        conflictDialog.open()
    }

    /// `text/uri-list` for drags (each path segment percent-encoded).
    function uriList(paths) {
        return paths.map(p => "file://" + p.split("/").map(encodeURIComponent).join("/")).join("\r\n")
    }

    /// Handles a drop of files onto folder `dir`: move by default, copy
    /// when the user holds Ctrl (or the source only allows copying).
    function dropInto(dir, drop) {
        const paths = drop.urls.map(u => actions.app.urlToPath(u.toString())).filter(p => p && p !== dir)
        if (paths.length === 0) return
        const copy = drop.proposedAction === Qt.CopyAction
        drop.accept(copy ? Qt.CopyAction : Qt.MoveAction)
        transfer(copy ? "copy" : "move", paths, dir)
    }

    function quickLook(path, navigate) { if (path) quickLookRequested(path, navigate) }

    function trash(paths) {
        if (paths.length > 0) ops.moveToTrash(paths)
    }

    function restore(ids) { ops.restoreFromTrash(ids) }

    function purge(ids, names) {
        confirmDialog.ask(
            qsTr("Endgültig löschen?"),
            ids.length === 1 ? qsTr("„%1“ wird unwiderruflich gelöscht.").arg(names[0])
                             : qsTr("%n Elemente werden unwiderruflich gelöscht.", "", ids.length),
            qsTr("Endgültig löschen"),
            () => ops.deleteFromTrash(ids))
    }

    function copyPaths(paths) {
        ops.copyText(paths.join("\n"))
        showMessage(qsTr("Pfad kopiert"))
    }

    // ---------------------------------------------------------- context menu

    /// `paths`: selected items (may be empty); `dir`: folder for new items and paste.
    function showMenu(paths, dir) {
        menu.paths = paths
        menu.dir = dir
        menu.actionList = actionsFor(paths)
        menu.popup()
    }

    Menu {
        id: menu
        property var paths: []
        property string dir
        /// User defined actions for the selection.
        property var actionList: []
        readonly property bool single: paths.length === 1
        readonly property bool any: paths.length > 0
        /// The clicked folder, or the folder the menu was opened in.
        function terminalDir() { return single && ops.isDir(paths[0]) ? paths[0] : dir }

        MenuItem {
            text: qsTr("Öffnen")
            visible: menu.single
            height: visible ? implicitHeight : 0
            onTriggered: actions.openRequested(menu.paths[0], false)
        }
        MenuItem {
            text: qsTr("In neuem Split öffnen")
            visible: menu.single
            height: visible ? implicitHeight : 0
            onTriggered: actions.openRequested(menu.paths[0], true)
        }
        MenuItem {
            text: qsTr("Im Editor öffnen")
            visible: menu.single && !ops.isDir(menu.paths[0])
            height: visible ? implicitHeight : 0
            onTriggered: actions.openInEditor(menu.paths[0], 0)
        }
        MenuSeparator { visible: menu.single; height: visible ? implicitHeight : 0 }
        MenuItem {
            text: qsTr("Terminal hier öffnen")
            onTriggered: actions.openTerminalRequested(menu.terminalDir(), "panel")
        }
        MenuItem {
            text: qsTr("Terminal als Tab öffnen")
            onTriggered: actions.openTerminalRequested(menu.terminalDir(), "tab")
        }
        Menu {
            id: actionsMenu
            title: menu.actionList.length ? qsTr("Aktionen (%1)").arg(menu.actionList.length) : qsTr("Aktionen")
            Instantiator {
                model: menu.actionList
                delegate: MenuItem {
                    required property var modelData
                    text: modelData.name
                    ToolTip.visible: hovered && modelData.description.length > 0
                    ToolTip.text: modelData.description
                    onTriggered: actions.runAction(modelData.id, menu.paths, 0)
                }
                onObjectAdded: (index, object) => actionsMenu.insertItem(index, object)
                onObjectRemoved: (index, object) => actionsMenu.removeItem(object)
            }
            MenuItem {
                text: qsTr("Keine passende Aktion")
                enabled: false
                visible: menu.actionList.length === 0
                height: visible ? implicitHeight : 0
            }
            MenuSeparator {}
            MenuItem { text: qsTr("Projektaktionen bearbeiten …"); enabled: !!app.projectRoot; onTriggered: actions.editActions(false) }
            MenuItem { text: qsTr("Globale Aktionen bearbeiten …"); onTriggered: actions.editActions(true) }
        }
        MenuSeparator {}
        MenuItem { text: qsTr("Neue Datei …"); onTriggered: actions.newFile(menu.dir) }
        MenuItem { text: qsTr("Neuer Ordner …"); onTriggered: actions.newFolder(menu.dir) }
        MenuSeparator {}
        MenuItem { text: qsTr("Ausschneiden\tStrg+X"); enabled: menu.any; onTriggered: actions.cut(menu.paths) }
        MenuItem { text: qsTr("Kopieren\tStrg+C"); enabled: menu.any; onTriggered: actions.copy(menu.paths) }
        MenuItem { text: qsTr("Einfügen\tStrg+V"); enabled: actions.canPaste(); onTriggered: actions.paste(menu.dir) }
        MenuSeparator {}
        MenuItem { text: qsTr("Umbenennen …\tF2"); enabled: menu.single; onTriggered: actions.rename(menu.paths[0]) }
        MenuItem { text: qsTr("Pfad kopieren"); enabled: menu.any; onTriggered: actions.copyPaths(menu.paths) }
        MenuItem {
            text: qsTr("Mit Standardprogramm öffnen")
            visible: menu.single
            height: visible ? implicitHeight : 0
            onTriggered: actions.openWithSystem(menu.paths[0])
        }
        MenuItem {
            text: qsTr("Im Dateimanager zeigen")
            visible: menu.single
            height: visible ? implicitHeight : 0
            onTriggered: actions.showInFileManager(menu.paths[0])
        }
        MenuItem {
            text: qsTr("Eigenschaften …\tAlt+Enter")
            visible: menu.single
            height: visible ? implicitHeight : 0
            onTriggered: actions.showProperties(menu.paths[0])
        }
        MenuSeparator {}
        MenuItem { text: qsTr("In den Papierkorb\tEntf"); enabled: menu.any; onTriggered: actions.trash(menu.paths) }
        MenuSeparator {}
        MenuItem { text: qsTr("Externen Editor festlegen …"); onTriggered: actions.configureEditor() }
    }

    // --------------------------------------------------------------- dialogs

    Dialog {
        id: nameDialog
        property var acceptAction: null
        property bool isFolder: false
        anchors.centerIn: Overlay.overlay
        width: 420
        modal: true
        standardButtons: Dialog.Ok | Dialog.Cancel

        function ask(title, initial, isFolder, onAccept) {
            nameDialog.title = title
            nameDialog.acceptAction = onAccept
            nameDialog.isFolder = isFolder
            nameField.text = initial
            nameField.textChanged()
            open()
            nameField.forceActiveFocus()
            // Select the name without extension, like other file managers.
            const dot = initial.lastIndexOf(".")
            nameField.select(0, !isFolder && dot > 0 ? dot : initial.length)
        }

        onAccepted: if (acceptAction && !nameError.text) acceptAction(nameField.text.trim())

        ColumnLayout {
            anchors.fill: parent
            TextField {
                id: nameField
                Layout.fillWidth: true
                selectByMouse: true
                onTextChanged: {
                    nameError.text = ops.checkName(text.trim())
                    const ok = nameDialog.standardButton(Dialog.Ok)
                    if (ok) ok.enabled = !nameError.text
                }
                onAccepted: if (!nameError.text) nameDialog.accept()
            }
            Label {
                id: nameError
                Layout.fillWidth: true
                visible: text.length > 0
                color: Theme.error
                wrapMode: Text.Wrap
            }
        }
    }

    Dialog {
        id: conflictDialog
        property string kind
        property var paths: []
        property string dest
        property var clashes: []
        anchors.centerIn: Overlay.overlay
        width: 460
        modal: true
        title: qsTr("Namen bereits vorhanden")

        function finish(policy) {
            close()
            ops.transfer(kind, paths, dest, policy)
        }

        ColumnLayout {
            anchors.fill: parent
            spacing: 10
            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: conflictDialog.clashes.length === 1
                    ? qsTr("„%1“ existiert bereits in „%2“.").arg(actions.baseName(conflictDialog.clashes[0] || "")).arg(actions.baseName(conflictDialog.dest))
                    : qsTr("%1 Elemente existieren bereits in „%2“.").arg(conflictDialog.clashes.length).arg(actions.baseName(conflictDialog.dest))
            }
            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                color: Theme.textMuted
                font.pixelSize: Theme.smallFontSize
                text: qsTr("„Ersetzen“ verschiebt die vorhandenen Dateien in den Papierkorb.")
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Button { text: qsTr("Abbrechen"); onClicked: conflictDialog.close() }
                Button { text: qsTr("Überspringen"); onClicked: conflictDialog.finish("skip") }
                Button { text: qsTr("Beide behalten"); onClicked: conflictDialog.finish("keep-both") }
                Button { text: qsTr("Ersetzen"); onClicked: conflictDialog.finish("replace") }
            }
        }
    }

    Dialog {
        id: confirmDialog
        property var confirmAction: null
        property alias text: confirmLabel.text
        property alias confirmText: confirmButton.text
        anchors.centerIn: Overlay.overlay
        width: 420
        modal: true

        function ask(title, text, confirmText, action) {
            confirmDialog.title = title
            confirmDialog.text = text
            confirmDialog.confirmText = confirmText
            confirmDialog.confirmAction = action
            open()
        }

        ColumnLayout {
            anchors.fill: parent
            spacing: 12
            Label { id: confirmLabel; Layout.fillWidth: true; wrapMode: Text.Wrap }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                Button { text: qsTr("Abbrechen"); onClicked: confirmDialog.close() }
                Button {
                    id: confirmButton
                    onClicked: { confirmDialog.close(); if (confirmDialog.confirmAction) confirmDialog.confirmAction() }
                }
            }
        }
    }

    PropertiesDialog { id: propertiesDialog; ops: ops }

    // „Im Editor öffnen“: command template in ~/.config/atlas/config.toml.
    Dialog {
        id: editorDialog
        anchors.centerIn: Overlay.overlay
        width: 560
        modal: true
        title: qsTr("Externer Editor")
        standardButtons: Dialog.Save | Dialog.Cancel
        onAboutToShow: {
            editorField.text = app.editorCommand()
            editorTerminal.checked = app.editorInTerminal()
            editorError.text = ""
        }
        ColumnLayout {
            anchors.fill: parent
            spacing: 8
            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                color: Theme.textMuted
                text: qsTr("Befehl für „Im Editor öffnen“. Platzhalter: {file}, {line}, {column}, {dir}. Leer lassen, um $VISUAL/$EDITOR oder das Standardprogramm des Systems zu verwenden.")
            }
            TextField {
                id: editorField
                Layout.fillWidth: true
                placeholderText: "code --goto {file}:{line}:{column}"
                onAccepted: editorDialog.accept()
            }
            CheckBox { id: editorTerminal; text: qsTr("Im Terminal von Atlas ausführen (vim, helix, nano …)") }
            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                color: Theme.textFaint
                font.pixelSize: Theme.smallFontSize
                text: qsTr("Beispiele: kate --line {line} {file} · gnome-text-editor +{line} {file} · nvim +{line} {file}")
            }
            Label { id: editorError; Layout.fillWidth: true; wrapMode: Text.Wrap; color: Theme.error; visible: text !== "" }
        }
        onAccepted: {
            const error = app.setEditorCommand(editorField.text.trim(), editorTerminal.checked)
            if (error) { editorError.text = error; open() }
            else showMessage(qsTr("Externer Editor gespeichert"))
        }
    }

    Dialog {
        id: errorDialog
        property alias text: errorLabel.text
        anchors.centerIn: Overlay.overlay
        width: 460
        modal: true
        title: qsTr("Vorgang fehlgeschlagen")
        standardButtons: Dialog.Ok
        Label { id: errorLabel; width: parent.width; wrapMode: Text.Wrap }
    }
}
