// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtWebEngine
import Atlas
import AtlasStyle

// Scrollbars in Atlas' colours for pages that bring none (extension views,
// HTML files): Chromium otherwise draws its light default ones, also in the
// dark scheme. Thin like the preview's (web/preview.css). The rules go first
// into the page, so a page's own scrollbar styles still win. Follows the
// colour scheme. Place it inside the view.
QtObject {
    id: style
    /// The WebEngineView (set by the parent).
    required property var view

    readonly property string css:
        "::-webkit-scrollbar{width:10px;height:10px}"
        + "::-webkit-scrollbar-thumb{background:" + Colors.scrollHandle + ";border-radius:5px;"
        + "border:2px solid transparent;background-clip:padding-box}"
        + "::-webkit-scrollbar-thumb:hover{background:" + Colors.scrollHandleHover + ";border:2px solid transparent;background-clip:padding-box}"
        + "::-webkit-scrollbar-thumb:active{background:" + Colors.scrollHandlePressed + ";border:2px solid transparent;background-clip:padding-box}"
        + "::-webkit-scrollbar-track,::-webkit-scrollbar-corner{background:transparent}"
    /// Puts (or updates) the rules at the start of the page's head.
    readonly property string source:
        "(function(){var s=document.getElementById('atlas-scrollbars');"
        + "if(!s){s=document.createElement('style');s.id='atlas-scrollbars';"
        + "var h=document.head||document.documentElement;h.insertBefore(s,h.firstChild)}"
        + "s.textContent=" + JSON.stringify(css) + "})()"

    function install() {
        if (!view) return
        for (const old of view.userScripts.find("atlas-scrollbars")) view.userScripts.remove(old)
        const script = WebEngine.script()
        script.name = "atlas-scrollbars"
        script.sourceCode = source
        script.injectionPoint = WebEngineScript.DocumentReady
        script.worldId = WebEngineScript.MainWorld
        script.runsOnSubFrames = true
        view.userScripts.insert(script)
        // The page shown now (a new scheme while it is open).
        view.runJavaScript(source)
    }
    Component.onCompleted: install()
    onSourceChanged: install()
}
