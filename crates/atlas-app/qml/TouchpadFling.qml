// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick

// Momentum for touchpad scrolling (#4). Wayland compositors send no
// kinetic scrolling: when the fingers leave the touchpad the content stops
// at once. This measures the speed of the last moments of a two-finger
// swipe and lets the Flickable glide on, slowing down; touching again
// stops it. Mouse wheels (whole notches) are left alone.
//
// Either it watches the wheel events itself (`handleWheel`, for plain
// Flickables, which keep scrolling as before) or a view with its own wheel
// handling reports each step with sample().
Item {
    id: fling
    required property Flickable flickable
    /// The setting (Einstellungen → Dateien).
    property bool active: true
    /// Watch the wheel here; false: the view calls sample() itself.
    property bool handleWheel: true
    /// How often a fling was started (for the smoke test).
    property int count: 0
    /// AppController: its touchpadHeld signal (fingers put down, Wayland
    /// hold gesture) stops the momentum.
    property var app: null
    /// Momentum relative to the swipe speed.
    readonly property real boost: 1.6

    /// Recent steps: {t, dx, dy} in pixels (content moves by -d).
    property var samples: []

    /// A touchpad step; true if it was one (pixel delta, no whole notch).
    function isTouchpad(event) {
        const pixel = (p, a) => p !== 0 && (a === 0 || a % 120 !== 0)
        return pixel(event.pixelDelta.y, event.angleDelta.y) || pixel(event.pixelDelta.x, event.angleDelta.x)
    }
    /// Handles the phase of a touchpad gesture: lifting the fingers
    /// (ScrollEnd, Wayland's axis stop) starts the momentum at once instead
    /// of after a pause; a new gesture stops it. True if the event was only
    /// that (no movement).
    function phase(event) {
        if (event.phase === Qt.ScrollBegin) {
            endTimer.stop()
            samples = []
            flickable.cancelFlick()
        } else if (event.phase === Qt.ScrollEnd) {
            finish()
        }
        return event.pixelDelta.x === 0 && event.pixelDelta.y === 0 && event.angleDelta.x === 0 && event.angleDelta.y === 0
    }
    function sample(dx, dy) {
        flickable.cancelFlick()
        const now = Date.now()
        samples = samples.filter(s => now - s.t < 100).concat([{ t: now, dx: dx, dy: dy }])
        endTimer.restart()
    }
    function stop() {
        endTimer.stop()
        samples = []
        flickable.cancelFlick()
    }
    Connections {
        target: fling.app
        ignoreUnknownSignals: true
        function onTouchpadHeld() { fling.stop() }
    }

    /// The swipe ended: glide on with its speed.
    function finish() {
        endTimer.stop()
        const s = fling.samples
        fling.samples = []
        if (!fling.active || s.length < 2) return
        // Steps arrive every few milliseconds; synthetic ones may come at
        // once, so assume at least 8 ms between them.
        const span = Math.max(s[s.length - 1].t - s[0].t, 8 * (s.length - 1)) / 1000
        let dx = 0, dy = 0
        for (const p of s) { dx += p.dx; dy += p.dy }
        const vx = dx / span * fling.boost, vy = dy / span * fling.boost
        if (Math.abs(vx) < 300 && Math.abs(vy) < 300) return
        fling.count += 1
        // A higher limit only for this fling: mouse wheels stay as they are.
        const f = fling.flickable
        const limit = f.maximumFlickVelocity
        f.maximumFlickVelocity = 8000
        f.flick(vx, vy)
        f.maximumFlickVelocity = limit
    }

    // No step for a moment: the fingers left the touchpad (without an end
    // phase, e.g. on X11).
    Timer {
        id: endTimer
        interval: 70
        onTriggered: fling.finish()
    }

    // Only watches: the Flickable below scrolls as always (blocking: false
    // passes every wheel event on).
    WheelHandler {
        enabled: fling.handleWheel && fling.active
        blocking: false
        target: null
        acceptedDevices: PointerDevice.AllDevices
        acceptedPointerTypes: PointerDevice.AllPointerTypes
        acceptedModifiers: Qt.NoModifier
        onWheel: (event) => {
            if (fling.phase(event)) return
            if (fling.isTouchpad(event)) fling.sample(event.pixelDelta.x, event.pixelDelta.y)
            // Any other step (e.g. the start of a new gesture) ends a fling.
            else if (fling.flickable.flicking) fling.stop()
        }
    }
}
