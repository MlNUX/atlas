// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Shapes
import Atlas

// Pomodoro timer for learning, fixed to the bottom edge of the context bar
// (on every tab): a ring with the remaining time, the phase, the rounds and
// round buttons. Lengths come from the settings (`[context]`).
Rectangle {
    id: timer
    required property AppController app
    /// FileActions, for the message at the end of a phase.
    property var actions: null

    readonly property var settings: { try { return JSON.parse(app.contextModules || "{}") } catch (e) { return {} } }

    /// "work", "break" or "long".
    property string phase: "work"
    property bool running: false
    /// Remaining seconds; while running counted from the end time (exact
    /// even when the interface stalls).
    property int remaining: minutesOf("work") * 60
    property real endsAt: 0
    /// Finished work rounds; every fourth is followed by a long break.
    property int rounds: 0

    function minutesOf(p) {
        return p === "work" ? (settings.pomodoro_work || 25) : p === "break" ? (settings.pomodoro_break || 5) : (settings.pomodoro_long_break || 15)
    }
    function phaseText(p) { return p === "work" ? qsTr("Lernen") : p === "break" ? qsTr("Kurze Pause") : qsTr("Lange Pause") }
    function startPause() {
        if (running) {
            remaining = Math.max(0, Math.round((endsAt - Date.now()) / 1000))
            running = false
        } else {
            endsAt = Date.now() + remaining * 1000
            running = true
        }
    }
    function resetTimer() {
        running = false
        remaining = minutesOf(phase) * 60
    }
    /// Ends the phase (also by hand: „Überspringen“) and prepares the next.
    function nextPhase(finished) {
        if (phase === "work") {
            if (finished) rounds += 1
            phase = finished && rounds % 4 === 0 ? "long" : "break"
        } else {
            phase = "work"
        }
        running = false
        remaining = minutesOf(phase) * 60
        if (finished && actions)
            actions.showMessage(phase === "work" ? qsTr("Pause vorbei – weiter geht's") : qsTr("Pomodoro geschafft – Zeit für eine %1").arg(phaseText(phase)))
    }
    // A new length in the settings applies at once while the timer stands.
    readonly property int phaseMinutes: minutesOf(phase)
    onPhaseMinutesChanged: if (!running) remaining = phaseMinutes * 60

    Timer {
        interval: 250
        running: timer.running
        repeat: true
        onTriggered: {
            const left = Math.max(0, Math.round((timer.endsAt - Date.now()) / 1000))
            timer.remaining = left
            if (left === 0) timer.nextPhase(true)
        }
    }

    function pad(n) { return (n < 10 ? "0" : "") + n }
    readonly property string clock: pad(Math.floor(remaining / 60)) + ":" + pad(remaining % 60)
    readonly property real progress: 1 - remaining / Math.max(1, phaseMinutes * 60)
    readonly property color tint: phase === "work" ? Theme.accent : Theme.ok
    /// Dots of the current set of four rounds.
    readonly property int dots: phase === "long" ? 4 : rounds % 4

    implicitHeight: 78
    radius: 12
    color: Qt.rgba(tint.r, tint.g, tint.b, running ? 0.12 : 0.07)
    border.color: Qt.rgba(tint.r, tint.g, tint.b, running ? 0.45 : 0.22)
    Behavior on color { ColorAnimation { duration: 200 } }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 10
        spacing: 12

        // Ring: remaining time of the phase.
        Item {
            Layout.preferredWidth: 56
            Layout.preferredHeight: 56
            Shape {
                anchors.fill: parent
                preferredRendererType: Shape.CurveRenderer
                ShapePath {
                    strokeColor: Qt.rgba(timer.tint.r, timer.tint.g, timer.tint.b, 0.18)
                    strokeWidth: 4
                    fillColor: "transparent"
                    PathAngleArc { centerX: 28; centerY: 28; radiusX: 25; radiusY: 25; startAngle: 0; sweepAngle: 360 }
                }
                ShapePath {
                    strokeColor: timer.progress > 0 ? timer.tint : "transparent"
                    strokeWidth: 4
                    fillColor: "transparent"
                    capStyle: ShapePath.RoundCap
                    PathAngleArc {
                        centerX: 28; centerY: 28; radiusX: 25; radiusY: 25
                        startAngle: -90
                        sweepAngle: 360 * Math.max(0.001, timer.progress)
                    }
                }
            }
            Icon {
                anchors.centerIn: parent
                name: timer.phase === "work" ? "focus" : "coffee"
                size: 20
                color: timer.tint
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 1
            Text {
                text: timer.clock
                color: Theme.textStrong
                font.pixelSize: 24
                font.weight: Font.DemiBold
                font.family: Theme.monoFont
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 6
                Text {
                    text: timer.phaseText(timer.phase) + (timer.running || timer.progress === 0 ? "" : qsTr(" · angehalten"))
                    color: timer.running ? timer.tint : Theme.textMuted
                    font.pixelSize: Theme.smallFontSize
                    elide: Text.ElideRight
                }
                Row {
                    Layout.fillWidth: true
                    spacing: 3
                    ToolTip.visible: dotsHover.hovered
                    ToolTip.text: qsTr("%1 von 4 Runden bis zur langen Pause").arg(timer.dots)
                    HoverHandler { id: dotsHover }
                    Repeater {
                        model: 4
                        Rectangle {
                            required property int index
                            anchors.verticalCenter: parent.verticalCenter
                            width: 6; height: 6; radius: 3
                            color: index < timer.dots ? timer.tint : Theme.border
                        }
                    }
                }
            }
        }

        // Main button: start/pause; small ones: reset and skip.
        Column {
            spacing: 4
            Row {
                spacing: 4
                RoundButton {
                    id: playButton
                    width: 36; height: 36
                    padding: 0
                    highlighted: true
                    onClicked: timer.startPause()
                    contentItem: Item { Icon { anchors.centerIn: parent; name: timer.running ? "pause" : "play"; size: 16; color: "white" } }
                    background: Rectangle { radius: 18; color: playButton.hovered ? Qt.lighter(timer.tint, 1.15) : timer.tint }
                    ToolTip.visible: hovered
                    ToolTip.text: timer.running ? qsTr("Anhalten") : qsTr("Starten")
                }
                Column {
                    spacing: 0
                    IconButton { iconName: "refresh"; iconSize: 13; tip: qsTr("Phase neu beginnen"); onClicked: timer.resetTimer() }
                    IconButton { iconName: "skip"; iconSize: 13; tip: qsTr("Überspringen"); onClicked: timer.nextPhase(false) }
                }
            }
        }
    }
}
