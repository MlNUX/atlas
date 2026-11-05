// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Window

// Keeps a WebEngineView from showing black after a workspace switch on
// Wayland (e.g. Hyprland): Chromium sends no frame for the exposed window
// until something changes, and even a requested one takes a moment.
// - When the window loses focus (leaving the workspace), a snapshot of the
//   view is taken; coming back, it covers the view at once until Chromium
//   has painted again – the content is there instead of a black area. The
//   page reports that frame (two animation frames later), so large pages
//   that take longer stay covered until they are really there.
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
    /// The snapshot is showing / ready to show.
    readonly property bool covering: cover.visible
    readonly property bool hasSnapshot: cover.status === Image.Ready
    /// Frames the page reported as painted (for the smoke test).
    property int painted: 0
    /// Number of the frame request the cover waits for.
    property int waiting: 0
    anchors.fill: parent

    function request() {
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
    /// Shows the snapshot until Chromium has painted again.
    function showCover() {
        if (!cover.source.toString() || !view || !view.visible) return
        cover.visible = true
        waiting = count + 1
        pending.restart()
        uncover.restart()
    }

    Connections {
        target: repaint.Window.window
        ignoreUnknownSignals: true
        function onActiveChanged() {
            if (repaint.Window.window.active) { repaint.showCover(); repaint.request() }
            else repaint.snapshot()
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
            if (cover.visible && Number(message.slice(14)) >= repaint.waiting) { pending.stop(); settle.restart() }
        }
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
        onTriggered: { uncover.stop(); cover.visible = false }
    }
    // At the latest: never keep an old picture over a page that does not answer.
    Timer {
        id: uncover
        interval: 2000
        onTriggered: { pending.stop(); cover.visible = false }
    }

    Image {
        id: cover
        anchors.fill: parent
        visible: false
        cache: false
        smooth: false
        fillMode: Image.Stretch
    }
}
