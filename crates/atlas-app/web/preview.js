// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

// Markdown preview page. Atlas renders the HTML in Rust and passes it in via
// `atlas.set()`; this page only adds math (KaTeX), diagrams (Mermaid, loaded
// on first use) and code highlighting. Clicks on links and the scroll
// position are reported to QML through console messages.
"use strict";

window.atlas = (function () {
    let mermaidLoading = null;
    let reportScroll = true;

    function post(kind, data) { console.log("atlas:" + kind + ":" + JSON.stringify(data)); }

    function renderMath(root) {
        root.querySelectorAll("span.math[data-tex]").forEach((el) => {
            try {
                katex.render(el.dataset.tex, el, { displayMode: el.classList.contains("display"), throwOnError: false });
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
        mermaidLoading.then(() => mermaid.run({ nodes: blocks, suppressErrors: true }));
    }

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
            broken: !!d.broken,
            newTab: ev.ctrlKey || ev.button === 1,
        });
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
            renderMermaid(main);
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
        find(text, back) { return window.find(text, false, !!back, true); },
    };
})();
