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
    // The PDF export prints on white paper: diagrams in light colours then.
    let printMode = false;
    // The light design of Atlas (see `setLight`).
    let light = false;
    // Diagrams in the colours of the page (navy design), see preview.css.
    const DARK_DIAGRAMS = {
        theme: "base",
        themeVariables: {
            darkMode: true, background: "transparent", fontFamily: "system-ui, Inter, Noto Sans, sans-serif",
            primaryColor: "#1b2a44", primaryTextColor: "#eef3fb", primaryBorderColor: "#3d8bfd",
            secondaryColor: "#22304a", secondaryTextColor: "#eef3fb", secondaryBorderColor: "#5aa2ff",
            tertiaryColor: "#152036", tertiaryTextColor: "#c5d0e0", tertiaryBorderColor: "#2c3b58",
            lineColor: "#8a97ad", textColor: "#c5d0e0", mainBkg: "#1b2a44", nodeBorder: "#3d8bfd",
            clusterBkg: "#152036", clusterBorder: "#2c3b58", edgeLabelBackground: "#0f1726", titleColor: "#eef3fb",
            noteBkgColor: "#2b2a1e", noteTextColor: "#f3e3b0", noteBorderColor: "#e0b33a",
            actorBkg: "#1b2a44", actorBorder: "#3d8bfd", actorTextColor: "#eef3fb", signalColor: "#c5d0e0", signalTextColor: "#c5d0e0",
            labelBoxBkgColor: "#1b2a44", labelTextColor: "#eef3fb", sequenceNumberColor: "#0f1726",
            pie1: "#3d8bfd", pie2: "#7fdcaa", pie3: "#c9a8ff", pie4: "#f3cf74", pie5: "#ff9ca1", pieTitleTextColor: "#eef3fb",
            pieSectionTextColor: "#0f1726", pieLegendTextColor: "#c5d0e0", pieStrokeColor: "#0f1726",
            gridColor: "#2c3b58", sectionBkgColor: "#152036", altSectionBkgColor: "#0f1726", taskBkgColor: "#1b2a44",
            taskBorderColor: "#3d8bfd", taskTextColor: "#eef3fb", taskTextOutsideColor: "#c5d0e0", todayLineColor: "#f2575f",
        },
    };

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

    function initMermaid() {
        mermaid.initialize(Object.assign({ startOnLoad: false, securityLevel: "strict" },
            printMode || light ? { theme: "default" } : DARK_DIAGRAMS));
    }

    function renderMermaid(root) {
        const blocks = root.querySelectorAll("pre.mermaid");
        if (!blocks.length) return;
        if (!mermaidLoading) {
            mermaidLoading = new Promise((resolve) => {
                const s = document.createElement("script");
                s.src = "vendor/mermaid/mermaid.min.js";
                s.onload = () => {
                    initMermaid();
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

    // Code blocks: a copy button, and for shell code a button that runs it
    // in Atlas' terminal (in the folder of the note).
    const SHELL = /^(bash|sh|shell|zsh|console|shellsession)$/;
    function codeTools(root) {
        root.querySelectorAll("pre").forEach((pre) => {
            if (pre.classList.contains("mermaid") || pre.parentElement.classList.contains("code-box")) return;
            const code = pre.querySelector("code");
            const lang = code ? (code.className.match(/language-(\S+)/) || [])[1] || "" : "";
            const box = document.createElement("div");
            box.className = "code-box";
            pre.parentNode.insertBefore(box, pre);
            box.appendChild(pre);
            const tools = document.createElement("div");
            tools.className = "code-tools";
            const text = () => pre.innerText.replace(/\n$/, "");
            const button = (label, title, action) => {
                const b = document.createElement("button");
                b.textContent = label;
                b.title = title;
                b.addEventListener("click", (ev) => { ev.stopPropagation(); action(b); });
                tools.appendChild(b);
            };
            if (SHELL.test(lang)) {
                button("▶ Ausführen", "Im Terminal von Atlas ausführen (Ordner der Notiz)", () => {
                    let src = text();
                    // Console transcripts: only the commands after the prompt.
                    if (lang === "console" || lang === "shellsession")
                        src = src.split("\n").filter((l) => /^\$ /.test(l)).map((l) => l.slice(2)).join("\n");
                    post("run", { code: src });
                });
            }
            button("Kopieren", "Code kopieren", (b) => {
                post("copy", { text: text() });
                b.textContent = "✓ Kopiert";
                setTimeout(() => { b.textContent = "Kopieren"; }, 1200);
            });
            box.appendChild(tools);
        });
    }

    // Headings fold their section (until the next heading of the same or a
    // higher level); folded ones stay folded while the note is edited.
    const folded = new Set();
    function foldable(root) {
        root.querySelectorAll("h1, h2, h3, h4, h5, h6").forEach((h) => {
            const toggle = document.createElement("span");
            toggle.className = "fold";
            toggle.title = "Abschnitt ein-/ausklappen";
            h.insertBefore(toggle, h.firstChild);
            toggle.addEventListener("click", (ev) => {
                ev.stopPropagation();
                if (folded.has(h.id)) folded.delete(h.id); else folded.add(h.id);
                applyFolds(root);
            });
        });
        applyFolds(root);
    }
    function applyFolds(root) {
        let hideBelow = 0; // level of the folded heading in charge, 0 = none
        for (const el of Array.from(root.children)) {
            const m = /^H([1-6])$/.exec(el.tagName);
            if (m) {
                const level = +m[1];
                if (hideBelow && level <= hideBelow) hideBelow = 0;
                el.classList.toggle("hidden-by-fold", hideBelow > 0);
                const isFolded = !hideBelow && folded.has(el.id);
                el.classList.toggle("folded", isFolded);
                if (isFolded) hideBelow = level;
            } else {
                el.classList.toggle("hidden-by-fold", hideBelow > 0);
            }
        }
    }

    // Pictures: a button enlarges them to the size of the view (click or
    // Esc closes it again).
    function zoomablePictures(root) {
        root.querySelectorAll("img").forEach((img) => {
            if (img.closest(".img-box") || img.closest("a")) return;
            const box = document.createElement("span");
            box.className = "img-box";
            img.parentNode.insertBefore(box, img);
            box.appendChild(img);
            const b = document.createElement("button");
            b.className = "img-zoom";
            b.title = "Vergrößern";
            b.textContent = "⤢";
            b.addEventListener("click", (ev) => { ev.stopPropagation(); showPicture(img); });
            box.appendChild(b);
            img.addEventListener("dblclick", () => showPicture(img));
        });
    }
    // Atlas shows the picture in a window of its own.
    function showPicture(img) {
        post("image", { src: img.getAttribute("src") || "", alt: img.alt || "" });
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

    // The position is reported once per frame: as scrolling by the user
    // (the editor follows), or `own` for movements made for the editor and
    // changes of the page height (QML only keeps track of where it is).
    let scrollPending = false;
    let scrollOwn = true;
    function report(own) {
        scrollOwn = scrollOwn && own;
        if (scrollPending) return;
        scrollPending = true;
        requestAnimationFrame(() => {
            scrollPending = false;
            // The middle of the view is where content lines up with the editor.
            const mid = Math.round(window.innerHeight / 2);
            post("scroll", { line: topLine(), exact: exactLineAt(mid), at: mid, y: window.scrollY,
                             height: document.documentElement.scrollHeight, own: scrollOwn });
            scrollOwn = true;
        });
    }
    window.addEventListener("scroll", () => {
        let own = !reportScroll;
        if (reportScroll) {
            own = ownScroll >= 0 && Math.abs(window.scrollY - ownScroll) <= 1;
            ownScroll = -1;
        }
        report(own);
    });
    new ResizeObserver(() => report(true)).observe(document.documentElement);

    function scrollToAnchor(slug) {
        const el = document.getElementById(slug);
        if (el) el.scrollIntoView();
    }

    /// Marks every occurrence of `term` (a hit of the project search) and
    /// shows the one at or after source line `line`; the number of marks.
    function markHits(term, caseSensitive, line) {
        const main = document.getElementById("content");
        for (const old of main.querySelectorAll("mark.search-hit")) old.replaceWith(...old.childNodes);
        main.normalize();
        if (!term) return 0;
        const fold = (t) => caseSensitive ? t : t.toLowerCase();
        const needle = fold(term);
        const walker = document.createTreeWalker(main, NodeFilter.SHOW_TEXT, {
            acceptNode(node) {
                return node.parentElement.closest("script, style, .katex, svg, .code-tools")
                    ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT;
            },
        });
        const nodes = [];
        while (walker.nextNode()) nodes.push(walker.currentNode);
        const marks = [];
        for (let node of nodes) {
            let at = fold(node.data).indexOf(needle);
            while (at >= 0) {
                const hit = node.splitText(at);
                node = hit.splitText(needle.length);
                const mark = document.createElement("mark");
                mark.className = "search-hit";
                hit.replaceWith(mark);
                mark.appendChild(hit);
                marks.push(mark);
                at = fold(node.data).indexOf(needle);
            }
        }
        if (!marks.length) return 0;
        let marker = null;
        for (const m of markers()) {
            if (+m.dataset.line > line) break;
            marker = m;
        }
        const current = marks.find((m) => !marker
            || (marker.compareDocumentPosition(m) & Node.DOCUMENT_POSITION_FOLLOWING)) || marks[0];
        current.classList.add("current");
        reportScroll = false;
        current.scrollIntoView({ block: "center" });
        setTimeout(() => { reportScroll = true; }, 120);
        return marks.length;
    }

    return {
        markHits,
        set(html) {
            const y = window.scrollY;
            const main = document.getElementById("content");
            main.innerHTML = html;
            renderMath(main);
            highlight(main);
            codeTools(main);
            zoomablePictures(main);
            foldable(main);
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
        /// The PDF export: light diagrams (call before `set`).
        setPrintMode(on) { printMode = !!on; },
        /// Light colours for the page and later diagrams (call before `set`).
        setLight(on) {
            if (light === !!on) return;
            light = !!on;
            document.documentElement.classList.toggle("light", light);
            if (window.mermaid) initMermaid();
        },
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
