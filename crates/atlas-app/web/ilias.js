// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

// Reads the courses of an ILIAS dashboard with their folders and files.
// Injected into a page of the logged-in ILIAS (IliasService.qml): fetches
// pages with the session's cookies and reads them with DOMParser – like a
// browser, without downloading files. Reports via console messages
// "atlas:ilias:{json}" ({type: "progress" | "login" | "done" | "error"}).

"use strict";

window.atlasIlias = window.atlasIlias || (function () {
    const KINDS = ["crs", "grp", "sess", "fold", "file"];
    const MAX_DEPTH = 10;
    const PARALLEL = 4;

    function post(data) { console.log("atlas:ilias:" + JSON.stringify(data)); }
    function clean(text) { return (text || "").replace(/\s+/g, " ").trim(); }

    /// Logged in: ILIAS shows a logout link on every page.
    function loggedIn(doc) {
        return !!doc.querySelector('a[href*="logout"], a[href*="cmd=showLogout"], form[action*="logout"]');
    }

    /// {kind, id} of an ILIAS object link, or null.
    function classify(href, link) {
        let url = href;
        try { url = decodeURIComponent(href); } catch (e) {}
        let m = /[?&]target=(crs|grp|sess|fold|file)_(\d+)/i.exec(url)
            || /goto_[^/?#]*?_(crs|grp|sess|fold|file)_(\d+)/i.exec(url)
            || /\/go\/(crs|grp|sess|fold|file)\/(\d+)/i.exec(url);
        if (m) return { kind: m[1].toLowerCase(), id: m[2] };
        // Other link forms (ref_id=…): the item's icon tells the type.
        const ref = /[?&]ref_id=(\d+)/i.exec(url);
        const item = link && link.closest(".il_ContainerListItem, .il-std-item, .il-item, .ilObjListRow, li, tr");
        const icon = item && item.querySelector('img[src*="icon_"]');
        const im = icon && /icon_(crs|grp|sess|fold|file)\b/i.exec(icon.getAttribute("src"));
        if (ref && im) return { kind: im[1].toLowerCase(), id: ref[1] };
        return null;
    }

    /// Direct download address of a file.
    function downloadUrl(href, id) {
        if (/_download\b|\/download\b|cmd=sendfile|cmd=download/i.test(href)) return href;
        if (/[?&]target=file_\d+/i.test(href)) return href.replace(/(target=file_\d+)/i, "$1_download");
        return new URL("goto.php?target=file_" + id + "_download", location.origin + "/").href;
    }

    /// The object links of the main content (no navigation, tabs or
    /// breadcrumbs: they lead back up or sideways).
    function objects(doc, base) {
        const root = doc.querySelector("#il_center_col, main, [role=main]") || doc.body;
        const out = [];
        const seen = new Set();
        for (const a of root.querySelectorAll("a[href]")) {
            if (a.closest("nav, header, .breadcrumb, .il-breadcrumbs, .ilTabs, .il-maincontrols, #mainspacekeeper, .ilLocator")) continue;
            const href = new URL(a.getAttribute("href"), base).href;
            if (new URL(href).host !== new URL(base).host) continue;
            const c = classify(href, a);
            if (!c || seen.has(c.kind + c.id)) continue;
            const item = a.closest(".il_ContainerListItem, .il-std-item, .il-item, .ilObjListRow, li, tr");
            const title = item && item.querySelector(".il_ContainerItemTitle, .il-item-title");
            const name = clean((title && title.innerText) || a.innerText || a.getAttribute("title") || a.getAttribute("aria-label"));
            if (!name) continue;
            seen.add(c.kind + c.id);
            const props = item ? [...item.querySelectorAll(".il_ItemProperties, .il_ItemProperty, .il-item-properties, .il-item-property-value")] : [];
            const info = clean(props.map(p => p.innerText).join(" · "));
            const o = { kind: c.kind, id: c.id, name: name, url: href, info: info };
            if (c.kind === "file") o.download = downloadUrl(href, c.id);
            out.push(o);
        }
        return out;
    }

    class LoginNeeded extends Error {}

    async function page(url) {
        const res = await fetch(url, { credentials: "include" });
        const doc = new DOMParser().parseFromString(await res.text(), "text/html");
        if (!loggedIn(doc)) throw new LoginNeeded(url);
        return { doc: doc, url: res.url || url };
    }

    /// Runs `jobs` (functions returning promises), at most `n` at a time.
    async function pool(jobs, n) {
        const results = new Array(jobs.length);
        let next = 0;
        async function worker() {
            while (next < jobs.length) {
                const i = next++;
                results[i] = await jobs[i]();
            }
        }
        await Promise.all(Array.from({ length: Math.min(n, jobs.length) }, worker));
        return results;
    }

    let pages = 0;
    async function container(node, visited, depth) {
        const key = node.kind + node.id;
        if (visited.has(key) || depth > MAX_DEPTH) return node;
        visited.add(key);
        const p = await page(node.url);
        pages++;
        if (pages % 5 === 0) post({ type: "progress", pages: pages, text: node.name });
        const children = objects(p.doc, p.url).filter(o => o.kind !== "crs" && o.kind + o.id !== key && !visited.has(o.kind + o.id));
        node.children = await pool(children.map(c => () =>
            c.kind === "file" ? Promise.resolve(c) : container(c, visited, depth + 1).catch(e => {
                if (e instanceof LoginNeeded) throw e;
                c.error = String(e);
                return c;
            })), PARALLEL);
        return node;
    }

    async function scan(base) {
        pages = 0;
        try {
            const dashboards = [
                "ilias.php?baseClass=ilDashboardGUI&cmd=jumpToMemberships",
                "ilias.php?baseClass=ilmembershipoverviewgui",
                "ilias.php?baseClass=ilDashboardGUI&cmd=jumpToSelectedItems",
                "ilias.php?baseClass=ilDashboardGUI&cmd=show",
            ];
            const courses = [];
            const seen = new Set();
            for (const d of dashboards) {
                let p;
                try { p = await page(new URL(d, base).href); } catch (e) { if (e instanceof LoginNeeded) throw e; continue; }
                for (const o of objects(p.doc, p.url))
                    if ((o.kind === "crs" || o.kind === "grp") && !seen.has(o.kind + o.id)) { seen.add(o.kind + o.id); courses.push(o); }
            }
            post({ type: "progress", pages: 0, text: courses.length + " Kurse" });
            const visited = new Set();
            const tree = await pool(courses.map(c => () => container(c, visited, 0).catch(e => {
                if (e instanceof LoginNeeded) throw e;
                c.error = String(e);
                return c;
            })), PARALLEL);
            post({ type: "done", courses: tree, pages: pages });
        } catch (e) {
            if (e instanceof LoginNeeded) post({ type: "login" });
            else post({ type: "error", message: String(e) });
        }
    }

    return { scan: scan, loggedIn: () => loggedIn(document), objects: (html) => objects(new DOMParser().parseFromString(html, "text/html"), location.href) };
})();
