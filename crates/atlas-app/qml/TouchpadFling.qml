// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick

// Scrolling with touchpad and mouse wheel over a Flickable, meant to feel
// like a web browser:
//
// - Touchpad: the content follows the fingers exactly, also very slow
//   movements, applied once per frame. Qt on Wayland sends each step's
//   movement as pixelDelta rounded to whole pixels (0 for slow movements)
//   and as angleDelta = 12 × the exact movement – so a touchpad step of
//   exactly 10 px looks like a wheel notch (120) and must not be taken for
//   one (it made the view jump). Touchpad gestures come in scroll phases
//   (ScrollBegin … ScrollEnd), mouse wheels never do.
// - Momentum (#4): Wayland sends no kinetic scrolling; when the fingers
//   leave the touchpad (ScrollEnd) the view glides on with the speed of the
//   last moments, slowing down; touching again stops it.
// - Mouse wheel: each notch scrolls `wheelStep` pixels, animated over a
//   few frames instead of jumping.
//
// With `handleWheel` it takes the wheel events of the area it covers
// (anchors.fill the Flickable); a view with its own wheel handling can
// report touchpad steps with sample() instead.
Item {
    id: fling
    required property Flickable flickable
    /// The setting (Einstellungen → Dateien): momentum after swipes.
    property bool active: true
    /// Scroll here; false: the view handles the wheel and calls sample().
    property bool handleWheel: true
    /// Pixels per mouse wheel notch.
    property real wheelStep: 100
    /// Content pixels per touchpad pixel.
    property real touchpadScale: 1.0
    /// Moves the content by (dx, dy) pixels (down/right positive); the
    /// default keeps the Flickable within its bounds.
    property var scroller: null
    /// How often a fling was started (for the smoke test).
    property int count: 0
    /// AppController: its touchpadHeld signal (fingers put down, Wayland
    /// hold gesture) stops the momentum.
    property var app: null
    /// Momentum relative to the swipe speed.
    readonly property real boost: 1.6

    /// Recent touchpad steps: {t, dx, dy} in pixels (content moves by -d).
    property var samples: []
    /// Movement waiting for the next frame: touchpad (at once) and the rest
    /// of wheel notches (eased in).
    property real pendingX: 0
    property real pendingY: 0
    property real wheelX: 0
    property real wheelY: 0
    /// A touchpad gesture is going on (ScrollBegin seen, no ScrollEnd yet).
    property bool inGesture: false

    /// A touchpad step: in a scroll phase, or with a pixel delta (mouse
    /// wheels have neither).
    function isTouchpad(event) {
        return event.phase === Qt.ScrollBegin || event.phase === Qt.ScrollUpdate || event.phase === Qt.ScrollEnd
            || event.pixelDelta.x !== 0 || event.pixelDelta.y !== 0
    }
    /// Movement of a touchpad step in pixels (positive: content moves
    /// down/right, i.e. towards the start): the exact value behind
    /// angleDelta if there is one, else the pixel delta.
    function touchpadDelta(event) {
        const exact = (angle, pixel) => angle !== 0 ? angle / 12 : pixel
        return Qt.point(exact(event.angleDelta.x, event.pixelDelta.x) * touchpadScale,
                        exact(event.angleDelta.y, event.pixelDelta.y) * touchpadScale)
    }

    /// Handles the phase of a touchpad gesture: lifting the fingers
    /// (ScrollEnd, Wayland's axis stop) starts the momentum at once; a new
    /// gesture stops it. True if the event was only that (no movement).
    function phase(event) {
        if (event.phase === Qt.ScrollBegin) {
            inGesture = true
            endTimer.stop()
            samples = []
            wheelX = wheelY = 0
            flickable.cancelFlick()
        } else if (event.phase === Qt.ScrollEnd) {
            inGesture = false
            flush()
            finish()
        }
        return event.pixelDelta.x === 0 && event.pixelDelta.y === 0 && event.angleDelta.x === 0 && event.angleDelta.y === 0
    }
    function sample(dx, dy) {
        flickable.cancelFlick()
        const now = Date.now()
        samples = samples.filter(s => now - s.t < 100).concat([{ t: now, dx: dx, dy: dy }])
        // Without phases (X11) the end is a pause; with them ScrollEnd, and a
        // longer pause in case a compositor never sends it.
        endTimer.interval = inGesture ? 250 : 70
        endTimer.restart()
    }
    function stop() {
        endTimer.stop()
        samples = []
        wheelX = wheelY = 0
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

    /// Moves the content (down/right positive), within bounds.
    function move(dx, dy) {
        if (scroller) { scroller(dx, dy); return }
        const f = flickable
        if (dy) {
            const minY = f.originY - f.topMargin
            const maxY = Math.max(minY, f.originY + f.contentHeight + f.bottomMargin - f.height)
            f.contentY = Math.max(minY, Math.min(maxY, f.contentY + dy))
        }
        if (dx) {
            const minX = f.originX - f.leftMargin
            const maxX = Math.max(minX, f.originX + f.contentWidth + f.rightMargin - f.width)
            f.contentX = Math.max(minX, Math.min(maxX, f.contentX + dx))
        }
    }
    /// Applies the touchpad movement waiting for its frame.
    function flush() {
        const x = pendingX, y = pendingY
        pendingX = pendingY = 0
        if (x || y) move(-x, -y)
    }
    /// Applies all waiting movement at once (wheel animation included).
    function settle() {
        flush()
        const x = wheelX, y = wheelY
        wheelX = wheelY = 0
        if (x || y) move(-x, -y)
    }
    /// One wheel event (touchpad or mouse) as this view scrolls.
    function scroll(event) {
        if (phase(event)) return
        if (isTouchpad(event)) {
            const d = touchpadDelta(event)
            wheelX = wheelY = 0
            sample(d.x, d.y)
            pendingX += d.x
            pendingY += d.y
        } else {
            // A notch during a fling ends it; notches add up.
            if (flickable.flicking) stop()
            wheelX += event.angleDelta.x / 120 * wheelStep
            wheelY += event.angleDelta.y / 120 * wheelStep
        }
    }

    // Once per frame: the touchpad movement at once, wheel notches eased
    // in (most of a notch within about 120 ms, like browsers do).
    FrameAnimation {
        running: fling.pendingX !== 0 || fling.pendingY !== 0 || fling.wheelX !== 0 || fling.wheelY !== 0
        onTriggered: {
            fling.flush()
            if (fling.wheelX === 0 && fling.wheelY === 0) return
            const k = 1 - Math.exp(-Math.max(frameTime, 0.004) / 0.045)
            let x = fling.wheelX * k, y = fling.wheelY * k
            // The last bit at once (no endless crawl).
            if (Math.abs(fling.wheelX - x) < 0.5) x = fling.wheelX
            if (Math.abs(fling.wheelY - y) < 0.5) y = fling.wheelY
            fling.wheelX -= x
            fling.wheelY -= y
            fling.move(-x, -y)
        }
    }

    WheelHandler {
        enabled: fling.handleWheel
        target: null
        acceptedDevices: PointerDevice.AllDevices
        acceptedPointerTypes: PointerDevice.AllPointerTypes
        acceptedModifiers: Qt.NoModifier
        onWheel: (event) => {
            fling.scroll(event)
            event.accepted = true
        }
    }
}
