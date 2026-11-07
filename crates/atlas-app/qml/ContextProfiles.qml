// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

pragma Singleton
import QtQuick
import Atlas

// The context bar as a set of blocks (docs/concept/kontextleiste.html):
// which blocks there are, and which profile lays them out for a file.
//
// The rules live in atlas_core::context_profiles (lists of a profile over
// „Allgemein“, the layout of the bar, editing pages and areas, what to
// store); ContextProfilesCore gives QML access to them. Here are the texts
// of the interface and the glue that keeps the callers' list objects: an
// operation that changes lists writes the core's result back into the same
// object, so `change(l => …)` callbacks work as before.
QtObject {
    id: profiles

    function core(op, args) { return JSON.parse(ContextProfilesCore.call(op, JSON.stringify(args || {}))) }
    /// Ids, icons and rules of the blocks; names and details are below.
    readonly property var meta: core("catalog")

    /// Name and line in the gallery of each built-in block.
    readonly property var texts: ({
        annotations: [qsTr("Anmerkungen"), qsTr("Markierungen und Kommentare der PDF")],
        outline: [qsTr("Gliederung"), qsTr("Lesezeichen der PDF bzw. Überschriften der Notiz")],
        links: [qsTr("Verknüpfungen"), qsTr("Links und Backlinks mit Filtern")],
        tags: [qsTr("Tags"), qsTr("#Tags der Notiz, Klick sucht im Projekt")],
        fileinfo: [qsTr("Dateiinfo"), qsTr("Größe, Seiten, geändert, Pfad")],
        graph: [qsTr("Graph"), qsTr("Verbundene Dateien rund um diese")],
        actions: [qsTr("Aktionen"), qsTr("Eingebaute und eigene Aktionen für die Datei, kurz")],
        "builtin-actions": [qsTr("Für diese Datei"), qsTr("Eingebaute Aktionen: zitieren, exportieren, ausführen …")],
        "user-actions": [qsTr("Eigene Aktionen"), qsTr("Aktionen des Projekts und eigene Befehle")],
        today: [qsTr("Heute"), qsTr("Termine, Blöcke und Fristen von heute")],
        week: [qsTr("Diese Woche"), qsTr("Fristen und Blöcke der nächsten 7 Tage")],
        tasks: [qsTr("Aufgaben"), qsTr("Offene Aufgaben abhaken und neue anlegen")],
        pomodoro: [qsTr("Lernzeit"), qsTr("Pomodoro: lernen, Pause, nach vier Runden lange Pause")]
    })
    /// Blocks: name, icon, whether it may be a tab or pinned, group of the
    /// gallery and a line for it.
    readonly property var catalog: {
        const out = {}
        for (const b of meta.blocks) {
            const t = texts[b.id] || [b.id, ""]
            out[b.id] = { name: t[0], detail: t[1], icon: b.icon, tab: b.tab, pin: b.pin, group: b.group }
        }
        return out
    }
    readonly property var groups: [["file", qsTr("Diese Datei")], ["plan", qsTr("Planung")], ["ext", qsTr("Erweiterungen")]]

    /// The layout without any profile (as the bar was before profiles).
    readonly property var builtin: meta.builtin
    /// Titles of the built-in tabs.
    readonly property var builtinTitles: ({ links: qsTr("Verknüpfungen"), graph: qsTr("Graph"), actions: qsTr("Aktionen") })
    /// Profiles, in the order of the settings. Others than `default` exist
    /// only once set; until then „Allgemein“ applies.
    readonly property var names: meta.names
    function title(name) {
        return ({ default: qsTr("Allgemein"), pdf: qsTr("PDF"), markdown: qsTr("Markdown"), code: qsTr("Code"),
                  calendar: qsTr("Kalender") })[name] || name
    }
    readonly property var keys: meta.keys

    /// Profile for a tab of the editor area by its kind.
    function kindOf(tab) { return core("kindOf", { kind: tab ? tab.kind : "" }) }

    function info(id, extensions) {
        if (catalog[id]) return catalog[id]
        if (id.startsWith("ext:")) {
            const e = (extensions || []).find(x => "ext:" + x.name === id)
            return { name: e ? (e.title || e.name) : id.slice(4), icon: "grid", tab: false, pin: false, group: "ext",
                     detail: e ? (e.description || "") : "", letter: (e ? (e.title || e.name) : id.slice(4)).charAt(0).toUpperCase() }
        }
        return null
    }

    /// Title of tab `id` when its page has none (built-in tab, single block).
    function tabTitle(id) { return builtinTitles[id] || (catalog[id] ? catalog[id].name : id) }
    function named(page, id) {
        if (page && !page.title) page.title = tabTitle(id)
        return page
    }
    /// Writes the lists of a core result into `l` (arrays in place) and
    /// returns the operation's result.
    function apply(l, out) {
        if (!out) return null
        for (const k of keys) {
            if (Array.isArray(l[k])) l[k].splice(0, l[k].length, ...out.lists[k])
            else l[k] = out.lists[k]
        }
        l.pages = out.lists.pages
        return out.result
    }

    function copy(p) {
        const out = {}
        for (const k of keys) out[k] = (p[k] || []).slice()
        out.pages = JSON.parse(JSON.stringify(p.pages || {}))
        return out
    }
    /// The page of tab `id` in lists `l`: {title, blocks}, or null.
    function pageOf(l, id) { return named(core("pageOf", { lists: l, id: id }), id) }
    /// The list of an area: "tabs", "overview", "pinned", or a page (its
    /// blocks; a built-in page becomes stored on the first change).
    function areaList(l, area) {
        if (keys.indexOf(area) >= 0) return l[area]
        apply(l, core("areaList", { lists: l, area: area }))
        return l.pages[area].blocks
    }
    function isPage(area) { return keys.indexOf(area) < 0 }
    /// Adds a new tab of one's own; returns its name.
    function newPage(l, title) { return apply(l, core("newPage", { lists: l, title: title })) }
    function renamePage(l, id, title) { apply(l, core("renamePage", { lists: l, id: id, title: title })) }
    /// Removes tab `id`; a page of one's own goes with it.
    function removeTab(l, id) { apply(l, core("removeTab", { lists: l, id: id })) }
    /// Moves block `id` from area `from` to the end of area `to`.
    function moveTo(l, id, from, to) { apply(l, core("moveTo", { lists: l, id: id, from: from, to: to })) }
    function same(a, b) { return a.length === b.length && a.every((x, i) => x === b[i]) }

    /// `default` with all lists (built in, or as stored; old switches as
    /// hidden blocks while there are no profiles).
    function base(config) { return core("base", { config: config }) }
    /// Whether profile `name` is set (`default` always is).
    function isSet(config, name) { return core("isSet", { config: config, name: name }) }
    /// The profile used for `kind`.
    function profileFor(config, kind) { return core("profileFor", { config: config, kind: kind }) }
    /// All lists of profile `name`: as stored over `default`; a profile not
    /// set yet starts from the suggestion.
    function lists(config, name) { return core("lists", { config: config, name: name }) }

    /// What the bar shows for profile `name`: {name, tabs, pages, overview,
    /// pinned, collapsed, hidden} – only known blocks; extension cards that
    /// are shown but not placed come at the end of the overview.
    function layout(config, name, extensions) {
        const exts = (extensions || []).map(e => ({ name: e.name, shown: !!e.shown }))
        const out = core("layout", { config: config, name: name, extensions: exts })
        for (const id of out.tabs) named(out.pages[id], id)
        return out
    }

    /// Stores profile `name` with the lists `full`: `default` completely,
    /// another one only with what differs from `default`.
    function save(app, config, name, full) {
        app.setContextProfile(name, JSON.stringify(core("save", { config: config, name: name, lists: full })))
    }
    /// Changes profile `name` by `change(lists)` and stores it. The first
    /// change of all also stores `default` (taking over the old switches).
    function edit(app, config, name, change) {
        const l = lists(config, name)
        change(l)
        const fresh = JSON.parse(app.contextModules || "{}")
        for (const w of core("commit", { config: fresh, name: name, lists: l }) || [])
            app.setContextProfile(w[0], JSON.stringify(w[1]))
    }
    /// Changes `default` and every other profile that is set by the same
    /// `change` („Gilt für: alle Dateitypen“): a profile with its own list
    /// would otherwise hide the change.
    function editAll(app, config, change) {
        edit(app, config, "default", change)
        for (const name of names)
            if (name !== "default" && (JSON.parse(app.contextModules || "{}").profiles || {})[name])
                edit(app, JSON.parse(app.contextModules || "{}"), name, change)
    }
    /// The stored profiles, to undo a change (see restore()).
    function snapshot(config) { return JSON.parse(JSON.stringify(config.profiles || {})) }
    function restore(app, config, snap) {
        const all = Object.keys(config.profiles || {}).concat(Object.keys(snap))
        for (const name of all) app.setContextProfile(name, snap[name] ? JSON.stringify(snap[name]) : "")
    }
    /// Removes `id` from every area (not from hidden / collapsed).
    function unplace(l, id) { apply(l, core("unplace", { lists: l, id: id })) }
    function toggle(list, id, on) {
        const i = list.indexOf(id)
        if (on && i < 0) list.push(id)
        if (!on && i >= 0) list.splice(i, 1)
    }
    /// Moves `id` within its area by `delta`, skipping blocks not shown
    /// (`visible`: the ids the bar shows there, in order).
    function move(l, area, id, delta, visible) {
        return !!apply(l, core("move", { lists: l, area: area, id: id, delta: delta, visible: visible || null }))
    }
    /// One line for the settings: the overview, then the pinned blocks.
    function summary(config, name, extensions) {
        const l = layout(config, name, extensions)
        const names = (ids) => ids.map(id => (info(id, extensions) || { name: id }).name).join(" · ")
        return names(l.overview) + (l.pinned.length ? " · " + qsTr("unten: %1").arg(names(l.pinned)) : "")
    }
}
