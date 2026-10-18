// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Atlas

// One terminal instance: the painted TerminalView plus input forwarding,
// scrollbar and context menu.
Rectangle {
    id: pane
    property string workingDirectory
    /// Program and arguments instead of the shell (e.g. an editor).
    property var command: []
    property bool active: false
    readonly property alias title: term.title
    function cwd() { return term.currentDirectory() || pane.workingDirectory }
    readonly property alias view: term

    signal closed()
    signal activated()

    color: Theme.terminal
    border.color: active ? Theme.border : "transparent"

    function focusTerminal() { term.forceActiveFocus() }

    TerminalView {
        id: term
        anchors.fill: parent
        anchors.leftMargin: 10
        anchors.topMargin: 4
        anchors.rightMargin: 14
        anchors.bottomMargin: 2
        workingDirectory: pane.workingDirectory
        command: pane.command
        fontFamily: Theme.monoFont
        fontPixelSize: 14
        focus: true
        focused: activeFocus
        activeFocusOnTab: false

        onWidthChanged: ensureStarted()
        onHeightChanged: ensureStarted()
        onActiveFocusChanged: if (activeFocus) pane.activated()
        onExited: pane.closed()

        function ensureStarted() {
            if (width < 10 || height < 10) return
            if (!running) start()
            else relayout()
        }

        // The terminal gets every key, except the few shortcuts Atlas uses
        // itself; this mirrors VS Code's terminal behaviour.
        Keys.onShortcutOverride: (event) => {
            const ctrlShift = (event.modifiers & Qt.ControlModifier) && (event.modifiers & Qt.ShiftModifier)
            const own = event.key === Qt.Key_QuoteLeft && (event.modifiers & Qt.ControlModifier)
            event.accepted = !own && !(ctrlShift && (event.key === Qt.Key_C || event.key === Qt.Key_V || event.key === Qt.Key_F))
        }
        Keys.onPressed: (event) => {
            const ctrlShift = (event.modifiers & Qt.ControlModifier) && (event.modifiers & Qt.ShiftModifier)
            if (ctrlShift && event.key === Qt.Key_C) { term.copy(); event.accepted = true; return }
            if ((ctrlShift && event.key === Qt.Key_V) || (event.key === Qt.Key_Insert && (event.modifiers & Qt.ShiftModifier))) {
                term.paste(); event.accepted = true; return
            }
            if (term.sendKey(event.key, event.modifiers, event.text)) event.accepted = true
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: term
        acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
        hoverEnabled: term.mouseReporting
        cursorShape: Qt.IBeamCursor
        property int clicks: 0
        property real lastPress: 0
        property point lastPos

        onPressed: (m) => {
            term.forceActiveFocus()
            const now = Date.now()
            const near = Math.abs(m.x - lastPos.x) < 4 && Math.abs(m.y - lastPos.y) < 4
            clicks = (now - lastPress < 400 && near) ? Math.min(3, clicks + 1) : 1
            lastPress = now
            lastPos = Qt.point(m.x, m.y)
            if (m.button === Qt.RightButton && !term.mouseReporting) {
                contextMenu.popup()
                return
            }
            term.mousePress(m.x, m.y, m.button, m.modifiers, clicks)
        }
        onPositionChanged: (m) => term.mouseMove(m.x, m.y, m.buttons, m.modifiers)
        onReleased: (m) => term.mouseRelease(m.x, m.y, m.button, m.modifiers)
        onWheel: (w) => term.wheel(w.x, w.y, w.angleDelta.y, w.modifiers)
    }

    // Scrollback indicator/scrollbar.
    ScrollBar {
        id: scrollbar
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        orientation: Qt.Vertical
        visible: term.historySize > 0
        readonly property real total: term.historySize + term.lines
        size: total > 0 ? term.lines / total : 1
        position: total > 0 ? (term.historySize - term.displayOffset) / total : 0
        onPositionChanged: if (pressed) term.scrollTo(Math.round(term.historySize - position * total))
    }

    Menu {
        id: contextMenu
        MenuItem { text: qsTr("Kopieren"); enabled: term.hasSelection(); onTriggered: term.copy() }
        MenuItem { text: qsTr("Einfügen"); onTriggered: term.paste() }
        MenuSeparator {}
        MenuItem { text: qsTr("Auswahl aufheben"); onTriggered: term.clearSelection() }
    }
}
