// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

// Jupyter-Notebook-Ansicht (siehe notebook.html). Ablauf:
//
// 1. Die Datei kommt über die Brücke (atlas.call("readFile")), gespeichert
//    wird sie wieder über die Brücke (nbformat 4, Einrückung 1, sortierte
//    Schlüssel wie bei Jupyter).
// 2. Der Server läuft auf 127.0.0.1:<port>; sein Token schreibt server.sh nach
//    .atlas/cache/jupyter/token im Projekt. Läuft keiner, startet der Knopf
//    den Befehl „server“ der Erweiterung im Terminal von Atlas.
// 3. Pro Notebook eine Sitzung (POST /api/sessions, beim Wiederöffnen
//    dieselbe); die Zellen laufen über den WebSocket des Kernels
//    (execute_request, Ausgaben auf iopub), eine nach der anderen.
//
// HTML aus Ausgaben und Markdown wird bereinigt; Skripte laufen nie.
"use strict";

const $ = (id) => document.getElementById(id);

const state = {
    ctx: null,
    file: "",
    nb: null,
    cells: [],          // {model, el, editor, prompt, outputs, running, queued}
    selected: null,
    lastText: null,     // zuletzt gelesener oder geschriebener Dateiinhalt
    dirty: false,
    saveTimer: 0,
    base: "",
    token: "",
    session: null,      // {id, kernel: {id, name}}
    ws: null,
    clientId: uuid(),
    kernelState: "off",
    queue: [],          // Zellen, die noch laufen sollen
    current: null,      // {cell, msgId, idle, reply}
    pendingClear: new WeakMap(),
    specs: null,
    restarting: false,  // eigener Neustart läuft
    infoWaiters: new Map(), // msg_id von kernel_info_request → Rückruf
};

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

function basename(p) { return p.split("/").pop(); }

function dirname(p) { const i = p.lastIndexOf("/"); return i > 0 ? p.slice(0, i) : "/"; }

function relative(path) {
    const root = (state.ctx.project || "").replace(/\/$/, "");
    return root && path.startsWith(root + "/") ? path.slice(root.length + 1) : path;
}

// ------------------------------------------------------------------- Status

function setSaved(text, error) {
    const el = $("saved");
    el.textContent = text;
    el.classList.toggle("error", !!error);
}

function banner(text, opts = {}) {
    const el = $("banner");
    el.hidden = !text;
    el.classList.toggle("error", !!opts.error);
    $("banner-text").textContent = text || "";
    $("start-server").hidden = !opts.start;
    $("retry").hidden = !opts.retry;
}

function setKernelState(s, label) {
    state.kernelState = s;
    const el = $("kernel");
    el.className = "kernel " + s;
    el.textContent = label || ({ idle: "bereit", busy: "rechnet …", starting: "startet …", restarting: "startet neu …", dead: "Kernel beendet", off: "kein Kernel" }[s] || s);
    const connected = s !== "off" && s !== "dead";
    $("run-all").disabled = !connected;
    $("interrupt").disabled = !connected;
    $("restart").disabled = !connected;
    $("restart-all").disabled = !connected;
    for (const c of state.cells) c.runButton.disabled = !connected && c.model.cell_type === "code";
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
    root.querySelectorAll("span.math[data-tex]").forEach((el) => {
        if (!window.katex) { el.textContent = el.dataset.tex; return; }
        try {
            katex.render(el.dataset.tex, el, { displayMode: el.classList.contains("display"), throwOnError: false });
        } catch (e) {
            el.textContent = el.dataset.tex;
        }
    });
}

/// Links in gerendertem Markdown: Dateien öffnen in Atlas, Webseiten im Browser.
function wireLinks(root) {
    root.querySelectorAll("a[data-atlas-path]").forEach((a) => {
        a.addEventListener("click", (e) => { e.preventDefault(); atlas.open(a.dataset.atlasPath); });
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
    wireLinks(out);
    return out;
}

// --------------------------------------------------------------------- ANSI

const ANSI_COLORS = ["#000000", "#c0392b", "#2e8b57", "#b7791f", "#2f6fdb", "#a23bc4", "#138d90", "#a0a0a0",
    "#666666", "#e74c3c", "#27ae60", "#d4a017", "#4a90e2", "#c158dc", "#1abc9c", "#ffffff"];

/// Text mit ANSI-Farben (Tracebacks von IPython) als HTML.
function ansiHtml(text) {
    let out = "";
    let style = { fg: null, bg: null, bold: false };
    let open = false;
    const parts = String(text).split(/\x1b\[([0-9;]*)m/);
    for (let i = 0; i < parts.length; i++) {
        if (i % 2 === 0) {
            if (parts[i]) out += escapeHtml(parts[i].replace(/\x1b\[[0-9;?]*[A-Za-z]/g, ""));
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

// ----------------------------------------------------------------- Ausgaben

/// Wendet Wagenrücklauf und Rückschritt an wie ein Terminal (Fortschrittsbalken).
function terminalText(text) {
    return text.split("\n").map((line) => {
        if (line.includes("\r")) {
            const parts = line.split("\r").filter((p, i, all) => p !== "" || i === all.length - 1);
            line = parts.length ? parts[parts.length - 1] : "";
        }
        while (/[^\x08]\x08/.test(line)) line = line.replace(/[^\x08]\x08/, "");
        return line;
    }).join("\n");
}

const MIME_ORDER = ["application/vnd.jupyter.widget-view+json", "text/html", "image/svg+xml", "image/png", "image/jpeg", "image/gif",
    "text/markdown", "text/latex", "application/json", "text/plain"];

function renderData(data, container) {
    const types = Object.keys(data || {});
    let mime = MIME_ORDER.find((m) => types.includes(m) && m !== "application/vnd.jupyter.widget-view+json");
    if (!mime) mime = types.find((t) => t.startsWith("text/"));
    if (!mime) { container.textContent = "(Ausgabe im Format " + types.join(", ") + " – hier nicht darstellbar)"; return; }
    const value = data[mime];
    if (mime === "text/html") {
        container.innerHTML = sanitize(joined(value));
        renderMath(container);
    } else if (mime === "image/svg+xml") {
        const img = document.createElement("img");
        img.src = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(joined(value));
        container.appendChild(img);
    } else if (mime.startsWith("image/")) {
        const img = document.createElement("img");
        img.src = "data:" + mime + ";base64," + joined(value).replace(/\s/g, "");
        container.appendChild(img);
    } else if (mime === "text/markdown") {
        const holder = document.createElement("div");
        holder.className = "md";
        container.appendChild(holder);
        markdownHtml(joined(value)).then((el) => holder.replaceChildren(...el.childNodes));
    } else if (mime === "text/latex") {
        const tex = joined(value).trim().replace(/^\$\$?|\$\$?$/g, "");
        const holder = document.createElement("div");
        if (window.katex) katex.render(tex, holder, { displayMode: true, throwOnError: false });
        else holder.textContent = joined(value);
        container.appendChild(holder);
    } else if (mime === "application/json") {
        const pre = document.createElement("pre");
        pre.textContent = JSON.stringify(value, null, 2);
        container.appendChild(pre);
    } else {
        const pre = document.createElement("pre");
        pre.innerHTML = ansiHtml(terminalText(joined(value)));
        container.appendChild(pre);
    }
}

function outputElement(out) {
    const el = document.createElement("div");
    el.className = "output";
    if (out.output_type === "stream") {
        el.classList.add(out.name === "stderr" ? "stderr" : "stdout");
        const pre = document.createElement("pre");
        pre.innerHTML = ansiHtml(terminalText(joined(out.text)));
        el.appendChild(pre);
    } else if (out.output_type === "error") {
        el.classList.add("error");
        const pre = document.createElement("pre");
        const tb = (out.traceback || []).join("\n");
        pre.innerHTML = ansiHtml(tb || `${out.ename}: ${out.evalue}`);
        el.appendChild(pre);
    } else {
        renderData(out.data, el);
    }
    return el;
}

function renderOutputs(cell) {
    const outs = cell.model.outputs || [];
    cell.outputs.replaceChildren(...outs.map(outputElement));
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
        cell.outputs.lastChild.replaceWith(outputElement(last));
    } else {
        if (out.output_type === "stream") out.text = terminalText(joined(out.text));
        outs.push(out);
        cell.outputs.appendChild(outputElement(out));
    }
    changed();
}

// -------------------------------------------------------------------- Zellen

function promptText(cell) {
    if (cell.model.cell_type !== "code") return "";
    if (cell.running || cell.queued) return "[*]:";
    const n = cell.model.execution_count;
    return "[" + (n == null ? " " : n) + "]:";
}

function updatePrompt(cell) {
    cell.prompt.textContent = promptText(cell);
    cell.el.classList.toggle("running", !!(cell.running || cell.queued));
}

function autosize(textarea) {
    textarea.style.height = "auto";
    textarea.style.height = textarea.scrollHeight + 2 + "px"; // + Rahmen
}

function select(cell, focus) {
    if (state.selected && state.selected !== cell) state.selected.el.classList.remove("selected");
    state.selected = cell;
    if (!cell) return;
    cell.el.classList.add("selected");
    if (focus) {
        if (cell.model.cell_type === "markdown" && cell.el.classList.contains("rendered")) cell.el.focus();
        else cell.editor.focus();
    }
}

async function renderMarkdownCell(cell) {
    const out = await markdownHtml(cell.editor.value, cell.model.attachments);
    cell.markdownOut.replaceChildren(...out.childNodes);
    cell.el.classList.add("rendered");
}

function editMarkdown(cell) {
    cell.el.classList.remove("rendered");
    autosize(cell.editor);
    cell.editor.focus();
}

function button(text, title, onClick, cls) {
    const b = document.createElement("button");
    b.textContent = text;
    b.title = title;
    if (cls) b.className = cls;
    b.addEventListener("click", (e) => { e.stopPropagation(); onClick(); });
    return b;
}

function makeCell(model) {
    const cell = { model, running: false, queued: false };
    const el = document.createElement("div");
    el.className = "cell " + model.cell_type;
    el.tabIndex = -1;
    const prompt = document.createElement("div");
    prompt.className = "prompt";
    const body = document.createElement("div");
    body.className = "body";

    const tools = document.createElement("div");
    tools.className = "tools";
    cell.runButton = button("▶", "Zelle ausführen (Strg+Enter)", () => runCell(cell, "stay"));
    const typeSelect = document.createElement("select");
    for (const [v, t] of [["code", "Code"], ["markdown", "Markdown"], ["raw", "Roh"]]) typeSelect.add(new Option(t, v));
    typeSelect.value = model.cell_type;
    typeSelect.title = "Art der Zelle";
    typeSelect.addEventListener("change", () => setCellType(cell, typeSelect.value));
    tools.append(
        cell.runButton, typeSelect,
        button("↑", "Nach oben", () => moveCell(cell, -1)),
        button("↓", "Nach unten", () => moveCell(cell, 1)),
        button("+", "Neue Code-Zelle darunter", () => insertCell(cell, "code", true)),
        button("✕", "Zelle löschen", () => deleteCell(cell), "danger"),
    );

    const editor = document.createElement("textarea");
    editor.className = "source";
    editor.rows = 1;
    editor.spellcheck = model.cell_type === "markdown";
    editor.value = joined(model.source);
    editor.addEventListener("input", () => {
        model.source = editor.value;
        autosize(editor);
        changed();
    });
    editor.addEventListener("keydown", (e) => onEditorKey(cell, e));
    editor.addEventListener("focus", () => select(cell));

    const markdownOut = document.createElement("div");
    markdownOut.className = "markdown-out";
    markdownOut.addEventListener("dblclick", () => editMarkdown(cell));

    const outputs = document.createElement("div");
    outputs.className = "outputs";

    body.append(tools, editor, markdownOut, outputs);
    el.append(prompt, body);
    el.addEventListener("mousedown", () => select(cell));
    el.addEventListener("keydown", (e) => {
        if (e.target !== el) return;
        if (e.key === "Enter" && !e.shiftKey && !e.ctrlKey && !e.altKey) { e.preventDefault(); editMarkdown(cell); }
        else if (e.key === "Enter") onEditorKey(cell, e);
    });

    Object.assign(cell, { el, prompt, editor, markdownOut, outputs, typeSelect });
    updatePrompt(cell);
    if (model.cell_type === "code") renderOutputs(cell);
    if (model.cell_type === "markdown") renderMarkdownCell(cell);
    return cell;
}

function onEditorKey(cell, e) {
    if (e.key === "Enter" && e.shiftKey) { e.preventDefault(); runCell(cell, "next"); return; }
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); runCell(cell, "stay"); return; }
    if (e.key === "Enter" && e.altKey) { e.preventDefault(); runCell(cell, "insert"); return; }
    if (e.target !== cell.editor) return;
    const ta = cell.editor;
    if (e.key === "Tab" && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        const { selectionStart: s, selectionEnd: end, value } = ta;
        if (e.shiftKey) {
            const start = value.lastIndexOf("\n", s - 1) + 1;
            const remove = value.slice(start, start + 4).match(/^ {1,4}/);
            if (remove) {
                ta.setRangeText("", start, start + remove[0].length, "end");
                ta.selectionStart = ta.selectionEnd = Math.max(start, s - remove[0].length);
            }
        } else {
            ta.setRangeText("    ", s, end, "end");
        }
        ta.dispatchEvent(new Event("input"));
    } else if (e.key === "Enter" && !e.shiftKey && cell.model.cell_type === "code") {
        // Einrückung der Zeile übernehmen, nach „:“ eine Stufe mehr.
        const { selectionStart: s, selectionEnd: end, value } = ta;
        const line = value.slice(value.lastIndexOf("\n", s - 1) + 1, s);
        let indent = line.match(/^\s*/)[0];
        if (/:\s*(#.*)?$/.test(line)) indent += "    ";
        e.preventDefault();
        ta.setRangeText("\n" + indent, s, end, "end");
        ta.dispatchEvent(new Event("input"));
    } else if (e.key === "Escape") {
        cell.el.focus();
    }
}

function newModel(type) {
    const m = { cell_type: type, metadata: {}, source: "" };
    if (state.nb.nbformat > 4 || (state.nb.nbformat === 4 && state.nb.nbformat_minor >= 5)) m.id = cellId();
    if (type === "code") { m.execution_count = null; m.outputs = []; }
    return m;
}

function insertCell(after, type, focus) {
    const cell = makeCell(newModel(type));
    if (type === "markdown") cell.el.classList.remove("rendered");
    const index = after ? state.cells.indexOf(after) + 1 : state.cells.length;
    state.cells.splice(index, 0, cell);
    const next = state.cells[index + 1];
    $("cells").insertBefore(cell.el, next ? next.el : null);
    autosize(cell.editor);
    select(cell, focus);
    changed();
    return cell;
}

function deleteCell(cell) {
    if (cell.running) return;
    const i = state.cells.indexOf(cell);
    state.queue = state.queue.filter((c) => c !== cell);
    state.cells.splice(i, 1);
    cell.el.remove();
    select(state.cells[Math.min(i, state.cells.length - 1)] || null);
    changed();
}

function moveCell(cell, delta) {
    const i = state.cells.indexOf(cell);
    const j = i + delta;
    if (j < 0 || j >= state.cells.length) return;
    state.cells.splice(i, 1);
    state.cells.splice(j, 0, cell);
    const after = state.cells[j + 1];
    $("cells").insertBefore(cell.el, after ? after.el : null);
    changed();
}

function setCellType(cell, type) {
    if (type === cell.model.cell_type) return;
    const old = cell.model;
    const model = { cell_type: type, metadata: old.metadata || {}, source: cell.editor.value };
    if (old.id) model.id = old.id;
    if (type === "code") { model.execution_count = null; model.outputs = []; }
    if (type === "markdown" && old.attachments) model.attachments = old.attachments;
    const fresh = makeCell(model);
    const i = state.cells.indexOf(cell);
    state.cells[i] = fresh;
    cell.el.replaceWith(fresh.el);
    if (type === "markdown") fresh.el.classList.remove("rendered");
    autosize(fresh.editor);
    select(fresh, true);
    setKernelState(state.kernelState);
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
    const nb = Object.assign({}, state.nb, { cells });
    return JSON.stringify(sortKeys(nb), null, 1) + "\n";
}

function changed() {
    state.dirty = true;
    setSaved("ungespeichert");
    clearTimeout(state.saveTimer);
    state.saveTimer = setTimeout(save, 800);
}

async function save() {
    clearTimeout(state.saveTimer);
    if (!state.nb || !state.dirty) return;
    const text = serialize();
    state.dirty = false;
    try {
        await atlas.call("writeFile", { path: state.file, text });
        state.lastText = text;
        setSaved("gespeichert");
    } catch (e) {
        state.dirty = true;
        setSaved("nicht gespeichert: " + e.message, true);
    }
}

/// Von außen geändert (z. B. Git oder ein anderer Editor): neu laden, solange
/// hier nichts Ungespeichertes ist und nichts läuft.
async function checkExternalChange() {
    if (!state.nb || state.dirty || state.current || state.queue.length) return;
    let text;
    try { text = await atlas.call("readFile", { path: state.file }); } catch (e) { return; }
    if (text !== state.lastText) {
        loadNotebook(text);
        atlas.message("Notebook neu geladen – die Datei wurde außerhalb geändert");
    }
}

// --------------------------------------------------------------- Laden

function loadNotebook(text) {
    let nb;
    try {
        nb = JSON.parse(text);
    } catch (e) {
        banner("Die Datei ist kein gültiges Notebook (JSON): " + e.message, { error: true });
        return false;
    }
    if (!nb || !Array.isArray(nb.cells)) {
        banner(nb && nb.nbformat < 4 ? "Notebooks im Format 3 und älter werden nicht unterstützt – bitte mit „jupyter nbconvert --to notebook“ umwandeln."
            : "Die Datei ist kein Notebook (es fehlen die Zellen).", { error: true });
        return false;
    }
    nb.metadata = nb.metadata || {};
    state.nb = nb;
    state.lastText = text;
    state.dirty = false;
    const selectedIndex = state.selected ? state.cells.indexOf(state.selected) : 0;
    state.cells = nb.cells.map((m) => {
        m.source = joined(m.source);
        return makeCell(m);
    });
    delete nb.cells;
    $("cells").replaceChildren(...state.cells.map((c) => c.el));
    state.cells.forEach((c) => autosize(c.editor));
    $("add-row").hidden = false;
    select(state.cells[Math.max(0, Math.min(selectedIndex, state.cells.length - 1))] || null);
    setKernelState(state.kernelState);
    setSaved("");
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

function tokenPath() { return (state.ctx.project || "").replace(/\/$/, "") + "/.atlas/cache/jupyter/token"; }

/// Verbindet mit dem Server; `wait`: so lange warten, bis er läuft (nach dem Start).
async function connect(wait) {
    if (!state.ctx.project) {
        banner("Notebooks laufen nur in einem Projektordner (dort startet der Jupyter-Server).", { error: true });
        return;
    }
    const deadline = Date.now() + (wait ? 90000 : 0);
    for (;;) {
        try {
            state.token = (await atlas.call("readFile", { path: tokenPath() })).trim();
            await api("GET", "/api/status");
            break;
        } catch (e) {
            if (Date.now() < deadline) {
                banner("Warte auf den Jupyter-Server …");
                await new Promise((r) => setTimeout(r, 1000));
                continue;
            }
            setKernelState("off");
            if (e.status === 403 || e.status === 401) {
                banner(`Auf Port ${portOf()} läuft ein anderer Jupyter-Server (Token passt nicht). Port in den Einstellungen der Erweiterung ändern oder den anderen Server beenden.`, { error: true, retry: true });
            } else {
                banner(`Kein Jupyter-Server auf 127.0.0.1:${portOf()}. Er startet in einem Terminal-Tab von Atlas.`, { start: true, retry: true });
            }
            return;
        }
    }
    banner("");
    await startSession();
}

function portOf() { return Math.round(Number(state.ctx.settings && state.ctx.settings.port) || 8899); }

async function loadSpecs() {
    if (state.specs) return state.specs;
    state.specs = await api("GET", "/api/kernelspecs");
    const sel = $("kernelspec");
    sel.replaceChildren(...Object.entries(state.specs.kernelspecs).map(([name, s]) => new Option(s.spec.display_name || name, name)));
    sel.hidden = false;
    return state.specs;
}

async function startSession() {
    const specs = await loadSpecs();
    const wanted = (state.nb.metadata.kernelspec || {}).name;
    const name = wanted && specs.kernelspecs[wanted] ? wanted : specs.default;
    if (wanted && name !== wanted) atlas.message(`Kernel „${wanted}“ fehlt auf dem Server – nehme „${name}“`);
    setKernelState("starting");
    const path = relative(state.file);
    try {
        // Dieselbe Sitzung wie beim letzten Öffnen (Variablen bleiben erhalten).
        const sessions = await api("GET", "/api/sessions");
        state.session = sessions.find((s) => s.path === path && s.kernel) || null;
        if (!state.session) {
            state.session = await api("POST", "/api/sessions", { path, name: basename(path), type: "notebook", kernel: { name } });
        }
    } catch (e) {
        setKernelState("dead", "Kernel startet nicht");
        banner("Der Kernel startet nicht: " + e.message, { error: true, retry: true });
        return;
    }
    $("kernelspec").value = state.session.kernel.name;
    openSocket();
}

function openSocket() {
    if (state.ws) { state.ws.onclose = null; state.ws.close(); }
    const kernel = state.session.kernel.id;
    const url = state.base.replace(/^http/, "ws") + `/api/kernels/${kernel}/channels?session_id=${state.clientId}&token=${encodeURIComponent(state.token)}`;
    const ws = new WebSocket(url);
    state.ws = ws;
    ws.onopen = () => {
        setKernelState("idle");
        send("shell", "kernel_info_request", {});
    };
    ws.onmessage = (e) => { try { onMessage(JSON.parse(e.data)); } catch (err) { console.warn(err); } };
    ws.onclose = () => {
        if (state.ws !== ws) return;
        state.ws = null;
        failRunning("Verbindung zum Kernel getrennt");
        setKernelState("off", "getrennt");
        banner("Die Verbindung zum Jupyter-Server ist getrennt.", { retry: true, start: true });
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

// --------------------------------------------------------------- Ausführen

function runCell(cell, then) {
    if (cell.model.cell_type === "markdown") renderMarkdownCell(cell);
    else if (cell.model.cell_type === "code") enqueue([cell]);
    let i = state.cells.indexOf(cell);
    if (then === "next" || then === "insert") {
        if (then === "insert" || i === state.cells.length - 1) insertCell(cell, "code", true);
        else select(state.cells[i + 1], true);
    }
}

function enqueue(cells) {
    if (!state.ws || state.ws.readyState !== WebSocket.OPEN) {
        atlas.message("Kein Kernel verbunden – erst den Jupyter-Server starten");
        return;
    }
    for (const c of cells) {
        if (c.model.cell_type !== "code" || c.queued || c.running) continue;
        c.queued = true;
        updatePrompt(c);
        state.queue.push(c);
    }
    next();
}

function next() {
    if (state.current) return;
    const cell = state.queue.shift();
    if (!cell) return;
    if (!state.cells.includes(cell)) { next(); return; }
    cell.queued = false;
    cell.running = true;
    updatePrompt(cell);
    // Die alten Ausgaben verschwinden mit der ersten neuen (kein Flackern).
    state.pendingClear.set(cell, true);
    const msgId = send("shell", "execute_request", {
        code: cell.editor.value, silent: false, store_history: true, user_expressions: {}, allow_stdin: false, stop_on_error: true,
    });
    state.current = { cell, msgId, idle: false, reply: null };
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
    updatePrompt(cell);
    changed();
    if (reply && reply.status !== "ok") clearQueue();
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
        updatePrompt(cell);
    }
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
        const { cell, out } = target;
        out.data = c.data;
        out.metadata = c.metadata || {};
        renderOutputs(cell);
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
    try { await api("POST", `/api/kernels/${state.session.kernel.id}/interrupt`); } catch (e) { atlas.message("Unterbrechen ging nicht: " + e.message); }
}

/// Wartet, bis der Kernel antwortet (kernel_info_reply), höchstens `ms`.
function kernelReady(ms) {
    return new Promise((resolve) => {
        const deadline = Date.now() + ms;
        const ask = () => {
            if (!state.ws || state.ws.readyState !== WebSocket.OPEN || Date.now() > deadline) { state.infoWaiters.clear(); resolve(false); return; }
            // Während des Neustarts gehen Anfragen verloren: jede Sekunde neu fragen.
            state.infoWaiters.set(send("shell", "kernel_info_request", {}), () => { clearInterval(timer); resolve(true); });
        };
        const timer = setInterval(ask, 1000);
        ask();
    });
}

async function restart(thenRunAll) {
    clearQueue();
    if (state.current) failRunning("Kernel neu gestartet");
    state.restarting = true;
    setKernelState("restarting");
    try {
        await api("POST", `/api/kernels/${state.session.kernel.id}/restart`);
        // Erst laufen lassen, wenn der neue Kernel antwortet (die Statusmeldungen
        // des Neustarts kommen sonst erst danach).
        if (!(await kernelReady(30000))) throw new Error("der Kernel antwortet nicht");
    } catch (e) {
        atlas.message("Neustart ging nicht: " + e.message);
        return;
    } finally {
        state.restarting = false;
        state.infoWaiters.clear();
    }
    setKernelState("idle");
    if (thenRunAll) enqueue(state.cells);
}

async function changeKernel(name) {
    if (!state.session) return;
    clearQueue();
    try {
        state.session = await api("PATCH", `/api/sessions/${state.session.id}`, { kernel: { name } });
    } catch (e) {
        atlas.message("Kernelwechsel ging nicht: " + e.message);
        return;
    }
    const spec = state.specs.kernelspecs[name].spec;
    state.nb.metadata.kernelspec = { display_name: spec.display_name, language: spec.language, name };
    changed();
    openSocket();
}

// --------------------------------------------------------------- Ordner

/// Ohne Notebook (Ansicht auf dem Projekt geöffnet): Liste und „Neues Notebook“.
async function showBrowser(dir) {
    $("browser").hidden = false;
    $("hint").hidden = true;
    $("title").textContent = "Jupyter-Notebooks";
    const list = $("notebooks");
    let files = [];
    try { files = await atlas.call("listFiles", { dir, pattern: "*.ipynb" }); } catch (e) { /* leer */ }
    files = files.filter((f) => !f.dir);
    list.replaceChildren(...(files.length ? files.map((f) => {
        const li = document.createElement("li");
        const a = document.createElement("a");
        a.textContent = f.name;
        a.addEventListener("click", () => atlas.open(f.path));
        li.appendChild(a);
        return li;
    }) : [Object.assign(document.createElement("li"), { textContent: "Noch keine Notebooks in diesem Ordner.", className: "hint" })]));
    $("new-notebook").onclick = async () => {
        let name = $("new-name").value.trim() || "Unbenannt.ipynb";
        if (!name.endsWith(".ipynb")) name += ".ipynb";
        const path = dir.replace(/\/$/, "") + "/" + name;
        if (await atlas.call("exists", { path })) { atlas.message(`„${name}“ gibt es schon`); return; }
        const nb = {
            cells: [{ cell_type: "code", execution_count: null, id: cellId(), metadata: {}, outputs: [], source: [] }],
            metadata: { kernelspec: { display_name: "Python 3 (ipykernel)", language: "python", name: "python3" }, language_info: { name: "python" } },
            nbformat: 4, nbformat_minor: 5,
        };
        await atlas.call("writeFile", { path, text: JSON.stringify(sortKeys(nb), null, 1) + "\n" });
        atlas.open(path);
        showBrowser(dir);
    };
}

// --------------------------------------------------------------- Start

async function main() {
    const ctx = await atlas.ready;
    state.ctx = ctx;
    state.file = ctx.file || "";
    state.base = "http://127.0.0.1:" + portOf();

    $("run-all").onclick = () => enqueue(state.cells);
    $("interrupt").onclick = interrupt;
    $("restart").onclick = () => restart(false);
    $("restart-all").onclick = () => restart(true);
    $("clear").onclick = () => {
        for (const c of state.cells) if (c.model.cell_type === "code") { c.model.outputs = []; c.model.execution_count = null; c.outputs.replaceChildren(); updatePrompt(c); }
        changed();
    };
    $("add-code").onclick = () => insertCell(null, "code", true);
    $("add-markdown").onclick = () => insertCell(null, "markdown", true);
    $("kernelspec").onchange = (e) => changeKernel(e.target.value);
    $("start-server").onclick = async () => {
        try {
            await atlas.runCommand("server");
            connect(true);
        } catch (e) {
            banner("Der Server startet nicht: " + e.message, { error: true, retry: true });
        }
    };
    $("retry").onclick = () => connect(false);
    document.addEventListener("keydown", (e) => {
        if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") { e.preventDefault(); state.dirty = true; save(); }
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
        banner("Die Datei ist nicht lesbar: " + e.message, { error: true });
        return;
    }
    if (!loadNotebook(text)) return;
    await connect(false);
}

main().catch((e) => banner("Fehler: " + e.message, { error: true }));
