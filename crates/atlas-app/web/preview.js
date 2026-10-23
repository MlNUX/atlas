// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

// Markdown preview page. Atlas renders the HTML in Rust and passes it in via
// `atlas.set()`; this page only adds math (KaTeX), diagrams (Mermaid, loaded
// on first use) and code highlighting. Clicks on links and the scroll
// position are reported to QML through console messages, as are clicks on
// task list checkboxes.
"use strict";

window.atlas = (function () {
    let mermaidLoading = null;
    let reportScroll = true;

    function post(kind, data) { console.log("atlas:" + kind + ":" + JSON.stringify(data)); }

    function renderMath(root) {
        root.querySelectorAll("span.math[data-tex]").forEach((el) => {
            try {
                // strict "ignore": KaTeX renders what LaTeX would reject (e.g. an
                // array with fewer columns than given) without a console
                // warning per formula and render, which ended up in the log.
                katex.render(el.dataset.tex, el, {
                    displayMode: el.classList.contains("display"),
                    throwOnError: false,
                    strict: "ignore",
                });
            } catch (e) {
                el.textContent = el.dataset.tex;
                el.classList.add("math-error");
            }
        });
    }

    function renderMermaid(root) {
        const blocks = root.querySelectorAll("pre.mermaid");
        if (!blocks.length) return;
        if (!mermaidLoading) {
            mermaidLoading = new Promise((resolve) => {
                const s = document.createElement("script");
                s.src = "vendor/mermaid/mermaid.min.js";
                s.onload = () => {
                    mermaid.initialize({ startOnLoad: false, theme: "default", securityLevel: "strict" });
                    resolve();
                };
                document.head.appendChild(s);
            });
        }
        return mermaidLoading.then(() => mermaid.run({ nodes: blocks, suppressErrors: true }));
    }

    // Set while diagrams and fonts are still being drawn (the PDF export
    // waits for it).
    let busy = false;

    function highlight(root) {
        root.querySelectorAll("pre code[class*=language-]").forEach((el) => {
            const lang = el.className.replace(/.*language-(\S+).*/, "$1");
            if (hljs.getLanguage(lang)) hljs.highlightElement(el);
        });
    }

    function markers() { return Array.from(document.querySelectorAll("a.line-marker")); }

    // Source line at the top edge of the view, with a fraction between two
    // blocks (scroll sync with the editor, #7). A block counts from the top
    // of its box, not from the marker before it: the marker sits above the
    // block's margin, which put the preview's text lower than the editor's.
    function anchorTop(marker) {
        const block = marker.nextElementSibling;
        return (block && !block.classList.contains("line-marker") ? block : marker).getBoundingClientRect().top;
    }
    /// Source line (fractional) at height `y` of the view.
    function exactLineAt(y) {
        const list = markers();
        if (!list.length) return 1;
        let i = 0;
        while (i + 1 < list.length && anchorTop(list[i + 1]) <= y) i++;
        const a = list[i], b = list[i + 1];
        const la = +a.dataset.line, ya = anchorTop(a);
        if (ya > y || !b) return la;
        const lb = +b.dataset.line, yb = anchorTop(b);
        return yb > ya ? la + (lb - la) * Math.min(1, (y - ya) / (yb - ya)) : la;
    }
    function exactTopLine() { return exactLineAt(0); }
    function topLine() { return Math.floor(exactTopLine()); }
    /// Page offset that puts source line `line` (fractional) at the top edge.
    function offsetOf(line) {
        const list = markers();
        if (!list.length) return 0;
        let i = 0;
        while (i + 1 < list.length && +list[i + 1].dataset.line <= line) i++;
        const a = list[i], b = list[i + 1];
        const la = +a.dataset.line, ya = anchorTop(a) + window.scrollY;
        if (!b || line <= la) return ya;
        const lb = +b.dataset.line, yb = anchorTop(b) + window.scrollY;
        return ya + (yb - ya) * (line - la) / (lb - la);
    }

    // Following the editor: set in step with it every frame (the editor
    // already scrolls smoothly – a second smoothing here lagged and caught
    // up in jerks). Movements made for the editor are not reported back.
    let ownScroll = -1;
    /// Puts source line `line` at height `at` of the view (default: top).
    function follow(line, at) {
        const max = document.documentElement.scrollHeight - window.innerHeight;
        const y = Math.max(0, Math.min(max, offsetOf(line) - (at || 0)));
        if (Math.abs(y - window.scrollY) < 0.5) return;
        ownScroll = y;
        window.scrollTo(0, y);
    }

    document.addEventListener("click", (ev) => {
        const a = ev.target.closest("a[href]");
        if (!a) return;
        ev.preventDefault();
        const d = a.dataset;
        if (d.atlasAnchor && !d.atlasPath) { scrollToAnchor(d.atlasAnchor); return; }
        post("link", {
            external: d.atlasExternal || "",
            path: d.atlasPath || "",
            line: +(d.atlasLine || 0),
            page: +(d.atlasPage || 0),
            anchor: d.atlasAnchor || "",
            annot: d.atlasAnnot || "",
            broken: !!d.broken,
            newTab: ev.ctrlKey || ev.button === 1,
        });
    });
    // Task list checkboxes: QML toggles `[ ]`/`[x]` on that line of the
    // source; the preview shows the change at once.
    document.addEventListener("change", (ev) => {
        const box = ev.target;
        if (box.matches && box.matches("input.task")) post("task", { line: +box.dataset.line, checked: box.checked });
    });
    document.addEventListener("auxclick", (ev) => { if (ev.button === 1) ev.target.dispatchEvent(new MouseEvent("click", { bubbles: true, ctrlKey: true })); });

    // Scrolling by the user is reported once per frame (the editor follows);
    // movements made for the editor are not.
    let scrollPending = false;
    window.addEventListener("scroll", () => {
        if (!reportScroll) return;
        if (ownScroll >= 0 && Math.abs(window.scrollY - ownScroll) <= 1) { ownScroll = -1; return; }
        ownScroll = -1;
        if (scrollPending) return;
        scrollPending = true;
        requestAnimationFrame(() => {
            scrollPending = false;
            // The middle of the view is where content lines up with the editor.
            const mid = Math.round(window.innerHeight / 2);
            post("scroll", { line: topLine(), exact: exactLineAt(mid), at: mid, y: window.scrollY });
        });
    });

    function scrollToAnchor(slug) {
        const el = document.getElementById(slug);
        if (el) el.scrollIntoView();
    }

    return {
        set(html) {
            const y = window.scrollY;
            const main = document.getElementById("content");
            main.innerHTML = html;
            renderMath(main);
            highlight(main);
            busy = true;
            Promise.resolve(renderMermaid(main))
                .catch(() => {})
                .then(() => document.fonts.ready)
                .then(() => { busy = false; });
            // Restoring the position is no scrolling by the user (it would
            // drag the editor along in the split view).
            ownScroll = y;
            window.scrollTo(0, y);
        },
        scrollToLine(line) {
            let target = null;
            for (const m of markers()) {
                if (+m.dataset.line > line) break;
                target = m;
            }
            reportScroll = false;
            window.scrollTo(0, target ? target.getBoundingClientRect().top + window.scrollY - 8 : 0);
            setTimeout(() => { reportScroll = true; }, 120);
        },
        scrollToY(y) { window.scrollTo(0, y); },
        /// Follows the editor smoothly to source line `line` (fractional).
        follow,
        exactTopLine,
        exactLineAt,
        scrollToAnchor,
        busy() { return busy; },
        // For the PDF export: Qt prints without page margins and ignores
        // @page, so a table whose empty header and footer repeat on every
        // page makes the top and bottom margin (the sides are padding).
        preparePrint() {
            const main = document.getElementById("content");
            if (main.querySelector(":scope > table.print-frame")) return;
            const frame = document.createElement("table");
            frame.className = "print-frame";
            frame.innerHTML = "<thead><tr><td><div class='page-space'></div></td></tr></thead>"
                + "<tbody><tr><td class='print-body'></td></tr></tbody>"
                + "<tfoot><tr><td><div class='page-space'></div></td></tr></tfoot>";
            const body = frame.querySelector(".print-body");
            while (main.firstChild) body.appendChild(main.firstChild);
            main.appendChild(frame);
        },
        find(text, back) { return window.find(text, false, !!back, true); },
    };
})();
