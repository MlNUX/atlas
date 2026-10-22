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

    function topLine() {
        let line = 1;
        for (const m of markers()) {
            if (m.getBoundingClientRect().top > 4) break;
            line = +m.dataset.line;
        }
        return line;
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

    let scrollTimer = 0;
    window.addEventListener("scroll", () => {
        if (!reportScroll) return;
        clearTimeout(scrollTimer);
        scrollTimer = setTimeout(() => post("scroll", { line: topLine(), y: window.scrollY }), 80);
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
