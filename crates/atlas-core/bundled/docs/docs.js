// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

// Docs search: loads the index of the packages (written by Atlas, pydocs, into
// the extension's cache folder), filters it by text, return type,
// parameters, class/module, kind and package, and shows the docstring of
// the selected entry (numpydoc/reST, rendered here).
"use strict";

const LIMIT = 300;
const KINDS = [
    { key: "function", label: "Funktion", alias: ["funktion", "func", "function", "fn", "f"] },
    { key: "method", label: "Methode", alias: ["methode", "method", "meth", "m"] },
    { key: "class", label: "Klasse", alias: ["klasse", "class", "cls", "c", "exception", "ausnahme"] },
    { key: "property", label: "Eigenschaft", alias: ["eigenschaft", "property", "prop", "attr", "attribut", "e"] },
    { key: "module", label: "Modul", alias: ["modul", "module", "mod"] },
];
const KIND_LABEL = { function: "Funktion", method: "Methode", class: "Klasse", exception: "Ausnahme", property: "Eigenschaft", module: "Modul", attribute: "Attribut" };
const SECTION_TITLES = {
    see: "Siehe auch", notes: "Hinweise", examples: "Beispiele", raises: "Fehler (Raises)", warns: "Warnungen",
    refs: "Quellen", attrs: "Attribute", methods: "Methoden",
};
const ADMONITIONS = {
    warning: "Achtung", caution: "Vorsicht", danger: "Gefahr", attention: "Achtung", important: "Wichtig",
    note: "Hinweis", tip: "Tipp", hint: "Tipp", seealso: "Siehe auch",
    versionadded: "Neu in Version", versionchanged: "Geändert in Version", deprecated: "Veraltet seit Version",
};
// Classes and modules people use most: their members rank higher.
const HOT = new Set([
    "pandas", "pandas.DataFrame", "pandas.Series", "pandas.Index", "pandas.DataFrame.plot", "pandas.Series.str", "pandas.Series.dt",
    "numpy", "numpy.ndarray", "numpy.linalg", "numpy.random", "numpy.random.Generator",
    "matplotlib.pyplot", "matplotlib.axes.Axes", "matplotlib.figure.Figure",
    "scipy.stats", "scipy.optimize", "seaborn", "sklearn.model_selection", "sklearn.metrics",
]);
// Usual import names in the docstrings' examples.
const ALIASES = { "pd.": "pandas.", "np.": "numpy.", "plt.": "matplotlib.pyplot.", "mpl.": "matplotlib.", "sns.": "seaborn." };
const ONLINE = {
    pandas: (e) => e.k === "module" ? null : `https://pandas.pydata.org/docs/reference/api/${e.id}.html`,
    numpy: (e) => e.k === "module" ? null : `https://numpy.org/doc/stable/reference/generated/${e.id}.html`,
    matplotlib: (e) => {
        if (e.k === "module") return `https://matplotlib.org/stable/api/${e.id.slice(11)}_api.html`;
        if (/^matplotlib\.(pyplot|axes\.Axes)\./.test(e.id) || !["method", "property"].includes(e.k)) return `https://matplotlib.org/stable/api/_as_gen/${e.id}.html`;
        return `https://matplotlib.org/stable/api/_as_gen/${e.parent}.html#${e.id}`;
    },
    scipy: (e) => e.k === "module" ? null : `https://docs.scipy.org/doc/scipy/reference/generated/${e.id}.html`,
    sklearn: (e) => e.k === "module" ? null : ["method", "property"].includes(e.k)
        ? `https://scikit-learn.org/stable/modules/generated/${e.parent}.html#${e.id}`
        : `https://scikit-learn.org/stable/modules/generated/${e.id}.html`,
    seaborn: (e) => e.k === "module" ? null : `https://seaborn.pydata.org/generated/${e.id}.html`,
};

const $ = (id) => document.getElementById(id);
const els = {
    query: $("query"), returns: $("f-returns"), params: $("f-params"), within: $("f-in"), full: $("f-full"), dep: $("f-dep"),
    kinds: $("kinds"), packages: $("packages"), results: $("results"), count: $("count"), detail: $("detail"),
    status: $("status"), rebuild: $("rebuild"), help: $("help"),
};
const state = {
    ctx: null, summary: null, entries: [], byId: new Map(), packages: [],
    kinds: new Set(), pkgs: new Set(), results: [], selected: -1, shown: null, history: [], building: false,
    matchedParams: new Map(),
};

// ---------------------------------------------------------------- helpers

function esc(text) {
    return String(text ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}
function reEsc(text) { return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"); }
function setStatus(text, kind) {
    els.status.textContent = text;
    els.status.className = "status" + (kind ? " " + kind : "");
    els.status.title = text;
}
async function copy(text) {
    try { await navigator.clipboard.writeText(text); return true; } catch (e) { /* not a secure context */ }
    const area = document.createElement("textarea");
    area.value = text;
    document.body.appendChild(area);
    area.select();
    const ok = document.execCommand("copy");
    area.remove();
    return ok;
}
function dateText(seconds) {
    if (!seconds) return "";
    return new Date(seconds * 1000).toLocaleString("de-DE", { dateStyle: "medium", timeStyle: "short" });
}
/** Words (and "phrases") with the filter prefixes of the search syntax. */
function parseQuery(text) {
    const q = { terms: [], returns: [], params: [], within: [], kinds: [], pkgs: [] };
    const tokens = [];
    text.replace(/"([^"]*)"|(\S+)/g, (_, phrase, word) => { tokens.push(phrase !== undefined ? { phrase } : { word }); });
    for (let i = 0; i < tokens.length; i++) {
        const t = tokens[i];
        if (t.phrase !== undefined) { if (t.phrase.trim()) q.terms.push(t.phrase.trim().toLowerCase()); continue; }
        let w = t.word, m;
        if (w === "->" || w === "→") { if (tokens[i + 1]?.word) q.returns.push(tokens[++i].word); continue; }
        if ((m = /^(?:->|→)(.+)$/.exec(w))) { q.returns.push(m[1]); continue; }
        if ((m = /^(?:r|ret|returns?|rückgabe):(.*)$/i.exec(w))) { if (m[1]) q.returns.push(m[1]); continue; }
        if ((m = /^(?:p|param|parameter):(.*)$/i.exec(w))) { if (m[1]) q.params.push(m[1]); continue; }
        if ((m = /^(?:in|klasse|modul):(.*)$/i.exec(w))) { if (m[1]) q.within.push(m[1]); continue; }
        if ((m = /^(?:k|kind|art):(.*)$/i.exec(w))) {
            const kind = KINDS.find((k) => k.alias.includes(m[1].toLowerCase()));
            if (kind) q.kinds.push(kind.key);
            continue;
        }
        if ((m = /^(?:pkg|paket|package):(.*)$/i.exec(w))) { if (m[1]) q.pkgs.push(m[1].toLowerCase()); continue; }
        // `pd.merge(` → pandas.merge
        w = w.replace(/\(.*$/, "");
        for (const [alias, full] of Object.entries(ALIASES)) if (w.startsWith(alias)) w = full + w.slice(alias.length);
        if (w) q.terms.push(w.toLowerCase());
    }
    return q;
}
function splitList(text) { return text.split(",").map((s) => s.trim()).filter(Boolean); }
/** Matches `word` as a whole word (with * as wildcard) in `text`. */
function wordRe(word) {
    const body = reEsc(word.toLowerCase()).replace(/\\\*/g, "[\\w.]*");
    return new RegExp("(^|[^\\w])" + body + "($|[^\\w])");
}

// ---------------------------------------------------------------- index

async function readJson(path) {
    return JSON.parse(await atlas.call("readFile", { path }));
}

function prepare(e, pkg, version) {
    e.pkg = pkg;
    e.ver = version;
    const dot = e.id.lastIndexOf(".");
    e.short = dot >= 0 ? e.id.slice(dot + 1) : e.id;
    e.parent = dot >= 0 ? e.id.slice(0, dot) : "";
    e.lid = e.id.toLowerCase();
    e.lshort = e.short.toLowerCase();
    e.lsum = (e.sum || "").toLowerCase();
    e.pnames = (e.p || []).map((p) => p.n.replace(/^\*+/, "").toLowerCase());
    e.ptypes = (e.p || []).map((p) => (p.t || "").toLowerCase());
    e.rtext = [...(e.r || []).map((r) => r.t || ""), e.ra || ""].join(" | ").toLowerCase();
    let base = -e.id.split(".").length * 2;
    if (HOT.has(e.parent)) base += 14;
    else if (e.parent === pkg) base += 10;
    if (e.k === "class" || e.k === "function") base += 3;
    if (e.dep) base -= 30;
    if (e.from) base -= 2;
    e.base = base;
}

async function loadIndex() {
    const cache = state.ctx.cache;
    let summary = null;
    try { summary = await readJson(cache + "/index.json"); } catch (e) { /* none yet */ }
    state.summary = summary;
    state.entries = [];
    state.byId = new Map();
    state.packages = [];
    if (!summary || !summary.packages) {
        setStatus("Noch kein Index.");
        render();
        return false;
    }
    const problems = [];
    for (const [name, info] of Object.entries(summary.packages)) {
        if (info.error) { problems.push(`${name} fehlt`); continue; }
        try {
            const data = await readJson(`${cache}/${name}.json`);
            for (const e of data.entries) {
                prepare(e, name, data.version);
                state.entries.push(e);
                state.byId.set(e.id, e);
            }
            state.packages.push({ name, version: data.version, count: data.entries.length });
        } catch (err) {
            problems.push(`${name}: ${err.message}`);
        }
    }
    state.packages.sort((a, b) => a.name.localeCompare(b.name));
    // Packages in the settings that are not in the index yet.
    const wanted = String(state.ctx.settings?.pakete || "").split(/[\s,;]+/).filter(Boolean);
    const missing = wanted.filter((p) => !summary.packages[p]);
    const parts = [state.packages.map((p) => `${p.name} ${p.version}`).join(" · ") || "leer",
                   `${state.entries.length.toLocaleString("de-DE")} Einträge`];
    // Packages may come from different environments (each from the first that has it).
    const sources = [...new Set(Object.values(summary.packages).filter((i) => !i.error && i.source).map((i) => i.source))];
    if (sources.length) parts.push(`aus ${sources.join(", ")}`);
    if (summary.built) parts.push(`gebaut ${dateText(summary.built)}`);
    if (problems.length) parts.push(problems.join(", "));
    if (missing.length) parts.push(`noch nicht im Index: ${missing.join(", ")} – „Index neu bauen“`);
    setStatus(parts.join(" — "), problems.length || missing.length ? "warn" : "");
    renderChips();
    render();
    return true;
}

async function rebuild() {
    if (state.building) return;
    state.building = true;
    els.rebuild.disabled = true;
    setStatus("Index wird gebaut …", "busy");
    try {
        await atlas.call("docsBuild");
    } catch (err) {
        state.building = false;
        els.rebuild.disabled = false;
        setStatus("Index nicht gestartet: " + err.message, "error");
        return;
    }
    const started = Date.now();
    const poll = async () => {
        let s = {};
        try { s = await atlas.call("docsStatus"); } catch (e) { /* older Atlas */ }
        const lines = String(s.log || "").trim().split("\n").filter(Boolean);
        if (s.running || (!("running" in s) && Date.now() - started < 3000)) {
            setStatus(`Index wird gebaut … ${lines[lines.length - 1] || ""}`, "busy");
            setTimeout(poll, 500);
            return;
        }
        state.building = false;
        els.rebuild.disabled = false;
        // Settings may have changed since the page was opened.
        try { state.ctx.settings = (await atlas.call("context")).settings || state.ctx.settings; } catch (e) { /* keep */ }
        state.buildLog = s.code !== 0 ? lines : null;
        const ok = await loadIndex();
        if (s.code !== 0 || !ok) {
            const reason = lines.filter((l) => !/^\s/.test(l) && l !== "Gesucht in:").slice(-2).join(" · ");
            setStatus("Index nicht gebaut: " + (reason || `Exit-Code ${s.code}`), "error");
            render();
        }
    };
    setTimeout(poll, 400);
}

// ---------------------------------------------------------------- search

function currentQuery() {
    const q = parseQuery(els.query.value);
    q.returns.push(...els.returns.value.split(/\s+/).filter(Boolean));
    q.params.push(...splitList(els.params.value));
    if (els.within.value.trim()) q.within.push(els.within.value.trim());
    for (const k of state.kinds) q.kinds.push(k);
    for (const p of state.pkgs) q.pkgs.push(p);
    q.full = els.full.checked;
    q.dep = els.dep.checked;
    return q;
}

function paramMatcher(spec) {
    // name, name:type, :type, name* (prefix)
    const [name, type] = spec.includes(":") ? spec.split(/:(.*)/s) : [spec, ""];
    const nameRe = name ? new RegExp("^\\**" + reEsc(name.toLowerCase()).replace(/\\\*/g, ".*") + "$") : null;
    const typeRe = type ? wordRe(type) : null;
    return (e) => {
        const hits = [];
        for (let i = 0; i < e.pnames.length; i++) {
            if (nameRe && !nameRe.test(e.pnames[i])) continue;
            if (typeRe && !typeRe.test(e.ptypes[i])) continue;
            hits.push(e.p[i].n);
        }
        return hits;
    };
}

function textScore(e, t, full) {
    if (t.includes(".")) {
        if (e.lid === t || e.lid.endsWith("." + t)) return 110;
        if (e.lid.includes(t)) return 45;
    }
    if (e.lshort === t) return 100;
    if (e.lshort.startsWith(t)) return 60 - Math.min(20, e.lshort.length - t.length);
    if (e.lshort.includes(t)) return 35;
    if (e.lid.includes(t)) return 22;
    if (e.lsum.includes(t)) return 10;
    if (e.pnames.includes(t)) return 6;
    if (full) {
        if (e.ldoc === undefined) e.ldoc = (e.doc || "").toLowerCase() + " " + (e.p || []).map((p) => p.desc || "").join(" ").toLowerCase();
        if (e.ldoc.includes(t)) return 3;
    }
    return -1;
}

function search() {
    const q = currentQuery();
    const filtered = q.returns.length || q.params.length || q.within.length || q.kinds.length || q.pkgs.length;
    state.matchedParams = new Map();
    if (!q.terms.length && !filtered) return { q, list: [], hidden: 0, total: 0, idle: true };
    const returns = q.returns.map((r) => r.split("|").filter(Boolean).map(wordRe));
    const params = q.params.map(paramMatcher);
    const within = q.within.map((w) => w.toLowerCase());
    const kinds = new Set(q.kinds.flatMap((k) => k === "class" ? ["class", "exception"] : [k]));
    const pkgs = new Set(q.pkgs);
    const joined = q.terms.length > 1 && q.terms.every((t) => /^\w+$/.test(t)) ? q.terms.join("_") : "";
    const out = [];
    let hidden = 0;
    for (const e of state.entries) {
        if (kinds.size && !kinds.has(e.k)) continue;
        if (pkgs.size && !pkgs.has(e.pkg)) continue;
        if (within.length && !within.every((w) => e.lid.startsWith(w + ".") || e.lid.includes("." + w + ".") || e.parent.toLowerCase().endsWith("." + w))) continue;
        if (returns.length && !returns.every((alts) => alts.some((re) => re.test(e.rtext)))) continue;
        let matched = [];
        if (params.length) {
            let ok = true;
            for (const m of params) {
                const hits = m(e);
                if (!hits.length) { ok = false; break; }
                matched = matched.concat(hits);
            }
            if (!ok) continue;
        }
        let score = e.base;
        let ok = true;
        for (const t of q.terms) {
            const s = textScore(e, t, q.full);
            if (s < 0) { ok = false; break; }
            score += s;
        }
        if (!ok) continue;
        // "read csv" → read_csv
        if (joined && (e.lshort === joined || e.lshort.endsWith("_" + joined))) score += 80;
        else if (joined && e.lshort.includes(joined)) score += 30;
        if (e.dep && !q.dep) { hidden++; continue; }
        if (matched.length) state.matchedParams.set(e.id, matched);
        out.push({ e, score });
    }
    out.sort((a, b) => b.score - a.score || a.e.id.length - b.e.id.length || a.e.id.localeCompare(b.e.id));
    return { q, list: out.map((x) => x.e), hidden, total: out.length, idle: false };
}

// ---------------------------------------------------------------- results

function renderChips() {
    els.kinds.innerHTML = KINDS.map((k) => `<span class="chip${state.kinds.has(k.key) ? " on" : ""}" data-kind="${k.key}">${k.label}</span>`).join("");
    els.packages.innerHTML = state.packages.length > 1
        ? state.packages.map((p) => `<span class="chip${state.pkgs.has(p.name) ? " on" : ""}" data-pkg="${esc(p.name)}" title="${esc(p.name)} ${esc(p.version)}">${esc(p.name)}<span class="n">${esc(p.version)}</span></span>`).join("")
        : "";
}

function mark(text, terms) {
    let html = esc(text);
    for (const t of terms) {
        if (!t || t.includes(".")) continue;
        html = html.replace(new RegExp("(" + reEsc(esc(t)) + ")", "ig"), "<mark>$1</mark>");
    }
    return html;
}

/** reST markup as plain text (for the list): :func:`~a.b` → b, ``x`` → x. */
function plainRst(text) {
    return String(text || "")
        .replace(/(?::[\w.-]+)+:`~?([^`<]*?)(?:\s*<[^>]+>)?`/g, (_, n) => n.includes(".") && !/\s/.test(n) ? n.split(".").pop() : n)
        .replace(/``([^`]+)``/g, "$1")
        .replace(/`([^`]+)`_?/g, "$1")
        .replace(/\*\*([^*]+)\*\*/g, "$1");
}

function shortType(text) {
    // `~matplotlib.collections.PolyCollection` → PolyCollection
    const t = String(text || "").replace(/^['"]|['"]$/g, "").replace(/:\w+:/g, "").replace(/`~?([\w.]+)`/g, (_, n) => n.split(".").pop()).replace(/`/g, "");
    return t.length > 40 ? t.slice(0, 38) + "…" : t;
}

let lastSearch = null;
function render() {
    const result = search();
    lastSearch = result;
    state.results = result.list;
    const terms = result.q.terms;
    for (const el of document.querySelectorAll(".field")) el.classList.toggle("active", !!el.querySelector("input").value.trim());
    if (!state.entries.length) {
        els.count.textContent = "";
        els.results.innerHTML = "";
        renderWelcome();
        return;
    }
    if (result.idle) {
        els.count.textContent = `${state.entries.length.toLocaleString("de-DE")} Einträge im Index`;
        els.results.innerHTML = `<div class="empty"><p>Tippe einen Namen oder setze einen Filter.</p></div>`;
        state.selected = -1;
        if (!state.shown) renderWelcome();
        return;
    }
    const shown = result.list.slice(0, LIMIT);
    els.count.textContent = (result.total ? `${result.total.toLocaleString("de-DE")} Treffer` : "Keine Treffer")
        + (result.total > LIMIT ? `, die ersten ${LIMIT}` : "")
        + (result.hidden ? ` · ${result.hidden} veraltete ausgeblendet` : "");
    els.results.innerHTML = shown.map((e, i) => {
        const ret = e.k === "class" || e.k === "exception" ? "" : (e.r && e.r[0] && e.r[0].t) || e.ra || "";
        const matched = new Set(state.matchedParams.get(e.id) || []);
        const params = shownParams(e);
        const preview = params.length && e.k !== "property" && e.k !== "module"
            ? `<div class="pprev">(${params.slice(0, 6).map((p) => `<span class="${matched.has(p.n) ? "pm" : isRequired(p) ? "preq" : ""}">${esc(p.n)}</span>`).join(", ")}${params.length > 6 ? ", …" : ""})</div>`
            : "";
        return `<div class="hit" data-i="${i}">
  <div class="top">${pkgChip(e.pkg)}<span class="name">${mark(e.short, terms)}</span><span class="path">${e.parent && e.parent !== e.pkg ? "in " + esc(e.parent) : ""}</span></div>
  ${e.sum ? `<div class="sum">${mark(plainRst(e.sum), terms)}</div>` : ""}
  ${preview}
  <div class="meta"><span class="badge k-${e.k}">${KIND_LABEL[e.k] || e.k}</span>${ret ? `<span class="badge ret" title="Rückgabe: ${esc(ret)}">→ ${esc(shortType(ret))}</span>` : ""}${e.dep ? `<span class="badge dep">veraltet</span>` : ""}</div>
</div>`;
    }).join("") || `<div class="empty"><p>Nichts gefunden. Filter lockern oder „Volltext“ einschalten.</p></div>`;
    const keep = state.shown ? shown.indexOf(state.shown) : -1;
    select(keep >= 0 ? keep : shown.length ? 0 : -1, false);
}

function select(i, scroll = true) {
    const hits = els.results.querySelectorAll(".hit");
    if (!hits.length) { state.selected = -1; return; }
    i = Math.max(0, Math.min(hits.length - 1, i));
    hits[state.selected]?.classList.remove("sel");
    state.selected = i;
    hits[i].classList.add("sel");
    if (scroll) hits[i].scrollIntoView({ block: "nearest" });
    const e = state.results[i];
    if (e !== state.shown) show(e, false);
}

// ---------------------------------------------------------------- detail

function resolve(name, from) {
    if (!name) return null;
    let n = name.replace(/^[~!.]+/, "").replace(/\(\)$/, "").trim();
    for (const [alias, full] of Object.entries(ALIASES)) if (n.startsWith(alias)) n = full + n.slice(alias.length);
    const tries = [n];
    if (from) {
        let parent = from.parent;
        while (parent) { tries.push(parent + "." + n); parent = parent.includes(".") ? parent.slice(0, parent.lastIndexOf(".")) : ""; }
        tries.push(from.pkg + "." + n);
    }
    for (const p of state.packages) tries.push(p.name + "." + n);
    for (const t of tries) { const e = state.byId.get(t); if (e) return e; }
    return null;
}

function refLink(target, label, from) {
    const e = resolve(target, from);
    return e ? `<a class="ref" data-ref="${esc(e.id)}" title="${esc(e.id)}">${esc(label)}</a>` : `<code>${esc(label)}</code>`;
}

/** Inline reST: ``code``, :role:`target`, `ref`, **bold**, *em*, links. */
function inline(text, from) {
    const keep = [];
    const put = (html) => { keep.push(html); return `\u0000${keep.length - 1}\u0000`; };
    let s = String(text);
    s = s.replace(/``([^`]+)``/g, (_, c) => put(`<code>${esc(c)}</code>`));
    s = s.replace(/`([^`<]+?)\s*<(https?:[^>]+)>`_{1,2}/g, (_, label, url) => put(`<a class="ext" href="${esc(url)}" target="_blank">${esc(label)}</a>`));
    s = s.replace(/(?::[\w.-]+)+:`([^`]+)`/g, (_, body) => {
        const m = /^(.*?)\s*<([^>]+)>$/.exec(body);
        const target = m ? m[2] : body;
        const label = m ? m[1] : body.startsWith("~") ? body.slice(1).split(".").pop() : body;
        return put(refLink(target, label, from));
    });
    s = s.replace(/`([^`]+)`_?/g, (_, body) => put(/^[\w.~]+(\(\))?$/.test(body) ? refLink(body, body.replace(/^~/, ""), from) : `<code>${esc(body)}</code>`));
    s = s.replace(/https?:\/\/[^\s<>()]+[^\s<>().,;:]/g, (url) => put(`<a class="ext" href="${esc(url)}" target="_blank">${esc(url)}</a>`));
    s = esc(s);
    s = s.replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>").replace(/(^|[\s(])\*([^*\s][^*]*?)\*(?=[\s).,;:]|$)/g, "$1<i>$2</i>");
    return s.replace(/\u0000(\d+)\u0000/g, (_, i) => keep[Number(i)]);
}

function highlight(code) {
    try { return window.hljs ? hljs.highlight(code, { language: "python", ignoreIllegals: true }).value : esc(code); } catch (e) { return esc(code); }
}

/** A doctest block: code lines highlighted, output lines muted. */
function doctest(lines) {
    return `<pre class="code">${lines.map((l) => /^\s*(>>>|\.\.\.)( |$)/.test(l)
        ? `<span class="hljs-meta">${esc(l.match(/^\s*(>>>|\.\.\.)/)[0])}</span>${highlight(l.replace(/^\s*(>>>|\.\.\.) ?/, " "))}`
        : `<span class="out">${esc(l)}</span>`).join("\n")}</pre>`;
}

function indentOf(line) { return line.length - line.trimStart().length; }

/** Body text of a docstring (numpydoc sections marked with §key by pydocs). */
function renderRst(text, from) {
    const lines = String(text || "").split("\n");
    const out = [];
    let section = "";
    let i = 0;
    const takeIndented = (min) => {
        const block = [];
        while (i < lines.length && (!lines[i].trim() || indentOf(lines[i]) > min)) block.push(lines[i++]);
        while (block.length && !block[block.length - 1].trim()) block.pop();
        const cut = Math.min(...block.filter((l) => l.trim()).map(indentOf), 1e9);
        return block.map((l) => l.slice(cut === 1e9 ? 0 : cut));
    };
    while (i < lines.length) {
        const line = lines[i];
        const trimmed = line.trim();
        if (!trimmed) { i++; continue; }
        if (/^§/.test(trimmed)) {
            section = trimmed.slice(1);
            const title = section.startsWith("x:") ? section.slice(2) : SECTION_TITLES[section] || section;
            out.push(`<h3 id="s-${esc(section.replace(/[^\w-]/g, ""))}">${esc(title)}</h3>`);
            i++;
            continue;
        }
        // Lists of names with indented text below (See Also, Raises, Attributes …).
        if (["see", "raises", "warns", "attrs", "methods"].includes(section) && indentOf(line) === 0 && !/^\.\. /.test(trimmed)) {
            const items = [];
            while (i < lines.length && lines[i].trim() && indentOf(lines[i]) === 0 && !/^(§|\.\. |>>>)/.test(lines[i])) {
                const [term, ...desc] = lines[i].split(/\s:\s|\s:$/);
                i++;
                const more = takeIndented(0);
                items.push({ term: term.trim(), desc: [desc.join(" : ").trim(), ...more].join(" ").trim() });
            }
            out.push(`<dl class="see">${items.map((it) => `<dt>${section === "see" || section === "methods"
                ? it.term.split(/,\s*/).map((t) => refLink(t, t, from)).join(", ") : `<code>${esc(it.term)}</code>`}</dt>${it.desc ? `<dd>${inline(it.desc, from)}</dd>` : ""}`).join("")}</dl>`);
            continue;
        }
        if (/^>>>/.test(trimmed)) {
            const block = [];
            while (i < lines.length && lines[i].trim()) block.push(lines[i++].slice(indentOf(line)));
            out.push(doctest(block));
            continue;
        }
        let m;
        if ((m = /^\.\.\s+([\w-]+)::\s*(.*)$/.exec(trimmed))) {
            const [, directive, arg] = m;
            const base = indentOf(line);
            i++;
            const body = takeIndented(base);
            const content = body.filter((l) => !/^:\w+:/.test(l.trim()));
            if (["code-block", "code", "sourcecode", "ipython", "plot", "testcode", "math"].includes(directive)) {
                const code = content.join("\n").trim();
                if (code) out.push(/^>>>/.test(code) ? doctest(code.split("\n")) : `<pre class="code">${directive === "math" ? esc(code) : highlight(code)}</pre>`);
            } else if (ADMONITIONS[directive]) {
                const title = ADMONITIONS[directive] + (arg && directive.startsWith("version") || directive === "deprecated" ? " " + arg : "");
                const rest = directive.startsWith("version") || directive === "deprecated" ? content : [arg, ...content];
                out.push(`<div class="admonition ${directive}"><div class="atitle">${esc(title)}</div>${renderRst(rest.join("\n"), from)}</div>`);
            } else if (!["autosummary", "toctree", "only", "currentmodule", "module", "index", "rubric"].includes(directive)) {
                if (content.length) out.push(`<pre class="code">${esc(content.join("\n"))}</pre>`);
            }
            continue;
        }
        if (/^\.\.\s+\[/.test(trimmed)) {
            const block = [trimmed];
            i++;
            block.push(...takeIndented(indentOf(line)));
            out.push(`<p class="inherited">${inline(block.join(" "), from)}</p>`);
            continue;
        }
        if (/^(={3,}|\+-{3,})/.test(trimmed)) {
            const block = [];
            while (i < lines.length && (lines[i].trim() || /^(={3,}|\+-)/.test(lines[i + 1]?.trim() || ""))) block.push(lines[i++]);
            out.push(`<pre class="code">${esc(block.join("\n"))}</pre>`);
            continue;
        }
        if (/^[-*•] /.test(trimmed)) {
            const items = [];
            const base = indentOf(line);
            while (i < lines.length && /^[-*•] /.test(lines[i].trim()) && indentOf(lines[i]) === base) {
                const first = lines[i++].trim().slice(2);
                const more = takeIndented(base + 1);
                items.push([first, ...more].join(" "));
            }
            out.push(`<ul>${items.map((it) => `<li>${inline(it, from)}</li>`).join("")}</ul>`);
            continue;
        }
        if (indentOf(line) > 0 && section !== "examples") {
            const block = takeIndented(indentOf(line) - 1);
            out.push(`<pre class="code">${esc(block.join("\n"))}</pre>`);
            continue;
        }
        // A paragraph; "::" at its end makes the indented block below code.
        const para = [];
        // (The first line always belongs to it, e.g. ".. _label:".)
        para.push(lines[i++].trim());
        while (i < lines.length && lines[i].trim() && indentOf(lines[i]) <= indentOf(line) && !/^(>>>|\.\. |§)/.test(lines[i].trim())) para.push(lines[i++].trim());
        let textPara = para.join(" ");
        const literal = /::$/.test(textPara);
        if (literal) textPara = textPara.replace(/\s*::$/, textPara.endsWith(" ::") ? "" : ":");
        if (textPara) out.push(`<p>${inline(textPara, from)}</p>`);
        if (literal) {
            while (i < lines.length && !lines[i].trim()) i++;
            const block = takeIndented(indentOf(line));
            if (block.length) out.push(`<pre class="code">${highlight(block.join("\n"))}</pre>`);
        }
    }
    return out.join("\n");
}

/** The signature, one parameter per line if long. */
function renderSignature(e) {
    if (!e.sig) return "";
    const sig = e.sig;
    let depth = 0, end = -1;
    for (let i = 0; i < sig.length; i++) {
        const c = sig[i];
        if ("([{".includes(c)) depth++;
        else if (")]}".includes(c)) { depth--; if (depth === 0) { end = i; break; } }
    }
    if (!sig.startsWith("(") || end < 0) return `<pre class="sig">${esc(e.short + sig)}</pre>`;
    const inner = sig.slice(1, end);
    const ret = sig.slice(end + 1).replace(/^\s*->\s*/, "").replace(/^['"]|['"]$/g, "");
    const parts = [];
    let cur = "", quote = "";
    depth = 0;
    for (const c of inner) {
        if (quote) { cur += c; if (c === quote) quote = ""; continue; }
        if (c === "'" || c === '"') { quote = c; cur += c; continue; }
        if ("([{".includes(c)) depth++;
        if (")]}".includes(c)) depth--;
        if (c === "," && depth === 0) { parts.push(cur.trim()); cur = ""; continue; }
        cur += c;
    }
    if (cur.trim()) parts.push(cur.trim());
    const hl = new Set(state.matchedParams.get(e.id) || []);
    const html = parts.map((p) => {
        const m = /^(\*{0,2}[\w]+|\/|\*)(?:\s*:\s*(.*?))?(?:\s*=\s*(.*))?$/s.exec(p);
        if (!m) return esc(p);
        const [, name, ann, def] = m;
        return `<span class="pn"${hl.has(name) ? ' style="font-weight:700"' : ""}>${esc(name)}</span>`
            + (ann ? `: <span class="ann">${esc(ann.replace(/^['"]|['"]$/g, ""))}</span>` : "")
            + (def !== undefined ? ` = <span class="def">${esc(def)}</span>` : "");
    });
    const long = inner.length > 60;
    const body = long ? "(\n    " + html.join(",\n    ") + "\n)" : "(" + html.join(", ") + ")";
    return `<pre class="sig"><b>${esc(e.short)}</b>${body}${ret ? ` → <span class="ret">${esc(ret)}</span>` : ""}</pre>`;
}

// Usual import names (how the docs and most code write it).
const PKG_ALIAS = { pandas: "pd", numpy: "np", "matplotlib.pyplot": "plt", matplotlib: "mpl", seaborn: "sns" };
// Usual variable names for objects of a class.
const VAR_NAME = {
    DataFrame: "df", Series: "s", Index: "idx", MultiIndex: "mi", DatetimeIndex: "idx", Timestamp: "ts", Timedelta: "td",
    Axes: "ax", Figure: "fig", Line2D: "line", ndarray: "arr", Generator: "rng", RandomState: "rs", GroupBy: "gb",
    DataFrameGroupBy: "gb", SeriesGroupBy: "gb", Rolling: "r", Resampler: "r",
};
const PKG_HUE = { pandas: 222, numpy: 192, matplotlib: 28, scipy: 268, seaborn: 150, sklearn: 330, json: 60 };

function pkgHue(pkg) {
    if (pkg in PKG_HUE) return PKG_HUE[pkg];
    let h = 0;
    for (const c of pkg) h = (h * 31 + c.charCodeAt(0)) % 360;
    return h;
}
function pkgChip(pkg, version) {
    return `<span class="pkg" style="--h:${pkgHue(pkg)}" title="Bibliothek ${esc(pkg)}${version ? " " + esc(version) : ""}">${esc(pkg)}${version ? `<span class="v">${esc(version)}</span>` : ""}</span>`;
}

/** Parameters worth listing (no `*` and `/` markers). */
function shownParams(e) { return (e.p || []).filter((p) => !["/", "*"].includes(p.n)); }
function isVariadic(p) { return p.n.startsWith("*"); }
function isRequired(p) { return p.d === undefined && !p.doc && !p.other && !isVariadic(p) && !/\boptional\b/.test(p.t || ""); }

/** "bool, default True" → type "bool", default "True"; "int, optional" → optional. */
function typeAndDefault(p) {
    let type = String(p.t || "").trim();
    let def = p.d;
    const m = /,?\s*default(?:\s*[:=]\s*|\s+)(.+)$/i.exec(type);
    if (m) {
        type = type.slice(0, m.index).trim();
        if (def === undefined) def = m[1].trim();
    }
    const optional = /,?\s*optional\s*$/i.test(type);
    if (optional) type = type.replace(/,?\s*optional\s*$/i, "").trim();
    return { type, def, optional };
}

/** The entry's owner (class or module) as an entry, if it has one. */
function ownerOf(e) { return e.parent ? state.byId.get(e.parent) : null; }

/** How it is usually written in code: `df.sum(axis)`, `pd.read_csv(filepath_or_buffer)`. */
function usage(e) {
    const aliased = (id) => {
        for (const [pkg, alias] of Object.entries(PKG_ALIAS).sort((a, b) => b[0].length - a[0].length)) {
            if (id === pkg) return alias;
            if (id.startsWith(pkg + ".")) return alias + id.slice(pkg.length);
        }
        return id;
    };
    const varOf = (cls) => VAR_NAME[cls] || (cls ? cls.replace(/^_+/, "").replace(/[A-Z]/g, (c, i) => (i ? "_" : "") + c.toLowerCase()).replace(/_+/g, "_") : "obj");
    let target;
    const owner = ownerOf(e);
    if (e.k === "module") return `import ${e.id}` + (PKG_ALIAS[e.id] ? ` as ${PKG_ALIAS[e.id]}` : "");
    if ((e.k === "method" || e.k === "property") && owner) {
        // An accessor in between: s.str.contains(…)
        const grand = ownerOf(owner);
        target = owner.k === "property" && grand ? `${varOf(grand.short)}.${owner.short}.${e.short}` : `${varOf(owner.short)}.${e.short}`;
    } else {
        target = aliased(e.id);
    }
    if (e.k === "property") return target;
    const params = shownParams(e).filter((p) => !p.doc && !p.other);
    const required = params.filter(isRequired).map((p) => p.n);
    const more = params.length > required.length ? (required.length ? ", …" : "…") : "";
    return `${target}(${required.join(", ")}${more})`;
}

/** Parameters: one card each – name, type, default, kind, then the text. */
function renderParams(e) {
    const params = shownParams(e);
    if (!params.length) return "";
    const hl = new Set(state.matchedParams.get(e.id) || []);
    const cards = params.map((p) => {
        const { type, def, optional } = typeAndDefault(p);
        const tags = [];
        if (isRequired(p)) tags.push(`<span class="tag req">Pflicht</span>`);
        else if (isVariadic(p)) tags.push(`<span class="tag">${p.n.startsWith("**") ? "beliebige Schlüsselwörter" : "beliebig viele Werte"}</span>`);
        else if (p.doc || p.other) tags.push(`<span class="tag">über **kwargs</span>`);
        else tags.push(`<span class="tag">optional</span>`);
        if (p.kw) tags.push(`<span class="tag">nur als Schlüsselwort</span>`);
        if (optional && !tags.some((t) => t.includes("optional"))) tags.push(`<span class="tag">optional</span>`);
        const paras = String(p.desc || "").split(/\n\s*\n/);
        const first = paras[0] ? renderRst(paras[0], e) : "";
        const rest = paras.slice(1).join("\n\n").trim();
        return `<div class="pcard${hl.has(p.n) ? " hl" : ""}" id="p-${esc(p.n.replace(/^\*+/, ""))}">
  <div class="phead"><span class="pname">${esc(p.n)}</span>${type ? `<span class="ptype" title="Typ">${esc(type)}</span>` : ""}${def !== undefined ? `<span class="pdef" title="Standardwert">= ${esc(def)}</span>` : ""}<span class="tags">${tags.join("")}</span></div>
  ${first ? `<div class="pdesc">${first}${rest ? `<details><summary>mehr</summary>${renderRst(rest, e)}</details>` : ""}</div>` : ""}
</div>`;
    });
    return `<section id="s-params"><h3>Parameter <span class="n">${params.length}</span></h3>${cards.join("")}</section>`;
}

function renderReturns(e) {
    if (e.k === "class" || e.k === "exception" || e.k === "module" || !e.r || !e.r.length) return "";
    const cards = e.r.map((r) => `<div class="pcard ret">
  <div class="phead">${r.n ? `<span class="pname">${esc(r.n)}</span>` : ""}<span class="ptype big">${esc(shortType(r.t || "") || "–")}</span></div>
  ${r.desc ? `<div class="pdesc">${renderRst(r.desc, e)}</div>` : ""}
</div>`);
    return `<section id="s-ret"><h3>Rückgabe</h3>${cards.join("")}</section>`;
}

/** The overview at the top: library, owner, kind, call, return, parameters. */
function renderFacts(e) {
    const owner = ownerOf(e);
    const rows = [];
    rows.push(["Bibliothek", `${pkgChip(e.pkg, e.ver)}`]);
    if (e.parent && e.parent !== e.pkg || owner) {
        const label = owner && (owner.k === "class" || owner.k === "exception") ? "Klasse" : owner && owner.k === "property" ? "Accessor" : "Modul";
        rows.push([label, owner ? `<a class="ref" data-ref="${esc(owner.id)}">${esc(owner.id)}</a>` : `<code>${esc(e.parent)}</code>`]);
    }
    if (e.from) rows.push(["Geerbt von", refLink(e.from, e.from, e)]);
    rows.push(["Art", `<span class="badge k-${e.k}">${KIND_LABEL[e.k] || e.k}</span>${e.dep ? ' <span class="badge dep">veraltet</span>' : ""}`]);
    if (e.k !== "module") rows.push(["Aufruf", `<code class="call">${esc(usage(e))}</code> <button class="mini" data-act="copy-call" title="In die Zwischenablage">kopieren</button>`]);
    const ret = (e.r || []).filter((r) => r.t).map((r) => shortType(r.t));
    if (ret.length && e.k !== "class" && e.k !== "exception" && e.k !== "module") {
        rows.push(["Rückgabe", ret.map((t) => `<span class="ptype big">${esc(t)}</span>`).join(" ")
            + (e.r[0].desc ? `<span class="rsum">${inline(e.r[0].desc.split(/\n\s*\n/)[0].replace(/\s+/g, " ").slice(0, 160), e)}</span>` : "")]);
    }
    const params = shownParams(e);
    if (params.length && e.k !== "property") {
        const hl = new Set(state.matchedParams.get(e.id) || []);
        const FEW = 16;
        rows.push([`Parameter`, `<div class="pchips${params.length > FEW + 4 ? " folded" : ""}">${params.map((p, i) => {
            const { def } = typeAndDefault(p);
            const cls = [isRequired(p) ? "req" : "", hl.has(p.n) ? "hl" : "", p.doc || p.other ? "kw" : ""].join(" ");
            const extra = i >= FEW && params.length > FEW + 4 ? " extra" : "";
            return `<a class="pchip ${cls}${extra}" data-jump="p-${esc(p.n.replace(/^\*+/, ""))}" title="${isRequired(p) ? "Pflicht" : def !== undefined ? "Standard: " + esc(def) : "optional"}">${esc(p.n)}</a>`;
        }).join("")}${params.length > FEW + 4 ? `<a class="pchip more" data-act="more-params">+ ${params.length - FEW} weitere</a>` : ""}</div>`]);
    }
    return `<div class="facts">${rows.map(([k, v]) => `<div class="fk">${k}</div><div class="fv">${v}</div>`).join("")}</div>`;
}

/** The text before the first section, and the sections. */
function splitDoc(e) {
    const doc = stripSummary(e);
    const i = doc.search(/^§/m);
    return i < 0 ? { head: doc, rest: "" } : { head: doc.slice(0, i), rest: doc.slice(i) };
}

function show(e, push = true) {
    if (!e) return;
    if (push && state.shown && state.shown !== e) state.history.push(state.shown);
    state.shown = e;
    const parts = e.id.split(".");
    const crumbs = parts.slice(0, -1).map((p, i) => {
        const id = parts.slice(0, i + 1).join(".");
        return `<a data-crumb="${esc(id)}" title="${state.byId.has(id) ? "Anzeigen" : "Als Filter „In“"}">${esc(p)}</a>`;
    }).join(".");
    const online = ONLINE[e.pkg] ? ONLINE[e.pkg](e) : null;
    const { head, rest } = splitDoc(e);
    const desc = head.trim() ? `<section id="s-desc"><h3>Beschreibung</h3>${renderRst(head, e)}</section>` : "";
    const params = renderParams(e);
    const returns = renderReturns(e);
    const sections = renderRst(rest, e);
    const sig = e.sig ? `<section id="s-sig"><h3>Signatur</h3>${renderSignature(e)}</section>` : "";
    const toc = [
        desc && ["s-desc", "Beschreibung"],
        params && ["s-params", `Parameter (${shownParams(e).length})`],
        returns && ["s-ret", "Rückgabe"],
        ...[...rest.matchAll(/^§(\S+)/gm)].map((m) => [`s-${m[1].replace(/[^\w-]/g, "")}`, m[1].startsWith("x:") ? m[1].slice(2) : SECTION_TITLES[m[1]] || m[1]]),
        sig && ["s-sig", "Signatur"],
    ].filter(Boolean);
    els.detail.innerHTML = `
<div class="crumbs">${crumbs}${crumbs ? "." : ""}</div>
<h1>${esc(e.short)}</h1>
${e.sum ? `<p class="summary">${inline(e.sum, e)}</p>` : `<p class="summary none">Keine Beschreibung im Paket.</p>`}
${renderFacts(e)}
<div class="tools">
  ${state.history.length ? '<button data-act="back" title="Zurück zum vorigen Eintrag (Rücktaste)">← Zurück</button>' : ""}
  <button data-act="copy-name" title="${esc(e.id)}">Name kopieren</button>
  ${e.k === "class" || e.k === "module" ? `<button data-act="members" title="Alle Einträge darin">Inhalt zeigen</button>` : ""}
  <span class="spacer"></span><span id="copied" class="copied"></span>
  ${online ? `<a class="ext" href="${esc(online)}" target="_blank" title="${esc(online)}">Online-Doku ↗</a>` : ""}
</div>
${toc.length > 1 ? `<nav class="toc">${toc.map(([id, label]) => `<a data-jump="${id}">${esc(label)}</a>`).join("")}</nav>` : ""}
${desc}
${params}
${returns}
${sections}
${sig}`;
    els.detail.scrollTop = 0;
}

/** The text without the summary shown above it. */
function stripSummary(e) {
    const paras = (e.doc || "").split(/\n[ \t]*\n/);
    if (e.sum && paras[0].replace(/\s+/g, " ").trim() === e.sum.trim()) paras.shift();
    return paras.join("\n\n");
}

/** Why the index could not be built: the message and the folders searched. */
function buildProblem(lines) {
    const searched = lines.filter((l) => /^\s/.test(l)).map((l) => l.trim());
    const messages = lines.filter((l) => !/^\s/.test(l) && !/wird gelesen|^fertig$|Gesucht in:$/.test(l));
    return `<div class="admonition warning" style="text-align:left">
<div class="atitle">Index nicht gebaut</div>
${messages.slice(-4).map((m) => `<p>${esc(m)}</p>`).join("")}
${searched.length ? `<p>Gesucht in diesen Ordnern:</p><pre class="code">${esc(searched.join("\n"))}</pre>` : ""}
<p>Abhilfe: In den Einstellungen der Doku-Suche bei <b>Umgebung</b> die Umgebung eintragen, in der pandas & Co. installiert sind – ihren Ordner (<code>~/projekt/.venv</code>, <code>~/miniforge3/envs/daten</code>) oder ihr Python (<code>…/.venv/bin/python</code>).
Im Terminal dieser Umgebung zeigt <code>python -c "import pandas; print(pandas.__file__)"</code>, wo ein Paket liegt.</p></div>`;
}

function renderWelcome() {
    const s = state.summary;
    const settings = state.ctx?.settings || {};
    if (!s || !state.entries.length) {
        els.detail.innerHTML = `<div class="empty">
<h2>Doku-Suche</h2>
<p>Die Dokumentation kommt aus den Paketen, die in deiner Python-Umgebung installiert sind – passend zu deren Versionen. Atlas liest dafür nur ihre Dateien: kein Python, kein Internet.</p>
<p>Pakete: <code>${esc(settings.pakete || "numpy, pandas, matplotlib")}</code><br>
Umgebung: <code>${esc(settings.python || "automatisch (Umgebungen im Projekt, Jupyter-Kernel, Conda, System)")}</code></p>
${state.buildLog && !state.building ? buildProblem(state.buildLog) : ""}
<p>${state.building ? "Der Index wird gerade gebaut …" : `<button data-act="rebuild">${state.buildLog ? "Erneut versuchen" : "Index jetzt bauen"}</button>`}</p>
<p class="inherited">Ändern unter Einstellungen → Erweiterungen → Doku-Suche.</p></div>`;
        return;
    }
    els.detail.innerHTML = `<div class="empty">
<h2>Doku-Suche</h2>
<p>${state.packages.map((p) => `<b>${esc(p.name)}</b> ${esc(p.version)} (${p.count.toLocaleString("de-DE")})`).join(" · ")}</p>
<p>Beispiele:</p>
<p>${["groupby", "read csv", "merge -> DataFrame", "p:axis in:DataFrame k:methode", "->Axes", "p:bins in:Axes", "p::bool in:pyplot", "k:funktion pkg:numpy mean"]
        .map((x) => `<a class="ref" data-example="${esc(x)}">${esc(x)}</a>`).join(" &nbsp; ")}</p>
<p class="inherited">Syntax mit „?“ oben rechts.</p></div>`;
}

// ---------------------------------------------------------------- events

let timer = 0;
function scheduleRender() { clearTimeout(timer); timer = setTimeout(render, 70); }

for (const input of [els.query, els.returns, els.params, els.within]) {
    input.addEventListener("input", scheduleRender);
    input.addEventListener("keydown", (ev) => {
        if (ev.key === "ArrowDown" || ev.key === "ArrowUp" || ev.key === "PageDown" || ev.key === "PageUp") {
            const step = { ArrowDown: 1, ArrowUp: -1, PageDown: 10, PageUp: -10 }[ev.key];
            select(state.selected + step);
            ev.preventDefault();
        } else if (ev.key === "Enter") {
            clearTimeout(timer);
            render();
            els.detail.focus();
            ev.preventDefault();
        }
    });
}
els.full.addEventListener("change", render);
els.dep.addEventListener("change", render);
$("clear").addEventListener("click", () => { clearAll(); els.query.focus(); });
$("help-toggle").addEventListener("click", () => { els.help.hidden = !els.help.hidden; });
els.rebuild.addEventListener("click", rebuild);

function clearAll() {
    const had = els.query.value || els.returns.value || els.params.value || els.within.value || state.kinds.size || state.pkgs.size;
    els.query.value = els.returns.value = els.params.value = els.within.value = "";
    state.kinds.clear();
    state.pkgs.clear();
    renderChips();
    render();
    return !!had;
}

document.addEventListener("keydown", (ev) => {
    if (ev.key === "Escape") {
        ev.preventDefault();
        if (document.activeElement === els.detail) { els.query.focus(); return; }
        if (!clearAll()) (atlas.close ? atlas.close() : atlas.call("close")).catch(() => {});
        return;
    }
    const inField = ["INPUT", "TEXTAREA"].includes(document.activeElement?.tagName);
    if (!inField && (ev.key === "/" || (ev.key.length === 1 && !ev.ctrlKey && !ev.altKey && !ev.metaKey && /\w/.test(ev.key)))) {
        els.query.focus();
        if (ev.key === "/") ev.preventDefault();
        return;
    }
    if ((ev.ctrlKey || ev.metaKey) && ev.key === "c" && !window.getSelection().toString() && state.shown && !(inField && document.activeElement.value)) {
        copy(state.shown.id);
        ev.preventDefault();
    }
    if (!inField && (ev.key === "ArrowDown" || ev.key === "ArrowUp") && document.activeElement !== els.detail) {
        select(state.selected + (ev.key === "ArrowDown" ? 1 : -1));
        ev.preventDefault();
    }
    if (ev.key === "Backspace" && !inField && state.history.length) { show(state.history.pop(), false); ev.preventDefault(); }
});

els.kinds.addEventListener("click", (ev) => {
    const chip = ev.target.closest("[data-kind]");
    if (!chip) return;
    const k = chip.dataset.kind;
    state.kinds.has(k) ? state.kinds.delete(k) : state.kinds.add(k);
    renderChips();
    render();
});
els.packages.addEventListener("click", (ev) => {
    const chip = ev.target.closest("[data-pkg]");
    if (!chip) return;
    const p = chip.dataset.pkg;
    state.pkgs.has(p) ? state.pkgs.delete(p) : state.pkgs.add(p);
    renderChips();
    render();
});
els.results.addEventListener("click", (ev) => {
    const hit = ev.target.closest(".hit");
    if (hit) select(Number(hit.dataset.i), false);
});
els.detail.addEventListener("click", async (ev) => {
    const ref = ev.target.closest("[data-ref]");
    if (ref) { show(state.byId.get(ref.dataset.ref)); return; }
    const jump = ev.target.closest("[data-jump]");
    if (jump) {
        const target = document.getElementById(jump.dataset.jump);
        if (target) {
            target.scrollIntoView({ block: "start", behavior: "smooth" });
            target.classList.add("flash");
            setTimeout(() => target.classList.remove("flash"), 900);
        }
        return;
    }
    const crumb = ev.target.closest("[data-crumb]");
    if (crumb) {
        const id = crumb.dataset.crumb;
        if (state.byId.has(id)) show(state.byId.get(id));
        else { els.within.value = id; render(); }
        return;
    }
    const example = ev.target.closest("[data-example]");
    if (example) { els.query.value = example.dataset.example; render(); els.query.focus(); return; }
    const act = ev.target.closest("[data-act]")?.dataset.act;
    const e = state.shown;
    const done = (text) => { const el = $("copied"); if (el) { el.textContent = text; setTimeout(() => { el.textContent = ""; }, 1500); } };
    if (act === "more-params") { ev.target.closest(".pchips").classList.remove("folded"); return; }
    if (act === "rebuild") rebuild();
    else if (act === "back" && state.history.length) show(state.history.pop(), false);
    else if (act === "copy-name" && e) { if (await copy(e.id)) done("kopiert"); }
    else if (act === "copy-call" && e) {
        if (await copy(usage(e).replace(/, …\)$|…\)$/, ")"))) done("kopiert");
    } else if (act === "members" && e) {
        els.within.value = e.id;
        els.query.value = "";
        render();
    }
});

window.addEventListener("atlas-show", () => { els.query.focus(); els.query.select(); });

/** Dark theme (for the package colours): from the background. */
function detectTheme() {
    const m = /(\d+),\s*(\d+),\s*(\d+)/.exec(getComputedStyle(document.body).backgroundColor);
    const dark = m ? (0.299 * m[1] + 0.587 * m[2] + 0.114 * m[3]) < 128 : false;
    document.documentElement.classList.toggle("dark", dark);
}

atlas.ready.then(async (ctx) => {
    state.ctx = ctx;
    detectTheme();
    renderChips();
    els.query.focus();
    const ok = await loadIndex();
    // The first time: build it right away.
    if (!ok && !state.summary) rebuild();
});
