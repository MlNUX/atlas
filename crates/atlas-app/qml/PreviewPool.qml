// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

pragma Singleton
import QtQuick
import QtWebEngine
import Atlas

// A Markdown preview loaded ahead: once a preview was needed, one Chromium
// view with preview.html waits (hidden, in `host`) for the next note, which
// takes it and renders at once instead of starting a browser process and
// loading the page first. A new spare is prepared a moment after each take.
QtObject {
    id: pool
    /// Hidden item in the main window holding the spare (set by Main.qml).
    property Item host: null
    /// The waiting view (null until a preview was needed once).
    property var spare: null
    /// How many views were taken loaded already (for the smoke test).
    property int warmTakes: 0

    readonly property Component viewComponent: Component {
        WebEngineView {
            /// preview.html is loaded and `atlas` is there.
            property bool ready: false
            url: "atlas://app/web/preview.html"
            backgroundColor: Theme.editor
            settings.localContentCanAccessFileUrls: true
            settings.localContentCanAccessRemoteUrls: false
            settings.javascriptCanOpenWindows: false
            settings.focusOnNavigationEnabled: false
            onLoadingChanged: (request) => { if (request.status === WebEngineLoadingInfo.LoadSucceededStatus) ready = true }
        }
    }
    // The next spare waits until the taken view has rendered its note.
    readonly property Timer refill: Timer {
        interval: 800
        onTriggered: pool.prepare()
    }

    function prepare() {
        if (spare || !host) return
        spare = viewComponent.createObject(host, { width: Math.max(host.width, 400), height: Math.max(host.height, 300) })
    }
    /// A preview view for `parent` (loaded or still loading; its `ready`
    /// tells). The caller destroys it when done.
    function take(parent) {
        let view = spare
        spare = null
        if (view && view.ready) warmTakes += 1
        if (!view) view = viewComponent.createObject(parent)
        else view.parent = parent
        refill.restart()
        return view
    }
}
