// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Window
import Atlas

// Keeps a WebEngineView from showing black after a workspace switch on
// Wayland (e.g. Hyprland): Chromium sends no frame for the exposed window
// until something changes, and even a requested one takes a moment.
// - When the window loses focus (leaving the workspace), a snapshot of the
//   view is taken; coming back, it covers the view at once until Chromium
//   has painted again – the content is there instead of a black area. The
//   page reports that frame (two animation frames later), so large pages
//   that take longer stay covered until they are really there.
// - The cover goes up already while the window is inactive (a moment after
//   it lost focus), so even the first frame after coming back is the
//   snapshot, never black. It goes again once the page painted, as soon as
//   the mouse is over the view or the content changes (a window beside
//   others stays live).
// - Without a snapshot the cover is the editor colour of the design (white
//   in the light one) instead of Chromium's black.
// - The page is asked for a fresh frame right away (and once more a
//   moment later) when the window is active or shown again, or the view
//   itself becomes visible.
// Place it inside the view.
Item {
    id: repaint
    /// The WebEngineView to refresh (by default the parent).
    property var view: parent
    /// How often a frame was requested (for the smoke test).
    property int count: 0
    /// The cover (snapshot, else the editor colour) is showing; a snapshot is ready.
    readonly property bool covering: coverBox.visible
    readonly property bool hasSnapshot: cover.status === Image.Ready
    /// Covered while the window is inactive (another workspace).
    property bool away: false
    /// Colour under the snapshot (the editor's: white in the light design).
    readonly property color coverColor: coverBox.color
    /// Frames the page reported as painted (for the smoke test).
    property int painted: 0
    /// Number of the frame request the cover waits for.
    property int waiting: 0
    anchors.fill: parent

    function request() {
        // Content changed while away: show the live page again.
        if (away) uncoverSoon()
        frame()
        retry.restart()
    }
    function frame() {
        if (!view || !view.visible) return
        count += 1
        // A tiny opacity change makes Chromium paint and hand Qt a new frame;
        // two animation frames later that frame is out, and the page says so.
        view.runJavaScript("(() => { const e = document.documentElement; if (!e) return; const s = e.style; s.opacity = '0.999'; "
            + "requestAnimationFrame(() => requestAnimationFrame(() => { s.opacity = ''; "
            + "requestAnimationFrame(() => requestAnimationFrame(() => console.info('atlas:painted:" + count + "'))); })); })()")
    }
    /// Takes the snapshot shown while coming back.
    function snapshot() {
        if (!view || !view.visible || view.width < 1 || view.height < 1) return
        view.grabToImage((result) => { cover.source = result.url })
    }
    /// Shows the cover until Chromium has painted again.
    function showCover() {
        if (!view || !view.visible) return
        coverBox.visible = true
        uncoverSoon()
    }
    /// The cover goes once the page reports a frame asked for from now on.
    function uncoverSoon() {
        away = false
        if (!coverBox.visible) return
        waiting = count + 1
        pending.restart()
        uncover.restart()
    }
    /// The window is inactive for a moment: cover the view already (the
    /// compositor may show the first frame after coming back before Atlas
    /// hears of it).
    function leave(force) {
        if (!view || !view.visible || (!force && repaint.Window.window && repaint.Window.window.active)) return
        pending.stop()
        settle.stop()
        uncover.stop()
        away = true
        coverBox.visible = true
    }

    Connections {
        target: repaint.Window.window
        ignoreUnknownSignals: true
        function onActiveChanged() {
            if (repaint.Window.window.active) { leaveTimer.stop(); repaint.showCover(); repaint.request() }
            else { repaint.snapshot(); leaveTimer.restart() }
        }
        function onVisibilityChanged() { repaint.showCover(); repaint.request() }
    }
    Connections {
        target: repaint.view
        ignoreUnknownSignals: true
        function onVisibleChanged() { if (repaint.view.visible) repaint.request() }
        // Content or size changed: an old snapshot would show something else.
        function onWidthChanged() { cover.source = "" }
        function onHeightChanged() { cover.source = "" }
        function onJavaScriptConsoleMessage(level, message) {
            if (!message.startsWith("atlas:painted:")) return
            repaint.painted += 1
            // A frame asked for after the cover went up: Chromium shows the page again.
            if (coverBox.visible && !repaint.away && Number(message.slice(14)) >= repaint.waiting) { pending.stop(); settle.restart() }
        }
    }
    // Inactive for a moment (not only a click into another window and back).
    Timer {
        id: leaveTimer
        interval: 300
        onTriggered: repaint.leave()
    }
    // The mouse over a covered view (a window beside others): the live page.
    HoverHandler {
        onHoveredChanged: if (hovered && repaint.away) repaint.showCover()
    }
    // Once more when the compositor surely shows the surface again.
    Timer {
        id: retry
        interval: 100
        onTriggered: repaint.frame()
    }
    // While covered, ask again until the page answers (a page that was
    // hidden a while paints its first frame only once it is shown).
    Timer {
        id: pending
        interval: 150
        repeat: true
        onTriggered: repaint.frame()
    }
    // The reported frame still has to reach the window.
    Timer {
        id: settle
        interval: 60
        onTriggered: { uncover.stop(); coverBox.visible = false }
    }
    // At the latest: never keep an old picture over a page that does not answer.
    Timer {
        id: uncover
        interval: 2000
        onTriggered: { pending.stop(); if (!repaint.away) coverBox.visible = false }
    }

    Rectangle {
        id: coverBox
        anchors.fill: parent
        visible: false
        color: Theme.editor
        Image {
            id: cover
            anchors.fill: parent
            cache: false
            smooth: false
            fillMode: Image.Stretch
        }
    }
}
