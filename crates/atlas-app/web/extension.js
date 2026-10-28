// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

// Bridge for the pages of extension views (docs/extensions.md, "Ansichten").
// A page includes it with
//
//   <script src="atlas://app/web/extension.js"></script>
//
// and then uses
//
//   const ctx = await atlas.ready      // {extension, project, file, settings, files, theme}
//   await atlas.call("readFile", { path })
//   atlas.open(path) · atlas.reveal(path) · atlas.message(text)
//
// Requests go to Atlas as console messages, answers come back through
// `atlas._reply`. Atlas checks every path against the extension's
// permissions. The theme arrives as CSS variables (--atlas-…) on <html>.
"use strict";
(() => {
    let next = 1;
    const pending = new Map();
    let started;
    const ready = new Promise((resolve) => { started = resolve; });
    const send = (id, method, params) => console.log("atlas-ext:" + JSON.stringify({ id, method, params: params || {} }));

    window.atlas = {
        /** Resolves with the context once Atlas has answered. */
        ready,
        /** One request; resolves with its result or rejects with an Error. */
        call(method, params) {
            const id = next++;
            return new Promise((resolve, reject) => {
                pending.set(id, { resolve, reject });
                send(id, method, params);
            });
        },
        /** Opens a file in an Atlas tab. */
        open(path) { return atlas.call("open", { path }); },
        /** Shows a file in the explorer of Atlas. */
        reveal(path) { return atlas.call("reveal", { path }); },
        /** A short message at the bottom of the window. */
        message(text) { return atlas.call("message", { text }); },

        _reply(id, ok, value) {
            const p = pending.get(id);
            if (!p) return;
            pending.delete(id);
            if (ok) p.resolve(value);
            else p.reject(new Error(value));
        },
        _start(context) {
            const root = document.documentElement;
            for (const [name, color] of Object.entries(context.theme || {})) root.style.setProperty("--atlas-" + name, color);
            started(context);
        },
    };
})();
