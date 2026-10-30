// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

pragma Singleton
import QtQuick

// Timing of opening and switching files, for finding where time goes:
// with ATLAS_TIMING=1 in the environment, each step is logged as
// "[timing] +<ms since the click> <step>", and slow calls with how long
// they took. Off, every call returns at once.
QtObject {
    id: timing
    /// Set by Main.qml from AppController.timing.
    property bool on: false
    /// What is being timed ("" if nothing) and since when (ms).
    property string what: ""
    property real since: 0
    /// Frames still logged after the switch.
    property int frames: 0
    /// A file is being opened (its tab switch is part of it).
    property bool opening: false

    function now() { return on ? Date.now() : 0 }
    /// A file is opened or a tab switched to.
    function start(label) {
        if (!on) return
        what = label
        since = Date.now()
        frames = 2
        console.log("[timing] ---- " + label)
    }
    /// A step of the current switch.
    function mark(step) {
        if (!on || !what) return
        console.log("[timing] +" + (Date.now() - since) + " ms  " + step)
    }
    /// How long a call took that started at `t` (from now()).
    function took(step, t) {
        if (!on || !t) return
        console.log("[timing]   " + step + ": " + (Date.now() - t) + " ms")
    }
    /// A frame was shown (Main.qml, frameSwapped).
    function frame() {
        if (!on || !what || frames <= 0) return
        mark(frames === 2 ? "erstes Bild" : "zweites Bild")
        frames -= 1
    }
}
