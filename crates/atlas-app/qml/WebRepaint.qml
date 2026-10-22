// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Window

// Keeps a WebEngineView from staying black: after a workspace switch on
// Wayland (e.g. Hyprland) the view showed nothing until it was scrolled,
// because Chromium sent no new frame for the exposed window. When the
// window becomes active or shown again, or the view itself visible, the
// page is asked for one fresh frame. Place it inside the view.
Item {
    id: repaint
    /// The WebEngineView to refresh (by default the parent).
    property var view: parent
    /// How often a frame was requested (for the smoke test).
    property int count: 0

    function request() { timer.restart() }

    Connections {
        target: repaint.Window.window
        ignoreUnknownSignals: true
        function onActiveChanged() { if (repaint.Window.window.active) repaint.request() }
        function onVisibilityChanged() { repaint.request() }
    }
    Connections {
        target: repaint.view
        ignoreUnknownSignals: true
        function onVisibleChanged() { if (repaint.view.visible) repaint.request() }
    }
    // A moment later, once the compositor shows the surface again. A tiny
    // opacity change makes Chromium paint and hand Qt a new frame.
    Timer {
        id: timer
        interval: 120
        onTriggered: {
            if (!repaint.view || !repaint.view.visible) return
            repaint.count += 1
            repaint.view.runJavaScript("(() => { const s = document.documentElement.style; s.opacity = '0.999'; "
                + "requestAnimationFrame(() => requestAnimationFrame(() => { s.opacity = ''; })); })()")
        }
    }
}
