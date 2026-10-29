// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

// Jupyter-Notebook-Ansicht (siehe notebook.html). Ablauf:
//
// 1. Die Datei kommt über die Brücke (atlas.call("readFile")) und wird über
//    sie gespeichert (nbformat 4, Einrückung 1, sortierte Schlüssel).
// 2. Der Jupyter-Server läuft im Hintergrund (Befehl „server“ der
//    Erweiterung, endet mit Atlas) auf 127.0.0.1:<port>; sein Token liegt in
//    .atlas/cache/jupyter/token im Projekt.
// 3. Pro Notebook eine Sitzung (POST /api/sessions, beim Wiederöffnen
//    dieselbe); Zellen laufen über den WebSocket des Kernels, eine nach der
//    anderen.
// 4. Umgebungen (venv, Conda, Umgebungsdateien, requirements.txt) findet die
//    Auswahl oben rechts; was dafür auf dem Rechner laufen muss (Umgebung
//    bauen, ipykernel, Kernel anmelden), läuft in einem Terminal des
//    Jupyter-Servers, die Ausgabe erscheint im Aufgabenfeld.
//
// HTML aus Ausgaben und Markdown wird bereinigt; Skripte laufen nie.
"use strict";

const $ = (id) => document.getElementById(id);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const state = {
    ctx: null,
    file: "",
    nb: null,
    lang: "python",
    cells: [],
    selected: null,
    lastText: null,     // zuletzt gelesener oder geschriebener Dateiinhalt
    dirty: false,
    saveTimer: 0,
    base: "",
    token: "",
    online: false,      // Server antwortet
    session: null,      // {id, kernel: {id, name}}
    ws: null,
    clientId: uuid(),
    kernelState: "off",
    languageInfo: null, // aus kernel_info_reply
    queue: [],
    current: null,      // {cell, msgId, idle, reply, t0}
    pendingClear: new WeakMap(),
    specs: null,
    restarting: false,
    infoWaiters: new Map(),
    task: null,         // sichtbare Aufgabe ({cancel})
    found: null,        // gefundene Umgebungen (discover)
    deleteArmed: null,  // Zelle nach dem ersten „d“
};

// ------------------------------------------------------------------ Kleinkram

function uuid() {
    if (crypto.randomUUID) return crypto.randomUUID();
    const b = crypto.getRandomValues(new Uint8Array(16));
    return [...b].map((x) => x.toString(16).padStart(2, "0")).join("");
}
function cellId() { return uuid().replace(/-/g, "").slice(0, 8); }
function joined(value) { return Array.isArray(value) ? value.join("") : (value == null ? "" : String(value)); }
/// Text als Zeilenliste wie bei nbformat: jede Zeile mit "\n", außer der letzten.
function lines(text) { return text.match(/[^\n]*\n|[^\n]+$/g) || []; }
function escapeHtml(s) {
    return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}
function basename(p) { return String(p).replace(/\/+$/, "").split("/").pop(); }
function dirname(p) { const i = p.lastIndexOf("/"); return i > 0 ? p.slice(0, i) : "/"; }
function project() { return (state.ctx.project || "").replace(/\/+$/, ""); }
function relative(path) {
    const root = project();
    return root && path.startsWith(root + "/") ? path.slice(root.length + 1) : path;
}
/// Pfad zum Lesen: im Projekt relativ, sonst ~/… statt des persönlichen Ordners.
function shortPath(path) {
    const rel = relative(path);
    if (rel !== path) return rel;
    const home = (state.home || "").replace(/\/+$/, "");
    return home && path.startsWith(home + "/") ? "~" + path.slice(home.length) : path;
}
/// Text in einfachen Anführungszeichen für sh.
function sq(text) { return "'" + String(text).replace(/'/g, "'\\''") + "'"; }
function base64(text) {
    let binary = "";
    for (const b of new TextEncoder().encode(text)) binary += String.fromCharCode(b);
    return btoa(binary);
}
function slug(text) { return String(text).toLowerCase().replace(/[^a-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "") || "python"; }
function shortHash(text) {
    let h = 0x811c9dc5;
    for (const c of String(text)) h = Math.imul(h ^ c.codePointAt(0), 16777619) >>> 0;
    return h.toString(16).padStart(8, "0").slice(0, 6);
}
function duration(ms) {
    if (ms < 1000) return `${Math.round(ms)} ms`;
    if (ms < 60000) return `${(ms / 1000).toFixed(1).replace(".", ",")} s`;
    return `${Math.floor(ms / 60000)} min ${Math.round((ms % 60000) / 1000)} s`;
}
function el(tag, props, ...children) {
    const e = document.createElement(tag);
    for (const [k, v] of Object.entries(props || {})) {
        if (k === "on") for (const [ev, fn] of Object.entries(v)) e.addEventListener(ev, fn);
        else if (k === "class") e.className = v;
        else if (v !== undefined && v !== null && v !== false) e[k] = v;
    }
    e.append(...children.filter((c) => c !== null && c !== undefined && c !== false));
    return e;
}
function button(text, title, onClick, cls) {
    return el("button", { class: cls || "", title, textContent: text, on: { click: (e) => { e.stopPropagation(); onClick(e); } } });
}

// ---------------------------------------------------------- Hinweis, Status

function setSaved(kind, text) {
    const s = $("saved");
    s.className = "saved " + (kind || "");
    s.title = text || "";
}

/// Hinweis unter der Werkzeugleiste; `actions`: [{label, run, primary}].
function notice(text, opts = {}) {
    $("notice").hidden = !text;
    $("notice").classList.toggle("error", !!opts.error);
    $("notice-text").textContent = text || "";
    $("notice-actions").replaceChildren(...(opts.actions || []).map((a) => button(a.label, a.title || "", a.run, a.primary ? "primary" : "")));
    $("notice-log").hidden = !opts.log;
    $("notice-log").textContent = opts.log || "";
}

const KERNEL_LABEL = { idle: "bereit", busy: "rechnet", starting: "startet", restarting: "startet neu", dead: "beendet", off: "" };

function setKernelState(s) {
    state.kernelState = s;
    const connected = s !== "off" && s !== "dead";
    const env = $("env");
    env.className = "env " + s;
    let label = "Umgebung wählen";
    if (state.session && connected) {
        const spec = state.specs && state.specs.kernelspecs[state.session.kernel.name];
        label = (spec && spec.spec.display_name) || state.session.kernel.name;
        if (state.languageInfo && state.languageInfo.version) label += " · " + state.languageInfo.version.split(" ")[0];
    } else if (s === "starting") label = "Kernel startet …";
    $("env-label").textContent = label;
    env.title = "Umgebung (Kernel) wählen" + (KERNEL_LABEL[s] ? ` – ${KERNEL_LABEL[s]}` : "");
    $("run-all").disabled = !connected;
    $("restart").disabled = !connected;
    $("restart-menu").disabled = !connected;
    $("interrupt").disabled = !connected || (!state.current && s !== "busy");
}

// ----------------------------------------------------------------- Bereinigen

const ALLOWED_TAGS = new Set(("a abbr b blockquote br caption code col colgroup dd del details div dl dt em figcaption figure " +
    "h1 h2 h3 h4 h5 h6 hr i img input ins kbd li mark ol p pre q s samp small span strong sub summary sup table tbody td " +
    "tfoot th thead tr u ul").split(" "));
const DROP_WITH_CONTENT = new Set(["script", "style", "iframe", "object", "embed", "template", "noscript", "link", "meta", "form", "svg", "math"]);
const ALLOWED_ATTRS = new Set(["class", "title", "alt", "colspan", "rowspan", "align", "width", "height", "border", "href", "src", "start", "type", "checked", "disabled", "style", "id", "name"]);

function safeUrl(url, image) {
    const u = String(url).trim();
    if (image) return /^(data:image\/(png|jpe?g|gif|webp|svg\+xml)[;,]|atlas:\/\/|https?:\/\/)/i.test(u) || !/^[a-z][a-z0-9+.-]*:/i.test(u);
    return /^(https?:|mailto:|#)/i.test(u) || !/^[a-z][a-z0-9+.-]*:/i.test(u);
}

/// Bereinigt HTML: nur unschädliche Elemente und Attribute, keine Skripte,
/// keine Ereignis-Attribute, keine javascript:-Adressen.
function sanitize(html) {
    const doc = new DOMParser().parseFromString("<body>" + html, "text/html");
    const walk = (node) => {
        for (const child of [...node.childNodes]) {
            if (child.nodeType === Node.COMMENT_NODE) { child.remove(); continue; }
            if (child.nodeType !== Node.ELEMENT_NODE) continue;
            const tag = child.tagName.toLowerCase();
            if (DROP_WITH_CONTENT.has(tag)) { child.remove(); continue; }
            if (!ALLOWED_TAGS.has(tag)) {
                walk(child);
                child.replaceWith(...child.childNodes);
                continue;
            }
            for (const attr of [...child.attributes]) {
                const name = attr.name.toLowerCase();
                const keep = (ALLOWED_ATTRS.has(name) || name.startsWith("data-"))
                    && !(name === "href" && !safeUrl(attr.value, false))
                    && !(name === "src" && !safeUrl(attr.value, true))
                    && !(name === "style" && /url\s*\(|expression|@import/i.test(attr.value))
                    && !(tag === "input" && name === "type" && attr.value.toLowerCase() !== "checkbox");
                if (!keep) child.removeAttribute(attr.name);
            }
            if (tag === "input") { child.setAttribute("type", "checkbox"); child.setAttribute("disabled", ""); }
            if (tag === "a") child.setAttribute("rel", "noopener");
            walk(child);
        }
    };
    walk(doc.body);
    return doc.body.innerHTML;
}

/// Formeln (span.math[data-tex] von Atlas) mit KaTeX setzen.
function renderMath(root) {
    root.querySelectorAll("span.math[data-tex]").forEach((e) => {
        if (!window.katex) { e.textContent = e.dataset.tex; return; }
        try {
            katex.render(e.dataset.tex, e, { displayMode: e.classList.contains("display"), throwOnError: false });
        } catch (err) {
            e.textContent = e.dataset.tex;
        }
    });
}

async function markdownHtml(text, attachments) {
    let html;
    try {
        html = (await atlas.call("renderMarkdown", { text, path: state.file })).html;
    } catch (e) {
        html = "<pre>" + escapeHtml(text) + "</pre>";
    }
    const out = document.createElement("div");
    out.innerHTML = sanitize(html);
    // Bilder als Anhang der Zelle (![](attachment:bild.png)).
    if (attachments) {
        out.querySelectorAll("img").forEach((img) => {
            const m = /attachment:(.+)$/.exec(decodeURIComponent(img.getAttribute("src") || ""));
            const data = m && attachments[m[1]];
            if (!data) return;
            const mime = Object.keys(data).find((k) => k.startsWith("image/"));
            if (mime) img.src = mime === "image/svg+xml" ? "data:image/svg+xml;charset=utf-8," + encodeURIComponent(joined(data[mime]))
                : "data:" + mime + ";base64," + joined(data[mime]).replace(/\s/g, "");
        });
    }
    renderMath(out);
    // Links auf Dateien öffnen in Atlas, Webseiten im Browser.
    out.querySelectorAll("a[data-atlas-path]").forEach((a) => {
        a.addEventListener("click", (e) => { e.preventDefault(); atlas.open(a.dataset.atlasPath); });
    });
    return out;
}

// --------------------------------------------------------------------- ANSI

const ANSI_COLORS = ["#000000", "#c0392b", "#2e8b57", "#b7791f", "#2f6fdb", "#a23bc4", "#138d90", "#a0a0a0",
    "#666666", "#e74c3c", "#27ae60", "#d4a017", "#4a90e2", "#c158dc", "#1abc9c", "#ffffff"];

/// Text mit ANSI-Farben (Tracebacks von IPython, pip) als HTML.
function ansiHtml(text) {
    let out = "";
    let style = { fg: null, bg: null, bold: false };
    let open = false;
    const parts = String(text).split(/\x1b\[([0-9;]*)m/);
    for (let i = 0; i < parts.length; i++) {
        if (i % 2 === 0) {
            if (parts[i]) out += escapeHtml(parts[i].replace(/\x1b\[[0-9;?]*[A-Za-z]|\x1b\][^\x07]*\x07/g, ""));
            continue;
        }
        const codes = parts[i] === "" ? [0] : parts[i].split(";").map(Number);
        for (let j = 0; j < codes.length; j++) {
            const c = codes[j];
            if (c === 0) style = { fg: null, bg: null, bold: false };
            else if (c === 1) style.bold = true;
            else if (c === 22) style.bold = false;
            else if (c >= 30 && c <= 37) style.fg = ANSI_COLORS[c - 30];
            else if (c >= 90 && c <= 97) style.fg = ANSI_COLORS[c - 90 + 8];
            else if (c === 39) style.fg = null;
            else if (c >= 40 && c <= 47) style.bg = ANSI_COLORS[c - 40];
            else if (c >= 100 && c <= 107) style.bg = ANSI_COLORS[c - 100 + 8];
            else if (c === 49) style.bg = null;
            else if ((c === 38 || c === 48) && codes[j + 1] === 5) {
                const n = codes[j + 2];
                const color = n < 16 ? ANSI_COLORS[n] : n >= 232 ? `rgb(${(n - 232) * 10 + 8},${(n - 232) * 10 + 8},${(n - 232) * 10 + 8})`
                    : `rgb(${Math.floor((n - 16) / 36) * 51},${Math.floor(((n - 16) % 36) / 6) * 51},${((n - 16) % 6) * 51})`;
                if (c === 38) style.fg = color; else style.bg = color;
                j += 2;
            } else if ((c === 38 || c === 48) && codes[j + 1] === 2) {
                const color = `rgb(${codes[j + 2]},${codes[j + 3]},${codes[j + 4]})`;
                if (c === 38) style.fg = color; else style.bg = color;
                j += 4;
            }
        }
        if (open) { out += "</span>"; open = false; }
        if (style.fg || style.bg || style.bold) {
            const css = (style.fg ? `color:${style.fg};` : "") + (style.bg ? `background:${style.bg};` : "");
            out += `<span${style.bold ? ' class="ansi-bold"' : ""}${css ? ` style="${css}"` : ""}>`;
            open = true;
        }
    }
    if (open) out += "</span>";
    return out;
}

/// Wendet Wagenrücklauf und Rückschritt an wie ein Terminal (Fortschrittsbalken).
function terminalText(text) {
    return String(text).replace(/\r\n/g, "\n").split("\n").map((line) => {
        if (line.includes("\r")) {
            const parts = line.split("\r").filter((p, i, all) => p !== "" || i === all.length - 1);
            line = parts.length ? parts[parts.length - 1] : "";
        }
        while (/[^\x08]\x08/.test(line)) line = line.replace(/[^\x08]\x08/, "");
        return line;
    }).join("\n");
}

// ----------------------------------------------------------------- Ausgaben

const MIME_ORDER = ["text/html", "image/svg+xml", "image/png", "image/jpeg", "image/gif", "text/markdown", "text/latex", "application/json", "text/plain"];

function renderData(data, container) {
    const types = Object.keys(data || {});
    const mime = MIME_ORDER.find((m) => types.includes(m)) || types.find((t) => t.startsWith("text/"));
    if (!mime) { container.textContent = "(Ausgabe im Format " + types.join(", ") + " – hier nicht darstellbar)"; return; }
    const value = data[mime];
    if (mime === "text/html") {
        container.innerHTML = sanitize(joined(value));
        renderMath(container);
    } else if (mime === "image/svg+xml") {
        container.appendChild(el("img", { src: "data:image/svg+xml;charset=utf-8," + encodeURIComponent(joined(value)) }));
    } else if (mime.startsWith("image/")) {
        container.appendChild(el("img", { src: "data:" + mime + ";base64," + joined(value).replace(/\s/g, "") }));
    } else if (mime === "text/markdown") {
        const holder = el("div", { class: "md" });
        container.appendChild(holder);
        markdownHtml(joined(value)).then((e) => holder.replaceChildren(...e.childNodes));
    } else if (mime === "text/latex") {
        const tex = joined(value).trim().replace(/^\$\$?|\$\$?$/g, "");
        const holder = el("div");
        if (window.katex) katex.render(tex, holder, { displayMode: true, throwOnError: false });
        else holder.textContent = joined(value);
        container.appendChild(holder);
    } else if (mime === "application/json") {
        container.appendChild(el("pre", { textContent: JSON.stringify(value, null, 2) }));
    } else {
        container.appendChild(el("pre", { innerHTML: ansiHtml(terminalText(joined(value))) }));
    }
}

function outputElement(out) {
    const e = el("div", { class: "output" });
    if (out.output_type === "stream") {
        e.classList.add(out.name === "stderr" ? "stderr" : "stdout");
        e.appendChild(el("pre", { innerHTML: ansiHtml(terminalText(joined(out.text))) }));
    } else if (out.output_type === "error") {
        e.classList.add("error");
        const tb = (out.traceback || []).join("\n");
        e.appendChild(el("pre", { innerHTML: ansiHtml(tb || `${out.ename}: ${out.evalue}`) }));
    } else {
        renderData(out.data, e);
    }
    return e;
}

/// Lange Ausgaben eingeklappt, mit „Alles zeigen“.
function collapse(cell) {
    requestAnimationFrame(() => {
        cell.outputs.querySelectorAll(".more-output").forEach((b) => b.remove());
        for (const o of cell.outputs.querySelectorAll(".output")) {
            o.classList.remove("collapsed");
            if (cell.expanded || o.scrollHeight <= 440) continue;
            o.classList.add("collapsed");
            const lineCount = (o.textContent.match(/\n/g) || []).length;
            o.after(button(lineCount > 30 ? `Alles zeigen (${lineCount} Zeilen)` : "Alles zeigen", "Ausgabe ganz zeigen",
                () => { cell.expanded = true; collapse(cell); }, "link more-output"));
        }
    });
}

function renderOutputs(cell) {
    cell.outputs.replaceChildren(...(cell.model.outputs || []).map(outputElement));
    collapse(cell);
}

/// Eine neue Ausgabe; aufeinanderfolgender Text desselben Stroms wird verbunden.
function addOutput(cell, out) {
    const outs = cell.model.outputs || (cell.model.outputs = []);
    if (state.pendingClear.get(cell)) {
        state.pendingClear.delete(cell);
        outs.length = 0;
        cell.outputs.replaceChildren();
    }
    const last = outs[outs.length - 1];
    if (out.output_type === "stream" && last && last.output_type === "stream" && last.name === out.name) {
        last.text = terminalText(joined(last.text) + joined(out.text));
        const old = [...cell.outputs.querySelectorAll(".output")].pop();
        if (old) old.replaceWith(outputElement(last));
    } else {
        if (out.output_type === "stream") out.text = terminalText(joined(out.text));
        outs.push(out);
        cell.outputs.appendChild(outputElement(out));
    }
    collapse(cell);
    changed();
}

// ------------------------------------------------------------------- Editor

function highlight(cell) {
    const text = cell.editor.value + "\n";
    const lang = cell.model.cell_type === "markdown" ? "markdown" : cell.model.cell_type === "code" ? state.lang : "";
    if (window.hljs && lang && hljs.getLanguage(lang)) {
        try {
            cell.hl.innerHTML = hljs.highlight(text, { language: lang, ignoreIllegals: true }).value;
            return;
        } catch (e) { /* unten */ }
    }
    cell.hl.textContent = text;
}

function autosize(cell) {
    const ta = cell.editor;
    ta.style.height = "auto";
    ta.style.height = ta.scrollHeight + "px";
}

function onEditorKey(cell, e) {
    if (e.key === "Enter" && e.shiftKey) { e.preventDefault(); runCell(cell, "next"); return; }
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); runCell(cell, "stay"); return; }
    if (e.key === "Enter" && e.altKey) { e.preventDefault(); runCell(cell, "insert"); return; }
    const ta = cell.editor;
    if (e.key === "Escape") { e.preventDefault(); if (cell.model.cell_type === "markdown") renderMarkdownCell(cell); cell.el.focus(); return; }
    if (e.key === "Tab" && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        const { selectionStart: s, selectionEnd: end, value } = ta;
        const start = value.lastIndexOf("\n", s - 1) + 1;
        if (e.shiftKey || s !== end) {
            // Mehrere Zeilen (oder Umschalt): ein- bzw. ausrücken.
            const stop = value.indexOf("\n", end - (end > s && value[end - 1] === "\n" ? 1 : 0));
            const blockEnd = stop < 0 ? value.length : stop;
            const block = value.slice(start, blockEnd);
            const changedBlock = e.shiftKey ? block.replace(/^ {1,4}/gm, "") : block.replace(/^/gm, "    ");
            ta.setRangeText(changedBlock, start, blockEnd, "select");
            if (s === end) ta.selectionStart = ta.selectionEnd = Math.max(start, s + (changedBlock.length - block.length));
        } else {
            ta.setRangeText("    ", s, end, "end");
        }
        ta.dispatchEvent(new Event("input"));
    } else if (e.key === "Enter" && cell.model.cell_type === "code") {
        // Einrückung der Zeile übernehmen, nach „:“ eine Stufe mehr.
        const { selectionStart: s, selectionEnd: end, value } = ta;
        const line = value.slice(value.lastIndexOf("\n", s - 1) + 1, s);
        let indent = line.match(/^\s*/)[0];
        if (/:\s*(#.*)?$/.test(line)) indent += "    ";
        e.preventDefault();
        ta.setRangeText("\n" + indent, s, end, "end");
        ta.dispatchEvent(new Event("input"));
    } else if (e.key === "Backspace" && ta.selectionStart === ta.selectionEnd) {
        // Eine ganze Einrückstufe löschen.
        const s = ta.selectionStart;
        const before = ta.value.slice(ta.value.lastIndexOf("\n", s - 1) + 1, s);
        if (before.length >= 4 && /^ +$/.test(before)) {
            e.preventDefault();
            ta.setRangeText("", s - (before.length % 4 || 4), s, "end");
            ta.dispatchEvent(new Event("input"));
        }
    }
}

/// Befehlsmodus (Zelle gewählt, Editor ohne Fokus) wie in Jupyter.
function onCommandKey(cell, e) {
    if (e.target !== cell.el || e.ctrlKey || e.metaKey || e.altKey) {
        if (e.target === cell.el && e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); runCell(cell, "stay"); }
        return;
    }
    const i = state.cells.indexOf(cell);
    const key = e.key;
    if (key !== "d") state.deleteArmed = null;
    if (key === "Enter" && e.shiftKey) { e.preventDefault(); runCell(cell, "next"); }
    else if (key === "Enter") { e.preventDefault(); edit(cell); }
    else if (key === "ArrowUp" || key === "k") { e.preventDefault(); if (i > 0) select(state.cells[i - 1], "cell"); }
    else if (key === "ArrowDown" || key === "j") { e.preventDefault(); if (i < state.cells.length - 1) select(state.cells[i + 1], "cell"); }
    else if (key === "a") { e.preventDefault(); insertCell(i, "code", "cell"); }
    else if (key === "b") { e.preventDefault(); insertCell(i + 1, "code", "cell"); }
    else if (key === "m") { e.preventDefault(); setCellType(cell, "markdown", "cell"); }
    else if (key === "y") { e.preventDefault(); setCellType(cell, "code", "cell"); }
    else if (key === "r") { e.preventDefault(); setCellType(cell, "raw", "cell"); }
    else if (key === "d") {
        e.preventDefault();
        if (state.deleteArmed === cell) { state.deleteArmed = null; deleteCell(cell); } else state.deleteArmed = cell;
    }
}

// -------------------------------------------------------------------- Zellen

function updatePrompt(cell) {
    const n = cell.model.execution_count;
    cell.count.textContent = cell.model.cell_type !== "code" ? "" : cell.running || cell.queued ? "[*]" : n == null ? "" : `[${n}]`;
    cell.el.classList.toggle("running", !!cell.running);
    cell.el.classList.toggle("queued", !!cell.queued);
    cell.runButton.textContent = cell.running ? "■" : cell.queued ? "…" : "▶";
    cell.runButton.title = cell.running ? "Unterbrechen" : cell.queued ? "Aus der Warteschlange nehmen" : "Zelle ausführen (Strg+Enter)";
}

function setStatusLine(cell, html) { cell.statusLine.innerHTML = html || ""; }

/// `focus`: "edit" (Editor), "cell" (Befehlsmodus) oder nichts.
function select(cell, focus) {
    if (state.selected && state.selected !== cell) state.selected.el.classList.remove("selected");
    state.selected = cell;
    if (!cell) return;
    cell.el.classList.add("selected");
    if (focus === "edit") edit(cell);
    else if (focus === "cell") { cell.el.focus({ preventScroll: true }); cell.el.scrollIntoView({ block: "nearest" }); }
}

function edit(cell) {
    if (cell.model.cell_type === "markdown") cell.el.classList.remove("rendered");
    autosize(cell);
    cell.editor.focus();
    cell.el.scrollIntoView({ block: "nearest" });
}

async function renderMarkdownCell(cell) {
    const out = await markdownHtml(cell.editor.value, cell.model.attachments);
    cell.markdownOut.replaceChildren(...out.childNodes);
    cell.el.classList.add("rendered");
}

function makeCell(model) {
    const cell = { model, running: false, queued: false, expanded: false };
    cell.runButton = el("button", { class: "run", on: { click: (e) => { e.stopPropagation(); onRunButton(cell); } } });
    cell.count = el("span", { class: "count" });
    cell.hl = el("pre", { class: "hl", ariaHidden: "true" });
    cell.editor = el("textarea", {
        class: "source", rows: 1, spellcheck: model.cell_type === "markdown", value: joined(model.source),
        placeholder: model.cell_type === "markdown" ? "Markdown …" : "",
        on: {
            input: () => { model.source = cell.editor.value; highlight(cell); autosize(cell); changed(); },
            keydown: (e) => onEditorKey(cell, e),
            focus: () => { select(cell); cell.el.classList.add("editing"); },
            blur: () => cell.el.classList.remove("editing"),
        },
    });
    cell.markdownOut = el("div", { class: "markdown-out", on: { dblclick: () => edit(cell) } });
    cell.statusLine = el("div", { class: "status-line" });
    cell.outputs = el("div", { class: "outputs" });
    const typeLabel = { code: "Code", markdown: "Markdown", raw: "Roh" };
    const tools = el("div", { class: "tools" },
        model.cell_type === "markdown" ? button("✓", "Anzeigen (Umschalt+Enter)", () => renderMarkdownCell(cell), "md-only") : null,
        button(model.cell_type === "code" ? "M↓" : "{ }", model.cell_type === "code" ? "In Markdown umwandeln (M)" : "In Code umwandeln (Y)",
            () => setCellType(cell, model.cell_type === "code" ? "markdown" : "code", "edit")),
        button("↑", "Nach oben", () => moveCell(cell, -1)),
        button("↓", "Nach unten", () => moveCell(cell, 1)),
        button("⧉", "Duplizieren", () => duplicateCell(cell)),
        button("🗑", "Zelle löschen (D D)", () => deleteCell(cell), "danger"),
    );
    tools.title = typeLabel[model.cell_type] || "";
    cell.el = el("div", {
        class: "cell " + model.cell_type, tabIndex: -1,
        on: {
            mousedown: () => select(cell),
            keydown: (e) => onCommandKey(cell, e),
        },
    },
    el("div", { class: "gutter" }, cell.runButton, cell.count),
    el("div", { class: "body" }, tools, el("div", { class: "editor" }, cell.hl, cell.editor), cell.markdownOut, cell.statusLine, cell.outputs));
    // Einfügen davor (zwischen den Zellen).
    const insert = el("div", { class: "insert" }, el("div", { class: "bar" },
        button("+ Code", "Code-Zelle hier einfügen", () => insertCell(state.cells.indexOf(cell), "code", "edit")),
        button("+ Markdown", "Markdown-Zelle hier einfügen", () => insertCell(state.cells.indexOf(cell), "markdown", "edit"))));
    cell.root = el("div", { class: "cellwrap" }, insert, cell.el);
    highlight(cell);
    updatePrompt(cell);
    if (model.cell_type === "code") renderOutputs(cell);
    if (model.cell_type === "markdown") renderMarkdownCell(cell);
    return cell;
}

function onRunButton(cell) {
    if (cell.running) return interrupt();
    if (cell.queued) {
        state.queue = state.queue.filter((c) => c !== cell);
        cell.queued = false;
        updatePrompt(cell);
        return;
    }
    runCell(cell, "stay");
}

function newModel(type, source) {
    const m = { cell_type: type, metadata: {}, source: source || "" };
    if (state.nb.nbformat > 4 || (state.nb.nbformat === 4 && state.nb.nbformat_minor >= 5)) m.id = cellId();
    if (type === "code") { m.execution_count = null; m.outputs = []; }
    return m;
}

function place(cell, index) {
    state.cells.splice(index, 0, cell);
    const next = state.cells[index + 1];
    $("cells").insertBefore(cell.root, next ? next.root : null);
    autosize(cell);
}

function insertCell(index, type, focus) {
    const cell = makeCell(newModel(type));
    if (type === "markdown") cell.el.classList.remove("rendered");
    place(cell, Math.max(0, Math.min(index, state.cells.length)));
    select(cell, focus || "edit");
    changed();
    return cell;
}

function duplicateCell(cell) {
    const model = JSON.parse(JSON.stringify(cell.model));
    if (model.id) model.id = cellId();
    const copy = makeCell(model);
    place(copy, state.cells.indexOf(cell) + 1);
    select(copy, "cell");
    changed();
}

function deleteCell(cell) {
    if (cell.running) return;
    const i = state.cells.indexOf(cell);
    state.queue = state.queue.filter((c) => c !== cell);
    state.cells.splice(i, 1);
    cell.root.remove();
    select(state.cells[Math.min(i, state.cells.length - 1)] || null, "cell");
    changed();
}

function moveCell(cell, delta) {
    const i = state.cells.indexOf(cell);
    const j = i + delta;
    if (j < 0 || j >= state.cells.length) return;
    state.cells.splice(i, 1);
    state.cells.splice(j, 0, cell);
    const after = state.cells[j + 1];
    $("cells").insertBefore(cell.root, after ? after.root : null);
    changed();
}

function setCellType(cell, type, focus) {
    if (type === cell.model.cell_type || cell.running) return;
    const old = cell.model;
    const model = { cell_type: type, metadata: old.metadata || {}, source: cell.editor.value };
    if (old.id) model.id = old.id;
    if (type === "code") { model.execution_count = null; model.outputs = []; }
    if (type === "markdown" && old.attachments) model.attachments = old.attachments;
    const fresh = makeCell(model);
    const i = state.cells.indexOf(cell);
    state.cells[i] = fresh;
    cell.root.replaceWith(fresh.root);
    if (type === "markdown" && focus === "edit") fresh.el.classList.remove("rendered");
    autosize(fresh);
    select(fresh, focus);
    changed();
}

// --------------------------------------------------------------- Speichern

function sortKeys(value) {
    if (Array.isArray(value)) return value.map(sortKeys);
    if (value && typeof value === "object") {
        const out = {};
        for (const k of Object.keys(value).sort()) out[k] = sortKeys(value[k]);
        return out;
    }
    return value;
}

/// Das Notebook im Format von Jupyter (Quelltexte und Texte als Zeilenlisten).
function serialize() {
    const cells = state.cells.map(({ model }) => {
        const m = Object.assign({}, model, { source: lines(joined(model.source)) });
        if (m.cell_type === "code") {
            m.outputs = (model.outputs || []).map((o) => {
                const out = Object.assign({}, o);
                if (out.output_type === "stream") out.text = lines(joined(out.text));
                if (out.data) {
                    out.data = Object.fromEntries(Object.entries(out.data).map(([k, v]) =>
                        [k, k.startsWith("text/") || k === "image/svg+xml" ? lines(joined(v)) : v]));
                }
                if ((out.output_type === "display_data" || out.output_type === "execute_result") && !out.metadata) out.metadata = {};
                delete out.transient;
                return out;
            });
            if (m.execution_count === undefined) m.execution_count = null;
        } else {
            delete m.outputs;
            delete m.execution_count;
        }
        if (!m.metadata) m.metadata = {};
        return m;
    });
    return JSON.stringify(sortKeys(Object.assign({}, state.nb, { cells })), null, 1) + "\n";
}

/// Speichert 0,8 s nach der letzten Änderung, spätestens 3 s nach der ersten.
function changed() {
    if (!state.dirty) state.dirtySince = Date.now();
    state.dirty = true;
    setSaved("dirty", "Ungespeicherte Änderungen");
    clearTimeout(state.saveTimer);
    state.saveTimer = setTimeout(save, Math.max(0, Math.min(800, 3000 - (Date.now() - state.dirtySince))));
}

async function save() {
    clearTimeout(state.saveTimer);
    if (!state.nb || !state.dirty) return;
    const text = serialize();
    state.dirty = false;
    try {
        await atlas.call("writeFile", { path: state.file, text });
        state.lastText = text;
        setSaved("", "Gespeichert");
    } catch (e) {
        state.dirty = true;
        setSaved("error", "Nicht gespeichert: " + e.message);
        atlas.message("Notebook nicht gespeichert: " + e.message);
    }
}

/// Von außen geändert (z. B. Git): neu laden, solange hier nichts
/// Ungespeichertes ist und nichts läuft.
async function checkExternalChange() {
    if (!state.nb || state.dirty || state.current || state.queue.length) return;
    let text;
    try { text = await atlas.call("readFile", { path: state.file }); } catch (e) { return; }
    if (text !== state.lastText) {
        loadNotebook(text);
        atlas.message("Notebook neu geladen – die Datei wurde außerhalb geändert");
    }
}

function loadNotebook(text) {
    let nb;
    try {
        nb = JSON.parse(text);
    } catch (e) {
        notice("Die Datei ist kein gültiges Notebook (JSON): " + e.message, { error: true });
        return false;
    }
    if (!nb || !Array.isArray(nb.cells)) {
        notice(nb && nb.nbformat < 4 ? "Notebooks im Format 3 und älter werden nicht unterstützt – bitte mit „jupyter nbconvert --to notebook“ umwandeln."
            : "Die Datei ist kein Notebook (es fehlen die Zellen).", { error: true });
        return false;
    }
    nb.metadata = nb.metadata || {};
    state.nb = nb;
    state.lastText = text;
    state.dirty = false;
    state.lang = ((nb.metadata.language_info || {}).name || (nb.metadata.kernelspec || {}).language || "python").toLowerCase();
    const selectedIndex = state.selected ? state.cells.indexOf(state.selected) : 0;
    state.cells = nb.cells.map((m) => { m.source = joined(m.source); return makeCell(m); });
    delete nb.cells;
    $("cells").replaceChildren(...state.cells.map((c) => c.root));
    state.cells.forEach(autosize);
    $("end").hidden = false;
    select(state.cells[Math.max(0, Math.min(selectedIndex, state.cells.length - 1))] || null);
    setKernelState(state.kernelState);
    setSaved("", "Gespeichert");
    return true;
}

// --------------------------------------------------------------- Server

async function api(method, path, body) {
    const res = await fetch(state.base + path, {
        method,
        headers: Object.assign({ Authorization: "token " + state.token }, body ? { "Content-Type": "application/json" } : {}),
        body: body ? JSON.stringify(body) : undefined,
    });
    if (!res.ok) {
        const err = new Error(`${method} ${path}: ${res.status} ${res.statusText}`);
        err.status = res.status;
        throw err;
    }
    return res.status === 204 ? null : res.json();
}

function tokenPath() { return project() + "/.atlas/cache/jupyter/token"; }
function portOf() { return Math.round(Number(state.ctx.settings && state.ctx.settings.port) || 8899); }

/// Der laufende Hintergrundbefehl dieser Erweiterung – die Einrichtung (sie
/// startet danach den Server) oder der Server –, mit `id`.
async function serverJob() {
    for (const id of ["setup", "server"]) {
        let s;
        try { s = await atlas.commandStatus(id); } catch (e) { return null; }
        if (s && s.running) return Object.assign(s, { id });
    }
    return null;
}

function sameDir(a, b) { return (a || "").replace(/\/+$/, "") === (b || "").replace(/\/+$/, ""); }
function logTail(log, n) { return terminalText(log || "").trimEnd().split("\n").slice(-(n || 12)).join("\n"); }

/// Verbindet mit dem Server. Läuft keiner, startet die Ansicht ihn im
/// Hintergrund (`start`: "server", "setup" oder `false`) und wartet.
async function connect(opts = {}) {
    if (!project()) {
        notice("Notebooks laufen nur in einem Projektordner (dort startet der Jupyter-Server).", { error: true });
        return;
    }
    const start = opts.start === undefined ? "server" : opts.start;
    let started = null;
    let since = 0;
    for (;;) {
        let error;
        try {
            state.token = (await atlas.call("readFile", { path: tokenPath() })).trim();
            await api("GET", "/api/status");
            state.online = true;
            break;
        } catch (e) {
            error = e;
            state.online = false;
        }
        setKernelState("off");
        const job = await serverJob();
        if (job && !sameDir(job.cwd, project())) {
            notice("Beende den Jupyter-Server des vorigen Projekts …");
            await atlas.stopCommand(job.id);
            await sleep(1500);
            continue;
        }
        if (job) {
            const setup = job.id === "setup";
            notice(setup ? "Jupyter wird im Projekt eingerichtet (.venv) …" : "Jupyter-Server startet …", { log: setup ? logTail(job.log, 4) : "" });
            setKernelState("starting");
            if (!since) since = Date.now();
            if (Date.now() - since < (setup ? 900 : 120) * 1000) { await sleep(1000); continue; }
            notice("Der Jupyter-Server antwortet nicht.", { error: true, log: logTail(job.log), actions: [{ label: "Erneut versuchen", run: () => connect() }] });
            return;
        }
        const retry = { label: "Erneut versuchen", run: () => connect() };
        const setupAction = { label: "Jupyter im Projekt einrichten", primary: true, title: "Legt im Projekt eine .venv mit jupyter_server und ipykernel an und startet den Server daraus", run: () => connect({ start: "setup" }) };
        if (started) {
            let s = {};
            try { s = await atlas.commandStatus(started); } catch (e) { /* älteres Atlas */ }
            const missing = /fehlt/.test(s.log || "");
            notice(missing ? "Jupyter ist nicht installiert." : "Der Jupyter-Server ließ sich nicht starten.",
                { error: true, log: logTail(s.log), actions: [setupAction, retry] });
            return;
        }
        if (error && (error.status === 403 || error.status === 401)) {
            notice(`Auf Port ${portOf()} läuft ein anderer Jupyter-Server (Token passt nicht). Port in den Einstellungen der Erweiterung ändern oder den anderen Server beenden.`,
                { error: true, actions: [retry] });
            return;
        }
        if (!start) {
            notice("Der Jupyter-Server läuft nicht.", { actions: [{ label: "Starten", primary: true, run: () => connect({ start: "server" }) }, setupAction] });
            return;
        }
        try {
            await atlas.runCommand(start);
        } catch (e) {
            notice("Der Jupyter-Server startet nicht: " + e.message, { error: true, actions: [setupAction, retry] });
            return;
        }
        started = start;
        since = Date.now();
        await sleep(500);
    }
    notice("");
    try { state.home = (await serverEnv()).home; } catch (e) { /* nur für Anzeige */ }
    if (state.nb) await startSession();
}

/// Server neu starten oder nur beenden.
async function restartServer(again) {
    const job = await serverJob();
    closeSocket();
    failRunning("Jupyter-Server beendet");
    state.session = null;
    state.online = false;
    setKernelState("off");
    if (job) {
        await atlas.stopCommand(job.id);
        for (let i = 0; i < 60 && (await serverJob()); i++) await sleep(250);
    }
    if (again) await connect({ start: "server" });
    else notice("Der Jupyter-Server ist beendet.", { actions: [{ label: "Starten", primary: true, run: () => connect({ start: "server" }) }] });
}

async function loadSpecs() {
    state.specs = await api("GET", "/api/kernelspecs");
    return state.specs;
}

function specLabel(name) {
    const s = state.specs && state.specs.kernelspecs[name];
    return (s && s.spec.display_name) || name;
}

async function startSession(forceName) {
    const specs = await loadSpecs();
    if (!Object.keys(specs.kernelspecs).length) {
        setKernelState("off");
        notice("Für dieses Notebook gibt es noch keine Umgebung.", { actions: [{ label: "Umgebung wählen", primary: true, run: openPicker }] });
        return;
    }
    const wanted = forceName || (state.nb.metadata.kernelspec || {}).name;
    const name = wanted && specs.kernelspecs[wanted] ? wanted : specs.kernelspecs[specs.default] ? specs.default : Object.keys(specs.kernelspecs)[0];
    setKernelState("starting");
    const path = relative(state.file);
    try {
        // Dieselbe Sitzung wie beim letzten Öffnen (Variablen bleiben erhalten).
        const sessions = await api("GET", "/api/sessions");
        state.session = sessions.find((s) => s.path === path && s.kernel) || null;
        if (state.session && state.session.kernel.name !== name && forceName) {
            state.session = await api("PATCH", `/api/sessions/${state.session.id}`, { kernel: { name } });
        }
        if (!state.session) state.session = await api("POST", "/api/sessions", { path, name: basename(path), type: "notebook", kernel: { name } });
    } catch (e) {
        setKernelState("dead");
        notice("Der Kernel startet nicht: " + e.message, { error: true, actions: [{ label: "Umgebung wählen", run: openPicker }, { label: "Erneut versuchen", run: () => connect() }] });
        return;
    }
    // Eine laufende Sitzung behält ihren Kernel (und ihre Variablen); das
    // Notebook nennt dann diesen.
    const running = state.session.kernel.name;
    if (running !== name && specs.kernelspecs[running]) {
        const spec = specs.kernelspecs[running].spec;
        state.nb.metadata.kernelspec = { display_name: spec.display_name, language: spec.language, name: running };
        changed();
    } else if (wanted && name !== wanted) {
        notice(`Dieses Notebook wurde mit „${wanted}“ geschrieben – die Umgebung gibt es hier nicht, es läuft mit „${specLabel(name)}“.`,
            { actions: [{ label: "Umgebung wählen", primary: true, run: openPicker }, { label: "Passt so", run: () => notice("") }] });
    }
    openSocket();
}

function closeSocket() {
    if (state.ws) { state.ws.onclose = null; state.ws.close(); state.ws = null; }
}

function openSocket() {
    closeSocket();
    const kernel = state.session.kernel.id;
    const url = state.base.replace(/^http/, "ws") + `/api/kernels/${kernel}/channels?session_id=${state.clientId}&token=${encodeURIComponent(state.token)}`;
    const ws = new WebSocket(url);
    state.ws = ws;
    state.languageInfo = null;
    ws.onopen = () => {
        setKernelState("idle");
        send("shell", "kernel_info_request", {});
    };
    ws.onmessage = (e) => { try { onMessage(JSON.parse(e.data)); } catch (err) { console.warn(err); } };
    ws.onclose = () => {
        if (state.ws !== ws) return;
        state.ws = null;
        failRunning("Verbindung zum Kernel getrennt");
        setKernelState("off");
        notice("Die Verbindung zum Jupyter-Server ist getrennt.", { error: true, actions: [{ label: "Neu verbinden", primary: true, run: () => connect() }] });
    };
}

function send(channel, type, content) {
    const msgId = uuid();
    state.ws.send(JSON.stringify({
        header: { msg_id: msgId, msg_type: type, username: "atlas", session: state.clientId, date: new Date().toISOString(), version: "5.3" },
        parent_header: {}, metadata: {}, content, channel, buffers: [],
    }));
    return msgId;
}

/// Diesen Kernel für das Notebook nehmen.
async function useKernel(name) {
    if (!state.nb) return;
    await loadSpecs();
    const spec = state.specs.kernelspecs[name];
    if (!spec) return;
    clearQueue();
    if (state.current) failRunning("Umgebung gewechselt");
    state.nb.metadata.kernelspec = { display_name: spec.spec.display_name, language: spec.spec.language, name };
    changed();
    notice("");
    if (state.session) {
        try {
            state.session = await api("PATCH", `/api/sessions/${state.session.id}`, { kernel: { name } });
            openSocket();
            return;
        } catch (e) { state.session = null; }
    }
    await startSession(name);
}

// --------------------------------------------------------------- Ausführen

function runCell(cell, then) {
    if (cell.model.cell_type === "markdown") renderMarkdownCell(cell);
    else if (cell.model.cell_type === "code") enqueue([cell]);
    const i = state.cells.indexOf(cell);
    if (then === "next" || then === "insert") {
        if (then === "insert" || i === state.cells.length - 1) insertCell(i + 1, "code", "edit");
        else select(state.cells[i + 1], state.cells[i + 1].model.cell_type === "code" ? "edit" : "cell");
    }
}

function enqueue(cells) {
    if (!state.ws || state.ws.readyState !== WebSocket.OPEN) {
        atlas.message("Kein Kernel verbunden – oben rechts eine Umgebung wählen");
        if (state.online && !state.session) openPicker();
        return;
    }
    for (const c of cells) {
        if (c.model.cell_type !== "code" || c.queued || c.running) continue;
        c.queued = true;
        setStatusLine(c, "");
        updatePrompt(c);
        state.queue.push(c);
    }
    next();
}

let ticker = 0;

function next() {
    if (state.current) return;
    const cell = state.queue.shift();
    if (!cell) { clearInterval(ticker); setKernelState(state.kernelState); return; }
    if (!state.cells.includes(cell)) { next(); return; }
    cell.queued = false;
    cell.running = true;
    updatePrompt(cell);
    // Die alten Ausgaben verschwinden mit der ersten neuen (kein Flackern).
    state.pendingClear.set(cell, true);
    cell.expanded = false;
    const msgId = send("shell", "execute_request", {
        code: cell.editor.value, silent: false, store_history: true, user_expressions: {}, allow_stdin: false, stop_on_error: true,
    });
    state.current = { cell, msgId, idle: false, reply: null, t0: performance.now() };
    setStatusLine(cell, `<span>läuft …</span>`);
    clearInterval(ticker);
    ticker = setInterval(() => {
        if (state.current) setStatusLine(state.current.cell, `<span>läuft · ${duration(performance.now() - state.current.t0)}</span>`);
    }, 1000);
    setKernelState(state.kernelState);
}

function finish() {
    const cur = state.current;
    if (!cur) return;
    const { cell, reply } = cur;
    state.current = null;
    cell.running = false;
    if (state.pendingClear.get(cell)) {
        state.pendingClear.delete(cell);
        cell.model.outputs = [];
        cell.outputs.replaceChildren();
    }
    if (reply && reply.execution_count != null) cell.model.execution_count = reply.execution_count;
    const ok = reply && reply.status === "ok";
    const took = duration(performance.now() - cur.t0);
    setStatusLine(cell, ok ? `<span class="ok">✓</span><span>${took}</span>`
        : reply && reply.status === "aborted" ? `<span class="fail">abgebrochen</span>`
        : `<span class="fail">✗ Fehler</span><span>${took}</span>`);
    updatePrompt(cell);
    changed();
    if (!ok) clearQueue();
    next();
}

function clearQueue() {
    for (const c of state.queue) { c.queued = false; updatePrompt(c); }
    state.queue = [];
}

function failRunning(text) {
    clearQueue();
    if (state.current) {
        const cell = state.current.cell;
        addOutput(cell, { output_type: "stream", name: "stderr", text: text + "\n" });
        state.current = null;
        cell.running = false;
        setStatusLine(cell, `<span class="fail">${escapeHtml(text)}</span>`);
        updatePrompt(cell);
    }
    clearInterval(ticker);
}

/// Zelle zur Ausgabe mit `display_id` (update_display_data).
const displays = new Map();

function onMessage(msg) {
    const type = msg.header.msg_type || msg.msg_type;
    const parent = (msg.parent_header || {}).msg_id;
    const c = msg.content || {};
    if (type === "kernel_info_reply") {
        const done = state.infoWaiters.get(parent);
        if (done) { state.infoWaiters.clear(); done(); }
        if (c.language_info) {
            state.languageInfo = c.language_info;
            const lang = (c.language_info.name || "python").toLowerCase();
            if (lang !== state.lang) { state.lang = lang; state.cells.forEach(highlight); }
            // Wie Jupyter: language_info steht im Notebook (ohne es dafür zu speichern).
            if (state.nb) state.nb.metadata.language_info = Object.assign({}, state.nb.metadata.language_info, c.language_info);
            setKernelState(state.kernelState);
        }
        return;
    }
    if (type === "status") {
        if (state.restarting) return;
        if (c.execution_state === "restarting" || c.execution_state === "dead") {
            failRunning(c.execution_state === "dead" ? "Kernel beendet" : "Kernel startet neu");
            setKernelState(c.execution_state);
            return;
        }
        if (!state.current || parent === state.current.msgId) setKernelState(c.execution_state === "starting" ? "starting" : c.execution_state);
        if (state.current && parent === state.current.msgId && c.execution_state === "idle") {
            state.current.idle = true;
            if (state.current.reply) finish();
        }
        return;
    }
    const cur = state.current && parent === state.current.msgId ? state.current : null;
    if (type === "execute_reply") {
        if (!cur) return;
        cur.reply = c;
        if (cur.idle) finish();
        return;
    }
    if (type === "update_display_data") {
        const target = displays.get((c.transient || {}).display_id);
        if (!target) return;
        target.out.data = c.data;
        target.out.metadata = c.metadata || {};
        renderOutputs(target.cell);
        changed();
        return;
    }
    if (!cur) return;
    const cell = cur.cell;
    switch (type) {
    case "execute_input":
        cell.model.execution_count = c.execution_count;
        break;
    case "stream":
        addOutput(cell, { output_type: "stream", name: c.name, text: c.text });
        break;
    case "display_data":
    case "execute_result": {
        const out = { output_type: type, data: c.data, metadata: c.metadata || {} };
        if (type === "execute_result") out.execution_count = c.execution_count;
        addOutput(cell, out);
        if (c.transient && c.transient.display_id) displays.set(c.transient.display_id, { cell, out });
        break;
    }
    case "error":
        addOutput(cell, { output_type: "error", ename: c.ename, evalue: c.evalue, traceback: c.traceback || [] });
        break;
    case "clear_output":
        if (c.wait) state.pendingClear.set(cell, true);
        else { cell.model.outputs = []; cell.outputs.replaceChildren(); changed(); }
        break;
    }
}

async function interrupt() {
    clearQueue();
    if (!state.session) return;
    try { await api("POST", `/api/kernels/${state.session.kernel.id}/interrupt`); } catch (e) { atlas.message("Unterbrechen ging nicht: " + e.message); }
}

/// Wartet, bis der Kernel antwortet (kernel_info_reply), höchstens `ms`.
function kernelReady(ms) {
    return new Promise((resolve) => {
        const deadline = Date.now() + ms;
        const ask = () => {
            if (!state.ws || state.ws.readyState !== WebSocket.OPEN || Date.now() > deadline) { clearInterval(timer); state.infoWaiters.clear(); resolve(false); return; }
            // Während des Neustarts gehen Anfragen verloren: jede Sekunde neu fragen.
            state.infoWaiters.set(send("shell", "kernel_info_request", {}), () => { clearInterval(timer); resolve(true); });
        };
        const timer = setInterval(ask, 1000);
        ask();
    });
}

/// Kernel neu starten; `then`: "all" (alle ausführen) oder "clear" (Ausgaben löschen).
async function restart(then) {
    if (!state.session) return;
    clearQueue();
    if (state.current) failRunning("Kernel neu gestartet");
    state.restarting = true;
    setKernelState("restarting");
    try {
        await api("POST", `/api/kernels/${state.session.kernel.id}/restart`);
        if (!(await kernelReady(30000))) throw new Error("der Kernel antwortet nicht");
    } catch (e) {
        atlas.message("Neustart ging nicht: " + e.message);
        return;
    } finally {
        state.restarting = false;
        state.infoWaiters.clear();
    }
    setKernelState("idle");
    if (then === "clear") clearOutputs();
    if (then === "all") enqueue(state.cells);
}

function clearOutputs() {
    for (const c of state.cells) {
        if (c.model.cell_type !== "code") continue;
        c.model.outputs = [];
        c.model.execution_count = null;
        c.outputs.replaceChildren();
        setStatusLine(c, "");
        updatePrompt(c);
    }
    changed();
}

// ------------------------------------------------------------- Menüs

function showMenu(anchor, entries) {
    const menu = $("menu");
    menu.replaceChildren(...entries.map((e) => e === "-" ? el("hr") : el("button", {
        textContent: e.label, disabled: !!e.disabled, title: e.title || "",
        on: { click: () => { hideMenu(); e.run(); } },
    })));
    menu.hidden = false;
    const r = anchor.getBoundingClientRect();
    menu.style.top = r.bottom + 4 + "px";
    menu.style.left = Math.max(8, Math.min(r.left, window.innerWidth - menu.offsetWidth - 8)) + "px";
}
function hideMenu() { $("menu").hidden = true; }

function moreMenu() {
    const i = state.selected ? state.cells.indexOf(state.selected) : -1;
    const connected = !!state.ws;
    showMenu($("more"), [
        { label: "Ab ausgewählter Zelle ausführen", disabled: !connected || i < 0, run: () => enqueue(state.cells.slice(i)) },
        { label: "Bis zur ausgewählten Zelle ausführen", disabled: !connected || i < 0, run: () => enqueue(state.cells.slice(0, i + 1)) },
        "-",
        { label: "Alle Ausgaben löschen", run: clearOutputs },
        "-",
        { label: "Jupyter-Server neu starten", disabled: !state.online, run: () => restartServer(true) },
        { label: "Jupyter-Server beenden", disabled: !state.online, run: () => restartServer(false) },
    ]);
}

function restartMenu() {
    showMenu($("restart-menu"), [
        { label: "Neu starten", run: () => restart() },
        { label: "Neu starten und alle ausführen", run: () => restart("all") },
        { label: "Neu starten und Ausgaben löschen", run: () => restart("clear") },
    ]);
}

// ------------------------------------------------- Aufgaben (im Server-Terminal)

// Befehle laufen in einem Terminal des Jupyter-Servers (dieselbe Maschine,
// derselbe Benutzer). Das Skript geht als base64 durch die Shell des
// Benutzers (bash, zsh, fish …) nach sh; Marken grenzen die Ausgabe ab.

/// Führt `script` (sh-Zeilen) aus. `onOutput(text)` bekommt die Ausgabe bis
/// jetzt; löst mit {code, output} auf (130: abgebrochen, -1: Verbindung weg).
async function shell(script, onOutput, control) {
    let term;
    try {
        term = await api("POST", "/api/terminals");
    } catch (e) {
        const text = e.status === 404 ? "Der Jupyter-Server hat keine Terminals (Paket jupyter_server_terminals fehlt)." : "Kein Terminal: " + e.message;
        return { code: -1, output: text };
    }
    // stdin leer: sh liest das Skript selbst von stdin, kein Befehl darf mitlesen.
    const wrapped = `printf '__ATLAS_%s__\\n' START\n(\nset -e\n${script.join("\n")}\n) </dev/null 2>&1\nprintf '__ATLAS_%s_%s__\\n' DONE $?\n`;
    const ws = new WebSocket(state.base.replace(/^http/, "ws") + `/terminals/websocket/${term.name}?token=${encodeURIComponent(state.token)}`);
    const result = await new Promise((resolve) => {
        let raw = "";
        let body = "";
        let done = false;
        const finishWith = (code) => { if (!done) { done = true; resolve({ code, output: body }); } };
        if (control) control.cancel = () => { try { ws.send(JSON.stringify(["stdin", "\x03"])); } catch (e) { /* zu */ } setTimeout(() => finishWith(130), 300); };
        ws.onopen = () => {
            ws.send(JSON.stringify(["set_size", 40, 160, 600, 1200]));
            ws.send(JSON.stringify(["stdin", `printf %s ${base64(wrapped)} | base64 -d | sh\r`]));
        };
        ws.onmessage = (e) => {
            let m;
            try { m = JSON.parse(e.data); } catch (err) { return; }
            if (m[0] !== "stdout") return;
            raw += m[1];
            const start = raw.indexOf("__ATLAS_START__");
            if (start < 0) return;
            body = raw.slice(start + 15).replace(/^\r?\n/, "");
            const end = /__ATLAS_DONE_(\d+)__/.exec(body);
            if (end) body = body.slice(0, end.index);
            body = terminalText(body);
            if (onOutput) onOutput(body);
            if (end) finishWith(Number(end[1]));
        };
        ws.onclose = () => finishWith(-1);
    });
    ws.onclose = null;
    ws.close();
    api("DELETE", `/api/terminals/${term.name}`).catch(() => {});
    return result;
}

/// Sichtbare Aufgabe im Feld unten rechts; löst mit dem Exit-Code auf.
async function task(title, script) {
    if (state.task) { atlas.message("Es läuft schon eine Aufgabe"); return -1; }
    const control = {};
    state.task = control;
    $("task").hidden = false;
    $("task-title").textContent = title;
    $("task-status").textContent = "startet …";
    $("task-status").className = "";
    $("task-spin").className = "spin";
    $("task-log").textContent = "";
    $("task-cancel").hidden = false;
    $("task-close").hidden = true;
    $("task-cancel").onclick = () => control.cancel && control.cancel();
    const { code, output } = await shell(script, (text) => {
        const log = $("task-log");
        const atEnd = log.scrollTop + log.clientHeight >= log.scrollHeight - 20;
        log.innerHTML = ansiHtml(text);
        if (atEnd) log.scrollTop = log.scrollHeight;
        const last = text.trimEnd().split("\n").pop() || "";
        $("task-status").textContent = last.replace(/\x1b\[[0-9;?]*[A-Za-z]/g, "").slice(0, 160);
    }, control);
    state.task = null;
    if (code === -1 && output) $("task-log").textContent = output;
    $("task-cancel").hidden = true;
    $("task-close").hidden = false;
    $("task-spin").className = "spin " + (code === 0 ? "done" : "failed");
    $("task-status").textContent = code === 0 ? "fertig" : code === 130 ? "abgebrochen" : code === -1 ? (output.split("\n")[0] || "Verbindung getrennt") : `fehlgeschlagen (Code ${code}) – Log ansehen`;
    $("task-status").className = code === 0 ? "ok" : "error";
    if (code !== 0) $("task-log").hidden = false;
    else setTimeout(() => { if (!state.task && $("task-log").hidden) $("task").hidden = true; }, 4000);
    return code;
}

/// Zeilen, die $SPY auf das Python des Servers setzen (dasselbe Programm wie
/// in server.sh, Python aus dessen Kopfzeile) und $SJ auf Jupyter.
function serverPythonLines() {
    const setting = (state.ctx.settings && state.ctx.settings.jupyter) || "jupyter";
    const venvJupyter = project() + "/.venv/bin/jupyter";
    return [
        `SJ=${sq(setting)}`,
        `if [ "$SJ" = jupyter ] && [ -x ${sq(venvJupyter)} ]; then SJ=${sq(venvJupyter)}; fi`,
        `SJ=$(command -v "$SJ") || { echo "Das Jupyter-Programm des Servers ist nicht zu finden."; exit 1; }`,
        `SPY=$(sed -n '1s/^#! *//p' "$SJ" | cut -d' ' -f1)`,
        `case "$(basename "$SPY")" in python*) ;; *) SPY="$(dirname "$SJ")/python" ;; esac`,
    ];
}

/// Persönlicher Ordner und Python-Programm der Maschine (für Anzeige und Vorschläge).
async function serverEnv() {
    if (state.serverEnvCache) return state.serverEnvCache;
    const { code, output } = await shell(['echo "HOME\t$HOME"']);
    const home = code === 0 ? ((/^HOME\t(.*)$/m.exec(output) || [])[1] || "") : "";
    state.serverEnvCache = { home };
    return state.serverEnvCache;
}

// ------------------------------------------------------------- Umgebungen

const MICROMAMBA_HOME = "$HOME/.local/share/atlas/micromamba";

/// Programm für Conda-Umgebungen: conda, mamba oder micromamba (auch das
/// von Atlas eingerichtete); setzt $T und $TK (Art).
function condaToolLines() {
    return [
        `T=""; TK=""`,
        `for c in micromamba mamba conda; do if command -v "$c" >/dev/null 2>&1; then T=$(command -v "$c"); TK=$c; break; fi; done`,
        `if [ -z "$T" ]; then for c in "${MICROMAMBA_HOME}/bin/micromamba" "$HOME/miniforge3/bin/conda" "$HOME/mambaforge/bin/conda" "$HOME/miniconda3/bin/conda" "$HOME/anaconda3/bin/conda" "$HOME/micromamba/bin/micromamba" /opt/conda/bin/conda; do if [ -x "$c" ]; then T=$c; TK=$(basename "$c"); break; fi; done; fi`,
        `case "$T" in "${MICROMAMBA_HOME}"/*) export MAMBA_ROOT_PREFIX="${MICROMAMBA_HOME}" ;; esac`,
    ];
}

/// Umgebungsdatei im Conda-Format lesen (name, Python, Pakete).
function parseCondaFile(text) {
    const out = { name: "", python: "", conda: [], pip: [] };
    let section = "";
    let pipIndent = -1;
    for (const raw of text.split("\n")) {
        const line = raw.replace(/\s+#.*$/, "").replace(/\s+$/, "");
        if (!line.trim() || /^\s*#/.test(raw)) continue;
        const indent = line.search(/\S/);
        const top = /^([A-Za-z_]+)\s*:\s*(.*)$/.exec(line);
        if (indent === 0 && top) {
            section = top[1];
            pipIndent = -1;
            if (section === "name") out.name = top[2].replace(/^["']|["']$/g, "");
            continue;
        }
        if (section !== "dependencies") continue;
        const item = /^(\s*)-\s*(.+)$/.exec(line);
        if (!item) continue;
        const value = item[2].replace(/^["']|["']$/g, "");
        if (/^pip\s*:$/.test(value)) { pipIndent = item[1].length; continue; }
        if (pipIndent >= 0 && item[1].length > pipIndent) { out.pip.push(value); continue; }
        pipIndent = -1;
        const spec = value.replace(/^[\w.-]+::/, "");
        const py = /^python\s*[=<>!~]*\s*([0-9.*]+)?/.exec(spec);
        if (py && /^python(\s|=|<|>|!|~|$)/.test(spec)) { out.python = (py[1] || "").replace(/\.\*$/, ""); continue; }
        if (/^pip(\s|=|$)/.test(spec)) continue;
        out.conda.push(spec);
    }
    return out;
}

/// Conda-Paketangabe als pip-Angabe (numpy=1.2 → numpy==1.2).
function condaToPip(spec) {
    return spec.replace(/\s+/g, "").replace(/^([\w.-]+)=([^=<>!~][^=]*)$/, "$1==$2").replace(/=([0-9.]+)=.*$/, "==$1");
}

/// Sucht Umgebungen: venvs im Projekt und Notebook-Ordner, Conda-Umgebungen,
/// Umgebungsdateien und requirements.txt; dazu die Kernel des Servers.
async function discover() {
    const found = { tool: "", envs: [], files: [], kernels: [] };
    // Ordner vom Notebook bis zum Projekt.
    const dirs = [];
    if (state.file && state.file.startsWith(project() + "/")) {
        for (let d = dirname(state.file); d.length >= project().length; d = dirname(d)) {
            dirs.push(d);
            if (d === project()) break;
        }
    }
    if (!dirs.includes(project())) dirs.push(project());
    const tasks = [];
    for (const dir of dirs) {
        for (const n of [".venv", "venv", "env", ".env"]) {
            const cfg = `${dir}/${n}/pyvenv.cfg`;
            tasks.push(atlas.call("exists", { path: cfg }).then(async (yes) => {
                if (!yes) return;
                let version = "";
                try { version = (/^\s*version(?:_info)?\s*=\s*([0-9.]+)/m.exec(await atlas.call("readFile", { path: cfg })) || [])[1] || ""; } catch (e) { /* egal */ }
                found.envs.push({ kind: "venv", path: `${dir}/${n}`, name: n, version });
            }).catch(() => {}));
        }
        tasks.push(atlas.call("listFiles", { dir }).then(async (entries) => {
            for (const f of entries) {
                if (f.dir || f.size > 256 * 1024) continue;
                if (/^requirements.*\.txt$/i.test(f.name)) {
                    let count = 0;
                    try { count = (await atlas.call("readFile", { path: f.path })).split("\n").filter((l) => l.trim() && !l.trim().startsWith("#")).length; } catch (e) { /* egal */ }
                    found.files.push({ kind: "requirements", path: f.path, count });
                } else if (/\.(ya?ml|sec)$/i.test(f.name) || /^environment/i.test(f.name)) {
                    let text = "";
                    try { text = await atlas.call("readFile", { path: f.path }); } catch (e) { continue; }
                    if (!/^dependencies\s*:/m.test(text)) continue;
                    const parsed = parseCondaFile(text);
                    found.files.push(Object.assign({ kind: "conda", path: f.path }, parsed, { name: parsed.name || f.name.replace(/\.[^.]+$/, "") }));
                }
            }
        }).catch(() => {}));
    }
    // Conda-Umgebungen (über das Terminal des Servers).
    if (state.online) {
        tasks.push(shell([
            ...condaToolLines(),
            `[ -n "$T" ] || exit 0`,
            `echo "TOOL	$TK	$T"`,
            `"$T" env list --json 2>/dev/null | tr ',[]{}' '\\n\\n\\n\\n\\n' | sed -n 's/^ *"\\(\\/[^"]*\\)" *$/\\1/p' | while read -r e; do`,
            `  [ -x "$e/bin/python" ] || continue`,
            `  v=$(ls "$e/conda-meta" 2>/dev/null | sed -n 's/^python-\\([0-9][0-9.]*\\)-.*/\\1/p' | head -n 1)`,
            `  echo "CONDA	$e	$v"`,
            `done`,
        ]).then(({ output }) => {
            for (const line of output.split("\n")) {
                const [kind, a, b] = line.split("\t");
                if (kind === "TOOL") { found.tool = a; found.toolPath = b; }
                if (kind === "CONDA" && a) found.envs.push({ kind: "conda", path: a, name: /\/envs\/[^/]+$/.test(a) ? basename(a) : "base", version: b || "" });
            }
        }));
        tasks.push(loadSpecs().then((s) => {
            found.kernels = Object.entries(s.kernelspecs).map(([name, k]) => ({ name, label: k.spec.display_name || name, python: (k.spec.argv || [])[0] || "", language: k.spec.language }));
        }).catch(() => {}));
    }
    await Promise.all(tasks);
    // Mit pip gebaute Umgebungen aus Conda-Dateien (.venv-<name> daneben).
    await Promise.all(found.files.filter((f) => f.kind === "conda").map(async (file) => {
        const path = dirname(file.path) + "/.venv-" + slug(file.name);
        if (found.envs.some((e) => e.path === path)) return;
        try {
            if (!(await atlas.call("exists", { path: path + "/pyvenv.cfg" }))) return;
            const cfg = await atlas.call("readFile", { path: path + "/pyvenv.cfg" });
            found.envs.push({ kind: "venv", path, name: basename(path), version: (/^\s*version(?:_info)?\s*=\s*([0-9.]+)/m.exec(cfg) || [])[1] || "" });
        } catch (e) { /* egal */ }
    }));
    // Kernel zu Umgebungen: der Kernel startet das Python der Umgebung.
    for (const env of found.envs) {
        const k = found.kernels.find((kk) => kk.python.startsWith(env.path + "/bin/"));
        if (k) { env.kernel = k.name; k.env = env; }
    }
    for (const file of found.files) {
        if (file.kind === "conda") {
            file.env = found.envs.find((e) => e.kind === "conda" && e.name === file.name)
                || found.envs.find((e) => e.path === dirname(file.path) + "/.venv-" + slug(file.name)) || null;
        }
        if (file.kind === "requirements") file.env = found.envs.find((e) => e.kind === "venv" && e.path === dirname(file.path) + "/.venv") || null;
    }
    found.envs.sort((a, b) => (a.kind === b.kind ? a.path.localeCompare(b.path) : a.kind === "venv" ? -1 : 1));
    state.found = found;
    return found;
}

function packageCount(n) { return n === 1 ? "1 Paket" : `${n} Pakete`; }

function envLabel(env) {
    if (env.kind === "conda") return `${env.name} (conda)`;
    if (env.name.startsWith(".venv-")) return `${env.name.slice(6)} (.venv)`;
    const dir = dirname(env.path);
    return `${basename(dir === project() ? project() : dir)} (${env.name})`;
}

/// Python in `envPath` als Kernel anmelden (mit ipykernel) und verwenden.
async function registerAndUse(envPath, label) {
    const existing = state.found && state.found.kernels.find((k) => k.python.startsWith(envPath + "/bin/"));
    if (existing) return useKernel(existing.name);
    const name = `atlas-${slug(label.replace(/\s*\(.*\)$/, ""))}-${shortHash(envPath)}`;
    const code = await task(`„${label}“ als Kernel einbinden`, [
        `PY=${sq(envPath + "/bin/python")}`,
        `if ! "$PY" -c 'import ipykernel' 2>/dev/null; then echo "Installiere ipykernel …"; "$PY" -m pip install --disable-pip-version-check ipykernel; fi`,
        `"$PY" -m ipykernel install --user --name ${sq(name)} --display-name ${sq(label)}`,
    ]);
    if (code !== 0) return;
    await useKernel(name);
}

/// Eine venv anlegen (falls nötig), Pakete hinein, als Kernel verwenden.
async function buildVenv(dir, label, installLines, title) {
    const python = (state.ctx.settings && state.ctx.settings.python) || "python3";
    const code = await task(title, [
        `V=${sq(dir)}`,
        `if [ ! -x "$V/bin/python" ]; then`,
        `  P=${sq(python)}`,
        `  command -v "$P" >/dev/null 2>&1 || { echo "„$P“ fehlt – in den Einstellungen der Erweiterung ein Python angeben."; exit 1; }`,
        `  echo "Lege $V an ($("$P" --version 2>&1)) …"`,
        `  "$P" -m venv "$V"`,
        `fi`,
        ...installLines,
    ]);
    if (code !== 0) return;
    await discover();
    await registerAndUse(dir, label);
}

/// Conda-Umgebung aus einer Datei bauen (oder aktualisieren) und verwenden.
async function buildCondaEnv(file) {
    const found = state.found || (await discover());
    if (file.env && file.env.kind === "venv") return buildFromCondaWithPip(file);
    if (!found.tool) return showCondaHelp(file);
    const update = !!file.env;
    const code = await task(`${update ? "Aktualisiere" : "Erstelle"} „${file.name}“ aus ${basename(file.path)}`, [
        ...condaToolLines(),
        `F=${sq(file.path)}`,
        `echo "Mit $T"`,
        update
            ? `case "$TK" in micromamba) "$T" install -y -n ${sq(file.name)} -f "$F" ;; *) "$T" env update -n ${sq(file.name)} -f "$F" --prune ;; esac`
            : `case "$TK" in micromamba) "$T" create -y -n ${sq(file.name)} -f "$F" ;; *) "$T" env create -n ${sq(file.name)} -f "$F" ;; esac`,
    ]);
    if (code !== 0) return;
    const again = await discover();
    const env = again.envs.find((e) => e.kind === "conda" && e.name === file.name);
    if (!env) { atlas.message(`„${file.name}“ wurde gebaut, ist aber nicht zu finden`); return; }
    await registerAndUse(env.path, envLabel(env));
}

/// Ohne conda/mamba/micromamba: micromamba einrichten oder pip nehmen.
function showCondaHelp(file) {
    openPicker(true);
    form([
        el("p", { class: "empty", textContent: `Für „${basename(file.path)}“ braucht es conda, mamba oder micromamba – keins davon ist installiert.` }),
        el("p", { class: "empty", textContent: "micromamba ist ein einzelnes Programm (rund 15 MB), Atlas legt es unter ~/.local/share/atlas/micromamba ab; die Umgebung bekommt dann genau die Python-Version der Datei." }),
        el("div", { class: "row" },
            button("Mit pip als .venv bauen", "Nur die Pakete, mit dem Python dieses Rechners", () => { closePicker(); buildFromCondaWithPip(file); }),
            button("micromamba einrichten und bauen", "Lädt micromamba von micro.mamba.pm", async () => { closePicker(); if ((await setupMicromamba()) === 0) { await discover(); buildCondaEnv(file); } }, "primary")),
    ]);
}

/// Notlösung ohne Conda: eine .venv mit den pip- und Conda-Paketen der Datei.
function buildFromCondaWithPip(file) {
    const dir = dirname(file.path) + "/.venv-" + slug(file.name);
    const packages = [...file.conda.map(condaToPip), ...file.pip].filter((p) => !/^(python|pip)([=<>!~ ]|$)/.test(p));
    return buildVenv(dir, `${file.name} (.venv)`, [
        file.python ? `W=${sq(file.python)}; H=$("$V/bin/python" -c 'import platform; print(platform.python_version())'); case "$H" in "$W"*) ;; *) echo "Achtung: die Datei will Python $W, die Umgebung hat $H – manche Pakete passen dann nicht."; esac` : "true",
        packages.length ? `"$V/bin/python" -m pip install --disable-pip-version-check ${packages.map(sq).join(" ")}` : "true",
    ], `Baue „${file.name}“ mit pip`);
}

async function setupMicromamba() {
    return task("micromamba einrichten", [
        `D="${MICROMAMBA_HOME}"`,
        `mkdir -p "$D/bin"`,
        `case "$(uname -s)-$(uname -m)" in`,
        `  Linux-x86_64) A=linux-64 ;; Linux-aarch64|Linux-arm64) A=linux-aarch64 ;; Linux-ppc64le) A=linux-ppc64le ;;`,
        `  Darwin-arm64) A=osx-arm64 ;; Darwin-x86_64) A=osx-64 ;;`,
        `  *) echo "Für $(uname -s) $(uname -m) gibt es kein micromamba."; exit 1 ;;`,
        `esac`,
        `PY=$(command -v python3 || true)`,
        `if [ -z "$PY" ]; then ${serverPythonLines().join("; ")}; PY=$SPY; fi`,
        `echo "Lade micromamba ($A) …"`,
        `"$PY" - "$A" "$D" <<'EOF'`,
        `import io, os, sys, tarfile, urllib.request`,
        `def fetch(url):`,
        `    print("  " + url, flush=True)`,
        `    return urllib.request.urlopen(url, timeout=300).read()`,
        `try:`,
        `    with tarfile.open(fileobj=io.BytesIO(fetch("https://micro.mamba.pm/api/micromamba/%s/latest" % sys.argv[1])), mode="r:bz2") as t:`,
        `        binary = t.extractfile("bin/micromamba").read()`,
        `except Exception as e:`,
        `    print("  nicht erreichbar (%s), nehme GitHub" % e, flush=True)`,
        `    binary = fetch("https://github.com/mamba-org/micromamba-releases/releases/latest/download/micromamba-%s" % sys.argv[1])`,
        `path = os.path.join(sys.argv[2], "bin", "micromamba")`,
        `with open(path + ".part", "wb") as f:`,
        `    f.write(binary)`,
        `os.chmod(path + ".part", 0o755)`,
        `os.replace(path + ".part", path)`,
        `print("micromamba liegt in", path)`,
        `EOF`,
        `"$D/bin/micromamba" --version`,
    ]);
}

async function buildFromRequirements(file) {
    const dir = dirname(file.path) + "/.venv";
    const label = `${basename(dirname(file.path))} (.venv)`;
    return buildVenv(dir, label, [`"$V/bin/python" -m pip install --disable-pip-version-check -r ${sq(file.path)}`], `Baue .venv aus ${basename(file.path)}`);
}

/// Pakete mit pip in die Umgebung des aktiven Kernels.
async function installPackages(text) {
    const spec = state.session && state.specs.kernelspecs[state.session.kernel.name];
    const packages = text.split(/\s+/).filter(Boolean);
    if (!spec || !packages.length) return;
    const argv0 = (spec.spec.argv || [])[0] || "";
    const code = await task(`Installiere ${packages.join(", ")} in „${spec.spec.display_name}“`, [
        ...(argv0.startsWith("/") ? [`PY=${sq(argv0)}`] : [...serverPythonLines(), `PY=$SPY`]),
        `"$PY" -m pip install --disable-pip-version-check ${packages.map(sq).join(" ")}`,
    ]);
    if (code === 0) atlas.message("Installiert – schon geladene Pakete ändern sich erst nach einem Neustart des Kernels");
}

async function removeKernel(name) {
    const code = await task(`Kernel „${specLabel(name)}“ abmelden`, [...serverPythonLines(), `"$SJ" kernelspec remove -y ${sq(name)}`]);
    if (code !== 0) return;
    if (state.session && state.session.kernel.name === name) {
        try { await api("DELETE", `/api/sessions/${state.session.id}`); } catch (e) { /* schon weg */ }
        state.session = null;
        closeSocket();
        setKernelState("off");
        notice("Die Umgebung des Notebooks ist abgemeldet.", { actions: [{ label: "Umgebung wählen", primary: true, run: openPicker }] });
    }
    await discover();
    renderPicker();
}

// ----------------------------------------------------------- Auswahl (UI)

function closePicker() { $("picker").hidden = true; }

function form(children) {
    const f = $("picker-form");
    f.replaceChildren(...children);
    f.hidden = !children.length;
    const input = f.querySelector("input");
    if (input) input.focus();
}

async function openPicker(keepForm) {
    hideMenu();
    const picker = $("picker");
    picker.hidden = false;
    const r = $("env").getBoundingClientRect();
    picker.style.top = r.bottom + 6 + "px";
    picker.style.left = Math.max(8, Math.min(r.right - picker.offsetWidth, window.innerWidth - picker.offsetWidth - 8)) + "px";
    if (keepForm !== true) form([]);
    $("picker-filter").value = "";
    $("picker-filter").focus();
    renderPicker(true);
    await discover();
    renderPicker();
}

function item(opts) {
    const node = el("div", {
        class: "item", tabIndex: 0, role: "button", title: opts.title || "",
        on: { click: opts.run, keydown: (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); opts.run(); } } },
    },
        el("span", { class: "mark", textContent: opts.active ? "●" : "" }),
        el("span", { class: "name", textContent: opts.name }),
        el("span", { class: "side" }, ...(opts.side || [])),
        el("span", { class: "meta", textContent: opts.meta || "" }));
    node.dataset.search = (opts.name + " " + (opts.meta || "")).toLowerCase();
    return node;
}

function badge(text, go) { return el("span", { class: "badge" + (go ? " go" : ""), textContent: text }); }

function removeButton(name) {
    const x = el("button", { class: "x", textContent: "✕", title: "Kernel abmelden (die Umgebung selbst bleibt)" });
    x.addEventListener("click", (e) => {
        e.stopPropagation();
        const row = x.closest(".item");
        if (!row.classList.contains("confirm")) {
            row.classList.add("confirm");
            x.textContent = "abmelden?";
            setTimeout(() => { row.classList.remove("confirm"); x.textContent = "✕"; }, 3000);
            return;
        }
        closePicker();
        removeKernel(name);
    });
    return x;
}

function renderPicker(loading) {
    const list = $("picker-list");
    const found = state.found || { envs: [], files: [], kernels: [] };
    const active = state.session && state.session.kernel.name;
    const sections = [];
    const add = (title, items) => { if (items.length) sections.push(el("h4", { textContent: title }), ...items); };

    if (!state.online) {
        sections.push(el("p", { class: "empty", textContent: "Der Jupyter-Server läuft nicht – Umgebungen lassen sich erst mit ihm wählen." }));
    }
    add("Umgebungen", found.envs.map((env) => item({
        name: envLabel(env),
        meta: [env.kind === "conda" ? "Conda" : "venv", shortPath(env.path), env.version && "Python " + env.version].filter(Boolean).join(" · "),
        active: env.kernel && env.kernel === active,
        side: env.kernel ? [removeButton(env.kernel)] : [badge("einbinden")],
        title: env.kernel ? "Diese Umgebung verwenden" : "ipykernel sicherstellen, als Kernel anmelden und verwenden",
        run: () => { closePicker(); env.kernel ? useKernel(env.kernel) : registerAndUse(env.path, envLabel(env)); },
    })));
    add("Aus Datei", found.files.map((file) => {
        const isConda = file.kind === "conda";
        const meta = isConda
            ? ["Conda-Datei", relative(file.path), file.python && "Python " + file.python, packageCount(file.conda.length + file.pip.length)].filter(Boolean).join(" · ")
            : ["requirements", relative(file.path), packageCount(file.count)].join(" · ");
        const exists = !!file.env;
        return item({
            name: isConda ? `${file.name}` : `${basename(dirname(file.path))} (.venv)`,
            meta,
            active: exists && file.env.kernel === active,
            side: exists ? [badge(isConda ? "aktualisieren" : "neu installieren")] : [badge("erstellen", true)],
            title: exists ? "Pakete der Datei erneut in die vorhandene Umgebung" : "Umgebung aus der Datei bauen, als Kernel anmelden und verwenden",
            run: () => { closePicker(); isConda ? buildCondaEnv(file) : buildFromRequirements(file); },
        });
    }));
    const others = found.kernels.filter((k) => !k.env);
    add("Weitere Kernel", others.map((k) => item({
        name: k.label,
        meta: k.python.startsWith("/") ? shortPath(k.python) : "Python des Jupyter-Servers",
        active: k.name === active,
        side: k.python.startsWith("/") ? [removeButton(k.name)] : [],
        run: () => { closePicker(); useKernel(k.name); },
    })));
    if (loading) sections.push(el("p", { class: "empty", textContent: "Suche Umgebungen …" }));
    else if (state.online && !found.envs.length && !found.files.length && !found.kernels.length) {
        sections.push(el("p", { class: "empty", textContent: "Noch keine Umgebung gefunden – unten eine .venv anlegen." }));
    }
    const actions = [
        item({ name: "+ Neue .venv …", meta: "Python-Umgebung im Projekt, mit Paketen", run: newVenvForm }),
    ];
    if (state.session) actions.push(item({ name: "⤓ Pakete installieren …", meta: `pip install in „${specLabel(active)}“`, run: packagesForm }));
    actions.push(item({ name: "↻ Neu suchen", meta: "", run: async () => { renderPicker(true); await discover(); renderPicker(); } }));
    add("Mehr", actions);
    if (found.tool) sections.push(el("p", { class: "empty", textContent: `Conda-Umgebungen mit ${found.tool} (${shortPath(found.toolPath || "")})` }));
    list.replaceChildren(...sections);
    filterPicker();
    $("server-info").textContent = state.online ? `Jupyter-Server · 127.0.0.1:${portOf()}` : "Jupyter-Server läuft nicht";
    $("server-stop").hidden = !state.online;
    $("server-restart").textContent = state.online ? "neu starten" : "starten";
}

function filterPicker() {
    const q = $("picker-filter").value.trim().toLowerCase();
    for (const node of $("picker-list").querySelectorAll(".item")) node.hidden = !!q && !node.dataset.search.includes(q);
    for (const h of $("picker-list").querySelectorAll("h4")) {
        let n = h.nextElementSibling;
        let any = false;
        while (n && n.tagName !== "H4") { if (n.classList.contains("item") && !n.hidden) any = true; n = n.nextElementSibling; }
        h.hidden = !any;
    }
}

function field(label, input) { return el("label", {}, label, input); }

function newVenvForm() {
    const dir = el("input", { type: "text", value: (state.found && state.found.envs.some((e) => e.path === project() + "/.venv")) ? ".venv-2" : ".venv" });
    const pkgs = el("input", { type: "text", value: "numpy pandas matplotlib", placeholder: "mit Leerzeichen getrennt" });
    const go = () => {
        const d = dir.value.trim().replace(/\/+$/, "") || ".venv";
        const path = d.startsWith("/") ? d : `${project()}/${d}`;
        const packages = pkgs.value.split(/\s+/).filter(Boolean);
        closePicker();
        buildVenv(path, `${basename(project())} (${basename(path)})`,
            packages.length ? [`"$V/bin/python" -m pip install --disable-pip-version-check ${packages.map(sq).join(" ")}`] : [],
            `Lege ${basename(path)} an`);
    };
    for (const i of [dir, pkgs]) i.addEventListener("keydown", (e) => { if (e.key === "Enter") go(); });
    form([field("Ordner (relativ zum Projekt)", dir), field("Pakete", pkgs),
        el("div", { class: "row" }, button("Abbrechen", "", () => form([])), button("Anlegen und verwenden", "", go, "primary"))]);
}

function packagesForm() {
    const pkgs = el("input", { type: "text", placeholder: "z. B. scipy seaborn" });
    const go = () => { const t = pkgs.value; closePicker(); installPackages(t); };
    pkgs.addEventListener("keydown", (e) => { if (e.key === "Enter") go(); });
    form([field(`Pakete für „${specLabel(state.session.kernel.name)}“`, pkgs),
        el("div", { class: "row" }, button("Abbrechen", "", () => form([])), button("Installieren", "", go, "primary"))]);
}

// --------------------------------------------------------------- Ordner

/// Ohne Notebook (Ansicht auf dem Projekt geöffnet): Liste und „Neues Notebook“.
async function showBrowser(dir) {
    $("browser").hidden = false;
    $("title").textContent = "Jupyter-Notebooks";
    $("env").hidden = true;
    document.querySelector(".actions").hidden = true;
    let files = [];
    try { files = await atlas.call("listFiles", { dir, pattern: "*.ipynb" }); } catch (e) { /* leer */ }
    files = files.filter((f) => !f.dir);
    $("notebooks").replaceChildren(...(files.length
        ? files.map((f) => el("li", {}, el("a", { textContent: f.name, on: { click: () => atlas.open(f.path) } })))
        : [el("li", { class: "empty", textContent: "Noch keine Notebooks in diesem Ordner." })]));
    $("new-notebook").onclick = async () => {
        let name = $("new-name").value.trim() || "Unbenannt.ipynb";
        if (!name.endsWith(".ipynb")) name += ".ipynb";
        const path = dir.replace(/\/$/, "") + "/" + name;
        if (await atlas.call("exists", { path })) { atlas.message(`„${name}“ gibt es schon`); return; }
        const nb = {
            cells: [{ cell_type: "code", execution_count: null, id: cellId(), metadata: {}, outputs: [], source: [] }],
            metadata: { language_info: { name: "python" } },
            nbformat: 4, nbformat_minor: 5,
        };
        await atlas.call("writeFile", { path, text: JSON.stringify(sortKeys(nb), null, 1) + "\n" });
        atlas.open(path);
        showBrowser(dir);
    };
}

// --------------------------------------------------------------- Start

function detectDark() {
    const m = /rgba?\((\d+),\s*(\d+),\s*(\d+)/.exec(getComputedStyle(document.body).backgroundColor);
    if (!m) return;
    const [r, g, b] = m.slice(1).map(Number);
    document.documentElement.toggleAttribute("data-dark", 0.2126 * r + 0.7152 * g + 0.0722 * b < 128);
}

async function main() {
    const ctx = await atlas.ready;
    state.ctx = ctx;
    state.file = ctx.file || "";
    state.base = "http://127.0.0.1:" + portOf();
    detectDark();

    $("run-all").onclick = () => enqueue(state.cells);
    $("interrupt").onclick = interrupt;
    $("restart").onclick = () => restart();
    $("restart-menu").onclick = (e) => { e.stopPropagation(); restartMenu(); };
    $("more").onclick = (e) => { e.stopPropagation(); moreMenu(); };
    $("env").onclick = (e) => { e.stopPropagation(); $("picker").hidden ? openPicker() : closePicker(); };
    $("add-code").onclick = () => insertCell(state.cells.length, "code", "edit");
    $("add-markdown").onclick = () => insertCell(state.cells.length, "markdown", "edit");
    $("picker-filter").addEventListener("input", filterPicker);
    $("picker-filter").addEventListener("keydown", (e) => {
        if (e.key === "Escape") closePicker();
        if (e.key === "Enter") { const first = [...$("picker-list").querySelectorAll(".item")].find((n) => !n.hidden); if (first) first.click(); }
    });
    for (const id of ["picker", "menu", "env", "more", "restart-menu", "task"]) $(id).addEventListener("mousedown", (e) => e.stopPropagation());
    $("server-restart").onclick = () => { closePicker(); restartServer(true); };
    $("server-stop").onclick = () => { closePicker(); restartServer(false); };
    $("task-log-toggle").onclick = () => { $("task-log").hidden = !$("task-log").hidden; };
    $("task-close").onclick = () => { $("task").hidden = true; };
    document.addEventListener("mousedown", () => { hideMenu(); closePicker(); });
    document.addEventListener("keydown", (e) => {
        if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") { e.preventDefault(); state.dirty = true; save(); }
        if (e.key === "Escape") { hideMenu(); if (!$("picker").hidden) closePicker(); }
    });
    window.addEventListener("focus", checkExternalChange);
    window.addEventListener("beforeunload", () => { if (state.dirty) save(); });

    if (!state.file.toLowerCase().endsWith(".ipynb")) {
        await showBrowser(state.file || ctx.project || "");
        return;
    }
    $("title").textContent = basename(state.file);
    $("title").title = state.file;
    let text;
    try {
        text = await atlas.call("readFile", { path: state.file });
    } catch (e) {
        notice("Die Datei ist nicht lesbar: " + e.message, { error: true });
        return;
    }
    if (!loadNotebook(text)) return;
    setKernelState("off");
    await connect();
}

main().catch((e) => notice("Fehler: " + e.message, { error: true }));
